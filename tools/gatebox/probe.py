"""Installed Linux/x86-64 qualification sensors and protected evidence writer.

Observations are made before candidate execution. Receipt authentication still
trusts the supervisor; the deferred broker-compromise race is not closed here.
"""
import base64
import ctypes
import errno
import fcntl
import hashlib
import importlib.util
import json
import mmap
import os
from pathlib import Path
import platform
import pty
import pwd
import re
import select
import selectors
import signal
import socket
import stat
import struct
import subprocess
import sys
import tempfile
import termios
import threading
import time
import uuid

HERE = Path(__file__).resolve().parent
CONFIG = Path('/etc/gatebox')
FIXTURE = '8e0d6ae293df999eb6530e875df1206deac872a8'
HARNESS = Path('/usr/libexec/gatebox/policy/phase2_live_sandbox.rs')
SESSION = None
CUSTODY_TESTS = {
    'C1-SEALED':'test_mothergate_cannot_unlink_rename_or_symlink_after_relocation',
    'C2-CANARY':'test_systemd_output_never_writes_disposable_root_canary',
    'C3-RESTART':'test_pending_manager_job_survives_collector_death_and_supervisor_restart'}
REQUIRED_ROWS = {'B1','B2','B3','B4','A1','A5','A6','A8','E2E','CLIENTS'} | set(CUSTODY_TESTS)
POLKIT_RULE = Path('/etc/polkit-1/rules.d/00-gatebox-deny.rules')

def sibling(name):
    spec = importlib.util.spec_from_file_location('probe_'+name,HERE/(name+'.py'))
    module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
    return module

def require(condition,message):
    if not condition: raise ValueError(message)

def harness_check(data,policy):
    require(hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest() == policy['source_blob'],'EXCLUSION_BLOB')
    names = re.findall(rb'"(p2[a-z][0-9]*_live_[a-z0-9_]+)"',data)
    require(len(names) == len(set(names)) == len(policy['cases']) == 39
            and {n.decode() for n in names} == set(policy['cases']),'EXCLUSION_CASES')

def parse_mounts(text):
    result = []
    try:
        for line in text.splitlines():
            left,right = line.split(' - '); fields,tail = left.split(),right.split()
            require(len(fields) >= 6 and len(tail) >= 3,'mount fields')
            result.append(dict(device=fields[2],root=fields[3],point=fields[4],
                               options=fields[5].split(','),type=tail[0],super=tail[2].split(',')))
    except (ValueError,IndexError) as exc: raise ValueError('malformed mountinfo') from exc
    require(result,'empty mountinfo'); return result

def descendant(pid,root,parents):
    seen = set()
    while pid in parents and pid not in seen:
        seen.add(pid); pid = parents[pid]
        if pid == root: return True
    return False

def usage():
    result = {}
    for name in ('/work','/run/gate','/tmp','/dev/shm'):
        data = os.statvfs(name)
        result[name] = dict(bytes_used=(data.f_blocks-data.f_bfree)*data.f_frsize,
                           bytes_total=data.f_blocks*data.f_frsize,
                           inodes_used=data.f_files-data.f_ffree,inodes_total=data.f_files)
    return result

class Peak:
    """Observed tmpfs high water at 50ms intervals; no exhaustion-proof claim."""
    def __init__(self,measure=None):
        self.measure = measure
        self.peaks,self.samples,self.errors = {},0,[]
        self.done = threading.Event()
    def sample(self):
        for path,values in (self.measure or usage)().items():
            old = self.peaks.setdefault(path,{})
            for key,value in values.items(): old[key] = max(old.get(key,0),value)
        self.samples += 1
    def start(self):
        self.sample()
        def work():
            try:
                while not self.done.wait(.05): self.sample()
            except Exception as exc: self.errors.append(str(exc))
        self.thread = threading.Thread(target=work); self.thread.start()
    def finish(self):
        self.done.set(); self.thread.join(timeout=2)
        require(not self.thread.is_alive() and not self.errors,'scratch sampler failed')
        self.sample()
        target = sum(p.stat().st_size for p in Path('/work/target').rglob('*') if p.is_file() and not p.is_symlink())
        return dict(observed_peaks=self.peaks,samples=self.samples,interval_seconds=.05,target_bytes=target)

def stop(proc):
    # Caller never reaps/polls first: the owned leader reserves its process group.
    try: os.killpg(proc.pid,signal.SIGKILL)
    except ProcessLookupError: pass
    return proc.wait(timeout=5)

def capture(argv,timeout=30,limit=1024**2,env=None):
    if env is None:
        env = (sibling('init').ENV if os.getpid()==1 else
               {'PATH':'/usr/sbin:/usr/bin:/sbin:/bin','HOME':'/root','LANG':'C.UTF-8'})
    proc = subprocess.Popen(argv,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,
                            close_fds=True,start_new_session=True,env=env)
    output = bytearray(); deadline = time.monotonic()+timeout
    selector = selectors.DefaultSelector(); selector.register(proc.stdout,selectors.EVENT_READ)
    try:
        while selector.get_map():
            require(time.monotonic() < deadline,'probe command deadline: '+str(argv))
            for key,_ in selector.select(.05):
                block = os.read(key.fd,8192)
                if not block: selector.unregister(key.fileobj)
                else: output.extend(block)
                require(len(output) <= limit,'probe command output bound')
        while os.waitid(os.P_PID,proc.pid,os.WEXITED|os.WNOHANG|os.WNOWAIT) is None:
            require(time.monotonic() < deadline,'probe exit deadline'); time.sleep(.01)
    finally:
        selector.close(); code = stop(proc); proc.stdout.close()
    require(code == 0,'probe command failed: '+str(argv)+' '+output.decode(errors='replace')[:1000])
    return output.decode(errors='replace')

def mounts(): return parse_mounts(Path('/proc/self/mountinfo').read_text())
def status(pid='self'): return dict(line.split(':',1) for line in Path('/proc',str(pid),'status').read_text().splitlines() if ':' in line)
def cmdline_tokens(raw):
    # Q23/Q27: Chrome rewrites its process title into one space-joined string, and Chromium trims
    # ASCII whitespace around arguments; split on NUL and every ASCII whitespace byte.
    return [t for part in raw.split(b'\0') for t in part.split() if t]
def chrome_args(raw):
    # Q27/Q28: NUL boundaries are exact and keep empty positions; only a rewritten title (one
    # string followed by NUL padding) is space-joined.
    body = raw.rstrip(b'\0')
    return body.split(b'\0') if b'\0' in body else body.split(b' ')
def chrome_process_is_renderer(raw):
    # Q26/Q27: the denial over-approximates every Chromium spelling (any '-' prefix, '=value',
    # surrounding whitespace, even after '--'); the renderer type follows Chromium's parser exactly.
    names = {t.lstrip(b'-').partition(b'=')[0] for t in cmdline_tokens(raw) if t.startswith(b'-')}
    require(not {b'no-sandbox',b'disable-setuid-sandbox'} & names,'unsafe Chrome flag')
    kind = None
    for arg in chrome_args(raw)[1:]:
        arg = arg.strip(b' \t\n\v\f\r')
        if arg==b'--': break
        body = arg[2:] if arg.startswith(b'--') else arg[1:] if arg.startswith(b'-') else None
        if body is None: continue  # '---type' yields name '-type', never 'type'
        name,_,value = body.partition(b'=')
        if name==b'type': kind = value  # Chromium: the last duplicate switch wins
    return kind==b'renderer'
def renderer_isolated(info,browser):
    # Q26: Seccomp=2 alone is systemd's filter; Chrome's own filter adds to the browser's filter count.
    filters,base = int(info['Seccomp_filters']),int(browser['Seccomp_filters'])
    levels = len(info['NSpid'].split())
    require(info['Seccomp'].strip()=='2' and levels>=3 and filters>base,'renderer isolation')
    return dict(seccomp=2,pid_levels=levels,seccomp_filters=filters,browser_seccomp_filters=base)
def select_renderer(candidates,parents,browser_pid,browser):
    # Q28: a renderer publishes its title before installing Chrome's filter. Return evidence only
    # when every descendant renderer in this sample is isolated; otherwise None (keep waiting
    # within the caller's bounded deadline, which fails closed).
    chosen = None
    for pid,info in sorted(candidates.items()):
        if not descendant(pid,browser_pid,parents): continue
        try: evidence = renderer_isolated(info,browser)
        except ValueError: return None
        chosen = chosen or dict(evidence,pid=pid,parents=parents)
    return chosen

def whole_mount(records, point, readonly=False):
    # Q30: the product's workspace rule (files::mount_of) on the live payload: exactly one entry
    # at the point, root '/', its device listed exactly once, and a whole tmpfs.
    at = [m for m in records if m['point']==point]
    require(len(at)==1,'whole mount: '+point)
    entry = at[0]
    require(entry['type']=='tmpfs' and entry['root']=='/'
            and sum(m['device']==entry['device'] for m in records)==1,'whole mount: '+point)
    if readonly: require('ro' in entry['options'],'read-only mount: '+point)
    return entry

def isolation(context):
    records = mounts(); current = status()
    points = {m['point']:m for m in records}
    gate = points['/run/gate']; run = points['/run']
    require(gate['type'] == 'tmpfs' and gate['root'] == '/' and sum(m['device']==gate['device'] for m in records)==1,'whole /run/gate tmpfs')
    require(run['type'] == 'tmpfs' and 'noexec' in run['options'] and 'size=16384k' in run['super'],'/run 16MiB noexec')
    whole_mount(records,'/',readonly=True); whole_mount(records,'/work'); whole_mount(records,'/tmp')
    require(os.getpid()==1 and os.getuid()==context['payload_uid'] and Path('/proc/self/uid_map').read_text().split()==['0','0','4294967295'],'PID/UID mapping')
    require(current['NoNewPrivs'].strip()=='1' and all(int(current[k],16)==0 for k in ('CapInh','CapPrm','CapEff','CapBnd','CapAmb')),'caps/NNP')
    namespaces = {n:os.readlink('/proc/self/ns/'+n) for n in ('pid','mnt','ipc','uts','net')}
    require(all(namespaces[n] != context['namespaces'][n] for n in namespaces),'shared host namespace')
    require(socket.gethostname()=='gatebox','UTS hostname')
    for name in ('/usr','/opt/google/chrome','/gate/bin','/gate/policy','/opt/rust','/inputs/registry','/input','/sys/fs/cgroup'):
        require(name in points and 'ro' in points[name]['options'],'readonly mount: '+name)
    require(points['/sys/fs/cgroup']['type']=='cgroup2','cgroup2 missing')
    for name in ('/gate/bin/init.py','/opt/google/chrome/chrome'):
        for path in [Path(name),*Path(name).parents]:
            info = path.stat(); require(info.st_uid==0 and not info.st_mode & 0o022,'unsafe code ancestry')
    tmp = Path('/run/gate/tmp')
    for path in [tmp,*tmp.parents]:
        info = path.lstat()
        require(info.st_uid in (0,os.getuid()) and (not info.st_mode & 0o022 or info.st_mode & stat.S_ISVTX),'tmp ancestry')
    require(stat.S_IMODE(tmp.stat().st_mode)==0o700,'TMPDIR mode')
    a,b = tmp/'fsync-a',tmp/'fsync-b'
    with a.open('xb') as stream: stream.write(b'x'); stream.flush(); os.fsync(stream.fileno())
    a.rename(b); fd = os.open(tmp,os.O_RDONLY|os.O_DIRECTORY)
    try: os.fsync(fd)
    finally: os.close(fd); b.unlink()
    for name in ('/gate/outside','/usr/gatebox-write-probe','/etc/gatebox-write-probe','/sys/fs/cgroup/gatebox-write-probe'):
        try: fd = os.open(name,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
        except OSError as exc: require(exc.errno in (errno.EROFS,errno.EACCES,errno.EPERM),'unexpected write error')
        else: os.close(fd); Path(name).unlink(); raise ValueError('outside write succeeded: '+name)
    loader = capture(['/usr/bin/awk','BEGIN {print "gatebox-awk"}']).strip()
    require(loader=='gatebox-awk','awk loader/alternatives chain')
    require(not any(Path('/work/target').iterdir()),'target must start cold')
    return dict(target_cold=True,pid=1,uid=os.getuid(),namespaces=namespaces,mounts=records,fsync=True,caps_zero=True,nnp=1,readonly=True,awk=loader)

def non_native_abi():
    # i386 getpid via int 0x80, independent of the Python/native syscall ABI.
    pid = os.fork()
    if pid == 0:
        try:
            with mmap.mmap(-1,4096,prot=mmap.PROT_READ|mmap.PROT_WRITE|mmap.PROT_EXEC) as code:
                code.write(bytes.fromhex('b814000000cd80c3'))
                address = ctypes.addressof(ctypes.c_char.from_buffer(code))
                result = ctypes.CFUNCTYPE(ctypes.c_int)(address)()
                os._exit(0 if result == -errno.EPERM else 1)
        except BaseException: os._exit(2)
    _,status = os.waitpid(pid,0)
    if os.WIFSIGNALED(status) and os.WTERMSIG(status)==signal.SIGSYS: return 'SIGSYS'
    require(os.WIFEXITED(status) and os.WEXITSTATUS(status)==0,'non-native ABI not denied')
    return 'EPERM'

def syscall_probe():
    require(platform.machine()=='x86_64','native ABI probe requires frozen x86_64 host')
    non_native = non_native_abi()
    libc = ctypes.CDLL(None,use_errno=True); libc.syscall.restype = ctypes.c_long; L = ctypes.c_long
    calls = {'ptrace':(101,16,999999,0,0),'process_vm_readv':(310,os.getpid(),0,0,0,0,0),
             'pidfd_getfd':(438,-1,0,0),'perf_event_open':(298,0,0,-1,-1,0)}
    def observed():
        result = {}
        for name,args in calls.items():
            ctypes.set_errno(0); value = libc.syscall(*[L(n) for n in args])
            result[name] = errno.errorcode.get(ctypes.get_errno()) if value < 0 else 'OK'
        return result
    class How(ctypes.Structure): _fields_ = [('flags',ctypes.c_ulonglong),('mode',ctypes.c_ulonglong),('resolve',ctypes.c_ulonglong)]
    directory = os.open('/run/gate/tmp',os.O_RDONLY|os.O_DIRECTORY)
    how = How(os.O_RDONLY|os.O_DIRECTORY,0,0x04|0x08)
    opened = libc.syscall(L(437),L(directory),ctypes.c_char_p(b'.'),ctypes.byref(how),ctypes.sizeof(how))
    os.close(directory); require(opened >= 0,'openat2 denied'); os.close(opened)
    outer = observed(); require(set(outer.values())=={'EPERM'},'outer syscall filter: '+str(outer))
    class Filter(ctypes.Structure): _fields_ = [('code',ctypes.c_ushort),('jt',ctypes.c_ubyte),('jf',ctypes.c_ubyte),('k',ctypes.c_uint)]
    class Program(ctypes.Structure): _fields_ = [('len',ctypes.c_ushort),('filter',ctypes.POINTER(Filter))]
    ins = [Filter(0x20,0,0,0)]
    for args in calls.values(): ins += [Filter(0x15,0,1,args[0]),Filter(0x06,0,0,0x50000|errno.ENOSYS)]
    ins.append(Filter(0x06,0,0,0x7fff0000)); array = (Filter*len(ins))(*ins)
    program = Program(len(ins),array)
    require(libc.syscall(L(317),L(1),L(0),ctypes.byref(program))==0,'nested seccomp installation')
    inner = observed(); require(set(inner.values())=={'ENOSYS'},'nested syscall precedence')
    return dict(openat2=True,outer=outer,inner=inner,abi=platform.machine(),non_native_abi=non_native)

def process_identity(text):
    # comm may contain spaces and parentheses; starttime is field 22, after comm.
    pid,separator,rest = text.partition(' (')
    fields = rest.rsplit(') ',1)
    require(separator and pid.isdecimal() and int(pid)>0 and len(fields)==2,'invalid process stat')
    tail = fields[1].split()
    require(len(tail)>19 and tail[19].isdecimal(),'missing process start time')
    return int(pid),int(tail[19])

def host_pty_denied(path):
    try: fd = os.open(path,os.O_RDWR|os.O_NOCTTY)
    except OSError as exc:
        require(exc.errno in (errno.EACCES,errno.EPERM,errno.ENOENT),'host PTY probe error')
    else:
        os.close(fd); raise ValueError('host PTY accessible')

def processes(context):
    pairs = [pty.openpty(),pty.openpty()]
    try:
        require(os.ttyname(pairs[0][1])!=os.ttyname(pairs[1][1]),'PTY collision')
        for master,slave in pairs:
            os.write(slave,b'gatebox\n'); ready,_,_ = select.select([master],[],[],2)
            require(ready and b'gatebox' in os.read(master,128),'PTY exchange')
            fcntl.ioctl(slave,termios.TIOCGWINSZ,bytes(8))
            try: fcntl.ioctl(slave,termios.TIOCSTI,b'x')
            except OSError as exc: require(exc.errno in (errno.EPERM,errno.EIO),'unexpected TIOCSTI error')
            else: raise ValueError('TIOCSTI injection allowed')
        host_pty_denied(context['pty']['path'])
    finally:
        for pair in pairs:
            for fd in pair: os.close(fd)
    require(not any(Path('/dev').glob('tty[0-9]*')),'VT exposed')
    require(Path('/proc/sys/dev/tty/legacy_tiocsti').read_text().strip()=='0','legacy_tiocsti enabled')
    host_proc = Path('/proc',str(context['pid']),'stat')
    require(not host_proc.exists() or process_identity(host_proc.read_text())!=process_identity(context['process_stat']),
            'host process visible')
    # The init is the parent and reaps this child; child establishes its own session.
    child = os.fork()
    if child == 0:
        try: os.setsid(); os.killpg(os.getpid(),0); os._exit(0)
        except BaseException: os._exit(1)
    require(os.waitpid(child,0)==(child,0),'setsid/killpg/reap')
    calls = json.loads(capture(['/usr/bin/python3','-I','-B',str(HERE/'probe.py'),'--syscalls']))
    return dict(ptys=2,tiocsti_denied=True,host_hidden=True,reaped=True,syscalls=calls)

def exchange(family,address):
    with socket.socket(family) as server,socket.socket(family) as client:
        server.settimeout(2); client.settimeout(2); server.bind(address); server.listen(1)
        client.connect(server.getsockname()); peer,_ = server.accept()
        with peer:
            peer.settimeout(2); client.sendall(b'x'); require(peer.recv(1)==b'x','private socket exchange')

def network(context):
    require(set(os.listdir('/sys/class/net'))=={'lo'},'non-loopback interface')
    exchange(socket.AF_INET,('127.0.0.1',0)); exchange(socket.AF_INET6,('::1',0))
    name = '/run/gate/tmp/socket-probe'
    try: exchange(socket.AF_UNIX,name)
    finally: Path(name).unlink(missing_ok=True)
    for family,address in ((socket.AF_INET,('127.0.0.1',context['port'])),
                           (socket.AF_UNIX,context['socket']),(socket.AF_UNIX,'\0'+context['abstract'])):
        with socket.socket(family) as client:
            client.settimeout(2)
            try: client.connect(address)
            except OSError as exc: require(exc.errno in (errno.ECONNREFUSED,errno.ENOENT,errno.EACCES),'host canary probe error')
            else: raise ValueError('host socket reachable')
    for name in ('/run/systemd/journal/socket','/run/systemd/journal/stdout','/run/systemd/journal/dev-log','/dev/log',
                 '/run/systemd/notify','/run/dbus/system_bus_socket','/run/docker.sock','/tmp/.X11-unix'):
        require(not os.path.lexists(name),'host service path exposed: '+name)
    descriptors = {}
    for path in Path('/proc/self/fd').iterdir():
        try: descriptors[path.name] = os.readlink(path)
        except FileNotFoundError: continue
    # Init's two private capture sockets are expected; candidate never inherits them.
    require(sum(t.startswith('socket:') for t in descriptors.values())==2,'unexpected socket descriptor')
    return dict(loopback4=True,loopback6=True,unix=True,host_denied=True,services_absent=True,fds=descriptors)

def chrome_observation():
    page = Path('/run/gate/tmp/chrome-local.html'); page.write_text('<!doctype html><p>gatebox-local</p>')
    chrome = '/opt/google/chrome/chrome'
    argv = [chrome,'--headless=new','--disable-gpu','--no-first-run','--no-default-browser-check',
            '--user-data-dir=/run/gate/tmp/chrome-observe',page.as_uri()]
    proc = subprocess.Popen(argv,stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE,
                            close_fds=True,start_new_session=True,env=sibling('init').ENV)
    deadline = time.monotonic()+30; diagnostic = bytearray(); renderer = None
    try:
        while renderer is None:
            require(time.monotonic()<deadline and os.waitid(os.P_PID,proc.pid,os.WEXITED|os.WNOHANG|os.WNOWAIT) is None,'Chrome renderer readiness')
            ready,_,_ = select.select([proc.stderr],[],[],.05)
            if ready: diagnostic.extend(os.read(proc.stderr.fileno(),8192))
            require(len(diagnostic)<=1024**2,'Chrome diagnostic bound')
            parents,candidates = {},{}
            for path in Path('/proc').iterdir():
                if not path.name.isdigit(): continue
                try:
                    info = status(path.name); pid = int(path.name); parents[pid] = int(info['PPid'])
                    if chrome_process_is_renderer((path/'cmdline').read_bytes()): candidates[pid] = info
                except (FileNotFoundError,ProcessLookupError): continue
            renderer = select_renderer(candidates,parents,proc.pid,status(proc.pid))
            label = Path('/proc',str(proc.pid),'attr/current').read_text().strip()
    finally: stop(proc); proc.stderr.close()
    base = [chrome,'--headless=new','--disable-gpu','--no-first-run','--user-data-dir=/run/gate/tmp/chrome-report','--dump-dom']
    # Q25: Chrome's sandbox self-report page is unavailable under --headless=new; the evidence is the
    # kernel-observed renderer above (Chrome's own seccomp filter, nested PID namespaces, lineage).
    local = capture(base+[page.as_uri()])
    require('gatebox-local' in local,'Chrome local rendering')
    return dict(argv=argv,browser_pid=proc.pid,renderer=renderer,browser_label=label,local_html=local,
                version=capture([chrome,'--version']).strip())

def desktop():
    # The registered live adapter starts its own displays/buses too. Prove tools here.
    read,write = os.pipe()
    proc = subprocess.Popen(['/usr/bin/Xvfb','-displayfd',str(write),'-screen','0','800x600x24','-nolisten','tcp'],
                            stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,
                            close_fds=True,pass_fds=(write,),start_new_session=True,env=sibling('init').ENV)
    os.close(write)
    try:
        ready,_,_ = select.select([read],[],[],10); require(ready,'Xvfb readiness')
        number = os.read(read,32).decode().strip(); require(number.isdecimal(),'Xvfb displayfd')
        bus = capture(['/usr/bin/dbus-run-session','--','/usr/bin/gdbus','call','--session',
                       '--dest','org.freedesktop.DBus','--object-path','/org/freedesktop/DBus','--method','org.freedesktop.DBus.ListNames'])
        return dict(display=':'+number,private_bus=bus)
    finally: os.close(read); stop(proc)

def validate_sensor(row,value):
    require(isinstance(value,dict),'sensor object')
    required = {'A1':('fsync','caps_zero','readonly'),'A5':('local_html','version','renderer','private_desktop'),
                'A6':('tiocsti_denied','host_hidden','reaped','syscalls'),
                'A8':('loopback4','loopback6','unix','host_denied','services_absent')}
    require(all(value.get(k) for k in required[row]),'missing '+row+' sensor evidence')
    if row=='A1': require(value.get('pid')==1 and value.get('nnp')==1 and value.get('uid',0)>0,'init identity')
    if row=='A5':
        renderer = value['renderer']
        require(renderer.get('seccomp')==2 and renderer.get('pid_levels',0)>=3
                and isinstance(renderer.get('browser_seccomp_filters'),int) and isinstance(renderer.get('seccomp_filters'),int)
                and renderer['seccomp_filters']>renderer['browser_seccomp_filters']>=1
                and descendant(renderer['pid'],value['browser_pid'],{int(k):v for k,v in renderer['parents'].items()}),'renderer lineage')
        require(not any('--no-sandbox' in a for a in value.get('argv',[])),'unsafe flags')
    if row=='A6':
        calls = value['syscalls']
        require(calls.get('openat2') and set(calls.get('outer',{}).values())=={'EPERM'}
                and set(calls.get('inner',{}).values())=={'ENOSYS'}
                and calls.get('non_native_abi') in ('SIGSYS','EPERM'),'syscall observations')

def payload(private_fds=()):
    context = json.loads(Path('/gate/policy/probe-context.json').read_bytes())
    script = 'import errno,os,sys\nfor fd in map(int,sys.argv[1:]):\n try: os.fstat(fd)\n except OSError as e: assert e.errno==errno.EBADF\n else: raise RuntimeError("private fd inherited")\ntry: os.listdir("/proc/1/fd")\nexcept PermissionError: pass\nelse: raise RuntimeError("init descriptors accessible")'
    capture(['/usr/bin/python3','-I','-c',script,*map(str,private_fds)])
    result = {'A1':isolation(context),'A6':processes(context),'A8':network(context)}
    result['A5'] = chrome_observation(); result['A5']['private_desktop'] = desktop()
    for row,value in result.items(): validate_sensor(row,value)
    versions = {name:capture(argv).strip() for name,argv in {'python':['/usr/bin/python3','--version'],
                'rust':['/opt/rust/bin/rustc','--version'],'cargo':['/opt/rust/bin/cargo','--version'],
                'chrome':['/opt/google/chrome/chrome','--version']}.items()}
    return result,dict(versions,kernel=platform.release())

def protected(path):
    path = Path(path)
    for item in [*reversed(path.parents),path]:
        info = item.lstat()
        require(info.st_uid==0 and not info.st_mode & 0o022 and not stat.S_ISLNK(info.st_mode),'unprotected host path: '+str(item))
        require(not any(n.startswith('system.posix_acl_') for n in os.listxattr(item)),'host ACL: '+str(item))

def config(name):
    path = CONFIG/name; protected(path); return json.loads(path.read_bytes())

def skeleton_check():
    root = Path('/usr/lib/gatebox/root'); expected = config('skeleton.json'); actual = {}
    for path in sorted(root.rglob('*')):
        key = str(path.relative_to(root)); info = path.lstat()
        require(info.st_uid==0 and (stat.S_ISLNK(info.st_mode) or not info.st_mode & 0o022),'skeleton owner/mode')
        if stat.S_ISLNK(info.st_mode): actual[key] = {'link':os.readlink(path)}
        elif stat.S_ISDIR(info.st_mode): protected(path); actual[key] = {'directory':True}
        elif stat.S_ISREG(info.st_mode): protected(path); actual[key] = {'sha256':hashlib.sha256(path.read_bytes()).hexdigest()}
        else: raise ValueError('special skeleton entry')
    require(actual==expected,'skeleton inventory changed')

def installed_identity():
    skeleton_check()
    inventory = config('installed-files.json')
    require(isinstance(inventory,dict) and inventory,'installed file inventory missing')
    for name,digest in inventory.items():
        path = Path(name); protected(path)
        require(hashlib.sha256(path.read_bytes()).hexdigest()==digest,'installed bytes changed: '+name)
    def digest(prefixes):
        pairs = sorted((n,h) for n,h in inventory.items() if any(n.startswith(p) for p in prefixes))
        require(pairs,'empty identity inventory')
        return hashlib.sha256(json.dumps(pairs,separators=(',',':')).encode()).hexdigest()
    bundle = config('bundle.json')['sha256']
    require(re.fullmatch('[0-9a-f]{64}',bundle),'bundle digest')
    return dict(bundle_sha256=bundle,policy_sha256=digest((str(HERE/'policy'),'/etc/gatebox/gates.json','/etc/gatebox/exclusions.json','/etc/systemd/','/etc/sudoers.d/','/etc/polkit-1/')),
                gates_sha256=digest((str(HERE/'policy/gates.json'),str(HERE/'policy/exclusions.json'))))

def effective_units():
    observations = {}
    for name in ('mothergate.socket','mothergate.service','mothergate-run@'+('a'*32)+'.service'):
        text = capture(['/usr/bin/systemctl','show',name,'--property=FragmentPath,DropInPaths,User,PrivatePIDs,PrivateNetwork,PrivateIPC,NoNewPrivileges,BindLogSockets,SystemCallArchitectures'])
        values = dict(line.split('=',1) for line in text.splitlines() if '=' in line)
        template = 'mothergate-run@.service' if name.startswith('mothergate-run@') else name
        require(values.get('FragmentPath')=='/etc/systemd/system/'+template and not values.get('DropInPaths'),'unit fragment/override changed')
        if template=='mothergate-run@.service':
            for key,expected in dict(User='gaterun',PrivatePIDs='yes',PrivateNetwork='yes',PrivateIPC='yes',
                                     NoNewPrivileges='yes',BindLogSockets='no',SystemCallArchitectures='native').items():
                require(values.get(key)==expected,'effective unit property: '+key)
        observations[name] = values
    return observations

def require_installed():
    sibling('verify').host_context()
    require(HERE==Path('/usr/libexec/gatebox'),'installed path required')
    current = installed_identity(); require(current==config('identity.json'),'stale installed identity')
    require(config('admission.json')==dict(identity=current,stage='qualifying'),'acceptance requires qualifying stage')
    harness_check(HARNESS.read_bytes(),json.loads((HERE/'policy/exclusions.json').read_bytes()))
    effective_units()
    for name in ('mothergate','gaterun'):
        account = pwd.getpwnam(name)
        require(account.pw_uid>0 and account.pw_shell=='/usr/sbin/nologin'
                and os.getgrouplist(name,account.pw_gid)==[account.pw_gid],'gate account')
    require(pwd.getpwnam('mothergate').pw_uid!=pwd.getpwnam('gaterun').pw_uid,'shared account')

def atomic_config(name,value):
    protected(CONFIG)
    destination = CONFIG/name
    if destination.exists(): protected(destination)
    temporary = CONFIG/(name+'.next')
    fd = os.open(temporary,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o644)
    with os.fdopen(fd,'wb') as stream:
        stream.write((json.dumps(value,sort_keys=True)+'\n').encode()); stream.flush(); os.fsync(stream.fileno())
    os.replace(temporary,destination)
    fd = os.open(CONFIG,os.O_RDONLY|os.O_DIRECTORY)
    try: os.fsync(fd)
    finally: os.close(fd)

def claude_context():
    pid,seen = os.getppid(),set()
    while pid>1 and pid not in seen:
        seen.add(pid)
        entry = Path('/proc',str(pid))
        args = (entry/'cmdline').read_bytes().split(b'\0')
        if any(Path(a.decode(errors='replace')).name in ('claude','claude-code')
               or b'/@anthropic-ai/claude-code/' in a for a in args):
            return {'context':'claude','ancestor_pid':pid}
        pid = int(status(pid)['PPid'])
    return {'context':'host'}

def client_request(sha,registration):
    require(registration['uid']>0 and pwd.getpwnam(registration['user']).pw_uid==registration['uid'],'registered submitter')
    argv = ['/usr/sbin/runuser','-u',registration['user'],'--','/usr/bin/python3','-I','-B',str(HERE/'client.py'),
            '--repo','mother','--sha',sha,'--gateset','full']
    # Refusals intentionally exit 1. Capture remains bounded; no root submit path.
    proc = subprocess.Popen(argv,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.PIPE,close_fds=True,start_new_session=True)
    try:
        output,error = proc.communicate(timeout=5)
    except BaseException:
        stop(proc); raise
    require(len(output)+len(error)<8192,'client reply bound')
    require(proc.returncode in (0,1) and not error,'client process failed')
    return json.loads(output)

def clients_check(value,summary,uid):
    require(isinstance(value,dict) and uid>0,'CLIENTS evidence missing')
    accepted,refused = value.get('accepted',{}),value.get('refused',{})
    require(value.get('uid')==uid and value.get('context')=='claude'
            and accepted.get('status')=='ACCEPTED' and re.fullmatch('[0-9a-f]{32}',accepted.get('run_id',''))
            and refused=={'status':'UNQUALIFIED','reason':'QUALIFICATION_ONLY'}
            and value.get('codex')=='unsupported (runner denies AF_UNIX)','CLIENTS acceptance/refusal/context')
    require(summary.get('peer_uid')==uid and summary.get('run_id')==accepted['run_id']
            and summary.get('request')==dict(repo_id='mother',full_sha=FIXTURE,gate_set_id='full')
            and summary.get('installed',{}).get('stage')=='qualifying','CLIENTS receipt binding')

def fixture():
    global SESSION
    if SESSION is not None: return SESSION
    require_installed(); registration = config('registry.json')['mother']
    context = claude_context()
    attempt = 'acceptance-'+uuid.uuid4().hex+'.json'
    provenance = dict(context,uid=registration['uid'],codex='unsupported (runner denies AF_UNIX)')
    atomic_config(attempt,dict(provenance,state='starting'))
    # Independent host canaries exist throughout the admitted fixture, never in children.
    with tempfile.TemporaryDirectory(prefix='gbx-canary-') as temporary:
        path = str(Path(temporary)/'host.sock'); sockets = []
        master,slave = pty.openpty()
        try:
            for family,address in ((socket.AF_INET,('127.0.0.1',0)),(socket.AF_UNIX,path),
                                   (socket.AF_UNIX,'\0gatebox-'+str(os.getpid()))):
                server = socket.socket(family); sockets.append(server); server.bind(address); server.listen(1)
            metadata = dict(payload_uid=pwd.getpwnam('gaterun').pw_uid,pid=os.getpid(),
                            process_stat=Path('/proc/self/stat').read_text(),port=sockets[0].getsockname()[1],
                            socket=path,abstract='gatebox-'+str(os.getpid()),
                            pty=dict(path=os.ttyname(slave)),
                            namespaces={n:os.readlink('/proc/self/ns/'+n) for n in ('pid','mnt','ipc','uts','net')})
            atomic_config('probe-context.json',metadata)
            refused = client_request('0'*40,registration)
            require(refused=={'status':'UNQUALIFIED','reason':'QUALIFICATION_ONLY'},'qualification refusal')
            provenance['refused'] = refused
            atomic_config(attempt,dict(provenance,state='refusal-observed'))
            accepted = client_request(FIXTURE,registration)
            provenance['accepted'] = accepted
            atomic_config(attempt,dict(provenance,state='submitted'))
            require(accepted.get('status')=='ACCEPTED' and re.fullmatch('[0-9a-f]{32}',accepted.get('run_id','')),'fixture not accepted')
            run = accepted['run_id']; deadline = time.monotonic()+14500
            while not (Path('/var/lib/mothergate/receipts')/run).exists():
                require(time.monotonic()<deadline,'fixture receipt deadline; attempt retained'); time.sleep(.5)
            result = sibling('verify').authenticate(run); summary = result['summary']
            atomic_config(attempt,dict(provenance,state='receipt-published',run_id=run,status=summary['status']))
            require(summary['status']=='PASS_SCOPED','fixture failed: '+str(summary['missing_evidence']))
            # Publication precedes the supervisor's staging/pending cleanup.
            # Wait only for this run; unrelated leftovers still fail qualify().
            cleanup_deadline = time.monotonic()+5
            while ((Path('/var/lib/mothergate/pending')/('.'+run)).exists()
                   or (Path('/var/lib/mothergate/staging')/run).exists()):
                remaining = cleanup_deadline-time.monotonic()
                require(remaining>0,'custody leftovers after proof; reviewed recovery required')
                time.sleep(min(.01,remaining))
            SESSION = (summary,dict(context,uid=registration['uid'],accepted=accepted,refused=refused,
                                   codex='unsupported (runner denies AF_UNIX)'))
        finally:
            for server in sockets: server.close()
            os.close(master); os.close(slave)
    return SESSION

def offline_oracles(row):
    import unittest
    spec = importlib.util.spec_from_file_location('installed_service_oracles',HERE/'tests/test_service.py')
    service = importlib.util.module_from_spec(spec); spec.loader.exec_module(service)
    names = [n for n in dir(service.OfflineServiceTests) if n.startswith('test_'+row.lower()+'_')]
    require(names,'missing '+row+' oracle inventory')
    suite = unittest.TestSuite(service.OfflineServiceTests(n) for n in names)
    if row in ('B1','B3'):
        spec = importlib.util.spec_from_file_location('installed_core_oracles',HERE/'tests/test_unit_core.py')
        core = importlib.util.module_from_spec(spec); spec.loader.exec_module(core)
        suite.addTests(unittest.defaultTestLoader.loadTestsFromTestCase(core.SnapshotTests if row=='B1' else core.ReceiptTests))
    # Pin all nested oracle temporaries outside the immutable installed tree,
    # independent of a caller's TMPDIR or tempfile's cached default.
    with tempfile.TemporaryDirectory(prefix='g',dir='/var/tmp') as temporary:
        previous = tempfile.tempdir
        try:
            tempfile.tempdir = temporary
            result = unittest.TestResult(); suite.run(result)
        finally: tempfile.tempdir = previous
    require(result.wasSuccessful() and not result.skipped,'installed negative controls failed/skipped: '+str(result.errors+result.failures+result.skipped))

def authority(summary):
    polkit_check()
    for user in ('mothergate','gaterun'):
        # No shell and no broad sudo rule; direct and transient manager calls must deny.
        for argv in (['/usr/bin/systemctl','--no-ask-password','start','mothergate-forbidden.service'],
                     ['/usr/bin/systemd-run','--no-ask-password','--unit=gatebox-forbidden-'+user,'/usr/bin/true']):
            proc = subprocess.run(['/usr/sbin/runuser','-u',user,'--',*argv],stdin=subprocess.DEVNULL,
                                  stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=10,close_fds=True)
            require(proc.returncode!=0 and any(word in proc.stderr.lower() for word in (b'access denied',b'permission denied',b'authentication')),'manager authority not denied')
    for user in (config('registry.json')['mother']['user'],'gaterun'):
        path = str(Path('/var/lib/mothergate/receipts')/summary['run_id']/'summary.json')
        script = 'import errno,os,sys\ntry: os.open(sys.argv[1],os.O_WRONLY)\nexcept OSError as e: sys.exit(0 if e.errno in (errno.EACCES,errno.EPERM) else 2)\nsys.exit(3)'
        capture(['/usr/sbin/runuser','-u',user,'--','/usr/bin/python3','-I','-c',script,path])

def polkit_check():
    protected(POLKIT_RULE)
    digest = config('installed-files.json').get(str(POLKIT_RULE))
    require(digest and hashlib.sha256(POLKIT_RULE.read_bytes()).hexdigest()==digest,'installed polkit rule missing/changed')

def custody_oracle(row):
    import unittest
    require(row in CUSTODY_TESTS,'unknown custody row')
    # Installed discovery needs test_service on its import path, just as unittest
    # discover does. All files have already passed installed_identity().
    directory = str(HERE/'tests')
    sys.path.insert(0,directory)
    try:
        spec = importlib.util.spec_from_file_location('installed_custody',HERE/'tests/test_custody.py')
        module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
        suite = unittest.defaultTestLoader.loadTestsFromName('InstalledCustodyTests.'+CUSTODY_TESTS[row],module)
        tests = list(suite); result = unittest.TestResult(); suite.run(result)
        require(result.testsRun==1 and result.wasSuccessful() and not result.skipped,
                'installed custody failed/skipped: '+str(result.errors+result.failures+result.skipped))
        observations = getattr(tests[0],'observations',None)
        require(isinstance(observations,dict),'missing custody observations')
        observations['row'] = row
        proof = dict(row=row,passed=True,observations=observations)
        validate_custody(row,proof)
        return proof
    finally: sys.path.remove(directory)

def validate_custody(row,proof):
    require(isinstance(proof,dict) and proof.get('row')==row and proof.get('passed') is True
            and isinstance(proof.get('observations'),dict) and bool(proof['observations']),
            'installed custody proof missing/failed: '+row)
    observed = proof['observations']
    require(isinstance(observed.get('run_id'),str) and re.fullmatch('[0-9a-f]{32}',observed['run_id']),
            'custody run_id missing/invalid')
    require(valid_boot(observed.get('boot_id')) and observed.get('row')==row,'custody boot_id/row missing/invalid')
    if row=='C1-SEALED':
        denied = observed.get('denied')
        require(isinstance(denied,list) and len(denied)==6 and all(isinstance(v,list) and len(v)==3
            and v[2] in (errno.EACCES,errno.EPERM) for v in denied) and
            {(v[0],v[1]) for v in denied}=={(k,a) for k in ('log','status') for a in ('unlink','rename','symlink')},
            'custody endpoint denials missing')
    elif row=='C2-CANARY':
        require(all(observed.get(k) is True for k in ('rejected','no_invocation','canary_unchanged',
                'dangling_absent','custody_held')),'custody pre-handoff proof missing')
    elif row=='C3-RESTART':
        before=observed.get('supervisor_before',{}).get('MainPID','')
        after=observed.get('supervisor_after',{}).get('MainPID','')
        require(observed.get('retained') is True and isinstance(before,str) and isinstance(after,str)
                and before.isdecimal() and after.isdecimal() and min(int(before),int(after))>1
                and before!=after,'custody restart evidence missing')
    else: raise ValueError('unknown custody row')

def valid_boot(value):
    return isinstance(value,str) and re.fullmatch('[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}',value) is not None

def boot_id():
    value = Path('/proc/sys/kernel/random/boot_id').read_text().strip()
    require(valid_boot(value),'current boot_id unavailable')
    return value

def clean_custody(held_run=None):
    # No manager calls. Inspect protected directory descriptors, not caller paths.
    # Used-ID markers alone are harmless; all other leftovers block qualification.
    bridge = sibling('lifecycle').Custody(owner=pwd.getpwnam('mothergate').pw_uid,
                                        owner_gid=pwd.getpwnam('mothergate').pw_gid)
    from contextlib import ExitStack
    with ExitStack() as stack:
        for path in ('/var/lib/mothergate/pending','/var/lib/mothergate/staging',
                     '/run/mothergate-control/incoming'):
            fd=stack.enter_context(bridge.directory(Path(path),bridge.owner,0o700))
            if os.listdir(fd): return False
        stack.enter_context(bridge.directory(bridge.control,mode=0o755))
        sealed=stack.enter_context(bridge.directory(bridge.control/'sealed',mode=0o700))
        names=set(os.listdir(sealed))
        for name in list(names):
            if re.fullmatch(r'\.used-[0-9a-f]{32}',name):
                fd=os.open(name,os.O_RDONLY|os.O_NOFOLLOW,dir_fd=sealed)
                try: bridge.root_file(fd)
                finally: os.close(fd)
                names.remove(name)
        if held_run is None: return not names
        require(re.fullmatch('[0-9a-f]{32}',held_run),'invalid held run')
        if names!={held_run,'.pending-'+held_run}: return False
        fd=os.open('.pending-'+held_run,os.O_RDONLY|os.O_NOFOLLOW,dir_fd=sealed)
        try: bridge.root_file(fd)
        finally: os.close(fd)
        directory=stack.enter_context(bridge.directory(bridge.control/'sealed'/held_run,mode=0o700))
        if set(os.listdir(directory))!={'log.sock','status.sock'}: return False
        for kind,target in (('log','canary'),('status','missing')):
            info=os.stat(kind+'.sock',dir_fd=directory,follow_symlinks=False)
            if not stat.S_ISLNK(info.st_mode) or info.st_uid!=bridge.owner or info.st_gid!=bridge.owner_gid: return False
            if os.readlink(kind+'.sock',dir_fd=directory)!='/run/gatebox-custody-'+held_run+'/'+target: return False
        return True

def acceptance_custody(value):
    for row in CUSTODY_TESTS: validate_custody(row,value.get('evidence',{}).get(row,{}).get('custody'))
    current = boot_id()
    require(valid_boot(current),'current boot_id unavailable')
    previous = value['evidence']['C2-CANARY']['custody']['observations']['boot_id']
    require(previous!=current,'C2 requires a different boot_id after reboot')
    require(clean_custody(),'custody leftovers; reviewed recovery required')

def e2e(summary):
    sibling('receipt').validate_summary(summary)
    require(summary['request']==dict(repo_id='mother',full_sha=FIXTURE,gate_set_id='full')
            and summary['cleanup'].get('complete') is True,'fixture identity/cleanup')
    require(summary['isolation']['A1'].get('target_cold') is True,'target not cold')
    require(0<int(summary['resources']['MemoryPeak'])<=24*1024**3*.8,'memory headroom missing/exceeded')
    require(0<summary['resources']['duration_seconds']<=14400*.8,'runtime headroom exceeded')
    scratch = summary['resources']['scratch']
    require(scratch['samples']>1 and scratch['target_bytes']>0,'scratch/target accounting missing')
    for data in scratch['observed_peaks'].values():
        require(0<data['bytes_total'] and 0<data['inodes_total'] and data['bytes_used']<=data['bytes_total']*.8
                and data['inodes_used']<=data['inodes_total']*.8,'scratch headroom exceeded')

def qualify(row):
    require(row in REQUIRED_ROWS,'unknown oracle')
    require_installed()
    prior = config('qualification.json')
    require('C2-CANARY' not in prior['passed'],'C2 already recorded; reboot then enable-qualified')
    if row=='C2-CANARY': require(set(prior['passed'])==REQUIRED_ROWS-{'C2-CANARY'},'C2 must run LAST')
    require(clean_custody(),'custody leftovers; reviewed recovery required')
    current_boot = boot_id()
    summary,clients = fixture()
    require(summary['installed']==dict(config('identity.json'),stage='qualifying'),'receipt installed binding')
    custody = None
    if row in CUSTODY_TESTS:
        custody = custody_oracle(row); validate_custody(row,custody)
        require(custody['observations']['boot_id']==current_boot,'custody proof from a different boot')
    elif row.startswith('A'): validate_sensor(row,summary['isolation'][row])
    elif row in ('B1','B2','B3','B4'):
        offline_oracles(row)
        if row in ('B2','B3'): authority(summary)
        if row=='B3': sibling('verify').authenticate(summary['run_id'])
        if row=='B4': require(summary['cleanup']['complete'] and not any(Path('/var/lib/mothergate/pending').iterdir()),'cleanup/pending')
    elif row=='E2E': e2e(summary)
    elif row=='CLIENTS': clients_check(clients,summary,config('registry.json')['mother']['uid'])
    else: raise ValueError('unknown oracle')
    require(boot_id()==current_boot,'boot changed during qualification')
    require(clean_custody(held_run=custody['observations']['run_id'] if row=='C2-CANARY' else None),
            'custody leftovers after proof; reviewed recovery required')
    # Serialize acceptance updates; only this host-root process writes them.
    fd = os.open(CONFIG/'qualification.lock',os.O_RDWR|os.O_CREAT|os.O_NOFOLLOW,0o600)
    try:
        fcntl.flock(fd,fcntl.LOCK_EX); protected(CONFIG/'qualification.lock')
        value = config('qualification.json'); identity = installed_identity()
        if value=={'identity':None,'passed':[]}: value = dict(identity=identity,passed=[],evidence={})
        require(value.get('identity')==identity,'stale acceptance evidence')
        require('C2-CANARY' not in value['passed'],'qualification already sealed by C2')
        if row=='C2-CANARY': require(set(value['passed'])==REQUIRED_ROWS-{'C2-CANARY'},'C2 must run LAST')
        value['evidence'][row] = dict(run_id=summary['run_id'],
            summary_sha256=hashlib.sha256(json.dumps(summary,sort_keys=True).encode()).hexdigest(),
            client_provenance=clients if row=='CLIENTS' else None)
        if custody is not None: value['evidence'][row]['custody'] = custody
        value['passed'] = sorted(set(value['passed'])|{row}); atomic_config('qualification.json',value)
    finally: os.close(fd)

if __name__=='__main__':
    require(sys.argv[1:]==['--syscalls'],'fixed sensor invocation only')
    print(json.dumps(syscall_probe()))
