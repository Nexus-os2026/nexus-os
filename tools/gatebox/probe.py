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
FIXTURE = '899e3872c5b61223c87b905f63c66947a816259b'
HARNESS = Path('/usr/libexec/gatebox/policy/phase2_live_sandbox.rs')
SESSION = None

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

def isolation(context):
    records = mounts(); current = status()
    points = {m['point']:m for m in records}
    gate = points['/run/gate']; run = points['/run']
    require(gate['type'] == 'tmpfs' and gate['root'] == '/' and sum(m['device']==gate['device'] for m in records)==1,'whole /run/gate tmpfs')
    require(run['type'] == 'tmpfs' and 'noexec' in run['options'] and 'size=16384k' in run['super'],'/run 16MiB noexec')
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

def syscall_probe():
    require(platform.machine()=='x86_64','native ABI probe requires frozen x86_64 host')
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
    return dict(openat2=True,outer=outer,inner=inner,abi=platform.machine())

def process_identity(text):
    # comm may contain spaces and parentheses; starttime is field 22, after comm.
    pid,separator,rest = text.partition(' (')
    fields = rest.rsplit(') ',1)
    require(separator and pid.isdecimal() and int(pid)>0 and len(fields)==2,'invalid process stat')
    tail = fields[1].split()
    require(len(tail)>19 and tail[19].isdecimal(),'missing process start time')
    return int(pid),int(tail[19])

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
        host = Path(context['pty']['path'])
        require(not host.exists() or (host.stat().st_dev,host.stat().st_ino) != tuple(context['pty']['identity']),'host PTY exposed')
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
                    args = (path/'cmdline').read_bytes().split(b'\0')
                    require(b'--no-sandbox' not in args and b'--disable-setuid-sandbox' not in args,'unsafe Chrome flag')
                    if b'--type=renderer' in args: candidates[pid] = info
                except (FileNotFoundError,ProcessLookupError): continue
            for pid,info in candidates.items():
                if descendant(pid,proc.pid,parents):
                    require(info['Seccomp'].strip()=='2' and len(info['NSpid'].split())>=3,'renderer isolation')
                    renderer = dict(pid=pid,seccomp=2,pid_levels=len(info['NSpid'].split()),parents=parents)
            label = Path('/proc',str(proc.pid),'attr/current').read_text().strip()
    finally: stop(proc); proc.stderr.close()
    base = [chrome,'--headless=new','--disable-gpu','--no-first-run','--user-data-dir=/run/gate/tmp/chrome-report','--dump-dom']
    local = capture(base+[page.as_uri()]); report = capture(base+['chrome://sandbox'])
    require('gatebox-local' in local and 'sandbox' in report.lower(),'Chrome local/sandbox rendering')
    return dict(argv=argv,browser_pid=proc.pid,renderer=renderer,browser_label=label,local_html=local,
                sandbox_html=report,version=capture([chrome,'--version']).strip())

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
    required = {'A1':('fsync','caps_zero','readonly'),'A5':('sandbox_html','version','renderer','private_desktop'),
                'A6':('tiocsti_denied','host_hidden','reaped','syscalls'),
                'A8':('loopback4','loopback6','unix','host_denied','services_absent')}
    require(all(value.get(k) for k in required[row]),'missing '+row+' sensor evidence')
    if row=='A1': require(value.get('pid')==1 and value.get('nnp')==1 and value.get('uid',0)>0,'init identity')
    if row=='A5':
        renderer = value['renderer']
        require(renderer.get('seccomp')==2 and renderer.get('pid_levels',0)>=3
                and descendant(renderer['pid'],value['browser_pid'],{int(k):v for k,v in renderer['parents'].items()}),'renderer lineage')
        require(not any('--no-sandbox' in a for a in value.get('argv',[])),'unsafe flags')
    if row=='A6':
        calls = value['syscalls']
        require(calls.get('openat2') and set(calls.get('outer',{}).values())=={'EPERM'}
                and set(calls.get('inner',{}).values())=={'ENOSYS'},'syscall observations')

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
                            pty=dict(path=os.ttyname(slave),identity=[os.fstat(slave).st_dev,os.fstat(slave).st_ino]),
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
    result = unittest.TestResult(); suite.run(result)
    require(result.wasSuccessful() and not result.skipped,'installed negative controls failed/skipped: '+str(result.errors+result.failures+result.skipped))

def authority(summary):
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
    require_installed(); summary,clients = fixture()
    require(summary['installed']==dict(config('identity.json'),stage='qualifying'),'receipt installed binding')
    if row.startswith('A'): validate_sensor(row,summary['isolation'][row])
    elif row in ('B1','B2','B3','B4'):
        offline_oracles(row)
        if row in ('B2','B3'): authority(summary)
        if row=='B3': sibling('verify').authenticate(summary['run_id'])
        if row=='B4': require(summary['cleanup']['complete'] and not any(Path('/var/lib/mothergate/pending').iterdir()),'cleanup/pending')
    elif row=='E2E': e2e(summary)
    elif row=='CLIENTS': clients_check(clients,summary,config('registry.json')['mother']['uid'])
    else: raise ValueError('unknown oracle')
    # Serialize acceptance updates; only this host-root process writes them.
    fd = os.open(CONFIG/'qualification.lock',os.O_RDWR|os.O_CREAT|os.O_NOFOLLOW,0o600)
    try:
        fcntl.flock(fd,fcntl.LOCK_EX); protected(CONFIG/'qualification.lock')
        value = config('qualification.json'); identity = installed_identity()
        if value=={'identity':None,'passed':[]}: value = dict(identity=identity,passed=[],evidence={})
        require(value.get('identity')==identity,'stale acceptance evidence')
        value['evidence'][row] = dict(run_id=summary['run_id'],
            summary_sha256=hashlib.sha256(json.dumps(summary,sort_keys=True).encode()).hexdigest(),
            client_provenance=clients if row=='CLIENTS' else None)
        value['passed'] = sorted(set(value['passed'])|{row}); atomic_config('qualification.json',value)
    finally: os.close(fd)

if __name__=='__main__':
    require(sys.argv[1:]==['--syscalls'],'fixed sensor invocation only')
    print(json.dumps(syscall_probe()))
