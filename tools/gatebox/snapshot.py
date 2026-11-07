"""Bounded SHA-1 object import. Trusted registration is supplied by the supervisor."""
import hashlib
import json
import os
from pathlib import Path
import re
import selectors
import stat
import subprocess
import time

LIMITS = {'seconds':300, 'objects':100000, 'entries':100000, 'bytes':3*1024**3,
          'export':2*1024**3, 'blob':64*1024**2, 'tree':8*1024**2, 'commit':1024**2,
          'depth':64, 'path':4096, 'component':255}
SHA = re.compile(r'[0-9a-f]{40}')
ENV = {'PATH':'/usr/bin:/bin', 'LANG':'C.UTF-8', 'HOME':'/nonexistent',
       'GIT_NO_REPLACE_OBJECTS':'1', 'GIT_NO_LAZY_FETCH':'1', 'GIT_CONFIG_NOSYSTEM':'1',
       'GIT_CONFIG_GLOBAL':'/dev/null', 'GIT_TERMINAL_PROMPT':'0', 'GIT_OPTIONAL_LOCKS':'0'}

def unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('duplicate JSON key')
        result[key] = value
    return result

def parse_request(frame, registry, uid):
    if not isinstance(frame, bytes) or len(frame) < 4:
        raise ValueError('request framing')
    size = int.from_bytes(frame[:4], 'big')
    if not 1 <= size <= 1024 or len(frame) != size + 4:
        raise ValueError('request length')
    try:
        request = json.loads(frame[4:].decode('utf-8'), object_pairs_hook=unique)
    except (UnicodeError, json.JSONDecodeError) as exc:
        raise ValueError('request encoding') from exc
    if not isinstance(request, dict) or set(request) != {'repo_id','full_sha','gate_set_id'}:
        raise ValueError('request fields')
    if not all(type(v) is str for v in request.values()):
        raise ValueError('request types')
    registration = registry.get(request['repo_id'], {})
    if (not SHA.fullmatch(request['full_sha']) or request['gate_set_id'] != 'full'
            or type(uid) is not int or registration.get('uid') != uid
            or registration.get('object_format') != 'sha1'):
        raise ValueError('unregistered request/peer or invalid SHA')
    return request

def validate_path(name):
    if (not isinstance(name, str) or not name or len(name.encode()) > LIMITS['path']
            or any(ord(c) < 32 or ord(c) == 127 for c in name) or '\\' in name):
        raise ValueError('unsafe path')
    if any(p in ('','.','..','.git') or len(p.encode()) > LIMITS['component'] for p in name.split('/')):
        raise ValueError('unsafe path component')
    return name

def validate_link(name, target):
    if (not target or target.startswith('/') or '\\' in target or len(target.encode()) > LIMITS['path']
            or any(ord(c) < 32 or ord(c) == 127 for c in target)):
        raise ValueError('unsafe link')
    stack = name.split('/')[:-1]
    for part in target.split('/'):
        if part == '..':
            if not stack:
                raise ValueError('link escape')
            stack.pop()
        elif part not in ('','.'):
            if part == '.git' or len(part.encode()) > LIMITS['component']:
                raise ValueError('unsafe link component')
            stack.append(part)

def manifest(root, deadline=None):
    records = []
    for base, dirs, files in os.walk(root, followlinks=False):
        for name in sorted(dirs + files):
            if deadline is not None and time.monotonic() >= deadline: raise ValueError('manifest deadline')
            path = Path(base)/name
            info = path.lstat()
            rel = path.relative_to(root).as_posix()
            if stat.S_ISLNK(info.st_mode):
                records.append([rel,'symlink',os.readlink(path)])
            elif stat.S_ISDIR(info.st_mode):
                records.append([rel,'dir'])
            elif stat.S_ISREG(info.st_mode):
                h = hashlib.sha256()
                with path.open('rb') as stream:
                    for block in iter(lambda: stream.read(65536), b''):
                        if deadline is not None and time.monotonic() >= deadline: raise ValueError('manifest deadline')
                        h.update(block)
                records.append([rel,'file',bool(info.st_mode & 0o111),info.st_size,h.hexdigest()])
            else:
                raise ValueError('special snapshot file')
    records.sort(key=lambda row: row[0])
    return hashlib.sha256(json.dumps(records,ensure_ascii=True,separators=(',',':')).encode()).hexdigest(), records

class GitSource:
    """Read-only Git plumbing; never uses caller environment or Git transports."""
    def __init__(self, path, device, inode):
        self.path, self.identity = Path(path), (device, inode)
        self.deadline = time.monotonic() + LIMITS['seconds']
        self.check(self.deadline)
    def command(self, args, deadline, cap, object_request=None):
        argv = ['/usr/bin/git','--no-replace-objects','--git-dir='+str(self.path),
                '-c','core.hooksPath=/dev/null','-c','protocol.allow=never',
                '-c','safe.directory='+str(self.path), *args]
        proc = subprocess.Popen(argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                stderr=subprocess.DEVNULL, env=ENV, close_fds=True)
        selector = selectors.DefaultSelector()
        selector.register(proc.stdout, selectors.EVENT_READ)
        data, header, expected = bytearray(), None, None
        try:
            if object_request: proc.stdin.write(object_request[0].encode()+b'\n')
            proc.stdin.close()
            while selector.get_map():
                remaining = deadline - time.monotonic()
                if remaining <= 0: raise ValueError('import deadline')
                if not selector.select(min(remaining,0.1)): continue
                block = os.read(proc.stdout.fileno(),65536)
                if not block:
                    selector.unregister(proc.stdout)
                    break
                data.extend(block)
                if object_request and header is None:
                    pos = data.find(b'\n')
                    if pos < 0:
                        if len(data) > 256: raise ValueError('object header')
                        continue
                    header = bytes(data[:pos]).split(b' ')
                    if len(header) != 3 or header[:2] != [object_request[0].encode(),object_request[1].encode()]:
                        raise ValueError('missing object or wrong type')
                    if not re.fullmatch(rb'0|[1-9][0-9]{0,10}',header[2]): raise ValueError('object size')
                    expected = int(header[2])
                    if expected > cap: raise ValueError('object size bound')
                    del data[:pos+1]
                if len(data) > cap + (1 if object_request else 0): raise ValueError('Git output bound')
            if proc.wait(timeout=max(0.001,deadline-time.monotonic())) != 0: raise ValueError('Git plumbing failed')
            if object_request:
                if header is None or len(data) != expected+1 or data[-1:] != b'\n': raise ValueError('truncated object')
                return bytes(data[:-1])
            return bytes(data)
        except (OSError, subprocess.TimeoutExpired) as exc:
            raise ValueError('Git import failed') from exc
        finally:
            selector.close()
            proc.stdout.close()
            if proc.poll() is None:
                proc.kill()
                proc.wait(timeout=5)
    def check(self, deadline):
        if not self.path.is_absolute() or self.path.resolve() != self.path or not self.path.is_dir():
            raise ValueError('noncanonical registered source')
        info = self.path.stat()
        if (info.st_dev,info.st_ino) != self.identity: raise ValueError('source identity changed')
        for name in ('alternates','http-alternates'):
            if os.path.lexists(self.path/'objects/info'/name): raise ValueError('alternate source')
        pack = self.path/'objects/pack'
        if pack.exists():
            with os.scandir(pack) as entries:
                for n, entry in enumerate(entries):
                    if n >= LIMITS['objects'] or entry.name.endswith('.promisor'): raise ValueError('promisor/bound')
        if self.command(['for-each-ref','--count=1','refs/replace'],deadline,4096):
            raise ValueError('replacement ref')
        config = self.command(['config','--null','--list'],deadline,1024**2).decode('utf-8')
        for record in config.split('\0'):
            key, _, value = record.partition('\n')
            if ('promisor' in key.lower() or key.lower() == 'extensions.partialclone'
                    or (key.lower() == 'extensions.objectformat' and value != 'sha1')):
                raise ValueError('unsupported object source/format')
    def read(self, key, kind, limit, deadline):
        return self.command(['cat-file','--batch'],deadline,limit,(key,kind))

def export(source, sha, dest):
    if not isinstance(sha,str) or not SHA.fullmatch(sha): raise ValueError('full SHA required')
    deadline = time.monotonic() + LIMITS['seconds']
    if isinstance(source,GitSource): deadline = min(deadline,source.deadline)
    seen, entries, links = set(), {}, {}
    total, regular = 0, 0
    def tick():
        if time.monotonic() >= deadline: raise ValueError('import deadline')
    def read(key, kind):
        nonlocal total
        tick()
        if key not in seen and len(seen) >= LIMITS['objects']: raise ValueError('object bound')
        limit = min(LIMITS[kind], LIMITS['bytes']-total)
        try: data = source.read(key,kind,limit,deadline)
        except (KeyError, OSError) as exc: raise ValueError('missing object') from exc
        if len(data) > limit: raise ValueError('object bytes bound')
        actual = hashlib.sha1(kind.encode()+b' '+str(len(data)).encode()+b'\0'+data).hexdigest()
        if actual != key: raise ValueError('object hash mismatch')
        if key not in seen:
            seen.add(key)
            total += len(data)
        return data
    if isinstance(source,GitSource): source.check(deadline)
    commit = read(sha,'commit')
    headers, separator, _ = commit.partition(b'\n\n')
    trees = [line[5:] for line in headers.splitlines() if line.startswith(b'tree ')]
    if not separator or len(trees) != 1 or not re.fullmatch(rb'[0-9a-f]{40}',trees[0]):
        raise ValueError('commit tree header')
    tree = trees[0].decode()
    def walk(key, prefix='', depth=0):
        if depth > LIMITS['depth']: raise ValueError('tree depth bound')
        data, offset = read(key,'tree'), 0
        local = set()
        while offset < len(data):
            tick()
            end = data.find(b'\0',offset)
            if end < 0 or end+21 > len(data): raise ValueError('malformed tree')
            metadata = data[offset:end].split(b' ')
            if len(metadata) != 2: raise ValueError('malformed tree entry')
            mode, rawname = metadata
            try: name = rawname.decode('utf-8')
            except UnicodeError as exc: raise ValueError('unsupported name') from exc
            if '/' in name: raise ValueError('noncomponent tree name')
            path = validate_path(prefix+name)
            if name in local or path in entries: raise ValueError('duplicate tree path')
            local.add(name)
            if len(entries) >= LIMITS['entries']: raise ValueError('entry bound')
            child = data[end+1:end+21].hex()
            offset = end+21
            if mode not in (b'40000',b'100644',b'100755',b'120000'): raise ValueError('unsupported mode/gitlink')
            entries[path] = (mode,child)
            if mode == b'40000': walk(child,path+'/',depth+1)
            elif mode == b'120000':
                try: target = read(child,'blob').decode('utf-8')
                except UnicodeError as exc: raise ValueError('unsupported link') from exc
                validate_link(path,target)
                links[path] = target
    walk(tree)
    # Follow link chains lexically too: individually safe targets can compose an escape.
    for name in links:
        todo, stack, hops = name.split('/'), [], 0
        while todo:
            part = todo.pop(0)
            if part in ('','.'): continue
            if part == '..':
                if not stack: raise ValueError('composed link escape')
                stack.pop()
                continue
            stack.append(part)
            path = '/'.join(stack)
            if path in links:
                hops += 1
                if hops > LIMITS['depth']: raise ValueError('link cycle/depth')
                stack.pop()
                todo = links[path].split('/') + todo
    if entries.get('Cargo.lock', (None,))[0] not in (b'100644',b'100755'):
        raise ValueError('regular Cargo.lock required')
    dest = Path(dest)
    if not dest.is_absolute() or dest.parent.resolve() != dest.parent: raise ValueError('unsafe staging parent')
    parent = dest.parent.stat()
    if parent.st_uid != os.getuid() or parent.st_mode & 0o022: raise ValueError('unprotected staging parent')
    dest.mkdir(mode=0o700,exist_ok=False)
    lock_digest = None
    for name, (mode,key) in entries.items():
        tick()
        path = dest/name
        if mode == b'40000': path.mkdir(mode=0o700)
        elif mode == b'120000': path.symlink_to(links[name])
        else:
            data = read(key,'blob')
            regular += len(data)
            if regular > LIMITS['export']: raise ValueError('export byte bound')
            with path.open('xb') as stream: stream.write(data)
            path.chmod(0o555 if mode == b'100755' else 0o444)
            if name == 'Cargo.lock': lock_digest = hashlib.sha256(data).hexdigest()
    if isinstance(source,GitSource): source.check(deadline)
    for name, (mode,_) in reversed(list(entries.items())):
        if mode == b'40000': (dest/name).chmod(0o555)
    dest.chmod(0o555)
    digest, inventory = manifest(dest,deadline)
    tick()
    return {'commit':sha,'tree':tree,'content_sha256':digest,'entries':inventory,'lockfile_sha256':lock_digest}
