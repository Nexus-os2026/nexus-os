#!/bin/bash
# Offline bootstrap. Never run from a builder-owned checkout.
set -euo pipefail
umask 022
exec /usr/bin/python3 -I -B - "$@" <<'PY'
import argparse
from contextlib import contextmanager
import fcntl
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import pwd
import grp
import re
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import time

DEST = Path('/usr/libexec/gatebox')
CONFIG = Path('/etc/gatebox')
ROOT = Path('/usr/lib/gatebox/root')
STATE = Path('/var/lib/mothergate')
CONTROL = Path('/run/mothergate-control')
CONTROL_POLICY = Path('/etc/tmpfiles.d/mothergate-control.conf')
BOOT = Path('/root/gatebox-slice1')
FIXTURE = '8e0d6ae293df999eb6530e875df1206deac872a8'
HARNESS = 'crates/nexus-verifier-sandbox/tests/phase2_live_sandbox.rs'
FILES = ['MISSION.md','snapshot.py','receipt.py','supervisor.py','init.py','client.py',
         'lifecycle.py','verify.py','probe.py','rootfs.py','install.sh','README.md','tests/test_service.py',
         'tests/test_unit_core.py','tests/test_unit_contract.py','tests/test_installed.py',
         'tests/test_custody.py','tests/test_install_composition.py',
         'policy/custody.json','policy/mothergate-control.conf',
         'policy/gates.json','policy/exclusions.json','policy/registry.json','policy/sudoers',
         'policy/deny-systemd.rules','units/mothergate.socket','units/mothergate.service',
         'units/mothergate-run@.service','units/mothergate-root@.service',
         # GB1 (SB3E-R18 P10): every route lane file, its policy and controls.
         'policy/route-cells.json','route/route_a7.py','route/route_a7_controls.py','route/route_a7_graph.py',
         'route/route_a7_sweep.py','route/route_rustc.c','tests/test_route_gate_set.py',
         # GB3 (SB3E-R48): route A1, which the box runs before A7, and its controls (run live after install).
         'route/route_a1.py','route/route_a1_controls.py']
# GB3-7 (SB3E-R48.3): every other tracked tools/gatebox file, reviewed and never packaged. The README PIN step
# requires FILES and NOT_INSTALLED to partition the tracked tree exactly.
NOT_INSTALLED = ['route/route_a2_controls.py','route/route_a3.py','route/route_a3_controls.py','route/route_a4.py',
                 'route/route_a5.json','route/route_a5.py','route/route_a6.py','route/route_exit.json','route/route_exit.py',
                 'route/route_plants.json','route/route_plants.py','route/route_seams.json']
# SB3E-R18 P12: built unprivileged by the README PIN step (reproducible); root only verifies the literal.
WRAPPER = 'route/route_rustc'
WRAPPER_SHA256 = 'd23fdea2e987259b7c07a8ab7fe039cbd0c330ecf40b317cb5ad4195c69a7762'
EXECUTABLE = ('install.sh','lifecycle.py','verify.py')
TOOLCHAIN = Path('/home/nexus/.rustup/toolchains/1.94.0-x86_64-unknown-linux-gnu')
UPGRADE_BOOT = Path('/root/gatebox-gb3')  # GB3: a new bootstrap directory; the reviewed GB1 and GB2 bundles stay where they are
SOURCE_GIT = Path('/home/nexus/NEXUS/nexus-os/.git')
HISTORY = CONFIG/'history'
ETC = Path('/etc')
RUN_UNIT = Path('/etc/systemd/system/mothergate-run@.service')
STATE_ROOT_OWNER = (0,0)
USED = re.compile(r'\.used-[0-9a-f]{32}')
INSTANCE = re.compile(r'mothergate-(run|root)@[0-9a-f]{32}\.service')
CGROUP = Path('/sys/fs/cgroup')
# SB3E-R22.1: template instances sit in their template slice ('-' escaped as \x2d); the manager in system.slice.
SLICES = {'run':'system-mothergate\\x2drun.slice','root':'system-mothergate\\x2droot.slice'}
LOCK_OWNER = CUSTODY_OWNER = (0,0)
MAINTENANCE_TIMEOUT = 5
SUPERSEDED = ('identity.json','qualification.json','admission.json','installed-files.json','bundle.json','probe-context.json')
PLACEHOLDER = {'identity':None,'passed':[]}
ENV = {'PATH':'/usr/sbin:/usr/bin:/sbin:/bin','LANG':'C.UTF-8','HOME':'/root'}

def protected(path):
    for item in [*reversed(path.parents),path]:
        info = item.lstat()
        if (info.st_uid or info.st_mode & 0o022 or stat.S_ISLNK(info.st_mode)
                or any(n.startswith('system.posix_acl_') for n in os.listxattr(item))):
            raise ValueError('unprotected installation path: '+str(item))

def render(text,uid,gid):
    if any(type(n) is not int or not 0 < n < 2**31 for n in (uid,gid)):
        raise ValueError('invalid installed account IDs')
    result = text.replace('@GATERUN_UID@',str(uid)).replace('@GATERUN_GID@',str(gid))
    if re.search(r'@[A-Z_]+@',result): raise ValueError('unknown unit substitution')
    return result

def bundle_files(path,digest):
    if path.stat().st_size > 32*1024**2: raise ValueError('bundle size bound')
    data = path.read_bytes()
    if not re.fullmatch('[0-9a-f]{64}',digest) or hashlib.sha256(data).hexdigest() != digest:
        raise ValueError('bundle SHA-256 mismatch')
    expected = {'tools/gatebox/'+n for n in FILES+[WRAPPER]}|{HARNESS}
    result = {}
    with tarfile.open(fileobj=io.BytesIO(data),mode='r:') as archive:
        for item in archive:
            name = item.name
            parts = Path(name).parts
            if item.isdir() and name in {str(p) for n in expected for p in Path(n).parents if str(p) != '.'}:
                continue
            if (name not in expected or name in result or not item.isfile()
                    or '..' in parts or item.size > 2*1024**2):
                raise ValueError('unsafe/extra/duplicate bundle member: '+name)
            result[name] = archive.extractfile(item).read()
    if set(result) != expected: raise ValueError('incomplete bundle')
    raw = result[HARNESS]
    if hashlib.sha1(b'blob '+str(len(raw)).encode()+b'\0'+raw).hexdigest() != '6cf914ba5ae1eeffc1b61376ba6b768be8a0ffdb':
        raise ValueError('pinned harness blob mismatch')
    if hashlib.sha256(result['tools/gatebox/'+WRAPPER]).hexdigest() != WRAPPER_SHA256:
        raise ValueError('reviewed wrapper SHA-256 mismatch')
    return result

def payload_policies(files):
    return {name:files['tools/gatebox/policy/'+name] for name in ('gates.json','exclusions.json','route-cells.json')}

def toolchain_check(files,path=TOOLCHAIN):
    # SB3E-R18 P8: content at checking time only; init checks the same pin before A7.
    scope = {'__name__':'gbx_manifest'}
    exec(compile(files['tools/gatebox/snapshot.py'],'snapshot.py','exec'),scope)
    pin = json.loads(files['tools/gatebox/policy/route-cells.json'])['toolchain']['manifest_sha256']
    if scope['manifest'](path,None)[0] != pin: raise ValueError('toolchain differs from the reviewed /opt/rust pin')

def load_module(path,name):
    spec = importlib.util.spec_from_file_location(name,path)
    module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
    return module

def run(argv):
    return subprocess.run(argv,env=ENV,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE,close_fds=True,timeout=30,check=True).stdout

def require_control_absent():
    # Reboot-only maintenance: no removal, repair, traversal or chown of an
    # existing runtime entry. Even a dangling link is retained and refused.
    protected(CONTROL.parent)
    fd = os.open(CONTROL.parent,os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
    try:
        try: os.stat(CONTROL.name,dir_fd=fd,follow_symlinks=False)
        except FileNotFoundError: return
        raise ValueError('reboot required after review of retained custody: '+str(CONTROL))
    finally: os.close(fd)

def prepare_control():
    # Stop activation as well as the collector before inspecting runtime state.
    # Any manager failure propagates; it never permits tmpfiles to proceed.
    run(['/usr/bin/systemctl','stop','mothergate.socket','mothergate.service'])
    require_control_absent()

def publish_units(pairs):
    # Read-only precheck: an old runtime entry must leave no published units.
    require_control_absent()
    outputs = []
    for staged,target in pairs:
        os.rename(staged,target); outputs.append(target)
    run(['/usr/bin/systemctl','daemon-reload'])
    return outputs

def create_control():
    # Recheck immediately before creation in case state appeared during install.
    require_control_absent()
    run(['/usr/bin/systemd-tmpfiles','--create',str(CONTROL_POLICY)])

def write(path,data,mode=0o644):
    protected(path.parent)
    fd = os.open(path,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,mode)
    with os.fdopen(fd,'wb') as stream:
        stream.write(data); stream.flush(); os.fchmod(stream.fileno(),mode); os.fsync(stream.fileno())

def record(path,value): write(path,(json.dumps(value,sort_keys=True)+'\n').encode())

def acceptance_check(identity,value,state,authenticate,*,sensors=None):
    custody = {'C1-SEALED','C2-CANARY','C3-RESTART'}
    rows = {'B1','B2','B3','B4','A1','A5','A6','A8','E2E','CLIENTS'} | custody
    if (value.get('identity') != identity or set(value.get('passed',[])) != rows
            or state != {'identity':identity,'stage':'qualifying'}
            or set(value.get('evidence',{})) != rows):
        raise ValueError('qualification incomplete/stale or provenance missing')
    if sensors is None: import probe as sensors
    sensors.acceptance_custody(value)
    for row, evidence in value['evidence'].items():
        summary = authenticate(evidence['run_id'])['summary']
        if (summary['status'] != 'PASS_SCOPED' or summary['request'] != dict(repo_id='mother',full_sha=FIXTURE,gate_set_id='full')
                or summary['installed'] != dict(identity,stage='qualifying')
                or evidence['summary_sha256'] != hashlib.sha256(json.dumps(summary,sort_keys=True).encode()).hexdigest()):
            raise ValueError('acceptance receipt binding: '+row)
        if row == 'CLIENTS':
            provenance = evidence.get('client_provenance',{})
            if (provenance.get('uid',0) <= 0 or provenance.get('uid') != summary.get('peer_uid')
                    or provenance.get('context') != 'claude' or provenance.get('ancestor_pid',0) <= 1
                    or provenance.get('accepted') != dict(status='ACCEPTED',run_id=summary.get('run_id'))
                    or provenance.get('refused') != dict(status='UNQUALIFIED',reason='QUALIFICATION_ONLY')
                    or provenance.get('codex') != 'unsupported (runner denies AF_UNIX)'):
                raise ValueError('CLIENTS provenance missing or unbound')

def preflight_accounts(submitter):
    users,groups = pwd.getpwall(),grp.getgrall()
    submit = next(u for u in users if u.pw_name==submitter)
    if submit.pw_uid==0: raise ValueError('non-root submitter required')
    for user in users:
        if user.pw_name in (submitter,'mothergate','gaterun'):
            if sum(u.pw_uid==user.pw_uid for u in users)!=1: raise ValueError('shared gate/submitter UID')
    group = next((g for g in groups if g.gr_name=='gatesubmit'),None)
    if group and (any(u.pw_gid==group.gr_gid and u.pw_name!=submitter for u in users)
                  or any(n!=submitter for g in groups if g.gr_gid==group.gr_gid for n in g.gr_mem)):
        raise ValueError('unexpected gatesubmit member')
    return submit

def plan_accounts():
    users,groups = pwd.getpwall(),grp.getgrall(); plan = []
    used = {u.pw_uid for u in users}|{g.gr_gid for g in groups}
    for name in ('mothergate','gaterun'):
        entry = next((u for u in users if u.pw_name==name),None)
        if entry:
            locked = run(['/usr/bin/passwd','--status',name]).split()[1]
            if (entry.pw_uid==0 or entry.pw_shell!='/usr/sbin/nologin' or locked!=b'L'
                    or os.getgrouplist(name,entry.pw_gid)!=[entry.pw_gid]
                    or any(u.pw_gid==entry.pw_gid and u.pw_name!=name for u in users)
                    or len([g for g in groups if g.gr_gid==entry.pw_gid and g.gr_name==name])!=1):
                raise ValueError('unsafe gate identity: '+name)
        else:
            if any(g.gr_name==name for g in groups): raise ValueError('orphan gate group: '+name)
            number = next(n for n in range(999,99,-1) if n not in used); used.add(number)
            entry = pwd.struct_passwd((name,'x',number,number,'','/nonexistent','/usr/sbin/nologin'))
        plan.append((entry,entry not in users))
    if plan[0][0].pw_gid==plan[1][0].pw_gid: raise ValueError('distinct groups required')
    return plan

def apply_accounts(plan,submitter):
    created = dict(users=[],groups=[])
    for entry,create in plan:
        if create:
            run(['/usr/sbin/groupadd','--system','--gid',str(entry.pw_gid),entry.pw_name])
            created['groups'].append(entry.pw_name)
            run(['/usr/sbin/useradd','--system','--uid',str(entry.pw_uid),'--gid',str(entry.pw_gid),
                 '--no-create-home','--home-dir','/nonexistent','--shell','/usr/sbin/nologin',entry.pw_name])
            created['users'].append(entry.pw_name)
    try: grp.getgrnam('gatesubmit')
    except KeyError:
        run(['/usr/sbin/groupadd','--system','gatesubmit'])
        created['groups'].append('gatesubmit')
    run(['/usr/sbin/usermod','--append','--groups','gatesubmit',submitter])
    return created

def stage_security(files,uid,gid,stage):
    protected(stage)
    if stat.S_IMODE(stage.stat().st_mode) != 0o700: raise ValueError('private staging required')
    outputs = []
    for name in ('mothergate.socket','mothergate.service','mothergate-run@.service','mothergate-root@.service'):
        path = stage/name
        write(path,render(files['tools/gatebox/units/'+name].decode(),uid,gid).encode())
        outputs.append((path,Path('/etc/systemd/system')/name))
    for name,target,mode in (('sudoers','/etc/sudoers.d/gatebox',0o440),
                             ('deny-systemd.rules','/etc/polkit-1/rules.d/00-gatebox-deny.rules',0o644)):
        path = stage/name; write(path,files['tools/gatebox/policy/'+name],mode)
        outputs.append((path,Path(target)))
    run(['/usr/sbin/visudo','-cf',str(stage/'sudoers')])
    run(['/usr/bin/systemd-analyze','verify',*[str(p) for p,t in outputs[:4]]])
    return outputs

def replace(path,data,mode=0o644):
    path.parent.mkdir(parents=True,exist_ok=True,mode=0o755)
    temporary = path.with_name('.'+path.name+'.next')
    write(temporary,data,mode); os.replace(temporary,path)

def verify_old_install(old_sha,identity_of):
    # SB3E-R19.5: the existing install is exactly its own record before anything changes.
    protected(DEST); protected(CONFIG)
    if json.loads((CONFIG/'bundle.json').read_bytes()) != {'sha256':old_sha}:
        raise ValueError('installed bundle is not the reviewed old bundle')
    inventory = json.loads((CONFIG/'installed-files.json').read_bytes())
    if not isinstance(inventory,dict) or not inventory: raise ValueError('installed file inventory missing')
    for name,digest in inventory.items():
        path = Path(name); protected(path)
        if not stat.S_ISREG(path.lstat().st_mode) or hashlib.sha256(path.read_bytes()).hexdigest() != digest:
            raise ValueError('installed bytes changed: '+name)
    for path in DEST.rglob('*'):
        if not stat.S_ISDIR(path.lstat().st_mode) and str(path) not in inventory:
            raise ValueError('unrecorded installed file: '+str(path))
    identity = json.loads((CONFIG/'identity.json').read_bytes())
    if identity_of() != identity: raise ValueError('installed identity is not its record')
    return identity,inventory

@contextmanager
def maintenance():
    """SB3E-R21.2: lifecycle.py's custody lock, then probe.py's qualification lock, both held from the first
    check of an upgrade or enable-qualified through its final re-check: no run starts, stops or settles
    and no qualification evidence is written meanwhile."""
    with held(CONTROL,'lifecycle.lock'), held(CONFIG,'qualification.lock'): yield

@contextmanager
def held(directory,name):
    protected(directory)
    parent = os.open(directory,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW|os.O_CLOEXEC)
    try: fd = os.open(name,os.O_RDWR|os.O_CREAT|os.O_NOFOLLOW|os.O_CLOEXEC,0o600,dir_fd=parent)
    finally: os.close(parent)
    try:
        info = os.fstat(fd)
        if (not stat.S_ISREG(info.st_mode) or (info.st_uid,info.st_gid) != LOCK_OWNER or stat.S_IMODE(info.st_mode) != 0o600
                or info.st_nlink != 1 or any('acl' in n.lower() for n in os.listxattr(fd))): raise ValueError('unsafe maintenance lock: '+name)
        deadline = time.monotonic()+MAINTENANCE_TIMEOUT
        while True:
            try:
                fcntl.flock(fd,fcntl.LOCK_EX|fcntl.LOCK_NB)
                break
            except BlockingIOError:
                if time.monotonic() >= deadline: raise TimeoutError('maintenance lock held: '+name+' (a run or qualification is in progress)')
                time.sleep(.05)
        yield
    finally: os.close(fd)

def unit_cgroup(unit):
    match = INSTANCE.fullmatch(unit)
    return '/system.slice/'+(SLICES[match.group(1)]+'/' if match else '')+unit

def settled_unit(unit):
    # SB3E-R21.2: no queued job, not active, and an empty (or absent) cgroup of its own identity, per unit.
    state = {}
    for line in run(['/usr/bin/systemctl','show','--property=Id,Job,ActiveState,ControlGroup',unit]).decode().splitlines():
        key,sep,value = line.partition('=')
        if not sep or key in state: raise ValueError('unit not settled: '+unit)
        state[key] = value
    if (set(state) != {'Id','Job','ActiveState','ControlGroup'} or state['Id'] != unit or state['Job'] != ''
            or state['ActiveState'] not in ('inactive','failed') or state['ControlGroup'] not in ('',unit_cgroup(unit))):
        raise ValueError('unit not settled: '+unit)
    empty_cgroup(CGROUP/unit_cgroup(unit)[1:],unit,bool(state['ControlGroup']))

def empty_cgroup(path,unit,required):
    if not required and not os.path.lexists(path): return
    try: events = (path/'cgroup.events').read_text().splitlines()
    except OSError as exc: raise ValueError('unresolved cgroup evidence: '+unit) from exc
    if 'populated 0' not in events: raise ValueError('cgroup populated: '+unit)

def require_settled():
    run(['/usr/bin/systemctl','stop','mothergate.socket','mothergate.service'])
    listed = run(['/usr/bin/systemctl','list-units','--all','--plain','--no-legend','mothergate-run@*','mothergate-root@*']).decode()
    units = ['mothergate.socket','mothergate.service']
    for line in listed.splitlines():
        fields = line.split()
        if not fields or not INSTANCE.fullmatch(fields[0]): raise ValueError('unexpected run unit: '+line)
        units.append(fields[0])
    for unit in units: settled_unit(unit)
    for path in sorted((CGROUP/'system.slice').glob('mothergate-*')):  # an unlisted instance's cgroup
        if path.name not in units: empty_cgroup(path,path.name,True)
    for name in SLICES.values():  # R22.1: every group in the template slice trees, listed, unlisted or foreign
        tree = CGROUP/'system.slice'/name
        if os.path.lexists(tree):
            for path in sorted(p for p in tree.iterdir() if p.is_dir()): empty_cgroup(path,path.name,True)
        empty_cgroup(tree,name,False)  # populated is recursive: any deeper group

def require_no_custody():
    # Custody and pending-state refusals are preserved: nothing is repaired, moved or removed.
    for path in (STATE/'pending',STATE/'staging',CONTROL/'incoming',CONTROL/'sealed'):
        fd = os.open(path,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW)
        try:
            names = os.listdir(fd)
            if any(path.name != 'sealed' or not USED.fullmatch(n) for n in names):
                raise ValueError('retained custody or pending state blocks upgrade: '+str(path))
            for name in names:  # SB3E-R21.2: lifecycle.marker's exact form: a root 0600 single-link regular file
                try: marker = os.open(name,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC,dir_fd=fd)
                except OSError as exc: raise ValueError('unsafe custody marker: '+name) from exc
                try:
                    info = os.fstat(marker)
                    if (not stat.S_ISREG(info.st_mode) or (info.st_uid,info.st_gid) != CUSTODY_OWNER or stat.S_IMODE(info.st_mode) != 0o600
                            or info.st_nlink != 1 or any('acl' in n.lower() for n in os.listxattr(marker))):
                        raise ValueError('unsafe custody marker: '+name)
                finally: os.close(marker)
        finally: os.close(fd)

def state_ownership():
    owner = pwd.getpwnam('mothergate')
    expected = {str(STATE):(*STATE_ROOT_OWNER,0o755)}
    expected.update({str(STATE/n):(owner.pw_uid,owner.pw_gid,0o755 if n=='receipts' else 0o700) for n in ('staging','pending','receipts')})
    actual = {}
    for name in expected:
        info = os.lstat(name)
        actual[name] = (info.st_uid,info.st_gid,stat.S_IMODE(info.st_mode)) if stat.S_ISDIR(info.st_mode) else None
    if actual != expected: raise ValueError('state ownership changed')
    payload = pwd.getpwnam('gaterun')
    return actual,(owner.pw_uid,owner.pw_gid,payload.pw_uid,payload.pw_gid)

def receipt_inventory(receipts):
    result = {}
    for path in sorted(receipts.iterdir()):
        info = path.lstat()
        if not stat.S_ISDIR(info.st_mode): raise ValueError('unexpected receipt entry: '+path.name)
        files = {}
        for item in sorted(path.iterdir()):
            meta = item.lstat()
            if not stat.S_ISREG(meta.st_mode): raise ValueError('unexpected receipt file: '+str(item))
            files[item.name] = (meta.st_uid,meta.st_gid,stat.S_IMODE(meta.st_mode),hashlib.sha256(item.read_bytes()).hexdigest())
        result[path.name] = (info.st_uid,info.st_gid,stat.S_IMODE(info.st_mode),files)
    return result

def stage_history(identity,inventory,stage,module):
    # The old gate policy, content-addressed by its recorded digest; receipts name it, never relabelled.
    manifest = {p:h for p,h in inventory.items() if p in {str(DEST/n) for n in module.GATE_POLICY}}
    if module.policy_digest(manifest) != identity['gates_sha256']: raise ValueError('old gate policy is not its recorded digest')
    entry = stage/identity['gates_sha256']; entry.mkdir(parents=True,mode=0o755); entry.parent.chmod(0o755)
    write(entry/'manifest.json',(json.dumps(manifest,sort_keys=True)+'\n').encode(),0o444)
    for path,digest in manifest.items():
        data = Path(path).read_bytes()
        if hashlib.sha256(data).hexdigest() != digest: raise ValueError('old gate policy changed')
        write(entry/Path(path).name,data,0o444)
    return entry

def authenticate_retained(module,owner):
    for path in sorted((STATE/'receipts').iterdir()): module.verify_integrity(path,owner)

def reusable(retained,staged):
    """SB3E-R22.2: True for a valid identical retained entry, False when absent. Opened without following links,
    a linked, foreign, writable or differing entry is refused, never followed."""
    unsafe, differs = 'unsafe retained historical policy: '+retained.name, 'retained historical policy differs: '+retained.name
    if os.path.lexists(retained.parent): protected(retained.parent)
    try: fd = os.open(retained,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW)
    except FileNotFoundError: return False
    except OSError as exc: raise ValueError(unsafe) from exc
    try:
        info, owner = os.fstat(fd), os.geteuid()
        if info.st_uid != owner or info.st_mode & 0o022 or any('acl' in n.lower() for n in os.listxattr(fd)): raise ValueError(unsafe)
        names = sorted(os.listdir(fd))
        if names != sorted(p.name for p in staged.iterdir()): raise ValueError(differs)
        for name in names:
            try: member = os.open(name,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK,dir_fd=fd)
            except OSError as exc: raise ValueError(unsafe) from exc
            with os.fdopen(member,'rb') as handle:
                info = os.fstat(member)
                if (not stat.S_ISREG(info.st_mode) or info.st_nlink != 1 or info.st_uid != owner or info.st_mode & 0o222
                        or any('acl' in n.lower() for n in os.listxattr(member))): raise ValueError(unsafe)
                old, new = handle.read(), (staged/name).read_bytes()
            if (json.loads(old) != json.loads(new)) if name == 'manifest.json' else old != new: raise ValueError(differs)
        return True
    finally: os.close(fd)

def registry_row(uid,user,source,observed):
    # SB3E-R26: the boot-stable source identity (filesystem f_fsid + inode); st_dev is not recorded.
    return {'mother':dict(uid=uid,user=user,object_format='sha1',common_directory=str(source),filesystem=observed['filesystem'],
                          inode=observed['inode'],fixture_sha=FIXTURE,qualification_gate_set='full')}

def migrate_registry(snapshot,unique):
    """SB3E-R26: GB1 pinned the source as (st_dev, st_ino), and st_dev follows kernel device naming, which differs
    between boots, so the old device is never compared. The new identity is computed live by the new bundle's
    snapshot code and bound to the installed row by its inode. Preflight only: a refusal changes nothing."""
    old = json.loads((CONFIG/'registry.json').read_bytes(),object_pairs_hook=unique)  # bytes verified by verify_old_install
    row = old.get('mother') if isinstance(old,dict) and set(old) == {'mother'} else None
    fixed = dict(user='nexus',object_format='sha1',common_directory=str(SOURCE_GIT),fixture_sha=FIXTURE,qualification_gate_set='full')
    try:
        if (not isinstance(row,dict) or set(row) not in (set(fixed)|{'uid','inode','device'},set(fixed)|{'uid','inode','filesystem'})
                or any(row[k] != v for k,v in fixed.items()) or any(type(row.get(k,1)) is not int or row.get(k,1) <= 0 for k in ('uid','inode','device'))):
            raise ValueError('unreviewed registry row')
        if 'filesystem' in row: snapshot.registered(row['filesystem'],row['inode'])  # a GB2 row's own identity is well formed
    except ValueError as exc: raise ValueError('installed registry is not a reviewed form') from exc
    live = snapshot.observe(SOURCE_GIT)
    if live['inode'] != row['inode']: raise ValueError('source inode differs from the installed registry')
    if 'filesystem' in row and row['filesystem'] != live['filesystem']: raise ValueError('source filesystem differs from the installed registry')
    return registry_row(row['uid'],row['user'],SOURCE_GIT,live)

def upgrade(args):
    with maintenance(): return upgrade_locked(args)

def upgrade_locked(args):
    # SB3E-R18 P3 / R19.5: a separate reviewed action; fresh installs keep the existing-output refusal.
    if not re.fullmatch('[0-9a-f]{64}',args.old_bundle_sha256 or ''): raise ValueError('old bundle SHA-256 required')
    protected(UPGRADE_BOOT); protected(UPGRADE_BOOT/'bundle.tar')
    files = bundle_files(UPGRADE_BOOT/'bundle.tar',args.bundle_sha256 or '')
    for name in FILES+[WRAPPER]:
        source = UPGRADE_BOOT/'tools/gatebox'/name; protected(source)
        if source.read_bytes() != files['tools/gatebox/'+name]: raise ValueError('bootstrap bytes differ')
    toolchain_check(files)
    identity,inventory = verify_old_install(args.old_bundle_sha256,load_module(DEST/'probe.py','gbx_old_probe').installed_identity)
    kept = set(FILES)|{WRAPPER,'policy/phase2_live_sandbox.rs'}
    if any(p.startswith(str(DEST)+'/') and p[len(str(DEST))+1:] not in kept for p in inventory):
        raise ValueError('old installed file absent from the new bundle')
    require_settled()
    require_no_custody()
    state,accounts = state_ownership()
    plan = plan_accounts(); manager,payload = [entry for entry,create in plan]
    if any(create for entry,create in plan) or (manager.pw_uid,manager.pw_gid,payload.pw_uid,payload.pw_gid) != accounts:
        raise ValueError('gate accounts changed')
    if render((DEST/'units/mothergate-run@.service').read_text(),payload.pw_uid,payload.pw_gid) != RUN_UNIT.read_text():
        raise ValueError('installed run unit is not rendered for the current gaterun account')
    receipts = receipt_inventory(STATE/'receipts')
    new = load_module(UPGRADE_BOOT/'tools/gatebox/receipt.py','gbx_new_receipt')
    registry = migrate_registry(load_module(UPGRADE_BOOT/'tools/gatebox/snapshot.py','gbx_new_snapshot'),new.unique)
    with tempfile.TemporaryDirectory(prefix='.gatebox-',dir=ETC) as temporary:
        stage = Path(temporary)
        entry = stage_history(identity,inventory,stage/'history',new)
        # R21.4: every retained generation, plus the staged old policy (root: main requires the host root namespace).
        new.HISTORY,new.STAGED_HISTORY,new.HISTORY_OWNER = HISTORY,stage/'history',os.geteuid()
        authenticate_retained(new,accounts[0])
        reusable(HISTORY/entry.name,entry)  # R22.2: refused here, before staging and any mutation
        staged_security = stage_security(files,payload.pw_uid,payload.pw_gid,stage)
        # Mutation starts here. Until the new identity is recorded, admission stays bound to the old one.
        HISTORY.mkdir(mode=0o755,exist_ok=True); protected(HISTORY)
        if not reusable(HISTORY/entry.name,entry): os.rename(entry,HISTORY/entry.name)  # content-addressed reuse
        archive = HISTORY/('install-'+identity['bundle_sha256']); archive.mkdir(mode=0o755)
        for name in [n for n in SUPERSEDED if (CONFIG/n).exists()]+sorted(p.name for p in CONFIG.glob('acceptance-*.json')):
            os.rename(CONFIG/name,archive/name)  # kept for audit; never reused under the new identity
        os.rename(CONFIG/'registry.json',archive/'registry.json')  # SB3E-R26: the old pin, kept for audit
        record(CONFIG/'registry.json',registry)
        for name in FILES: replace(DEST/name,files['tools/gatebox/'+name],0o755 if name in EXECUTABLE else 0o644)
        replace(DEST/WRAPPER,files['tools/gatebox/'+WRAPPER],0o755)
        replace(DEST/'policy/phase2_live_sandbox.rs',files[HARNESS])
        outputs = []
        for staged,target in staged_security:
            os.replace(staged,target); outputs.append(target)
        run(['/usr/bin/systemctl','daemon-reload'])
        replace(CONTROL_POLICY,files['tools/gatebox/policy/mothergate-control.conf']); outputs.append(CONTROL_POLICY)
        for name,data in payload_policies(files).items():
            replace(CONFIG/name,data); outputs.append(CONFIG/name)
        record(CONFIG/'bundle.json',{'sha256':args.bundle_sha256}); outputs.append(CONFIG/'bundle.json')
        outputs += [DEST/n for n in FILES]+[DEST/WRAPPER,DEST/'policy/phase2_live_sandbox.rs',CONFIG/'skeleton.json',
                                            CONFIG/'registry.json',CONFIG/'created-accounts.json']
        record(CONFIG/'installed-files.json',{str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in outputs})
    current = load_module(DEST/'probe.py','gbx_new_probe').installed_identity()
    record(CONFIG/'identity.json',current)
    record(CONFIG/'qualification.json',PLACEHOLDER)
    record(CONFIG/'admission.json',{'stage':'qualifying','identity':current})
    if receipt_inventory(STATE/'receipts') != receipts or state_ownership() != (state,accounts):
        raise ValueError('upgrade changed retained receipts or ownership')
    require_no_custody()
    authenticate_retained(load_module(DEST/'receipt.py','gbx_installed_receipt'),accounts[0])
    print('Upgraded in qualifying stage; receipts retained; qualify, then enable-qualified.')

def enable_qualified():
    with maintenance():
        protected(DEST); protected(CONFIG)
        sys.path.insert(0,str(DEST))
        import probe
        import supervisor
        import verify
        current = probe.installed_identity()
        state = supervisor.read_config('admission.json')
        acceptance_check(current,supervisor.read_config('qualification.json'),state,verify.authenticate)
        record(CONFIG/'admission.next',{'stage':'normal','identity':current})
        os.replace(CONFIG/'admission.next',CONFIG/'admission.json')
        fd = os.open(CONFIG,os.O_RDONLY|os.O_DIRECTORY)
        try: os.fsync(fd)
        finally: os.close(fd)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('action',choices=['install','enable-qualified','upgrade'])
    for name in ('bundle-sha256','old-bundle-sha256','repo-id','source-git','submit-user','fixture-sha'):
        parser.add_argument('--'+name)
    args = parser.parse_args()
    if os.geteuid() or Path('/proc/self/uid_map').read_text().split() != ['0','0','4294967295']:
        raise ValueError('host root namespace required')
    if args.action == 'enable-qualified': return enable_qualified()
    if args.action == 'upgrade': return upgrade(args)
    if (args.repo_id,args.source_git,args.submit_user,args.fixture_sha) != (
            'mother',str(SOURCE_GIT),'nexus',FIXTURE):
        raise ValueError('only frozen registration supported')
    protected(BOOT); protected(BOOT/'bundle.tar')
    files = bundle_files(BOOT/'bundle.tar',args.bundle_sha256 or '')
    for name in FILES+[WRAPPER]:
        source = BOOT/'tools/gatebox'/name
        protected(source)
        if source.read_bytes() != files['tools/gatebox/'+name]: raise ValueError('bootstrap bytes differ')
    targets = [DEST,CONFIG,STATE,ROOT,CONTROL_POLICY,Path('/var/lib/gatebox-source'),
               Path('/etc/sudoers.d/gatebox'),Path('/etc/polkit-1/rules.d/00-gatebox-deny.rules')]
    targets += [Path('/etc/systemd/system')/n for n in ('mothergate.socket','mothergate.service','mothergate-run@.service','mothergate-root@.service')]
    for path in targets:
        if path.exists() or path.is_symlink(): raise ValueError('existing output needs review: '+str(path))
        ancestor = path.parent
        while not ancestor.exists(): ancestor = ancestor.parent
        protected(ancestor)
    for executable in ('git','systemctl','systemd-tmpfiles','systemd-analyze','python3','Xvfb','xdotool','dbus-run-session','gdbus','awk','passwd','runuser','visudo','useradd','groupadd','usermod'):
        if not shutil.which(executable,path=ENV['PATH']): raise ValueError('missing offline prerequisite: '+executable)
    if int(run(['/usr/bin/systemctl','--version']).split()[1]) < 259: raise ValueError('systemd 259 required')
    for path,expected in (('/proc/sys/dev/tty/legacy_tiocsti','0'),('/proc/sys/kernel/apparmor_restrict_unprivileged_userns','1')):
        if Path(path).read_text().strip() != expected: raise ValueError('unexpected host policy: '+path)
    source = Path(args.source_git)
    if source.resolve() != source or not source.is_dir(): raise ValueError('canonical Git common directory required')
    submit = pwd.getpwnam(args.submit_user)
    # SB3E-R26: the boot-stable identity, by the verified bootstrap copy of the bundle's snapshot code.
    observed = load_module(BOOT/'tools/gatebox/snapshot.py','gbx_boot_snapshot').observe(source)
    if submit.pw_uid == 0: raise ValueError('non-root submitter required')
    for path in ('/home/nexus/.rustup/toolchains/1.94.0-x86_64-unknown-linux-gnu','/home/nexus/.cargo/registry'):
        if not Path(path).is_dir() or not any(Path(path).iterdir()): raise ValueError('missing offline input: '+path)
    toolchain_check(files)
    protected(Path('/opt/google/chrome/chrome'))
    for name in ('ld.so.cache','localtime','fonts','alternatives','dbus-1'):
        host = Path('/etc')/name
        for path in ([host,*host.rglob('*')] if host.is_dir() else [host]):
            if path.is_file(): path.read_bytes()
            elif not path.is_dir() and not path.is_symlink(): raise ValueError('unsupported host input: '+str(path))
    for name,data in payload_policies(files).items(): json.loads(data)
    for name in FILES:
        if name.endswith('.py'): compile(files['tools/gatebox/'+name],name,'exec')
    preflight_accounts(args.submit_user)
    plan = plan_accounts(); manager,payload = [entry for entry,create in plan]
    protected(Path('/etc'))
    for parent in (Path('/etc/systemd/system'),Path('/etc/sudoers.d'),Path('/etc/polkit-1/rules.d')):
        if parent.stat().st_dev != Path('/etc').stat().st_dev: raise ValueError('cross-filesystem security staging')
    require_control_absent()
    with tempfile.TemporaryDirectory(prefix='.gatebox-',dir='/etc') as temporary:
        staged_security = stage_security(files,payload.pw_uid,payload.pw_gid,Path(temporary))
        # Publish validated units and load them before the fixed stop. On a
        # fresh host, stopping unknown units would fail and prevent bootstrap.
        outputs = publish_units(staged_security[:4])
        prepare_control()
        created = apply_accounts(plan,args.submit_user)
        for path in (DEST,CONFIG,STATE,ROOT,Path('/var/lib/gatebox-source')): path.mkdir(parents=True,mode=0o755)
        record(CONFIG/'created-accounts.json',created)
        for name in FILES:
            target = DEST/name; target.parent.mkdir(parents=True,exist_ok=True,mode=0o755)
            write(target,files['tools/gatebox/'+name],0o755 if name in EXECUTABLE else 0o644)
        write(DEST/WRAPPER,files['tools/gatebox/'+WRAPPER],0o755)
        write(DEST/'policy/phase2_live_sandbox.rs',files[HARNESS])
        for name in ('staging','pending','receipts'):
            path = STATE/name; path.mkdir(mode=0o755 if name == 'receipts' else 0o700)
            os.chown(path,manager.pw_uid,manager.pw_gid)
        for name in ('usr/bin','usr/sbin','usr/lib','usr/lib64','opt/google/chrome','opt/rust','inputs/registry',
                     'gate/bin','gate/policy','input','etc','var','run','work','tmp','dev/shm','proc','sys','root'):
            (ROOT/name).mkdir(parents=True,exist_ok=True,mode=0o755)
        write(ROOT/'gate/request.json',b'',0o444)
        for name,target in {'bin':'usr/bin','sbin':'usr/sbin','lib':'usr/lib','lib64':'usr/lib64','var/run':'/run','var/tmp':'/tmp'}.items():
            (ROOT/name).symlink_to(target)
        for name,text in {'passwd':f'root:x:0:0::/nonexistent:/usr/sbin/nologin\ngaterun:x:{payload.pw_uid}:{payload.pw_gid}::/work/home:/usr/sbin/nologin\n',
                          'group':f'root:x:0:\ngaterun:x:{payload.pw_gid}:\n','nsswitch.conf':'passwd: files\ngroup: files\nhosts: files\n',
                          'hosts':'127.0.0.1 localhost\n::1 localhost\n','resolv.conf':''}.items():
            write(ROOT/'etc'/name,text.encode())
        write(ROOT/'etc/machine-id',b'0123456789abcdef0123456789abcdef\n')
        for name in ('ld.so.cache','localtime','fonts','alternatives','dbus-1'):
            host = Path('/etc')/name
            if host.is_dir(): shutil.copytree(host,ROOT/'etc'/name,symlinks=True)
            else: shutil.copyfile(host,ROOT/'etc'/name)
        # Enumerate every skeleton byte/link. The manifest is checked before qualification.
        inventory = {}
        for path in sorted(ROOT.rglob('*')):
            key = str(path.relative_to(ROOT)); mode = path.lstat().st_mode
            if stat.S_ISLNK(mode): inventory[key] = {'link':os.readlink(path)}
            elif stat.S_ISREG(mode): inventory[key] = {'sha256':hashlib.sha256(path.read_bytes()).hexdigest()}
            elif stat.S_ISDIR(mode): inventory[key] = {'directory':True}
            else: raise ValueError('special skeleton entry: '+key)
        record(CONFIG/'skeleton.json',inventory)
        record(CONFIG/'registry.json',registry_row(submit.pw_uid,args.submit_user,source,observed))
        for staged,target in staged_security[4:]:
            os.rename(staged,target); outputs.append(target)
        write(CONTROL_POLICY,files['tools/gatebox/policy/mothergate-control.conf'])
        outputs.append(CONTROL_POLICY)
        create_control()
        for name,data in payload_policies(files).items():
            write(CONFIG/name,data); outputs.append(CONFIG/name)
        record(CONFIG/'bundle.json',{'sha256':args.bundle_sha256}); outputs.append(CONFIG/'bundle.json')
        outputs += [DEST/n for n in FILES]+[DEST/WRAPPER,DEST/'policy/phase2_live_sandbox.rs',CONFIG/'skeleton.json',CONFIG/'registry.json',CONFIG/'created-accounts.json']
        record(CONFIG/'installed-files.json',{str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in outputs})
        sys.path.insert(0,str(DEST))
        import probe
        current = probe.installed_identity()
        record(CONFIG/'identity.json',current)
        record(CONFIG/'qualification.json',PLACEHOLDER)
        record(CONFIG/'admission.json',{'stage':'qualifying','identity':current})
        print('Installed in qualifying stage; start socket, qualify, then enable-qualified.')


if __name__ == '__main__': main()
PY
