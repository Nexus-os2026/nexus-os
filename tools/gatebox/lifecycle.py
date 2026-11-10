#!/usr/bin/python3 -I
"""Fixed root bridge: seal two endpoint leaves before any manager start."""
from contextlib import contextmanager, ExitStack
import fcntl
import os
from pathlib import Path
import pwd
import re
import stat
import subprocess
import sys
import time

ENV = {'PATH':'/usr/bin:/bin','LANG':'C.UTF-8','HOME':'/nonexistent'}
RUN = re.compile('[0-9a-f]{32}')
LOCK_TIMEOUT = 5
CGROUP = Path('/sys/fs/cgroup')
KINDS = ('log','status')
DIR_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
# SB3E-R22.1: systemd places each template instance in its template slice ('-' escaped as \x2d); the manager
# units sit in system.slice. supervisor.py imports this; install.sh keeps a copy pinned equal by a test.
INSTANCE = re.compile(r'mothergate-(run|root)@[0-9a-f]{32}\.service')
SLICES = {'run':'system-mothergate\\x2drun.slice','root':'system-mothergate\\x2droot.slice'}

def unit_cgroup(unit):
    match = INSTANCE.fullmatch(unit)
    return '/system.slice/'+(SLICES[match.group(1)]+'/' if match else '')+unit

def command(args):
    if len(args) != 2 or args[0] not in ('start','stop') or not RUN.fullmatch(args[1]):
        raise ValueError('expected start|stop and lowercase run ID')
    return ['/usr/bin/systemctl',args[0],'mothergate-run@'+args[1]+'.service']

def no_acl(path):
    if any('acl' in name.lower() for name in os.listxattr(path)):
        raise ValueError('unexpected launch ACL')

def check_start(run_id, staging=Path('/var/lib/mothergate/staging'),
                control=Path('/run/mothergate-control'), owner=None, endpoints=True):
    """Bind inputs retain Slice 1's disclosed staging races; never confer custody.

    endpoints=True is a legacy preflight exercised by offline B2 only.
    Production passes False; only post-relocation validation confers custody.
    """
    if not RUN.fullmatch(run_id): raise ValueError('invalid run ID')
    if owner is None: owner = pwd.getpwnam('mothergate').pw_uid
    expected = [(staging,stat.S_ISDIR),(staging/run_id,stat.S_ISDIR),
                (staging/run_id/'snapshot',stat.S_ISDIR),
                (staging/run_id/'request.json',stat.S_ISREG)]
    if endpoints:
        expected += [(control/'incoming'/(run_id+'.'+s+'.sock'),stat.S_ISSOCK) for s in KINDS]
    for path, kind in expected:
        try:
            for ancestor in reversed(path.parents):
                info = ancestor.lstat()
                if (not stat.S_ISDIR(info.st_mode) or info.st_uid not in (0,owner)
                        or (info.st_mode & 0o022 and not info.st_mode & stat.S_ISVTX)):
                    raise ValueError('unsafe launch ancestry')
            info = path.lstat()
            if info.st_uid != owner or not kind(info.st_mode) or info.st_mode & 0o022:
                raise ValueError('unsafe snapshot/request/capture endpoint')
        except OSError as exc: raise ValueError('missing launch input') from exc

class Custody:
    def __init__(self, staging=Path('/var/lib/mothergate/staging'),
                 control=Path('/run/mothergate-control'), owner=None, root_owner=0,
                 runner=subprocess.run, owner_gid=None):
        self.staging, self.control = Path(staging), Path(control)
        self.owner = pwd.getpwnam('mothergate').pw_uid if owner is None else owner
        self.owner_gid = self.owner if owner_gid is None else owner_gid
        self.root_owner, self.runner = root_owner, runner

    @contextmanager
    def directory(self, path, leaf_owner=None, mode=None):
        """Open every ancestor without following links; hold the final directory."""
        path = Path(path)
        if not path.is_absolute() or '..' in path.parts: raise ValueError('unsafe directory path')
        fd = os.open('/',DIR_FLAGS)
        try:
            for index, part in enumerate(path.parts):
                if index:
                    next_fd = os.open(part,DIR_FLAGS,dir_fd=fd)
                    os.close(fd)
                    fd = next_fd
                info = os.fstat(fd)
                final = index == len(path.parts)-1
                uid = leaf_owner if final and leaf_owner is not None else self.root_owner
                gid = self.owner_gid if uid == self.owner and leaf_owner is not None else self.root_owner
                if (not stat.S_ISDIR(info.st_mode) or info.st_uid not in (0,uid)
                        or info.st_mode & 0o022):
                    raise ValueError('unsafe protected ancestry')
                if final and (info.st_uid != uid or info.st_gid != gid
                              or (mode is not None and stat.S_IMODE(info.st_mode) != mode)):
                    raise ValueError('unsafe custody directory')
                no_acl(fd)
            yield fd
        finally: os.close(fd)

    @contextmanager
    def locked(self, control):
        fd = os.open('lifecycle.lock',os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW | os.O_CLOEXEC,
                     0o600,dir_fd=control)
        try:
            self.root_file(fd)
            deadline = time.monotonic()+LOCK_TIMEOUT
            while True:
                try:
                    fcntl.flock(fd,fcntl.LOCK_EX | fcntl.LOCK_NB)
                    break
                except BlockingIOError:
                    if time.monotonic() >= deadline: raise TimeoutError('custody lock deadline')
                    time.sleep(.05)
            yield
        finally: os.close(fd)

    def root_file(self, fd):
        info = os.fstat(fd)
        if (not stat.S_ISREG(info.st_mode) or info.st_uid != self.root_owner
                or info.st_gid != self.root_owner or stat.S_IMODE(info.st_mode) != 0o600
                or info.st_nlink != 1):
            raise ValueError('unsafe root custody state')
        no_acl(fd)

    def marker(self, directory, name):
        fd = os.open(name,os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
                     0o600,dir_fd=directory)
        try: self.root_file(fd)
        finally: os.close(fd)

    def endpoints(self, directory, run):
        # All leaves must pass after relocation, before either start or deletion.
        for kind in KINDS:
            name = kind+'.sock'
            try:
                info = os.stat(name,dir_fd=directory,follow_symlinks=False)
                if (not stat.S_ISSOCK(info.st_mode) or info.st_uid != self.owner
                        or info.st_gid != self.owner_gid or stat.S_IMODE(info.st_mode) != 0o700
                        or info.st_nlink != 1):
                    raise ValueError('unsafe sealed endpoint')
                # Namespace and every ancestor are now root-protected. No follow.
                path = self.control/'sealed'/run/name
                if any('acl' in n.lower() for n in os.listxattr(path,follow_symlinks=False)):
                    raise ValueError('unexpected endpoint ACL')
            except OSError as exc: raise ValueError('missing/unsafe sealed endpoint') from exc

    def manager(self, argv, timeout):
        return self.runner(argv,env=ENV,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,
                           stderr=subprocess.DEVNULL,close_fds=True,timeout=timeout,
                           encoding='utf-8',check=True)

    def settled(self, run):
        unit = 'mothergate-run@'+run+'.service'
        output = self.manager(['/usr/bin/systemctl','show',unit,
                               '--property=Job,ActiveState,ControlGroup'],5).stdout
        state = {}
        for line in output.splitlines():
            key, sep, value = line.partition('=')
            if not sep or key in state: raise ValueError('ambiguous manager state')
            state[key] = value
        if (set(state) != {'Job','ActiveState','ControlGroup'} or state['Job'] != ''
                or state['ActiveState'] not in ('inactive','failed')):
            raise ValueError('manager not settled')
        expected = unit_cgroup(unit)
        if state['ControlGroup'] not in ('',expected): raise ValueError('unexpected cgroup identity')
        path = CGROUP/expected.lstrip('/')
        if state['ControlGroup'] or path.exists():
            try:
                with self.directory(path) as directory:
                    fd = os.open('cgroup.events',os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC,dir_fd=directory)
                    with os.fdopen(fd) as stream: events = stream.read(4096)
                    if 'populated 0' not in events.splitlines(): raise ValueError('unresolved descendants')
            except OSError as exc: raise ValueError('unresolved cgroup evidence') from exc
        return state

    def cleanup(self, sealed, run):
        try:
            with self.directory(self.control/'sealed'/run,mode=0o700) as directory:
                self.endpoints(directory,run)
                if set(os.listdir(directory)) != {k+'.sock' for k in KINDS}:
                    raise ValueError('unexpected quarantine contents')
                for kind in KINDS: os.unlink(kind+'.sock',dir_fd=directory)
            os.rmdir(run,dir_fd=sealed)
        except FileNotFoundError:
            # Missing run directory is acceptable only if no handoff was recorded.
            if '.pending-'+run in os.listdir(sealed): raise ValueError('custody disappeared')
        if '.pending-'+run in os.listdir(sealed): os.unlink('.pending-'+run,dir_fd=sealed)

    def run(self, verb, run):
        argv = command([verb,run])
        # Q12: Stop must still reach the manager when custody validation fails.
        # Only settle/cleanup require protected layout and the lifecycle lock.
        if verb == 'stop': self.manager(argv,25)
        with ExitStack() as stack:
            control = stack.enter_context(self.directory(self.control,mode=0o755))
            stack.enter_context(self.locked(control))
            sealed = stack.enter_context(self.directory(self.control/'sealed',mode=0o700))
            if verb == 'stop':
                state = self.settled(run)
                self.cleanup(sealed,run)
                return state
            if any(n.startswith('.pending-') for n in os.listdir(sealed)):
                raise ValueError('unresolved root custody blocks admission')
            incoming = stack.enter_context(self.directory(self.control/'incoming',self.owner,0o700))
            check_start(run,self.staging,self.control,self.owner,endpoints=False)
            try:
                self.marker(sealed,'.used-'+run)
                self.marker(sealed,'.pending-'+run)
                os.mkdir(run,0o700,dir_fd=sealed)
            except FileExistsError as exc: raise ValueError('duplicate run ID/custody collision') from exc
            directory = stack.enter_context(self.directory(self.control/'sealed'/run,mode=0o700))
            try:
                for kind in KINDS:
                    os.rename(run+'.'+kind+'.sock',kind+'.sock',src_dir_fd=incoming,dst_dir_fd=directory)
            except OSError as exc: raise ValueError('endpoint relocation failed; quarantine retained') from exc
            self.endpoints(directory,run)
            # A timeout does not cancel a manager job. Keep .pending and all leaves.
            self.manager(argv,40)

if __name__ == '__main__':
    command(sys.argv[1:])
    if os.geteuid() != 0: raise SystemExit('root bridge required')
    account = pwd.getpwnam('mothergate')
    Custody(owner=account.pw_uid,owner_gid=account.pw_gid).run(*sys.argv[1:])
