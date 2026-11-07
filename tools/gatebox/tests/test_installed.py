"""Installed oracles; offline discovery never emits a qualification PASS.

Q1 CLIENTS: registered Claude submitter accepts fixed qualification, refuses
another SHA while qualifying. Codex unsupported (runner denies AF_UNIX).
Q2: root drives client.py via runuser, never submits as root.
"""
import ast
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parents[1]
INSTALLED = HERE == Path('/usr/libexec/gatebox')
HARNESS = (HERE/'policy/phase2_live_sandbox.rs' if INSTALLED else
           HERE.parents[1]/'crates/nexus-verifier-sandbox/tests/phase2_live_sandbox.rs')
def load(name):
    spec = importlib.util.spec_from_file_location('install_test_'+name,HERE/(name+'.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

def installer():
    text = (HERE/'install.sh').read_text().split("<<'PY'\n",1)[1].rsplit('\nPY',1)[0]
    module = ast.parse(text)
    module.body = [n for n in module.body if not isinstance(n,ast.If)]
    scope = {'__name__':'oracle'}
    exec(compile(module,'install.sh','exec'),scope)
    return scope

class OfflineInstallTests(unittest.TestCase):
    def test_render_exact_unit_and_reject_substitutions(self):
        render = installer()['render']
        text = (HERE/'units/mothergate-run@.service').read_text()
        self.assertEqual(render(text,1234,5678),text.replace('@GATERUN_UID@','1234').replace('@GATERUN_GID@','5678'))
        for uid,gid in ((0,5),('1;id',5),(True,5),(5,-1)):
            with self.assertRaises(ValueError): render(text,uid,gid)
        with self.assertRaises(ValueError): render(text+'@UNKNOWN@',1234,5678)
    def test_verified_bundle_exact_bytes_and_harness(self):
        import io
        import tarfile
        check = installer()['bundle_files']
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive = root/'bundle.tar'
            harness = 'crates/nexus-verifier-sandbox/tests/phase2_live_sandbox.rs'
            names = installer()['FILES']
            with tarfile.open(archive,'w') as tar:
                for name in names+[harness]:
                    source = HARNESS if name == harness else HERE/name
                    data = source.read_bytes()
                    item = tarfile.TarInfo(name if name == harness else 'tools/gatebox/'+name)
                    item.size = len(data)
                    tar.addfile(item,io.BytesIO(data))
            digest = hashlib.sha256(archive.read_bytes()).hexdigest()
            result = check(archive,digest)
            self.assertEqual(result[harness],HARNESS.read_bytes())
            with self.assertRaises(ValueError): check(archive,'0'*64)
            with tarfile.open(archive,'a') as tar:
                bad = tarfile.TarInfo('tools/gatebox/../escape'); tar.addfile(bad)
            with self.assertRaises(ValueError): check(archive,hashlib.sha256(archive.read_bytes()).hexdigest())
    def test_harness_blob_and_cases(self):
        probe = load('probe')
        policy = json.loads((HERE/'policy/exclusions.json').read_bytes())
        probe.harness_check(HARNESS.read_bytes(),policy)
        for data in (b'',HARNESS.read_bytes()+b'\n'):
            with self.assertRaises(ValueError): probe.harness_check(data,policy)
    def test_parsers_reject_incomplete_evidence(self):
        probe = load('probe')
        line = '1 0 0:8 / /run/gate rw,nosuid - tmpfs tmpfs rw,size=4G\n'
        self.assertEqual(probe.parse_mounts(line)[0]['root'],'/')
        with self.assertRaises(ValueError): probe.parse_mounts('bad')
        self.assertTrue(probe.descendant(13,10,{13:12,12:10}))
        for parents in ({13:2},{13:12,12:13},{}):
            self.assertFalse(probe.descendant(13,10,parents))
        for row in ('A1','A5','A6','A8'):
            with self.assertRaises(ValueError): probe.validate_sensor(row,{})
    def test_q1_clients_requires_receipt_and_refusal(self):
        probe = load('probe')
        sha = '899e3872c5b61223c87b905f63c66947a816259b'
        value = dict(uid=1234,context='claude',accepted={'status':'ACCEPTED','run_id':'a'*32},
                     refused={'status':'UNQUALIFIED','reason':'QUALIFICATION_ONLY'},
                     codex='unsupported (runner denies AF_UNIX)')
        summary = dict(peer_uid=1234,request={'repo_id':'mother','full_sha':sha,'gate_set_id':'full'},
                       run_id='a'*32,installed={'stage':'qualifying'})
        probe.clients_check(value,summary,1234)
        for key in ('uid','context','accepted','refused'):
            bad = dict(value); bad.pop(key)
            with self.assertRaises(ValueError): probe.clients_check(bad,summary,1234)
        for key,val in (('peer_uid',0),('run_id','b'*32),('installed',{'stage':'normal'})):
            with self.assertRaises(ValueError): probe.clients_check(value,dict(summary,**{key:val}),1234)
    def test_placeholder_and_peak_accounting(self):
        self.assertEqual(installer()['PLACEHOLDER'],{'identity':None,'passed':[]})
        probe = load('probe')
        peak = probe.Peak()
        with patch.object(probe,'usage',side_effect=[{'/work':{'bytes_used':9,'inodes_used':5}},
                                                    {'/work':{'bytes_used':2,'inodes_used':7}}]):
            peak.sample(); peak.sample()
        self.assertEqual(peak.peaks,{'/work':{'bytes_used':9,'inodes_used':7}})
    def test_enable_rejects_unbound_or_missing_provenance(self):
        check = installer()['acceptance_check']
        for value in ({},{'identity':{},'passed':['E2E']},{'identity':{},'passed':[]}):
            with self.assertRaises(ValueError): check({},value,{},lambda run: None)

class InstallReviewOracles(unittest.TestCase):
    def test_empty_command_has_an_authenticated_empty_capture(self):
        init = load('init'); events = []
        runner = init.Runner(111,222)
        options = init.child_options
        with tempfile.TemporaryDirectory() as directory, \
                patch.object(init,'child_options',side_effect=lambda fd: dict(options(fd),cwd=directory)), \
                patch.object(init,'packet',side_effect=lambda fd,value: events.append((fd,value))):
            name,code,output = runner.command('format',[os.sys.executable,'-I','-c','pass'])
        self.assertEqual((code,output),(0,''))
        self.assertIn((111,{'id':name,'data':''}),events)
    def test_host_process_identity_ignores_volatile_counters(self):
        identify = load('probe').process_identity
        fields = ['0']*30; fields[19]='1234'
        first = '456 (host canary) '+' '.join(fields)
        fields[10]='9999'
        self.assertEqual(identify(first),identify('456 (host canary) '+' '.join(fields)))
        fields[19]='5678'
        self.assertNotEqual(identify(first),identify('456 (host canary) '+' '.join(fields)))
    def test_payload_policy_outputs_match_readonly_bind(self):
        files = {'tools/gatebox/policy/'+n:(HERE/'policy'/n).read_bytes()
                 for n in ('gates.json','exclusions.json')}
        outputs = installer()['payload_policies'](files)
        self.assertEqual(outputs,{'gates.json':files['tools/gatebox/policy/gates.json'],
                                  'exclusions.json':files['tools/gatebox/policy/exclusions.json']})
    def test_enable_requires_bound_clients_provenance(self):
        identity = {k:'a'*64 for k in ('bundle_sha256','policy_sha256','gates_sha256')}
        summary = dict(status='PASS_SCOPED',request=dict(repo_id='mother',full_sha=load('probe').FIXTURE,gate_set_id='full'),
                       installed=dict(identity,stage='qualifying'),run_id='a'*32,peer_uid=1234)
        provenance = dict(uid=1234,context='claude',ancestor_pid=456,accepted=dict(status='ACCEPTED',run_id='a'*32),
                          refused=dict(status='UNQUALIFIED',reason='QUALIFICATION_ONLY'),codex='unsupported (runner denies AF_UNIX)')
        rows = ['B1','B2','B3','B4','A1','A5','A6','A8','E2E','CLIENTS','C1-SEALED','C2-CANARY','C3-RESTART']
        value = dict(identity=identity,passed=rows,evidence={r:dict(run_id='a'*32,
                     summary_sha256=hashlib.sha256(json.dumps(summary,sort_keys=True).encode()).hexdigest()) for r in rows})
        value['evidence']['CLIENTS']['client_provenance'] = provenance
        for row in ('C1-SEALED','C2-CANARY','C3-RESTART'):
            value['evidence'][row]['custody'] = dict(row=row,passed=True,observations={'real':'decision fixture'})
        check = installer()['acceptance_check']; state = dict(identity=identity,stage='qualifying')
        check(identity,value,state,lambda run: {'summary':summary})
        for bad in ({},dict(provenance,context='host'),dict(provenance,uid=0),dict(provenance,ancestor_pid=0)):
            broken = copy.deepcopy(value); broken['evidence']['CLIENTS']['client_provenance'] = bad
            with self.assertRaises(ValueError): check(identity,broken,state,lambda run: {'summary':summary})

@unittest.skipUnless(INSTALLED,'not installed at /usr/libexec/gatebox; host qualification not exercised')
class InstalledAcceptance(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.probe = load('probe')
        cls.probe.require_installed()
    def check(self,row,line):
        # qualify checks assertions and writes protected identity/run-bound evidence.
        self.probe.qualify(row)
        print('PASS S1-'+row+' '+line,flush=True)
    def test_a1(self): self.check('A1','effective-isolation')
    def test_a5(self): self.check('A5','chrome-sandbox')
    def test_a6(self): self.check('A6','process-device-syscall-policy')
    def test_a8(self): self.check('A8','no-host-network-or-sockets')
    def test_b1(self): self.check('B1','request-and-exact-snapshot')
    def test_b2(self): self.check('B2','fixed-launch-authority')
    def test_b3(self): self.check('B3','protected-receipt')
    def test_b4(self): self.check('B4','complete-evidence-or-failure')
    def test_clients(self): self.check('CLIENTS','claude-qualification')
    def test_c1(self): self.check('C1-SEALED','sealed-DAC')
    def test_c2(self): self.check('C2-CANARY','executor-canary')
    def test_c3(self): self.check('C3-RESTART','pending-restart')
    def test_e2e(self): self.check('E2E','899e3872c5b61223c87b905f63c66947a816259b full PASS_SCOPED')

class Q8F1Oracles(unittest.TestCase):
    def test_canary_open_denial(self):
        import errno
        probe = load('probe')
        for error in (errno.EACCES,errno.EPERM,errno.ENOENT):
            with patch.object(probe.os,'open',side_effect=OSError(error,'denied')) as opened:
                probe.host_pty_denied('/dev/pts/99')
                opened.assert_called_once_with('/dev/pts/99',os.O_RDWR|os.O_NOCTTY)
        with patch.object(probe.os,'open',return_value=987), patch.object(probe.os,'close') as closed:
            with self.assertRaisesRegex(ValueError,'host PTY'): probe.host_pty_denied('/dev/pts/99')
            closed.assert_called_once_with(987)
        with patch.object(probe.os,'open',side_effect=OSError(errno.EIO,'broken')):
            with self.assertRaises(ValueError): probe.host_pty_denied('/dev/pts/99')
        self.assertIn("host_pty_denied(context['pty']['path'])",(HERE/'probe.py').read_text())

class Q8F2Oracles(unittest.TestCase):
    def test_api_mount_skeleton_inventory(self):
        tree = ast.parse((HERE/'install.sh').read_text().split("<<'PY'\n",1)[1].rsplit('\nPY',1)[0])
        loops = [n for n in ast.walk(tree) if isinstance(n,ast.For) and isinstance(n.iter,ast.Tuple)
                 and all(isinstance(e,ast.Constant) and isinstance(e.value,str) for e in n.iter.elts)]
        directories = next(ast.literal_eval(n.iter) for n in loops if 'dev/shm' in ast.literal_eval(n.iter))
        self.assertTrue({'proc','sys'} <= set(directories))
        text = (HERE/'install.sh').read_text()
        self.assertLess(text.index("'proc'"),text.index("record(CONFIG/'skeleton.json',inventory)"))

class Q8F3Oracles(unittest.TestCase):
    def test_private_stage_validates_before_publish(self):
        scope = installer()
        files = {'tools/gatebox/'+n:(HERE/n).read_bytes() for n in scope['FILES']}
        with tempfile.TemporaryDirectory() as tmp, patch.dict(scope,protected=lambda p: None):
            stage = Path(tmp); os.chmod(stage,0o700); calls = []
            def validate(argv):
                paths = argv[3:] if argv[1]=='verify' else argv[2:]
                self.assertTrue(paths)
                for p in paths: self.assertEqual(Path(p).parent,stage); self.assertTrue(Path(p).is_file())
                calls.append(argv)
            with patch.dict(scope,run=validate):
                staged = scope['stage_security'](files,1234,5678,stage)
            self.assertEqual(len(staged),5)
            self.assertEqual({c[0] for c in calls},{'/usr/sbin/visudo','/usr/bin/systemd-analyze'})
            self.assertEqual(os.stat(stage).st_mode & 0o777,0o700)
            for source,target in staged: self.assertEqual(source.parent,stage)
        text = (HERE/'install.sh').read_text()
        self.assertIn('os.rename(staged,target)',text)

class Q8F4Oracles(unittest.TestCase):
    def test_preflight_before_mutation_and_manual_removal(self):
        text = (HERE/'install.sh').read_text()
        self.assertLess(text.index('preflight_accounts(args.submit_user)'),text.index('apply_accounts(' ,text.index('def main():')))
        self.assertLess(text.index('stage_security(files,' ,text.index('def main():')),text.index('apply_accounts(',text.index('def main():')))
        doc = (HERE/'README.md').read_text()
        for fragment in ('Manual removal','systemctl disable --now mothergate.socket',
                         'systemctl stop mothergate.service','gpasswd -d nexus gatesubmit',
                         'userdel mothergate','userdel gaterun','/var/lib/gatebox-source',
                         '/usr/lib/gatebox/root','/etc/polkit-1/rules.d/00-gatebox-deny.rules'):
            self.assertIn(fragment,doc)

class Q8F6Oracles(unittest.TestCase):
    def test_active_non_native_abi_and_sensor_binding(self):
        import signal
        probe = load('probe')
        for status,expected in ((signal.SIGSYS,'SIGSYS'),(0,'EPERM'),(256,None)):
            with patch.object(probe.os,'fork',return_value=777), patch.object(probe.os,'waitpid',return_value=(777,status)):
                if expected: self.assertEqual(probe.non_native_abi(),expected)
                else:
                    with self.assertRaises(ValueError): probe.non_native_abi()
        text = (HERE/'probe.py').read_text()
        self.assertIn('int 0x80',text)
        self.assertIn('non_native_abi()',text[text.index('def syscall_probe'):text.index('def process_identity')])
        value = dict(tiocsti_denied=True,host_hidden=True,reaped=True,syscalls=dict(openat2=True,outer={'p':'EPERM'},inner={'p':'ENOSYS'}))
        with self.assertRaises(ValueError): probe.validate_sensor('A6',value)
        value['syscalls']['non_native_abi']='SIGSYS'; probe.validate_sensor('A6',value)

class Q8F7Oracles(unittest.TestCase):
    def test_clients_heuristic_and_claude_shell_prefix(self):
        doc = (HERE/'README.md').read_text()
        self.assertIn('process-ancestry heuristic',doc)
        self.assertIn('!sudo /usr/bin/python3 -I -B -m unittest discover',doc)
        self.assertIn('not an attestation',doc)

class Q8F8Oracles(unittest.TestCase):
    def test_archive_uses_hashed_buffer(self):
        import io
        scope = installer(); path = unittest.mock.Mock()
        path.stat.return_value.st_size = 0; path.read_bytes.return_value = b'checked bytes'
        class Stop(Exception): pass
        def opened(*args,**kwargs):
            self.assertEqual(args,())
            self.assertIsInstance(kwargs['fileobj'],io.BytesIO)
            self.assertEqual(kwargs['fileobj'].getvalue(),b'checked bytes')
            raise Stop
        with patch.object(scope['tarfile'],'open',side_effect=opened):
            with self.assertRaises(Stop): scope['bundle_files'](path,hashlib.sha256(b'checked bytes').hexdigest())

class Q8F9Oracles(unittest.TestCase):
    def test_reproducible_bundle_and_extraction_contract(self):
        doc = (HERE/'README.md').read_text()
        for fragment in ("tar --exclude='__pycache__'",'--files-from=-','members=members',
                         "for parent in path.parents",'§10','needs `--bundle-sha256'):
            self.assertIn(fragment,doc)

class Q8F10Oracles(unittest.TestCase):
    def test_synthetic_machine_identity(self):
        text = (HERE/'install.sh').read_text()
        self.assertIn("write(ROOT/'etc/machine-id',b'0123456789abcdef0123456789abcdef\\n')",text)
        self.assertNotIn("('ld.so.cache','machine-id'",text)

class Q8F12Oracles(unittest.TestCase):
    def test_account_collisions_and_existing_submit_members(self):
        from types import SimpleNamespace as NS
        scope = installer()
        users = [NS(pw_name=n,pw_uid=u,pw_gid=g,pw_shell='/usr/sbin/nologin') for n,u,g in
                 [('nexus',1000,1000),('mothergate',901,901),('gaterun',902,902)]]
        groups = [NS(gr_name=n,gr_gid=g,gr_mem=m) for n,g,m in
                  [('nexus',1000,[]),('mothergate',901,[]),('gaterun',902,[]),('gatesubmit',903,['nexus'])]]
        def check(): return scope['preflight_accounts']('nexus')
        with patch.object(scope['pwd'],'getpwall',return_value=users), patch.object(scope['grp'],'getgrall',return_value=groups), \
                patch.object(scope['os'],'getgrouplist',side_effect=lambda n,g:[g]), patch.dict(scope,run=lambda argv: b'name L'):
            check()
            for extra in (NS(pw_name='alias',pw_uid=901,pw_gid=999),NS(pw_name='alias',pw_uid=1000,pw_gid=999),
                          NS(pw_name='intruder',pw_uid=1111,pw_gid=903)):
                users.append(extra)
                with self.assertRaises(ValueError): check()
                users.pop()
            groups[-1].gr_mem.append('intruder')
            with self.assertRaises(ValueError): check()

class Q8F13Oracles(unittest.TestCase):
    def test_documented_host_effects_and_prerequisites(self):
        doc = (HERE/'README.md').read_text()
        for fragment in ("bash -euo pipefail <<'OWNER'",'persists at boot','Host changes',
                         '/etc/systemd/system/mothergate-run@.service','/run/mothergate-control',
                         '/var/lib/mothergate','groupdel gatesubmit'):
            self.assertIn(fragment,doc)
        text = (HERE/'install.sh').read_text()
        tree = ast.parse(text.split("<<'PY'\n",1)[1].rsplit('\nPY',1)[0])
        prerequisites = next(ast.literal_eval(n.iter) for n in ast.walk(tree) if isinstance(n,ast.For)
                             and isinstance(n.target,ast.Name) and n.target.id=='executable')
        self.assertIn('xdotool',prerequisites)
