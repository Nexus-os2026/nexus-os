"""Receipt schema, atomic publication and integrity; host authentication is PR-2.
A success receipt is validated against its own recorded gate policy (SB3E-R19.5)."""
import ctypes
import hashlib
import json
import os
from pathlib import Path
import re
import stat

HERE = Path(__file__).resolve().parent
MUTABLE = 'Host toolchain and registry are mutable trusted inputs; inventories do not defeat change-and-restore attacks.'
REQUIRED = {'schema_version','run_id','peer_uid','request','attempts','source','snapshot','installed',
            'host_inputs','versions','input_observations','environment','coverage','commands','resources',
            'logs','isolation','cleanup','missing_evidence','limitations','status'}
GROUPS = ('request','source','snapshot','installed','host_inputs','versions','input_observations',
          'environment','coverage','resources','isolation','cleanup')
NAME = re.compile(r'[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}')
RUN = re.compile(r'[0-9a-f]{32}')
HEX = re.compile(r'[0-9a-f]{64}')
MAX_FILE = 64*1024**2
# SB3E-R18 P2/P4/P11: one row per gate set; `full` keeps its exact request and receipt structure.
GATE_SETS = {
    'full': dict(policy='gates.json',status='PASS_SCOPED',request=('repo_id','full_sha','gate_set_id'),extra=(),cwd={}),
    'route-cells': dict(policy='route-cells.json',status='PASS_ROUTE_CELLS',
                        request=('repo_id','full_sha','gate_set_id','inventory_sha256'),extra=('lane',),
                        cwd={'a7':'/work/src','route-cells':'/work/src/mother/host'})}
UNFINISHED = ('FAILED','BLOCKED','INTERRUPTED')
GATE_POLICY = ('policy/exclusions.json','policy/gates.json','policy/route-cells.json')
HISTORY, HISTORY_OWNER = Path('/etc/gatebox/history'), 0
LANE = {'a7','verdict_sha256','evidence','rustc_vv','cargo_v','lockfile_sha256','wrapper_sha256','harness_sha256',
        'inventory_sha256','names','trust'}

def unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result: raise ValueError('duplicate JSON key')
        result[key] = value
    return result

def encode(value):
    return (json.dumps(value,sort_keys=True,indent=2,allow_nan=False)+'\n').encode()

def policy():
    return json.loads((HERE/'policy/gates.json').read_text()), json.loads((HERE/'policy/exclusions.json').read_text())

def gate_set(request):
    """The request's gate-set row after its exact fields, types and SHA checks."""
    name = request.get('gate_set_id') if isinstance(request,dict) else None
    if not isinstance(name,str) or name not in GATE_SETS: raise ValueError('unknown gate set')
    entry = GATE_SETS[name]
    if set(request) != set(entry['request']): raise ValueError('request fields')
    if (any(type(v) is not str for v in request.values()) or not re.fullmatch('[0-9a-f]{40}',request['full_sha'])
            or not HEX.fullmatch(request.get('inventory_sha256','0'*64))): raise ValueError('request types/SHA')
    return entry

def request_for(repo, sha, gate_set_id, inventory=None):
    request = dict(repo_id=repo,full_sha=sha,gate_set_id=gate_set_id)
    if inventory is not None: request['inventory_sha256'] = inventory
    gate_set(request)
    return request

def policy_digest(manifest):
    """probe.installed_identity's gates digest over {installed path: SHA-256}."""
    return hashlib.sha256(json.dumps(sorted(manifest.items()),separators=(',',':')).encode()).hexdigest()

def current_policy():
    return policy_digest({str(HERE/n):hashlib.sha256((HERE/n).read_bytes()).hexdigest() for n in GATE_POLICY})

def historical(digest):
    """A retained policy set, content-addressed by its digest, root-owned and read-only."""
    try: directory = open_directory(HISTORY/digest)
    except ValueError as exc: raise ValueError('unknown receipt policy') from exc
    try:
        check(os.fstat(directory),HISTORY_OWNER,True)
        no_acl(directory)
        manifest = json.loads(read_file(directory,'manifest.json',HISTORY_OWNER),object_pairs_hook=unique)
        names = {Path(p).name for p in manifest} if isinstance(manifest,dict) else set()
        if (not names >= {'gates.json','exclusions.json'} or not names <= {Path(n).name for n in GATE_POLICY}
                or len(names) != len(manifest) or any(not isinstance(p,str) or not Path(p).is_absolute() for p in manifest)
                or policy_digest(manifest) != digest or set(os.listdir(directory)) != names|{'manifest.json'}):
            raise ValueError('historical policy inventory')
        files = {}
        for path, expected in manifest.items():
            data = read_file(directory,Path(path).name,HISTORY_OWNER)
            if hashlib.sha256(data).hexdigest() != expected: raise ValueError('historical policy bytes')
            files[Path(path).name] = json.loads(data,object_pairs_hook=unique)
        return files
    finally: os.close(directory)

def policy_set(digest):
    """The parsed gate policy a receipt recorded: the current install's, or its retained set. Never another's."""
    if not isinstance(digest,str) or not HEX.fullmatch(digest): raise ValueError('installed identity')
    if digest == current_policy():
        return {Path(n).name:json.loads((HERE/n).read_bytes(),object_pairs_hook=unique) for n in GATE_POLICY}
    return historical(digest)

def completed(command, logs):
    if (type(command.get('exit_code')) is not int or command['exit_code'] != 0
            or command.get('signal') is not None
            or any(command.get(k) is not False for k in ('timed_out','oom'))
            or any(command.get(k) is not True for k in ('status_complete','collector_complete'))
            or not isinstance(command.get('log'),str) or command['log'] not in logs
            or not isinstance(command.get('log_sha256'),str) or not HEX.fullmatch(command['log_sha256'])
            or command['log_sha256'] != logs[command['log']]):
        raise ValueError('failed/incomplete command')

def string_list(value):
    return isinstance(value,list) and all(isinstance(item,str) for item in value)

def valid_invocation(command):
    if command.get('gate_id',command.get('id')) in ('host','whitespace'):
        return 'argv' in command and command['argv'] is None and command.get('internal') is True
    return ('internal' not in command and string_list(command.get('argv'))
            and bool(command['argv']))

def validate_summary(summary):
    request = summary.get('request') if isinstance(summary,dict) else None
    name = request.get('gate_set_id') if isinstance(request,dict) else None
    entry = GATE_SETS.get(name) if isinstance(name,str) else None
    extra = set(entry['extra']) if entry else set()
    if not isinstance(summary,dict) or set(summary) != REQUIRED|extra: raise ValueError('receipt fields')
    if (type(summary['schema_version']) is not int or summary['schema_version'] != 1
            or not isinstance(summary['run_id'],str) or not RUN.fullmatch(summary['run_id'])
            or type(summary['peer_uid']) is not int or summary['peer_uid'] < 0): raise ValueError('receipt identity/schema')
    if summary['status'] not in UNFINISHED+tuple(g['status'] for g in GATE_SETS.values()): raise ValueError('receipt status')
    if any(type(summary[k]) is not dict for k in GROUPS+tuple(extra)): raise ValueError('receipt evidence groups')
    if any(type(summary[k]) is not list for k in ('attempts','commands','logs','missing_evidence','limitations')):
        raise ValueError('receipt inventories')
    if MUTABLE not in summary['limitations']: raise ValueError('missing mutable-input limitation')
    if len(summary['commands']) > 512: raise ValueError('command bound')
    cwds = {'/work/src',*(entry['cwd'].values() if entry else ())}
    for command in summary['commands']:
        if (not isinstance(command,dict) or not isinstance(command.get('id'),str)
                or not valid_invocation(command)
                or command.get('cwd') not in cwds or not isinstance(command.get('environment'),dict)
                or any(key in command and not isinstance(command[key],str) for key in ('gate_id','artifact_id'))
                or ('artifact_ids' in command and not string_list(command['artifact_ids']))
                or ('inventory' in command and not isinstance(command['inventory'],dict))):
            raise ValueError('command invocation evidence')
    for log in summary['logs']:
        if (not isinstance(log,dict) or not isinstance(log.get('file'),str) or not NAME.fullmatch(log['file'])
                or not isinstance(log.get('sha256'),str) or not HEX.fullmatch(log['sha256'])):
            raise ValueError('log evidence')
    if len({log['file'] for log in summary['logs']}) != len(summary['logs']): raise ValueError('duplicate log')
    if summary['status'] in UNFINISHED: return
    if entry is None or summary['status'] != entry['status']: raise ValueError('success status/gate set mismatch')
    gate_set(request)
    for key in ('bundle_sha256','policy_sha256','gates_sha256'):
        if not isinstance(summary['installed'].get(key),str) or not HEX.fullmatch(summary['installed'][key]): raise ValueError('installed identity')
    policies = policy_set(summary['installed']['gates_sha256'])
    if name == 'route-cells': return validate_route(summary,policies)
    gates, exclusions = policies['gates.json'], policies['exclusions.json']
    if summary['environment'] != gates['environment']: raise ValueError('gate environment mismatch')
    expected = [g['id'] for g in gates['gates']]
    coverage = summary['coverage']
    if (summary['missing_evidence'] or summary['cleanup'].get('complete') is not True
            or any(not summary[k] for k in GROUPS) or not summary['logs']
            or summary['input_observations'].get('before') is None
            or summary['input_observations'].get('before') != summary['input_observations'].get('after')):
        raise ValueError('incomplete success evidence')
    if summary['snapshot'].get('commit') != request['full_sha']: raise ValueError('success snapshot/request')
    for key in ('content_sha256','lockfile_sha256'):
        if not isinstance(summary['snapshot'].get(key),str) or not HEX.fullmatch(summary['snapshot'][key]): raise ValueError('snapshot identity')
    if (any(coverage.get(k) != expected for k in ('expected','executed','passed'))
            or any(coverage.get(k) != [] for k in ('failed','ignored','filtered'))
            or coverage.get('excluded') != [{'name':n,'status':'excluded','reason':exclusions['reason']} for n in exclusions['cases']]):
        raise ValueError('incomplete gate/exclusion inventory')
    ids = [c['id'] for c in summary['commands']]
    if len(set(ids)) != len(ids): raise ValueError('duplicate command ID')
    gate_ids = [c.get('gate_id',c['id']) for c in summary['commands']]
    if set(gate_ids) != set(expected): raise ValueError('missing invocation evidence')
    artifacts = coverage.get('artifacts')
    if not isinstance(artifacts,list) or not artifacts: raise ValueError('missing compiled artifact inventory')
    eligible = {}
    for artifact in artifacts:
        if (not isinstance(artifact,dict) or not isinstance(artifact.get('id'),str)
                or artifact['id'] in eligible or artifact.get('adapter') != 'libtest'
                or not isinstance(artifact.get('executable'),str)
                or not artifact['executable'].startswith('/work/target/')
                or '..' in artifact['executable'].split('/')
                or any(not isinstance(artifact.get(k),str) or not artifact[k] for k in ('package','target'))
                or (artifact['package'],artifact['target']) == ('nexus-verifier-sandbox',exclusions['target'])):
            raise ValueError('unsupported/invalid artifact')
        eligible[artifact['id']] = artifact
    workspace = [c for c in summary['commands'] if c.get('gate_id',c['id']) == 'workspace']
    executed = [c.get('artifact_id') for c in workspace]
    if set(executed) != set(eligible) or len(executed) != len(eligible): raise ValueError('missing artifact execution')
    host = {a['id'] for a in artifacts if a['package'] == 'nexus-mother-host'}
    host_artifacts = coverage.get('host_artifacts')
    if (not host or not string_list(host_artifacts) or len(host_artifacts) != len(host)
            or set(host_artifacts) != host): raise ValueError('host artifacts missing')
    logs = {log['file']:log['sha256'] for log in summary['logs']}
    for command in summary['commands']:
        gate = next(g for g in gates['gates'] if g['id'] == command.get('gate_id',command['id']))
        if command['environment'] != gates['environment']: raise ValueError('command environment mismatch')
        if 'argv' in gate and command['argv'] != gate['argv']: raise ValueError('gate argv mismatch')
        if gate['id'] in ('workspace','desktop-live'):
            artifact = eligible.get(command.get('artifact_id'))
            if artifact is None or command['argv'] != [artifact['executable']]: raise ValueError('artifact invocation mismatch')
            if gate['id'] == 'desktop-live' and (artifact['package'],artifact['target']) != ('nexus-mother-desktop','live'):
                raise ValueError('desktop artifact mismatch')
        elif gate['id'] == 'host' and command.get('artifact_ids') != sorted(host):
            raise ValueError('host reconciliation evidence')
        completed(command,logs)
        if gate.get('adapter') in ('libtest','libtest-repeat','reconcile-all-artifacts'):
            inventory = command.get('inventory',{})
            names, ignored = inventory.get('expected'), inventory.get('ignored')
            # Q33: #[ignore] tests are recorded as ignored (a subset of the listed names, never passed);
            # every other listed test passed. Only a workspace artifact may be a zero-test target.
            if (not string_list(names) or len(set(names)) != len(names)
                    or not string_list(ignored) or len(set(ignored)) != len(ignored) or not set(ignored) <= set(names)
                    or inventory.get('passed') != [n for n in names if n not in set(ignored)]
                    or inventory.get('executed') != inventory['passed']
                    or (not names and gate['id'] != 'workspace')
                    or any(inventory.get(k) != [] for k in ('failed','filtered'))):
                raise ValueError('test reconciliation')
            if gate['id'] == 'desktop-live' and (names != gate['expected_names'] or ignored):
                raise ValueError('desktop inventory')
    host_names, host_passed, host_ignored = set(), set(), set()
    for command in workspace:
        if command['artifact_id'] in host:
            host_names.update(command['inventory']['expected'])
            host_passed.update(command['inventory']['passed'])
            host_ignored.update(command['inventory']['ignored'])
    for command in summary['commands']:
        if command.get('gate_id',command['id']) == 'host':
            inventory = command['inventory']
            # Q33 (review F6-3 note 4): the aggregate carries the workspace partition exactly.
            if (not host_names or set(inventory['expected']) != host_names
                    or set(inventory['passed']) != host_passed or set(inventory['ignored']) != host_ignored):
                raise ValueError('host test inventory mismatch')

def validate_route(summary, policies):
    """PASS_ROUTE_CELLS: A7 `run` PASS, sealed evidence, the committed harness inventory (SB3E-R18, R19, R20)."""
    route = policies.get('route-cells.json')
    if not isinstance(route,dict): raise ValueError('route-cells policy missing')
    request, lane, coverage, snapshot = summary['request'], summary['lane'], summary['coverage'], summary['snapshot']
    observed = summary['input_observations']
    if (summary['missing_evidence'] or summary['cleanup'].get('complete') is not True
            or any(not summary[k] for k in GROUPS+('lane',)) or not summary['logs']
            or snapshot.get('commit') != request['full_sha']
            or any(not isinstance(snapshot.get(k),str) or not HEX.fullmatch(snapshot[k]) for k in ('content_sha256','lockfile_sha256'))):
        raise ValueError('incomplete route success evidence')
    if summary['environment'] != route['environment']: raise ValueError('route environment')
    if (not isinstance(observed.get('before'),dict) or observed['before'] != observed.get('after')
            or observed['before'].get('rust') != route['toolchain']['manifest_sha256']): raise ValueError('route toolchain pin')
    if set(lane) != LANE: raise ValueError('route lane keys')
    if lane['a7'] != 'PASS': raise ValueError('route A7 verdict')
    if lane['wrapper_sha256'] != route['a7']['wrapper_sha256']: raise ValueError('route wrapper')
    if lane['trust'] != route['trust']: raise ValueError('route trust')
    names = lane['names']
    if (lane['lockfile_sha256'] != snapshot['lockfile_sha256'] or not isinstance(lane['harness_sha256'],str)
            or not HEX.fullmatch(lane['harness_sha256']) or any(not isinstance(lane[k],str) or not lane[k] for k in ('rustc_vv','cargo_v'))
            or not string_list(names) or not names or len(set(names)) != len(names)): raise ValueError('route lane identity')
    if lane['inventory_sha256'] != request['inventory_sha256']: raise ValueError('route inventory commitment')
    if hashlib.sha256('\n'.join(names).encode()).hexdigest() != request['inventory_sha256']: raise ValueError('route inventory names')
    logs = {log['file']:log['sha256'] for log in summary['logs']}
    evidence = {'a7-'+rel.replace('/','-')+'.log' for rel in route['a7']['evidence']}  # init log IDs plus '.log'
    if (logs.get('a7-verdict.json.log') != lane['verdict_sha256'] or not isinstance(lane['evidence'],dict)
            or set(lane['evidence']) != evidence or any(logs.get(n) != h for n,h in lane['evidence'].items())):
        raise ValueError('route A7 evidence')
    gates = route['gates']
    ids, harness, artifacts = [g['id'] for g in gates], gates[1], coverage.get('artifacts')
    if (set(coverage) != {'expected','executed','passed','failed','ignored','filtered','excluded','artifacts','discovery'}
            or any(coverage[k] != ids for k in ('expected','executed','passed'))
            or any(coverage[k] != [] for k in ('failed','ignored','filtered','excluded'))
            or not isinstance(artifacts,list) or len(artifacts) != 1 or not isinstance(artifacts[0],dict)
            or artifacts[0] != dict(id=harness['target'],adapter='libtest',executable=artifacts[0].get('executable'),
                                    package=harness['package'],target=harness['target'])
            or not isinstance(artifacts[0]['executable'],str) or not re.fullmatch(harness['executable'],artifacts[0]['executable'])):
        raise ValueError('route coverage')
    executable, cwd = artifacts[0]['executable'], GATE_SETS['route-cells']['cwd']
    listing = coverage['discovery']
    if (not isinstance(listing,list) or len(listing) != 1 or listing[0] != dict(id=listing[0].get('id'),gate_id='route-cells-list',
            argv=[executable,'--list','--format','terse'],cwd=cwd['route-cells'],environment=route['environment'],exit_code=0,signal=None)):
        raise ValueError('route discovery')
    commands = summary['commands']
    if ([c.get('gate_id') for c in commands] != ids or len({c['id'] for c in commands}) != len(commands)
            or any(c['argv'] != g.get('argv',[executable]) or c['cwd'] != cwd[g['id']] or c['environment'] != route['environment']
                   for c,g in zip(commands,gates)) or commands[1].get('artifact_id') != harness['target']):
        raise ValueError('route invocation')
    for command in commands: completed(command,logs)
    if commands[1].get('inventory') != dict(expected=names,executed=names,passed=names,failed=[],ignored=[],filtered=[]):
        raise ValueError('route test inventory')

def check(info, owner_uid, directory=False):
    if (info.st_uid != owner_uid or info.st_mode & 0o022
            or (directory and not stat.S_ISDIR(info.st_mode))
            or (not directory and (not stat.S_ISREG(info.st_mode) or info.st_nlink != 1 or info.st_mode & 0o200))):
        raise ValueError('unsafe receipt ownership/type/write mode')

def open_directory(path):
    """No symlink in any component; this alone is not protected-host ancestry."""
    path = Path(path)
    if not path.is_absolute() or '..' in path.parts: raise ValueError('absolute receipt directory required')
    fd = os.open('/',os.O_RDONLY|os.O_DIRECTORY)
    try:
        for name in path.parts[1:]:
            child = os.open(name,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW,dir_fd=fd)
            os.close(fd)
            fd = child
        return fd
    except OSError as exc:
        os.close(fd)
        raise ValueError('unsafe receipt ancestry') from exc

def no_acl(fd):
    if any(n in ('system.posix_acl_access','system.posix_acl_default') for n in os.listxattr(fd)):
        raise ValueError('receipt ACL requires host policy')

def read_file(directory, name, owner_uid):
    if not isinstance(name,str) or not NAME.fullmatch(name): raise ValueError('unsafe artifact name')
    try: fd = os.open(name,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK,dir_fd=directory)
    except OSError as exc: raise ValueError('missing/linked receipt file') from exc
    try:
        info = os.fstat(fd)
        check(info,owner_uid)
        no_acl(fd)
        if info.st_size > MAX_FILE: raise ValueError('receipt file bound')
        chunks, size = [], 0
        while True:
            block = os.read(fd,65536)
            if not block: break
            size += len(block)
            if size > MAX_FILE: raise ValueError('receipt file grew beyond bound')
            chunks.append(block)
        if size != info.st_size: raise ValueError('receipt changed during read')
        return b''.join(chunks)
    finally: os.close(fd)

def verify_integrity(root, owner_uid):
    root = Path(root)
    if not RUN.fullmatch(root.name): raise ValueError('invalid receipt ID')
    parent = open_directory(root.parent)
    directory = None
    try:
        check(os.fstat(parent),owner_uid,True)
        no_acl(parent)
        try: directory = os.open(root.name,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW,dir_fd=parent)
        except OSError as exc: raise ValueError('unsafe receipt directory') from exc
        check(os.fstat(directory),owner_uid,True)
        no_acl(directory)
        seal = json.loads(read_file(directory,'seal.json',owner_uid),object_pairs_hook=unique)
        if (not isinstance(seal,dict) or set(seal) != {'schema_version','files'}
                or type(seal['schema_version']) is not int or seal['schema_version'] != 1
                or not isinstance(seal['files'],dict) or len(seal['files']) > 513):
            raise ValueError('inventory schema')
        if set(os.listdir(directory)) != set(seal['files'])|{'seal.json'} or 'summary.json' not in seal['files']:
            raise ValueError('receipt inventory mismatch')
        summary, total = None, 0
        for name, entry in seal['files'].items():
            if (not isinstance(entry,dict) or set(entry) != {'bytes','sha256'}
                    or type(entry['bytes']) is not int or not 0 <= entry['bytes'] <= MAX_FILE
                    or not isinstance(entry['sha256'],str) or not HEX.fullmatch(entry['sha256'])):
                raise ValueError('artifact inventory schema')
            total += entry['bytes']
            if total > 2*MAX_FILE: raise ValueError('receipt total bound')
            data = read_file(directory,name,owner_uid)
            if entry != {'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()}: raise ValueError('artifact hash/size mismatch')
            if name == 'summary.json': summary = json.loads(data,object_pairs_hook=unique)
        validate_summary(summary)
        if summary['run_id'] != root.name: raise ValueError('run ID mismatch')
        for log in summary['logs']:
            if seal['files'].get(log['file'],{}).get('sha256') != log['sha256']: raise ValueError('summary log mismatch')
        return {'authentication':'UNAUTHENTICATED','summary':summary}
    finally:
        if directory is not None: os.close(directory)
        os.close(parent)

def publish(pending, receipts, run_id, summary, artifacts):
    """Retain failed pending data; Linux renameat2 provides atomic no-replace."""
    validate_summary(summary)
    if not RUN.fullmatch(run_id) or summary['run_id'] != run_id: raise ValueError('publication run ID')
    if (not isinstance(artifacts,dict) or len(artifacts) > 512
            or any(not NAME.fullmatch(n) or n in ('summary.json','seal.json','seal.new') for n in artifacts)):
        raise ValueError('publication artifact inventory')
    if any(type(data) is not bytes or len(data) > MAX_FILE for data in artifacts.values()): raise ValueError('artifact bound')
    if sum(len(data) for data in artifacts.values()) > MAX_FILE: raise ValueError('run artifact bound')
    for log in summary['logs']:
        data = artifacts.get(log['file'])
        if data is None or hashlib.sha256(data).hexdigest() != log['sha256']: raise ValueError('publication log mismatch')
    pending, receipts = Path(pending), Path(receipts)
    source = open_directory(pending)
    directory, target = None, None
    try:
        target = open_directory(receipts)
        for fd in (source,target):
            check(os.fstat(fd),os.getuid(),True)
            no_acl(fd)
        if os.fstat(source).st_dev != os.fstat(target).st_dev: raise ValueError('publication filesystem mismatch')
        if os.fstat(source).st_mode & 0o077: raise ValueError('pending must be private')
        if os.path.lexists(receipts/run_id): raise FileExistsError('receipt ID already published')
        os.mkdir(run_id,0o700,dir_fd=source)
        directory = os.open(run_id,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW,dir_fd=source)
        inventory = {}
        files = dict(artifacts, **{'summary.json':encode(summary)})
        for name, data in files.items():
            if len(data) > MAX_FILE: raise ValueError('summary/file bound')
            fd = os.open(name,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600,dir_fd=directory)
            with os.fdopen(fd,'wb') as stream:
                stream.write(data)
                stream.flush()
                os.fchmod(stream.fileno(),0o444)
                os.fsync(stream.fileno())
            inventory[name] = {'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()}
        seal = encode({'schema_version':1,'files':inventory})
        fd = os.open('seal.new',os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600,dir_fd=directory)
        with os.fdopen(fd,'wb') as stream:
            stream.write(seal)
            stream.flush()
            os.fchmod(stream.fileno(),0o444)
            os.fsync(stream.fileno())
        os.rename('seal.new','seal.json',src_dir_fd=directory,dst_dir_fd=directory)
        os.fchmod(directory,0o755)
        os.fsync(directory)
        os.fsync(source)
        libc = ctypes.CDLL(None,use_errno=True)
        rename = getattr(libc,'renameat2',None)
        if rename is None: raise ValueError('atomic no-replace unavailable')
        rename.argtypes = [ctypes.c_int,ctypes.c_char_p,ctypes.c_int,ctypes.c_char_p,ctypes.c_uint]
        rename.restype = ctypes.c_int
        if rename(source,run_id.encode(),target,run_id.encode(),1) != 0:
            error = ctypes.get_errno()
            raise OSError(error,os.strerror(error))
        os.fsync(target)
        os.fsync(source)
        return receipts/run_id
    finally:
        if directory is not None: os.close(directory)
        os.close(source)
        if target is not None: os.close(target)
