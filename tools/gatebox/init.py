"""Trusted payload init: isolated child environment and libtest reconciliation."""
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
import struct
import time
ENV = {'PATH':'/opt/rust/bin:/usr/bin:/bin','HOME':'/work/home','PWD':'/work/src',
       'CARGO_HOME':'/work/cargo','CARGO_TARGET_DIR':'/work/target','CARGO_BUILD_JOBS':'4',
       'RUST_TEST_THREADS':'4','CARGO_TERM_COLOR':'never','CARGO_NET_OFFLINE':'true',
       'RUST_BACKTRACE':'1','TMPDIR':'/run/gate/tmp','XDG_RUNTIME_DIR':'/run/gate/xdg',
       'XDG_CACHE_HOME':'/work/cache','LANG':'C.UTF-8','LC_ALL':'C.UTF-8','TZ':'UTC','CI':'true'}

def child_options(log_fd):
    return {'stdin':subprocess.DEVNULL,'stdout':log_fd,'stderr':subprocess.STDOUT,
            'close_fds':True,'env':dict(ENV),'cwd':'/work/src','start_new_session':True}

def reconcile(names, output):
    passed = re.findall(r'^test (.+) \.\.\. ok$',output,re.M)
    totals = re.findall(r'^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;',output,re.M)
    if (not names or len(set(names)) != len(names) or len(passed) != len(names)
            or set(passed) != set(names) or totals != [(str(len(names)),'0','0','0','0')]
            or re.search(r'^test .+ \.\.\. (?!ok$)',output,re.M)):
        raise ValueError('missing/unexpected/failed libtest names or totals')
    return {'expected':names,'executed':names,'passed':names,'failed':[],'ignored':[],'filtered':[]}

def packet(fd, value):
    data = json.dumps(value,separators=(',',':')).encode()
    if len(data) > 4096: raise ValueError('private frame bound')
    data = struct.pack('!I',len(data))+data
    while data:
        count = os.write(fd,data)
        if not count: raise ValueError('collector lost')
        data = data[count:]

class Runner:
    def __init__(self, log, status):
        self.log, self.status = log, status
        self.deadline, self.total, self.serial = time.monotonic()+14000, 0, 0
        self.children, self.artifacts, self.results = set(), [], {}
    def output(self, name, block):
        self.total += len(block)
        if self.total > 64*1024**2: raise ValueError('run log bound')
        for offset in range(0,len(block),2048):
            packet(self.log,{'id':name,'data':base64.b64encode(block[offset:offset+2048]).decode()})
    def command(self, gate, argv):
        self.serial += 1
        name = 'c'+str(self.serial)
        packet(self.status,dict(event='start',id=name,gate_id=gate,argv=argv,
                                cwd='/work/src',environment=ENV))
        proc = subprocess.Popen(argv,**child_options(subprocess.PIPE))
        self.children.add(proc)
        output = bytearray()
        selector = selectors.DefaultSelector()
        selector.register(proc.stdout,selectors.EVENT_READ)
        try:
            while selector.get_map():
                if time.monotonic() >= self.deadline: raise TimeoutError('payload deadline')
                for key, _ in selector.select(.1):
                    block = os.read(key.fd,65536)
                    if not block: selector.unregister(key.fileobj)
                    else:
                        self.output(name,block)
                        output.extend(block)
            code = proc.wait(timeout=max(.001,self.deadline-time.monotonic()))
            packet(self.status,dict(event='end',id=name,exit_code=code,signal=-code if code < 0 else None))
            self.results[name] = (gate,argv,code)
            return name, code, output.decode('utf-8',errors='replace')
        finally:
            selector.close()
            proc.stdout.close()
            # The owned unreaped leader reserves its process group until wait.
            if proc.poll() is None:
                os.killpg(proc.pid,signal.SIGKILL)
                proc.wait(timeout=5)
            self.children.discard(proc)
    def libtest(self, gate, artifact, expected=None):
        _, code, listing = self.command(gate+'-list',[artifact['executable'],'--list','--format','terse'])
        names = re.findall(r'^(.+): test$',listing,re.M)
        if code or ': benchmark' in listing or not names: raise ValueError('unknown/empty test adapter')
        if expected is not None:
            if set(names) != set(expected): raise ValueError('desktop names changed')
            names = expected
        name, code, output = self.command(gate,[artifact['executable']])
        inventory = reconcile(names,output)
        if code: raise ValueError('libtest failed')
        self.output(name+'-inventory',json.dumps(dict(artifact=artifact,inventory=inventory)).encode())
        return names
    def gates(self):
        policy = json.loads(Path('/gate/policy/gates.json').read_bytes())
        exclusions = json.loads(Path('/gate/policy/exclusions.json').read_bytes())
        failures = []
        for gate in policy['gates']:
            try:
                kind = gate['id']
                if 'argv' in gate:
                    _, code, output = self.command(kind,gate['argv'])
                    if code: raise ValueError(kind+' nonzero')
                    if kind == 'compile-tests':
                        for line in output.splitlines():
                            item = json.loads(line)
                            if item.get('reason') != 'compiler-artifact' or not item.get('executable'): continue
                            if not item.get('profile',{}).get('test'): continue
                            target = item['target']['name']
                            if target == exclusions['target']: continue
                            executable = item['executable']
                            if (not executable.startswith('/work/target/') or '..' in executable.split('/')
                                    or Path(executable).resolve() != Path(executable)):
                                raise ValueError('artifact escaped private target')
                            package = item['package_id'].split('#')[-1].split('@')[0].split(' ')[0]
                            self.artifacts.append(dict(id='a'+str(len(self.artifacts)),adapter='libtest',
                                                       package=package,target=target,executable=executable))
                elif kind == 'workspace':
                    if not self.artifacts: raise ValueError('missing compilation inventory')
                    for artifact in self.artifacts:
                        try: self.libtest(kind,artifact)
                        except (ValueError,OSError) as exc: failures.append(str(exc))
                elif kind == 'desktop-live':
                    desktop = [a for a in self.artifacts if (a['package'],a['target']) == (gate['package'],gate['target'])]
                    if len(desktop) != 1: raise ValueError('missing/duplicate desktop artifact')
                    self.libtest(kind,desktop[0],gate['expected_names'])
                elif kind == 'host':
                    if not any(a['package'] == gate['package'] for a in self.artifacts): raise ValueError('host coverage missing')
                elif kind == 'whitespace':
                    for path in Path('/work/src').rglob('*'):
                        if not path.is_file() or path.is_symlink(): continue
                        data = path.read_bytes()
                        if b'\0' not in data and any(line.rstrip(b' \t') != line for line in data.splitlines()):
                            raise ValueError('snapshot whitespace approximation: '+str(path))
                if kind in ('host','whitespace'):
                    self.serial += 1
                    name = 'c'+str(self.serial)
                    packet(self.status,dict(event='start',id=name,gate_id=kind,
                           argv=['trusted-init-internal',kind],cwd='/work/src',environment=ENV))
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

def resources():
    result = {}
    for name in ('/work','/run/gate'):
        data = os.statvfs(name)
        result[name] = {'bytes_used':(data.f_blocks-data.f_bfree)*data.f_frsize,
                        'bytes_total':data.f_blocks*data.f_frsize,
                        'inodes_used':data.f_files-data.f_ffree,'inodes_total':data.f_files}
    return result

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
    try:
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
        try: runner.gates()
        finally:
            runner.output('inputs',json.dumps({'before':before,'after':observations()}).encode())
            runner.output('resources',json.dumps(resources()).encode())
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
