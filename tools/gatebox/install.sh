#!/bin/bash
# Offline bootstrap. Never run from a builder-owned checkout.
set -euo pipefail
umask 022
exec /usr/bin/python3 -I -B - "$@" <<'PY'
import argparse
import hashlib
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

DEST = Path('/usr/libexec/gatebox')
CONFIG = Path('/etc/gatebox')
ROOT = Path('/usr/lib/gatebox/root')
STATE = Path('/var/lib/mothergate')
CONTROL = Path('/run/mothergate-control')
CONTROL_POLICY = Path('/etc/tmpfiles.d/mothergate-control.conf')
BOOT = Path('/root/gatebox-slice1')
FIXTURE = '899e3872c5b61223c87b905f63c66947a816259b'
HARNESS = 'crates/nexus-verifier-sandbox/tests/phase2_live_sandbox.rs'
FILES = ['MISSION.md','snapshot.py','receipt.py','supervisor.py','init.py','client.py',
         'lifecycle.py','verify.py','probe.py','install.sh','README.md','tests/test_service.py',
         'tests/test_unit_core.py','tests/test_unit_contract.py','tests/test_installed.py',
         'tests/test_custody.py','tests/test_install_composition.py',
         'policy/custody.json','policy/mothergate-control.conf',
         'policy/gates.json','policy/exclusions.json','policy/registry.json','policy/sudoers',
         'policy/deny-systemd.rules','units/mothergate.socket','units/mothergate.service',
         'units/mothergate-run@.service']
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
    expected = {'tools/gatebox/'+n for n in FILES}|{HARNESS}
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
    return result

def payload_policies(files):
    return {name:files['tools/gatebox/policy/'+name] for name in ('gates.json','exclusions.json')}

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

def acceptance_check(identity,value,state,authenticate):
    custody = {'C1-SEALED','C2-CANARY','C3-RESTART'}
    rows = {'B1','B2','B3','B4','A1','A5','A6','A8','E2E','CLIENTS'} | custody
    if (value.get('identity') != identity or set(value.get('passed',[])) != rows
            or state != {'identity':identity,'stage':'qualifying'}
            or set(value.get('evidence',{})) != rows):
        raise ValueError('qualification incomplete/stale or provenance missing')
    for row, evidence in value['evidence'].items():
        if row in custody:
            proof = evidence.get('custody')
            if (not isinstance(proof,dict) or proof.get('row') != row or proof.get('passed') is not True
                    or not isinstance(proof.get('observations'),dict) or not proof['observations']):
                raise ValueError('installed custody proof missing/failed: '+row)
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
    for name in ('mothergate.socket','mothergate.service','mothergate-run@.service'):
        path = stage/name
        write(path,render(files['tools/gatebox/units/'+name].decode(),uid,gid).encode())
        outputs.append((path,Path('/etc/systemd/system')/name))
    for name,target,mode in (('sudoers','/etc/sudoers.d/gatebox',0o440),
                             ('deny-systemd.rules','/etc/polkit-1/rules.d/00-gatebox-deny.rules',0o644)):
        path = stage/name; write(path,files['tools/gatebox/policy/'+name],mode)
        outputs.append((path,Path(target)))
    run(['/usr/sbin/visudo','-cf',str(stage/'sudoers')])
    run(['/usr/bin/systemd-analyze','verify',*[str(p) for p,t in outputs[:3]]])
    return outputs

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('action',choices=['install','enable-qualified'])
    for name in ('bundle-sha256','repo-id','source-git','submit-user','fixture-sha'):
        parser.add_argument('--'+name)
    args = parser.parse_args()
    if os.geteuid() or Path('/proc/self/uid_map').read_text().split() != ['0','0','4294967295']:
        raise ValueError('host root namespace required')
    if args.action == 'enable-qualified':
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
        return
    if (args.repo_id,args.source_git,args.submit_user,args.fixture_sha) != (
            'mother','/home/nexus/NEXUS/nexus-os/.git','nexus',FIXTURE):
        raise ValueError('only frozen registration supported')
    protected(BOOT); protected(BOOT/'bundle.tar')
    files = bundle_files(BOOT/'bundle.tar',args.bundle_sha256 or '')
    for name in FILES:
        source = BOOT/'tools/gatebox'/name
        protected(source)
        if source.read_bytes() != files['tools/gatebox/'+name]: raise ValueError('bootstrap bytes differ')
    targets = [DEST,CONFIG,STATE,ROOT,CONTROL_POLICY,Path('/var/lib/gatebox-source'),
               Path('/etc/sudoers.d/gatebox'),Path('/etc/polkit-1/rules.d/00-gatebox-deny.rules')]
    targets += [Path('/etc/systemd/system')/n for n in ('mothergate.socket','mothergate.service','mothergate-run@.service')]
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
    info = source.stat(); submit = pwd.getpwnam(args.submit_user)
    if submit.pw_uid == 0: raise ValueError('non-root submitter required')
    for path in ('/home/nexus/.rustup/toolchains/1.94.0-x86_64-unknown-linux-gnu','/home/nexus/.cargo/registry'):
        if not Path(path).is_dir() or not any(Path(path).iterdir()): raise ValueError('missing offline input: '+path)
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
        outputs = publish_units(staged_security[:3])
        prepare_control()
        created = apply_accounts(plan,args.submit_user)
        for path in (DEST,CONFIG,STATE,ROOT,Path('/var/lib/gatebox-source')): path.mkdir(parents=True,mode=0o755)
        record(CONFIG/'created-accounts.json',created)
        for name in FILES:
            target = DEST/name; target.parent.mkdir(parents=True,exist_ok=True,mode=0o755)
            write(target,files['tools/gatebox/'+name],0o755 if name in ('install.sh','lifecycle.py') else 0o644)
        write(DEST/'policy/phase2_live_sandbox.rs',files[HARNESS])
        for name in ('staging','pending','receipts'):
            path = STATE/name; path.mkdir(mode=0o755 if name == 'receipts' else 0o700)
            os.chown(path,manager.pw_uid,manager.pw_gid)
        for name in ('usr/bin','usr/sbin','usr/lib','usr/lib64','opt/google/chrome','opt/rust','inputs/registry',
                     'gate/bin','gate/policy','input','etc','var','run','work','tmp','dev/shm','proc','sys'):
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
        registration = {'mother':dict(uid=submit.pw_uid,user=args.submit_user,object_format='sha1',common_directory=str(source),
                                      device=info.st_dev,inode=info.st_ino,fixture_sha=FIXTURE,qualification_gate_set='full')}
        record(CONFIG/'registry.json',registration)
        for staged,target in staged_security[3:]:
            os.rename(staged,target); outputs.append(target)
        write(CONTROL_POLICY,files['tools/gatebox/policy/mothergate-control.conf'])
        outputs.append(CONTROL_POLICY)
        create_control()
        for name,data in payload_policies(files).items():
            write(CONFIG/name,data); outputs.append(CONFIG/name)
        record(CONFIG/'bundle.json',{'sha256':args.bundle_sha256}); outputs.append(CONFIG/'bundle.json')
        outputs += [DEST/n for n in FILES]+[DEST/'policy/phase2_live_sandbox.rs',CONFIG/'skeleton.json',CONFIG/'registry.json',CONFIG/'created-accounts.json']
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
