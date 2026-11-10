"""Unprivileged installed supervisor; all status evidence is fail-closed."""
import importlib.util
import json
import os
from pathlib import Path
import socket
import struct
import threading
import base64
import hashlib
import selectors
import shutil
import stat
import subprocess
import time
import uuid

HERE = Path(__file__).resolve().parent
def sibling(name):
    spec = importlib.util.spec_from_file_location('gbx_'+name,HERE/(name+'.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module
snapshot, receipt, lifecycle = sibling('snapshot'), sibling('receipt'), sibling('lifecycle')
ORACLES = {'B1','B2','B3','B4','A1','A5','A6','A8','E2E','CLIENTS','C1-SEALED','C2-CANARY','C3-RESTART'}
# PR-2b install.sh must create /etc/gatebox/qualification.json with this value
# before starting the service. It permits qualifying only; normal still fails closed.
QUALIFICATION_PLACEHOLDER = {'identity':None,'passed':[]}

def framing_budget(sock, deadline):
    remaining = deadline-time.monotonic()
    if remaining <= 0: raise TimeoutError('framing deadline')
    sock.settimeout(remaining)

def exact(sock, length, deadline):
    chunks = bytearray()
    while len(chunks) < length:
        framing_budget(sock,deadline)
        block = sock.recv(length-len(chunks))
        if not block: raise ValueError('truncated frame')
        chunks.extend(block)
    return bytes(chunks)

def receive_request(sock, registry):
    try:
        timeout = sock.gettimeout()
        deadline = time.monotonic()+min(2,timeout if timeout is not None else 2)
        uid = struct.unpack('3i',sock.getsockopt(socket.SOL_SOCKET,socket.SO_PEERCRED,12))[1]
        header = exact(sock,4,deadline)
        length = int.from_bytes(header,'big')
        if not 1 <= length <= 1024: raise ValueError('request length')
        frame = header+exact(sock,length,deadline)
        # Client closes its write half; the framing deadline covers trailing bytes.
        framing_budget(sock,deadline)
        if sock.recv(1): raise ValueError('trailing request data')
        return snapshot.parse_request(frame,registry,uid)
    except OSError as exc: raise ValueError('framing/reply deadline') from exc

class Admission:
    def __init__(self, pending):
        self.pending, self.lock = Path(pending), threading.Lock()
    def acquire(self):
        if not self.lock.acquire(blocking=False): return False
        if any(self.pending.iterdir()):
            self.lock.release()
            return False
        return True
    def release(self): self.lock.release()

def qualified(identity, evidence):
    return (isinstance(evidence,dict) and evidence.get('identity') == identity
            and isinstance(evidence.get('passed'),list) and set(evidence['passed']) == ORACLES)

def admission_stage(request, peer, registration, identity, state, evidence):
    if (not isinstance(state,dict) or state.get('identity') != identity
            or state.get('stage') not in ('qualifying','normal')):
        raise ValueError('QUALIFICATION_STATE: missing or stale installed binding')
    if peer == 0 or peer != registration.get('uid'):
        raise ValueError('QUALIFICATION_PEER: registered non-root submitter required')
    if state['stage'] == 'qualifying':
        if (request['full_sha'] != registration.get('fixture_sha')
                or request['gate_set_id'] != registration.get('qualification_gate_set')):
            raise ValueError('QUALIFICATION_ONLY')
    elif not qualified(identity,evidence):
        raise ValueError('QUALIFICATION_EVIDENCE: acceptance missing or stale')
    return state['stage']

class Evidence:
    def __init__(self, log_limit=64*1024**2):
        self.commands, self.active, self.complete = [], None, False
        self.events, self.log, self.log_limit = 0, bytearray(), log_limit
    def accept(self, event):
        self.events += 1
        if self.events > 512 or self.complete or not isinstance(event,dict): raise ValueError('status bound/order')
        kind = event.get('event')
        if kind == 'start':
            fields = {'event','id','gate_id','argv','cwd','environment'}
            if event.get('gate_id') in ('host','whitespace'): fields.add('internal')
            if (set(event) != fields or self.active
                    or not isinstance(event['id'],str) or any(c['id']==event['id'] for c in self.commands)
                    or not receipt.valid_invocation(event) or event['cwd'] not in ROUTE_CWDS
                    or not isinstance(event['environment'],dict)):
                raise ValueError('invalid command start')
            self.active = {k:v for k,v in event.items() if k != 'event'}
        elif kind == 'end':
            if (set(event) != {'event','id','exit_code','signal'} or self.active is None
                    or event['id'] != self.active['id'] or type(event['exit_code']) is not int
                    or event['signal'] != (-event['exit_code'] if event['exit_code'] < 0 else None)):
                raise ValueError('invalid command termination')
            self.active.update({k:v for k,v in event.items() if k != 'event'})
            self.commands.append(self.active)
            self.active = None
        elif kind == 'complete':
            if set(event) != {'event'} or not self.commands or self.active: raise ValueError('premature completion')
            self.complete = True
        else: raise ValueError('unknown private status record')
    def capture(self, block):
        available = self.log_limit-len(self.log)
        self.log.extend(block[:available])
        if len(block) > available: raise ValueError('log overflow')
    def finish(self, collector_complete, timed_out):
        if (not self.complete or self.active or not self.commands or not collector_complete or timed_out
                or any(c['exit_code'] != 0 for c in self.commands)):
            raise ValueError('missing/failed private lifecycle evidence')
        return self.commands

ROUTE_CWDS = {'/work/src',*receipt.GATE_SETS['route-cells']['cwd'].values()}  # exact per set: receipt.validate_summary

def route_summary(summary, evidence, logs, identity):
    """SB3E-R18 P4: commands, coverage and the lane group from the sealed A7 evidence and harness run."""
    route = receipt.policy_set(identity['gates_sha256'])['route-cells.json']
    digest = lambda data: hashlib.sha256(data).hexdigest()
    commands, discovery = [], []
    for item in evidence.commands:
        command = dict(item)
        if command['gate_id'] == 'route-cells-list':
            discovery.append(command)
            continue
        name = command['id']+'.log'
        command.update(log=name,log_sha256=digest(logs[name]),timed_out=False,oom=False,status_complete=True,collector_complete=True)
        if command['gate_id'] == 'route-cells':
            inventory = json.loads(logs[command['id']+'-inventory.log'])
            command.update(artifact_id=inventory['artifact']['id'],inventory=inventory['inventory'])
        commands.append(command)
    lane, verdict, ids = json.loads(logs['route-lane.log']), logs['a7-verdict.json.log'], [g['id'] for g in route['gates']]
    summary['commands'], summary['environment'] = commands, route['environment']
    summary['coverage'] = dict(expected=ids,executed=ids,passed=ids,failed=[],ignored=[],filtered=[],excluded=[],
                               artifacts=json.loads(logs['coverage.log'])['artifacts'],discovery=discovery)
    summary['lane'] = dict(a7='PASS' if json.loads(verdict).get('verdict') == 'pass' else 'FAIL',verdict_sha256=digest(verdict),
                           evidence={n:digest(logs[n]) for n in ('a7-'+r.replace('/','-')+'.log' for r in route['a7']['evidence'])},
                           rustc_vv=lane['rustc_vv'],cargo_v=lane['cargo_v'],lockfile_sha256=summary['snapshot']['lockfile_sha256'],
                           wrapper_sha256=route['a7']['wrapper_sha256'],harness_sha256=lane['harness_sha256'],
                           inventory_sha256=digest('\n'.join(lane['names']).encode()),names=lane['names'],trust=route['trust'])

STATE = Path('/var/lib/mothergate')
CONTROL = Path('/run/mothergate-control')
CONFIG = Path('/etc/gatebox')
HOST_INPUTS = (Path('/home/nexus/.rustup/toolchains/1.94.0-x86_64-unknown-linux-gnu'),
               Path('/home/nexus/.cargo/registry'))

def read_config(name):
    path = CONFIG/name
    for parent in [*reversed(path.parents),path]:
        info = parent.lstat()
        if info.st_uid != 0 or info.st_mode & 0o022 or stat.S_ISLNK(info.st_mode):
            raise ValueError('unprotected installed configuration')
    return json.loads(path.read_bytes(),object_pairs_hook=receipt.unique)

def shim(verb, run):
    subprocess.run(['/usr/bin/sudo','-n','/usr/libexec/gatebox/lifecycle.py',verb,run],
                   stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,
                   env=snapshot.ENV,close_fds=True,timeout=90,check=True)

def unit_state(run):
    unit = 'mothergate-run@'+run+'.service'
    result = subprocess.run(['/usr/bin/systemctl','show',unit,'--property=ActiveState,SubState,Result,ControlGroup,MemoryPeak,CPUUsageNSec'],
                            env=snapshot.ENV,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,
                            stderr=subprocess.DEVNULL,close_fds=True,timeout=5,check=True)
    return dict(line.split('=',1) for line in result.stdout.decode().splitlines() if '=' in line)

def stop_and_confirm(run):
    shim('stop',run)
    state = unit_state(run)
    if state.get('ActiveState') not in ('inactive','failed'): raise ValueError('unit still active after stop')
    group = state.get('ControlGroup')
    if group:
        if group != lifecycle.unit_cgroup('mothergate-run@'+run+'.service'): raise ValueError('unexpected cgroup identity')
        path = Path('/sys/fs/cgroup')/group.lstrip('/')
        try:
            if 'populated 0' not in (path/'cgroup.events').read_text().splitlines():
                raise ValueError('unresolved descendants')
        except FileNotFoundError:
            if path.exists(): raise ValueError('cgroup disappeared ambiguously')
    return {'complete':True,'manager':state}

def collect(run, listeners, evidence, logs):
    selector = selectors.DefaultSelector()
    streams, buffers, ended = {}, {}, set()
    for kind, sock in listeners.items(): selector.register(sock,selectors.EVENT_READ,('listen',kind))
    began = time.monotonic()
    deadline, next_poll = began+14430, began
    try:
        while len(ended) < 2:
            events = selector.select(.1)
            now = time.monotonic()
            if now >= deadline: raise TimeoutError('capture deadline')
            if not events and now >= next_poll:
                next_poll = now+1
                state = unit_state(run)
                if state.get('ActiveState') == 'failed' or (now-began >= 30 and state.get('ActiveState') == 'inactive'):
                    raise ValueError('unit exited before capture completion: '+str(state))
            for key, _ in events:
                mode, kind = key.data
                if mode == 'listen':
                    conn, _ = key.fileobj.accept()
                    if struct.unpack('3i',conn.getsockopt(socket.SOL_SOCKET,socket.SO_PEERCRED,12))[1] != 0:
                        conn.close()
                        raise ValueError('capture must be connected by system PID 1')
                    conn.setblocking(False)
                    streams[kind], buffers[kind] = conn, bytearray()
                    selector.unregister(key.fileobj)
                    selector.register(conn,selectors.EVENT_READ,('data',kind))
                else:
                    block = key.fileobj.recv(65536)
                    if not block:
                        if buffers[kind]: raise ValueError('truncated capture frame')
                        ended.add(kind)
                        selector.unregister(key.fileobj)
                        continue
                    buffer = buffers[kind]
                    buffer.extend(block)
                    while len(buffer) >= 4:
                        size = int.from_bytes(buffer[:4],'big')
                        if not 1 <= size <= 4096: raise ValueError('private capture frame bound')
                        if len(buffer) < 4+size: break
                        event = json.loads(buffer[4:4+size],object_pairs_hook=receipt.unique)
                        del buffer[:4+size]
                        if kind == 'status': evidence.accept(event)
                        else:
                            if (set(event) != {'id','data'} or not isinstance(event['id'],str)
                                    or not receipt.NAME.fullmatch(event['id'])): raise ValueError('log identity')
                            data = base64.b64decode(event['data'],validate=True)
                            evidence.capture(data)
                            if event['id'] not in logs and len(logs) >= 512: raise ValueError('log count bound')
                            logs.setdefault(event['id'],bytearray()).extend(data)
        evidence.finish(True,False)
        return evidence, {name+'.log':bytes(data) for name,data in logs.items()}
    finally:
        selector.close()
        for stream in streams.values(): stream.close()

def coverage(evidence, logs):
    gates, exclusions = receipt.policy()
    compiled = json.loads(logs['coverage.log'])
    if compiled['failures']: raise ValueError('payload reconciliation failed')
    artifacts = compiled['artifacts']
    hosts = sorted(a['id'] for a in artifacts if a['package'] == 'nexus-mother-host')
    commands, discovery = [], []
    for item in evidence.commands:
        command = dict(item)
        if command['gate_id'].endswith('-list'):
            discovery.append(command)
            continue
        name = command['id']+'.log'
        command.update(log=name,log_sha256=hashlib.sha256(logs[name]).hexdigest(),
                       timed_out=False,oom=False,status_complete=True,collector_complete=True)
        if command['gate_id'] in ('workspace','desktop-live'):
            inventory = json.loads(logs[command['id']+'-inventory.log'])
            command.update(artifact_id=inventory['artifact']['id'],inventory=inventory['inventory'])
        if command['gate_id'] == 'host':
            host = [c['inventory'] for c in commands if c['gate_id'] == 'workspace' and c['artifact_id'] in hosts]
            names = [n for i in host for n in i['expected']]
            executed = [n for i in host for n in i['passed']]
            ignored = [n for i in host for n in i['ignored']]
            command.update(artifact_ids=hosts,inventory=dict(expected=names,executed=executed,passed=executed,
                                                            failed=[],ignored=ignored,filtered=[]))
        commands.append(command)
    expected = [g['id'] for g in gates['gates']]
    return commands, dict(expected=expected,executed=expected,passed=expected,failed=[],ignored=[],filtered=[],
                          artifacts=artifacts,host_artifacts=hosts,discovery=discovery,
                          excluded=[dict(name=n,status='excluded',reason=exclusions['reason']) for n in exclusions['cases']])

def execute(run, request, peer, registration, identity, marker, stage_name):
    stage = STATE/'staging'/run
    # A route-cells receipt always carries its `lane` group, so even its failure publishes (SB3E-R18 P4).
    summary = {key:{} for key in receipt.GROUPS+receipt.GATE_SETS[request['gate_set_id']]['extra']}
    summary.update(schema_version=1,run_id=run,peer_uid=peer,request=request,attempts=[run],commands=[],logs=[],
                   missing_evidence=[],limitations=[receipt.MUTABLE,'codex: unsupported (runner denies AF_UNIX)'],
                   status='BLOCKED',installed=dict(identity,stage=stage_name),
                   source=registration,cleanup={'complete':False})
    listeners, logs, captured, started = {}, {}, {}, time.monotonic()
    evidence = Evidence()
    incoming, handed_off = [], False
    try:
        stage.mkdir(mode=0o700)
        (stage/'request.json').write_bytes(receipt.encode(request))
        (stage/'request.json').chmod(0o444)
        source = snapshot.GitSource(Path('/var/lib/gatebox-source'),registration['device'],registration['inode'])
        summary['snapshot'] = snapshot.export(source,request['full_sha'],stage/'snapshot')
        for kind in ('log','status'):
            sock = socket.socket(socket.AF_UNIX,socket.SOCK_STREAM)
            listeners[kind] = sock
            path = CONTROL/'incoming'/(run+'.'+kind+'.sock')
            sock.bind(str(path))
            incoming.append(path)  # Only a successful bind owns this incoming leaf.
            sock.listen(1)
        shim('start',run)
        handed_off = True
        _, logs = collect(run,listeners,evidence,captured)
        if request['gate_set_id'] == 'route-cells': route_summary(summary,evidence,logs,identity)
        else:
            summary['commands'], summary['coverage'] = coverage(evidence,logs)
            summary['environment'] = sibling('init').ENV
        summary['resources'] = dict(unit_state(run),scratch=json.loads(logs['resources.log']))
        summary['input_observations'] = json.loads(logs['inputs.log'])
        if summary['input_observations']['before'] != summary['input_observations']['after']:
            raise ValueError('mutable host input changed during run')
        # Qualification requires real probe output; absence can never become success.
        summary['isolation'] = json.loads(logs['isolation.log'])
        summary['host_inputs'] = {'paths':[str(p) for p in HOST_INPUTS],'mutable':True}
        summary['versions'] = json.loads(logs['versions.log'])
        summary['status'] = receipt.GATE_SETS[request['gate_set_id']]['status']
    except Exception as exc:
        summary['status'] = 'FAILED'
        summary['missing_evidence'].append(type(exc).__name__+': '+str(exc))
    finally:
        # A failed collector still leaves bounded partial diagnostics to publish.
        if not logs: logs = {name+'.log':bytes(data) for name,data in captured.items()}
        if not summary['commands']: summary['commands'] = evidence.commands
        try: summary['cleanup'] = stop_and_confirm(run)
        except Exception as exc:
            summary['status'] = 'BLOCKED'
            summary['missing_evidence'].append('cleanup: '+str(exc))
        for kind, sock in listeners.items():
            sock.close()
        if not handed_off:
            for path in incoming:
                try: os.unlink(path)
                except FileNotFoundError: pass  # Root may already have relocated it.
        summary['resources']['duration_seconds'] = time.monotonic()-started
        summary['logs'] = [dict(file=name,sha256=hashlib.sha256(data).hexdigest()) for name,data in logs.items()]
        try: receipt.validate_summary(summary)
        except Exception as exc:
            summary['status'] = 'FAILED'
            summary['missing_evidence'].append('validation: '+type(exc).__name__+': '+str(exc))
        try:
            receipt.publish(STATE/'pending',STATE/'receipts',run,summary,logs)
            if summary['cleanup']['complete']:
                # Retain staging on ambiguity; otherwise remove only this owned run.
                for base, dirs, files in os.walk(stage): Path(base).chmod(0o700)
                shutil.rmtree(stage)
                marker.unlink()
        except Exception:
            # Marker and any pending publication remain, blocking further admission.
            raise

def main():
    registry = read_config('registry.json')
    identity = read_config('identity.json')
    if os.environ.get('LISTEN_PID') != str(os.getpid()) or os.environ.get('LISTEN_FDS') != '1':
        raise ValueError('exactly one systemd submission descriptor required')
    listener = socket.socket(fileno=3)
    if listener.family != socket.AF_UNIX or listener.getsockname() != '/run/mothergate-submit.sock':
        raise ValueError('unexpected submission socket')
    admission, slots = Admission(STATE/'pending'), threading.BoundedSemaphore(8)
    def connection(sock):
        held, worker = False, False
        try:
            sock.settimeout(2)
            uid = struct.unpack('3i',sock.getsockopt(socket.SOL_SOCKET,socket.SO_PEERCRED,12))[1]
            request = receive_request(sock,registry)
            evidence = read_config('qualification.json')
            state = read_config('admission.json')
            stage_name = admission_stage(request,uid,registry[request['repo_id']],identity,state,evidence)
            held = admission.acquire()
            reply = {'status':'ACCEPTED' if held else 'BUSY'}
            if held:
                run = uuid.uuid4().hex
                marker = STATE/'pending'/('.'+run)
                with marker.open('xb') as stream:
                    stream.write(receipt.encode(dict(run_id=run,peer_uid=uid,request=request)))
                    stream.flush()
                    os.fsync(stream.fileno())
                reply['run_id'] = run
                def work():
                    try: execute(run,request,uid,registry[request['repo_id']],identity,marker,stage_name)
                    finally: admission.release()
                threading.Thread(target=work,daemon=False).start()
                worker = True
            data = json.dumps(reply).encode()
            sock.settimeout(2)
            sock.sendall(struct.pack('!I',len(data))+data)
        except (ValueError,OSError) as exc:
            if worker: return  # Admitted work survives a lost reply; it was not rejected.
            try:
                reply = {'status':'UNQUALIFIED' if str(exc).startswith('QUALIFICATION_') else 'REJECTED',
                         'reason':str(exc)}
                data = json.dumps(reply).encode()[:4000]
                sock.settimeout(2)
                sock.sendall(struct.pack('!I',len(data))+data)
            except OSError: pass  # admitted work and its marker are independent of reply delivery
        finally:
            if held and not worker: admission.release()
            sock.close()
            slots.release()
    while True:
        sock, _ = listener.accept()
        if not slots.acquire(blocking=False): sock.close()
        else: threading.Thread(target=connection,args=(sock,),daemon=False).start()

if __name__ == '__main__': main()
