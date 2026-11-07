#!/usr/bin/python3 -I
"""Fixed root bridge. No caller paths, environment, properties or unit names."""
import os
from pathlib import Path
import pwd
import re
import stat
import subprocess
import sys

ENV = {'PATH':'/usr/bin:/bin','LANG':'C.UTF-8','HOME':'/nonexistent'}
RUN = re.compile('[0-9a-f]{32}')
def command(args):
    if len(args) != 2 or args[0] not in ('start','stop') or not RUN.fullmatch(args[1]):
        raise ValueError('expected start|stop and lowercase run ID')
    return ['/usr/bin/systemctl',args[0]]+(['--no-block'] if args[0]=='start' else [])+[
        'mothergate-run@'+args[1]+'.service']

def check_start(run_id, staging=Path('/var/lib/mothergate/staging'),
                control=Path('/run/mothergate-control'), owner=None):
    if not RUN.fullmatch(run_id): raise ValueError('invalid run ID')
    if owner is None: owner = pwd.getpwnam('mothergate').pw_uid
    expected = [(staging,stat.S_ISDIR),(control,stat.S_ISDIR),
                (staging/run_id,stat.S_ISDIR),(staging/run_id/'snapshot',stat.S_ISDIR),
                (staging/run_id/'request.json',stat.S_ISREG)]
    expected += [(control/(run_id+'.'+suffix+'.sock'),stat.S_ISSOCK) for suffix in ('log','status')]
    for path, kind in expected:
        try:
            for ancestor in reversed(path.parents):
                info = ancestor.lstat()
                # Production ancestors cannot be writable by unrelated identities.
                if (not stat.S_ISDIR(info.st_mode) or info.st_uid not in (0,owner)
                        or (info.st_mode & 0o022 and not info.st_mode & stat.S_ISVTX)):
                    raise ValueError('unsafe launch ancestry')
            info = path.lstat()
            if info.st_uid != owner or not kind(info.st_mode) or info.st_mode & 0o022:
                raise ValueError('unsafe snapshot/request/capture endpoint')
        except OSError as exc: raise ValueError('missing launch input') from exc

if __name__ == '__main__':
    argv = command(sys.argv[1:])
    if os.geteuid() != 0: raise SystemExit('root bridge required')
    if sys.argv[1] == 'start': check_start(sys.argv[2])
    raise SystemExit(subprocess.run(argv,env=ENV,stdin=subprocess.DEVNULL,
                                   close_fds=True,timeout=25).returncode)
