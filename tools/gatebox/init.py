"""Trusted payload init: isolated child environment and libtest reconciliation.
route-cells (SB3E-R18 P1, R19, R20): no probe; quiescence before and after A7 `run`; the evidence is
sealed into the private stream before any unpinned candidate code; the harness runs only as the bytes
A7 inventoried, re-verified after its listing with only init alive."""
import os
import re
import subprocess
import base64
import ctypes
import hashlib
import importlib.util
import json
from pathlib import Path
import selectors
import shutil
import signal
import stat
import struct
import time
import tomllib
ENV = {'PATH':'/opt/rust/bin:/usr/bin:/bin','HOME':'/work/home','PWD':'/work/src',
       'CARGO_HOME':'/work/cargo','CARGO_TARGET_DIR':'/work/target','CARGO_BUILD_JOBS':'4',
       'RUST_TEST_THREADS':'4','CARGO_TERM_COLOR':'never','CARGO_NET_OFFLINE':'true',
       'RUST_BACKTRACE':'1','TMPDIR':'/run/gate/tmp','XDG_RUNTIME_DIR':'/run/gate/xdg',
       'XDG_CACHE_HOME':'/work/cache','LANG':'C.UTF-8','LC_ALL':'C.UTF-8','TZ':'UTC','CI':'true'}

def child_options(log_fd):
    return {'stdin':subprocess.DEVNULL,'stdout':log_fd,'stderr':subprocess.STDOUT,
            'close_fds':True,'env':dict(ENV),'cwd':'/work/src','start_new_session':True}

ROUTE_CWD = {'a7':'/work/src','route-cells':'/work/src/mother/host'}  # receipt.GATE_SETS literal (P11)
def requested():
    """The supervisor-validated request (snapshot.parse_request); it alone selects the gate set."""
    return json.loads(Path('/gate/request.json').read_bytes())
DIRECTORY = os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW|os.O_CLOEXEC

def open_beneath(path, final=os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC):
    """Open an absolute path with no symlink in any component."""
    parts = Path(path).parts
    if not Path(path).is_absolute() or '..' in parts: raise ValueError('unsafe route path: '+str(path))
    fd = os.open('/',DIRECTORY)
    try:
        for index, name in enumerate(parts[1:],1):
            child = os.open(name,final if index == len(parts)-1 else DIRECTORY,dir_fd=fd)
            os.close(fd)
            fd = child
        return fd
    except BaseException:
        os.close(fd)
        raise

def read_regular(path, limit=64*1024**2):
    try: fd = open_beneath(path)
    except OSError as exc: raise ValueError('ROUTE_EVIDENCE: missing/linked '+str(path)) from exc
    try:
        info = os.fstat(fd)
        if not stat.S_ISREG(info.st_mode) or info.st_size > limit: raise ValueError('ROUTE_EVIDENCE: not a bounded regular file '+str(path))
        chunks = []
        while block := os.read(fd,1024*1024): chunks.append(block)
        data = b''.join(chunks)
        if len(data) != info.st_size: raise ValueError('ROUTE_EVIDENCE: changed while sealed '+str(path))
        return data
    finally: os.close(fd)

SHOULD_PANIC = ' - should panic'  # libtest's marker after a #[should_panic] test's name

def reconcile(names, output):
    # Q33: every listed test either passes or is reported ignored by libtest (#[ignore]); ignored
    # names are recorded as ignored, never as passed, and the totals must agree. Nothing else.
    passed, ignored = [], []
    # libtest names are Rust paths (no whitespace); the marker is the only text before " ... ".
    for name, _, result in re.findall(r'^test (\S+)( - should panic)? \.\.\. (.*)$',output,re.M):
        if result == 'ok': passed.append(name)
        elif result == 'ignored' or result.startswith('ignored, '): ignored.append(name)
        else: raise ValueError('missing/unexpected/failed libtest names or totals')
    totals = re.findall(r'^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;',output,re.M)
    running = re.findall(r'^running (\d+) tests?$',output,re.M)
    run = [n for n in names if n not in set(ignored)]
    if (len(set(names)) != len(names) or len(set(passed+ignored)) != len(passed)+len(ignored)
            or sorted(passed) != sorted(run) or not set(ignored) <= set(names)
            or running != [str(len(names))] or totals != [(str(len(run)),'0',str(len(ignored)),'0','0')]):
        raise ValueError('missing/unexpected/failed libtest names or totals')
    ignored = [n for n in names if n in set(ignored)]
    return {'expected':names,'executed':run,'passed':run,'failed':[],'ignored':ignored,'filtered':[]}

def packet(fd, value):
    data = json.dumps(value,separators=(',',':')).encode()
    if len(data) > 4096: raise ValueError('private frame bound')
    data = struct.pack('!I',len(data))+data
    while data:
        count = os.write(fd,data)
        if not count: raise ValueError('collector lost')
        data = data[count:]

def children(task):
    # Q31: kernel list of a payload PID-1 thread's direct children (adopted orphans included).
    with open('/proc/self/task/'+task+'/children') as stream: return stream.read()

def artifact_package(item):
    return tomllib.loads(Path(item['manifest_path']).read_text())['package']['name']

class Runner:
    def __init__(self, log, status):
        self.log, self.status = log, status
        self.deadline, self.total, self.serial = time.monotonic()+14000, 0, 0
        self.children, self.artifacts, self.results = set(), [], {}
    def reap_orphans(self):
        # Q31: PID 1 adopts every orphan; reap exited ones promptly, as a host init does, so a
        # killed descendant never lingers as a zombie that kill -0 still reports. The runner's
        # own Popen children are never waited here: Popen keeps sole ownership of their status.
        owned = {proc.pid for proc in self.children}
        main = str(os.getpid())
        for task in os.listdir('/proc/self/task'):
            try: text = children(task)
            except FileNotFoundError:
                if task == main: raise
                continue  # A non-main thread exited meanwhile.
            for pid in map(int,text.split()):
                if pid in owned: continue
                try: os.waitpid(pid,os.WNOHANG)
                except ChildProcessError: pass  # Already reaped by an exiting thread's wait.
    def sealed(self, name, block):
        # An empty file still yields its log (as an empty command capture does): A7's start markers are empty.
        self.output(name,block) if block else packet(self.log,{'id':name,'data':''})
    def output(self, name, block):
        self.total += len(block)
        if self.total > 64*1024**2: raise ValueError('run log bound')
        for offset in range(0,len(block),2048):
            packet(self.log,{'id':name,'data':base64.b64encode(block[offset:offset+2048]).decode()})
    def command(self, gate, argv, env=None, cwd=None):
        self.serial += 1
        name = 'c'+str(self.serial)
        packet(self.status,dict(event='start',id=name,gate_id=gate,argv=argv,
                                cwd=cwd or '/work/src',environment=env or ENV))
        options = child_options(subprocess.PIPE)
        if env is not None: options.update(env=dict(env),cwd=cwd)
        # Parsed results are stdout only (cargo JSON, libtest listings/results); stderr, where test
        # helpers' panics land, is still logged but cannot split a parsed line (Q33, diag15 c75).
        if gate in ('compile-tests','workspace','workspace-list','desktop-live','desktop-live-list'):
            options['stderr'] = subprocess.PIPE
        proc = subprocess.Popen(argv,**options)
        self.children.add(proc)
        output = bytearray()
        log_start = self.total
        selector = selectors.DefaultSelector()
        selector.register(proc.stdout,selectors.EVENT_READ)
        if proc.stderr is not None: selector.register(proc.stderr,selectors.EVENT_READ)
        try:
            while selector.get_map():
                if time.monotonic() >= self.deadline: raise TimeoutError('payload deadline')
                self.reap_orphans()
                for key, _ in selector.select(.1):
                    block = os.read(key.fd,65536)
                    if not block: selector.unregister(key.fileobj)
                    else:
                        self.output(name,block)
                        if key.fileobj is proc.stdout: output.extend(block)
            code = proc.wait(timeout=max(.001,self.deadline-time.monotonic()))
            if self.total == log_start:
                packet(self.log,{'id':name,'data':''})
            packet(self.status,dict(event='end',id=name,exit_code=code,signal=-code if code < 0 else None))
            self.results[name] = (gate,argv,code)
            return name, code, output.decode('utf-8',errors='replace')
        finally:
            selector.close()
            proc.stdout.close()
            if proc.stderr is not None: proc.stderr.close()
            # The owned unreaped leader reserves its process group until wait.
            if proc.poll() is None:
                os.killpg(proc.pid,signal.SIGKILL)
                proc.wait(timeout=5)
            self.children.discard(proc)
    def libtest(self, gate, artifact, expected=None):
        _, code, listing = self.command(gate+'-list',[artifact['executable'],'--list','--format','terse'])
        # Q33: a zero-test target lists nothing; reconcile then requires libtest's own zero-test run.
        # One pass validates and extracts (review F6-3 B1): every line is "<path>: test", LF only.
        lines = listing.split('\n')
        if lines[-1] == '': lines.pop()
        matches = [re.fullmatch(r'(\S+): test',line) for line in lines]
        if code or not all(matches): raise ValueError('unknown test adapter')
        names = [match[1] for match in matches]
        if expected is not None:
            if set(names) != set(expected): raise ValueError('desktop names changed')
            names = expected
        name, code, output = self.command(gate,[artifact['executable']])
        inventory = reconcile(names,output)
        if code: raise ValueError('libtest failed')
        if expected is not None and inventory['ignored']: raise ValueError('desktop test ignored')
        self.output(name+'-inventory',json.dumps(dict(artifact=artifact,inventory=inventory)).encode())
        return names
    def descendants(self):
        pids = set()
        for task in os.listdir('/proc/self/task'):
            try: pids.update(map(int,children(task).split()))
            except FileNotFoundError: continue
        return pids
    def quiet(self, stage):
        """No process but this init: as PID 1 every process of the namespace is its descendant, and
        orphans are adopted. Exited ones are reaped first; any survivor is killed by its own PID
        (never kill(-1)) while still unreaped, so no PID reuse, and the run fails."""
        self.reap_orphans()
        survivors = self.descendants()
        if not survivors:
            self.quiescence[stage] = True
            return
        deadline, remaining = time.monotonic()+10, survivors
        while remaining and time.monotonic() < deadline:
            for pid in remaining:
                try: os.kill(pid,signal.SIGKILL)
                except ProcessLookupError: pass
            time.sleep(.05)
            self.reap_orphans()
            remaining = self.descendants()
        raise ValueError('QUIESCENCE: '+stage+': process survived and was killed: '+' '.join(map(str,sorted(survivors)))
                         +(' (still present: '+' '.join(map(str,sorted(remaining)))+')' if remaining else ''))
    def seal(self, a7):
        """Read A7's verdict, then on pass its exact evidence set, without following links, into the
        private stream. A failed verdict is reported as itself, not as missing evidence."""
        verdict = read_regular(a7['verdict'])
        self.sealed('a7-verdict.json',verdict)
        sealed, expected = {'verdict':verdict}, {}
        try: passed = json.loads(verdict).get('verdict') == 'pass'
        except (ValueError, AttributeError): passed = False
        if not passed: raise ValueError('A7: run verdict is not pass')
        for rel in a7['evidence']: expected.setdefault(str(Path(rel).parent),set()).add(Path(rel).name)
        for directory in [a7['evidence_dir']]+[a7['evidence_dir']+'/'+d for d in expected]:
            fd = open_beneath(directory,DIRECTORY)
            try: names = set(os.listdir(fd))
            finally: os.close(fd)
            want = set(expected) if directory == a7['evidence_dir'] else expected[directory.rsplit('/',1)[1]]
            if names != want: raise ValueError('ROUTE_EVIDENCE: unexpected evidence set in '+directory)
        for rel in a7['evidence']:
            sealed[rel] = read_regular(a7['evidence_dir']+'/'+rel)
            self.sealed('a7-'+rel.replace('/','-'),sealed[rel])
        return sealed
    def verify_harness(self, path, expected):
        try: fd = open_beneath(path)
        except OSError as exc: raise ValueError('HARNESS: missing/linked '+path) from exc
        try:
            info, digest = os.fstat(fd), hashlib.sha256()
            if not stat.S_ISREG(info.st_mode): raise ValueError('HARNESS: not a regular file')
            while block := os.read(fd,1024*1024): digest.update(block)
        finally: os.close(fd)
        if digest.hexdigest() != expected: raise ValueError('HARNESS: bytes differ from A7 lane inventory')
    def route(self, policy, request):
        """SB3E-R18 P1 sequence, R19.3 as replaced by R20. Any failure raises; nothing is retried."""
        (a7, harness), env, self.quiescence = policy['gates'], policy['environment'], {}
        self.quiet('before-a7')
        _, code, _ = self.command('a7',a7['argv'],env,ROUTE_CWD['a7'])
        self.quiet('after-a7')
        sealed = self.seal(policy['a7'])
        if code: raise ValueError('A7: run exited nonzero')
        meta, files = json.loads(sealed['lane/meta.json']), json.loads(sealed['lane/inventory.json'])['files']
        self.output('versions',json.dumps(dict(rustc=meta['rustc'],cargo=meta['cargo'])).encode())
        found = [rel for rel in files if re.fullmatch(harness['executable'],policy['a7']['target']+'/'+rel)]
        if len(found) != 1: raise ValueError('HARNESS: A7 lane inventory has no unique route_cells executable')
        path, expected = policy['a7']['target']+'/'+found[0], files[found[0]]
        artifact = dict(id=harness['target'],adapter='libtest',executable=path,package=harness['package'],target=harness['target'])
        self.output('coverage',json.dumps(dict(artifacts=[artifact],failures=[])).encode())
        self.verify_harness(path,expected)
        _, code, listing = self.command('route-cells-list',[path,'--list','--format','terse'],env,ROUTE_CWD['route-cells'])
        lines = listing.split('\n')
        if lines[-1] == '': lines.pop()
        matches = [re.fullmatch(r'(\S+): test',line) for line in lines]
        if code or not matches or not all(matches): raise ValueError('HARNESS: listing failed or malformed')
        names = [match[1] for match in matches]
        if hashlib.sha256('\n'.join(names).encode()).hexdigest() != request['inventory_sha256']:
            raise ValueError('INVENTORY: listed names differ from the submitted inventory_sha256')
        self.output('route-lane',json.dumps(dict(harness_sha256=expected,names=names,rustc_vv=meta['rustc'],cargo_v=meta['cargo'])).encode())
        self.quiet('after-list')
        self.verify_harness(path,expected)  # with only init alive, nothing runs between this and the exec
        name, code, output = self.command('route-cells',[path],env,ROUTE_CWD['route-cells'])
        inventory = reconcile(names,output)
        if code or inventory['ignored']: raise ValueError('route-cells: failed or ignored test')
        self.output(name+'-inventory',json.dumps(dict(artifact=artifact,inventory=inventory)).encode())
    def gates(self):
        policy = json.loads(Path('/gate/policy/gates.json').read_bytes())
        exclusions = json.loads(Path('/gate/policy/exclusions.json').read_bytes())
        failures, excluded = [], []
        for gate in policy['gates']:
            try:
                kind = gate['id']
                if kind == 'host':
                    if not any(a['package'] == gate['package'] for a in self.artifacts): raise ValueError('host coverage missing')
                elif kind == 'whitespace':
                    for path in Path('/work/src').rglob('*'):
                        if not path.is_file() or path.is_symlink(): continue
                        data = path.read_bytes()
                        if b'\0' not in data and any(line.rstrip(b' \t') != line for line in data.splitlines()):
                            raise ValueError('snapshot whitespace approximation: '+str(path))
                elif 'argv' in gate:
                    _, code, output = self.command(kind,gate['argv'])
                    if code: raise ValueError(kind+' nonzero')
                    if kind == 'compile-tests':
                        for line in output.splitlines():
                            item = json.loads(line)
                            if item.get('reason') != 'compiler-artifact' or not item.get('executable'): continue
                            if not item.get('profile',{}).get('test'): continue
                            target = item['target']['name']
                            executable = item['executable']
                            if (not executable.startswith('/work/target/') or '..' in executable.split('/')
                                    or Path(executable).resolve() != Path(executable)):
                                raise ValueError('artifact escaped private target')
                            package = artifact_package(item)
                            if (package,target) == ('nexus-verifier-sandbox',exclusions['target']):
                                excluded.append(item)
                                continue
                            self.artifacts.append(dict(id='a'+str(len(self.artifacts)),adapter='libtest',
                                                       package=package,target=target,executable=executable))
                        if len(excluded) != 1: raise ValueError('EXCLUSION_ARTIFACT: missing/duplicate test artifact')
                        source = Path('/work/src/crates/nexus-verifier-sandbox/tests/phase2_live_sandbox.rs')
                        if (Path(excluded[0]['target'].get('src_path','')) != source
                                or source.is_symlink() or source.resolve() != source):
                            raise ValueError('EXCLUSION_SOURCE: unexpected path or symlink')
                        data = source.read_bytes()
                        blob = hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()
                        if blob != exclusions['source_blob']: raise ValueError('EXCLUSION_BLOB: source identity changed')
                        names = re.findall(rb'"(p2[a-z][0-9]*_live_[a-z0-9_]+)"',data)
                        if (len(names) != 39 or len(set(names)) != 39 or len(exclusions['cases']) != 39
                                or {n.decode() for n in names} != set(exclusions['cases'])):
                            raise ValueError('EXCLUSION_CASES: missing/extra/duplicate name')
                elif kind == 'workspace':
                    if not self.artifacts: raise ValueError('missing compilation inventory')
                    for artifact in self.artifacts:
                        try: self.libtest(kind,artifact)
                        except (ValueError,OSError) as exc: failures.append(str(exc))
                elif kind == 'desktop-live':
                    desktop = [a for a in self.artifacts if (a['package'],a['target']) == (gate['package'],gate['target'])]
                    if len(desktop) != 1: raise ValueError('missing/duplicate desktop artifact')
                    self.libtest(kind,desktop[0],gate['expected_names'])
                if kind in ('host','whitespace'):
                    self.serial += 1
                    name = 'c'+str(self.serial)
                    packet(self.status,dict(event='start',id=name,gate_id=kind,
                           argv=None,internal=True,cwd='/work/src',environment=ENV))
                    self.output(name,(kind+' internal reconciliation complete\n').encode())
                    packet(self.status,dict(event='end',id=name,exit_code=0,signal=None))
            except (ValueError,OSError,subprocess.SubprocessError) as exc:
                failures.append(str(exc))
        self.output('coverage',json.dumps(dict(artifacts=self.artifacts,failures=failures)).encode())
        if failures: raise ValueError('; '.join(failures))

def observations():
    spec = importlib.util.spec_from_file_location('gbx_snapshot',Path(__file__).with_name('snapshot.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    result = {}
    for name, path in (('rust',Path('/opt/rust')),('registry',Path('/inputs/registry'))):
        if not path.is_dir() or not any(path.iterdir()): raise ValueError('missing/empty host input: '+name)
        result[name] = module.manifest(path,time.monotonic()+300)[0]
    return result

def probe_module():
    spec = importlib.util.spec_from_file_location('gbx_probe',os.path.join(os.path.dirname(__file__),'probe.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

def probe_payload(private_fds): return probe_module().payload(private_fds)

def resources():
    result = {}
    for name in ('/work','/run/gate','/tmp','/dev/shm'):
        data = os.statvfs(name)
        result[name] = {'bytes_used':(data.f_blocks-data.f_bfree)*data.f_frsize,
                        'bytes_total':data.f_blocks*data.f_frsize,
                        'inodes_used':data.f_files-data.f_ffree,'inodes_total':data.f_files}
    return result

def route_policy():
    return json.loads(Path('/gate/policy/route-cells.json').read_bytes())

def route_main(runner, request):
    policy = route_policy()
    for directory in ('/work/home','/work/home/.cargo','/run/gate/tmp'): Path(directory).mkdir(mode=0o700)
    Path('/work/home/.cargo/registry').symlink_to('/inputs/registry')
    shutil.copytree('/input','/work/src',symlinks=True)
    for base, dirs, files in os.walk('/work/src'):
        Path(base).chmod(0o700)
        for name in files:
            path = Path(base)/name
            if not path.is_symlink(): path.chmod(0o700 if path.stat().st_mode & 0o111 else 0o600)
    before = observations()
    if before['rust'] != policy['toolchain']['manifest_sha256']:  # SB3E-R18 P8, before A7
        raise ValueError('TOOLCHAIN_PIN: /opt/rust differs from the reviewed manifest')
    peak = probe_module().Peak(resources)
    peak.start()
    try:
        runner.route(policy,request)
        runner.output('isolation',json.dumps(dict(quiescence=runner.quiescence,probe='not run: route-cells (SB3E-R18 P1)')).encode())
    finally:
        runner.output('resources',json.dumps(peak.finish()).encode())
        runner.output('inputs',json.dumps({'before':before,'after':observations()}).encode())
    packet(runner.status,{'event':'complete'})

def main():
    if os.getpid() != 1 or os.getuid() == 0: raise ValueError('trusted init requires unprivileged PID 1')
    if ctypes.CDLL(None,use_errno=True).prctl(4,0,0,0,0) != 0: raise OSError('cannot disable dumpability')
    log, status = os.dup(1), os.dup(2)
    os.set_inheritable(log,False)
    os.set_inheritable(status,False)
    null = os.open('/dev/null',os.O_RDWR)
    for fd in (0,1,2): os.dup2(null,fd)
    os.close(null)
    runner = Runner(log,status)
    def terminate(signum, frame): raise SystemExit(128+signum)
    signal.signal(signal.SIGTERM,terminate)
    request = requested()
    try:
        if request.get('gate_set_id') == 'route-cells': return route_main(runner,request)
        for directory in ('/work/home','/work/cargo','/work/target','/work/cache','/run/gate/tmp','/run/gate/xdg'):
            Path(directory).mkdir(mode=0o700)
        Path('/work/cargo/registry').symlink_to('/inputs/registry')
        shutil.copytree('/input','/work/src',symlinks=True)
        for base, dirs, files in os.walk('/work/src'):
            Path(base).chmod(0o700)
            for name in files:
                path = Path(base)/name
                if not path.is_symlink(): path.chmod(0o700 if path.stat().st_mode & 0o111 else 0o600)
        before = observations()
        peak = probe_module().Peak(resources)
        peak.start()
        try:
            isolation, versions = probe_payload((log,status))
            runner.output('isolation',json.dumps(isolation).encode())
            runner.output('versions',json.dumps(versions).encode())
            runner.gates()
        finally:
            # Stop/join the sampler even if final input inventory or capture fails.
            accounting = peak.finish()
            runner.output('resources',json.dumps(accounting).encode())
            runner.output('inputs',json.dumps({'before':before,'after':observations()}).encode())
        packet(status,{'event':'complete'})
    finally:
        for proc in runner.children:
            if proc.poll() is None:
                os.killpg(proc.pid,signal.SIGKILL)
                proc.wait(timeout=5)
        # PID 1 owns orphan reaping; systemd's control-group stop owns remaining descendants.
        while True:
            try:
                if os.waitpid(-1,os.WNOHANG)[0] == 0: break
            except ChildProcessError: break
        os.close(log)
        os.close(status)

if __name__ == '__main__': main()
