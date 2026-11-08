#!/usr/bin/python3 -I
"""Q30: per-run whole-tmpfs payload root, copied from the manifest-checked skeleton.

The product admits a governed workspace only when "/" is a whole filesystem mount
(device listed once, root "/"). systemd v259 namespace.c self-binds RootDirectory=
only when it is not already a mount point, so this tmpfs becomes the payload "/".
Run only by mothergate-root@.service (root, host mount namespace); never by sudo.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys

RUN = re.compile('[0-9a-f]{32}')
SKELETON = Path('/usr/lib/gatebox/root')
MANIFEST = Path('/etc/gatebox/skeleton.json')
BASE = Path('/run/mothergate-root')
OPTIONS = 'mode=0755,uid=0,gid=0,nodev,nosuid,size=16M,nr_inodes=4096'
ENV = {'PATH':'/usr/bin:/bin','LANG':'C.UTF-8','HOME':'/nonexistent'}
NOFOLLOW = os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC

def require(condition, message):
    if not condition: raise ValueError(message)

def manager(argv):
    subprocess.run(argv,env=ENV,stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,
                   stderr=subprocess.DEVNULL,close_fds=True,timeout=20,check=True)

def run_id(value):
    require(isinstance(value,str) and RUN.fullmatch(value),'invalid run ID')
    return value

def checked_base(base, owner):
    info = os.lstat(base)
    require(stat.S_ISDIR(info.st_mode) and info.st_uid==owner and stat.S_IMODE(info.st_mode)==0o700,
            'unsafe root base')
    return Path(base)

def read_manifest(path, owner):
    fd = os.open(path,NOFOLLOW)
    try:
        info = os.fstat(fd)
        require(stat.S_ISREG(info.st_mode) and info.st_uid==owner and not info.st_mode & 0o022,'unsafe manifest')
        with os.fdopen(os.dup(fd),'rb') as stream: value = json.loads(stream.read())
    finally: os.close(fd)
    require(isinstance(value,dict) and value,'empty manifest')
    for key in value:
        parts = key.split('/')
        require(isinstance(key,str) and key and not key.startswith('/') and '' not in parts
                and '.' not in parts and '..' not in parts,'unsafe manifest entry')
    return value

def inventory(root):
    """The same identity probe.skeleton_check records: link target, file sha256, directory."""
    result = {}
    for path in sorted(Path(root).rglob('*')):
        key, mode = str(path.relative_to(root)), path.lstat().st_mode
        if stat.S_ISLNK(mode): result[key] = {'link':os.readlink(path)}
        elif stat.S_ISDIR(mode): result[key] = {'directory':True}
        elif stat.S_ISREG(mode): result[key] = {'sha256':hashlib.sha256(path.read_bytes()).hexdigest()}
        else: raise ValueError('special root entry: '+key)
    return result

def copy(skeleton, target, expected, owner):
    for key in sorted(expected):
        source, destination, entry = Path(skeleton)/key, Path(target)/key, expected[key]
        info = os.lstat(source)
        require(info.st_uid==owner and (stat.S_ISLNK(info.st_mode) or not info.st_mode & 0o022),
                'skeleton owner/mode: '+key)
        if stat.S_ISLNK(info.st_mode):
            link = os.readlink(source)
            require(entry=={'link':link},'skeleton link changed: '+key)
            os.symlink(link,destination)
        elif stat.S_ISDIR(info.st_mode):
            require(entry=={'directory':True},'skeleton type changed: '+key)
            os.mkdir(destination,0o700)
            os.chmod(destination,stat.S_IMODE(info.st_mode))
        elif stat.S_ISREG(info.st_mode):
            fd = os.open(source,NOFOLLOW)
            try:
                with os.fdopen(os.dup(fd),'rb') as stream: data = stream.read()
            finally: os.close(fd)
            require(entry=={'sha256':hashlib.sha256(data).hexdigest()},'skeleton bytes changed: '+key)
            fd = os.open(destination,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW|os.O_CLOEXEC,0o600)
            try:
                view = memoryview(data)
                while view: view = view[os.write(fd,view):]
                os.fchmod(fd,stat.S_IMODE(info.st_mode))
            finally: os.close(fd)
        else: raise ValueError('special skeleton entry: '+key)

def up(run, skeleton=SKELETON, manifest=MANIFEST, base=BASE, owner=0, mount=manager, ismount=os.path.ismount):
    run = run_id(run)
    if base == BASE and not os.path.lexists(base): os.mkdir(base,0o700)
    base = checked_base(base,owner)
    target = base/run
    os.mkdir(target,0o755)  # A reused run ID fails here: FileExistsError.
    try:
        mount(['/usr/bin/mount','-t','tmpfs','-o',OPTIONS,'gatebox-root',str(target)])
        require(ismount(str(target)),'root tmpfs not mounted')
        expected = read_manifest(manifest,owner)
        require(inventory(skeleton)==expected,'skeleton inventory changed')
        copy(skeleton,target,expected,owner)
        require(inventory(target)==expected,'payload root differs from manifest')
        os.chmod(target,0o755)
        mount(['/usr/bin/mount','-o','remount,ro,'+OPTIONS,str(target)])
    except BaseException as exc:
        # Unwind this run's mount; a cleanup failure stays visible beside the cause.
        try: down(run,base=base,owner=owner,mount=mount,ismount=ismount)
        except Exception as cleanup: exc.add_note('root cleanup failed: '+repr(cleanup))
        raise

def down(run, base=BASE, owner=0, mount=manager, ismount=os.path.ismount):
    run = run_id(run)
    if not os.path.lexists(base): return
    target = checked_base(base,owner)/run
    if ismount(str(target)): mount(['/usr/bin/umount',str(target)])
    if os.path.lexists(target): os.rmdir(target)

if __name__ == '__main__':
    if os.geteuid() != 0: raise SystemExit('root required')
    if len(sys.argv) != 3 or sys.argv[1] not in ('up','down'): raise SystemExit('usage: rootfs.py up|down RUN')
    (up if sys.argv[1] == 'up' else down)(sys.argv[2])
