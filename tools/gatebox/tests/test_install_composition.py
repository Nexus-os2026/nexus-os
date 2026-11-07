"""Q12 F-6 oracles, registered before installer composition.

Only extracted Python helpers run. Commands are mocked; temporary inodes are
real. The protected() boundary is mocked explicitly in decision tests, never
in the custody tests or as evidence about host ownership.
"""
import ast
from contextlib import ExitStack
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch
from test_installed import HERE, installer
from test_installed import load
import copy
import hashlib
import json
import inspect
from types import SimpleNamespace

STOP = ['/usr/bin/systemctl','stop','mothergate.socket','mothergate.service']
POLICY = Path('/etc/tmpfiles.d/mothergate-control.conf')
CREATE = ['/usr/bin/systemd-tmpfiles','--create',str(POLICY)]

class InstallerCompositionTests(unittest.TestCase):
    def setUp(self):
        self.scope = installer()
        for name in ('prepare_control','require_control_absent','create_control'):
            self.assertIn(name,self.scope,'F-6 installer lacks '+name)
        self.stack = ExitStack()
        self.addCleanup(self.stack.close)
        self.base = Path(self.stack.enter_context(tempfile.TemporaryDirectory()))
        self.control = self.base/'mothergate-control'
        self.calls = []
        self.protected = Mock()
        self.stack.enter_context(patch.dict(self.scope,CONTROL=self.control,
            CONTROL_POLICY=POLICY,protected=self.protected,run=self.calls.append))
    def provision(self):
        self.scope['prepare_control']()
        self.scope['create_control']()
    def test_stop_and_verified_absence_precede_tmpfiles(self):
        original = os.stat
        def inspected(path, **options):
            if path == self.control.name:
                self.assertEqual(self.calls,[STOP])
                self.assertFalse(options['follow_symlinks'])
                self.assertIn('dir_fd',options)
            return original(path,**options)
        with patch.object(os,'stat',side_effect=inspected) as checked, \
                patch.object(os,'open',wraps=os.open) as opened:
            self.provision()
        self.assertEqual(self.calls,[STOP,CREATE])
        self.assertEqual(sum(c.args==(self.control.name,) for c in checked.call_args_list),2)
        self.assertEqual(self.protected.call_args_list,[unittest.mock.call(self.base)]*2)
        for call in opened.call_args_list:
            self.assertTrue(call.args[1] & os.O_NOFOLLOW)
            self.assertTrue(call.args[1] & os.O_DIRECTORY)
    def test_existing_entries_refuse_reboot_without_deletion(self):
        for variant in ('directory','file','symlink','dangling'):
            with self.subTest(variant=variant):
                self.calls.clear()
                target = self.base/'canary'
                target.mkdir(exist_ok=True)
                (target/'evidence').write_bytes(b'retain custody')
                if variant=='directory':
                    self.control.mkdir()
                    (self.control/'pending').write_bytes(b'retain pending')
                elif variant=='file': self.control.write_bytes(b'unknown owner')
                else: self.control.symlink_to(target if variant=='symlink' else self.base/'absent')
                with patch.object(os,'unlink') as unlink, patch.object(os,'rmdir') as rmdir, \
                        patch.object(self.scope['shutil'],'rmtree') as recursive:
                    with self.assertRaisesRegex(ValueError,'(?i)reboot.*retained custody'):
                        self.provision()
                    unlink.assert_not_called(); rmdir.assert_not_called(); recursive.assert_not_called()
                self.assertEqual(self.calls,[STOP])
                self.assertEqual((target/'evidence').read_bytes(),b'retain custody')
                if variant=='directory':
                    self.assertEqual((self.control/'pending').read_bytes(),b'retain pending')
                    (self.control/'pending').unlink(); self.control.rmdir()
                else: self.control.unlink()
    def test_stop_failure_blocks_verification_and_tmpfiles(self):
        error = subprocess.CalledProcessError(1,STOP)
        with patch.dict(self.scope,run=Mock(side_effect=error)), \
                patch.object(os,'stat',wraps=os.stat) as checked:
            with self.assertRaises(subprocess.CalledProcessError): self.provision()
        checked.assert_not_called()
        self.protected.assert_not_called()
    def test_parent_verification_failure_blocks_tmpfiles(self):
        self.protected.side_effect = ValueError('unprotected installation path')
        with self.assertRaisesRegex(ValueError,'unprotected'): self.provision()
        self.assertEqual(self.calls,[STOP])
    def test_inspection_error_is_not_absence(self):
        with patch.object(os,'stat',side_effect=PermissionError('denied')):
            with self.assertRaises(PermissionError): self.provision()
        self.assertEqual(self.calls,[STOP])
    def test_reappearance_before_create_refuses(self):
        self.scope['prepare_control']()
        self.control.mkdir()
        with self.assertRaisesRegex(ValueError,'(?i)reboot.*retained custody'):
            self.scope['create_control']()
        self.assertEqual(self.calls,[STOP])
    def test_parent_symlink_is_not_followed(self):
        alias = self.base/'alias'
        alias.symlink_to(self.base,target_is_directory=True)
        with patch.dict(self.scope,CONTROL=alias/'mothergate-control'):
            with self.assertRaises(OSError): self.provision()
        self.assertEqual(self.calls,[STOP])

class InstallerWiringTests(unittest.TestCase):
    def test_custody_bundle_and_fixed_tmpfiles_target(self):
        scope = installer()
        self.assertTrue({'policy/custody.json','policy/mothergate-control.conf',
                         'tests/test_custody.py','tests/test_install_composition.py'} <= set(scope['FILES']))
        self.assertEqual(scope.get('CONTROL'),Path('/run/mothergate-control'))
        self.assertEqual(scope.get('CONTROL_POLICY'),POLICY)
    def test_preparation_before_mutations_and_creation_after_policy(self):
        body = (HERE/'install.sh').read_text().split("<<'PY'\n",1)[1].rsplit('\nPY',1)[0]
        tree = ast.parse(body)
        main = next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='main')
        calls = [n for n in ast.walk(main) if isinstance(n,ast.Call)]
        def line(name):
            found = [n.lineno for n in calls if isinstance(n.func,ast.Name) and n.func.id==name]
            self.assertEqual(len(found),1,name+' must be wired exactly once')
            return found[0]
        self.assertLess(line('stage_security'),line('prepare_control'))
        self.assertLess(line('prepare_control'),line('apply_accounts'))
        self.assertLess(line('prepare_control'),line('create_control'))
        policy_writes = [n for n in calls if isinstance(n.func,ast.Name) and n.func.id=='write'
                         and ast.unparse(n.args[0])=='CONTROL_POLICY']
        self.assertEqual(len(policy_writes),1)
        self.assertLess(policy_writes[0].lineno,line('create_control'))
        self.assertIn('systemd-tmpfiles',next(ast.literal_eval(n.iter) for n in ast.walk(main)
                      if isinstance(n,ast.For) and isinstance(n.target,ast.Name) and n.target.id=='executable'))
        # The target is preflighted and included in the authenticated installed inventory.
        self.assertIn('CONTROL_POLICY',next(ast.unparse(n.value) for n in ast.walk(main)
                      if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='targets' for t in n.targets)))
        self.assertTrue(any(isinstance(n,ast.Call) and isinstance(n.func,ast.Attribute)
                        and ast.unparse(n.func)=='outputs.append' and ast.unparse(n.args[0])=='CONTROL_POLICY'
                        for n in ast.walk(main)))

class Q13Oracles(unittest.TestCase):
    """Frozen Q13 offline decisions; never execute a host command."""
    custody = {'C1-SEALED','C2-CANARY','C3-RESTART'}
    rows = {'B1','B2','B3','B4','A1','A5','A6','A8','E2E','CLIENTS'} | custody

    def evidence(self):
        identity = {'bundle_sha256':'a'*64,'policy_sha256':'b'*64,'gates_sha256':'c'*64}
        summary = dict(status='PASS_SCOPED',request=dict(repo_id='mother',
            full_sha=load('probe').FIXTURE,gate_set_id='full'),installed=dict(identity,stage='qualifying'),
            run_id='a'*32,peer_uid=1234)
        value = dict(identity=identity,passed=sorted(self.rows),evidence={row:dict(run_id='a'*32,
            summary_sha256=hashlib.sha256(json.dumps(summary,sort_keys=True).encode()).hexdigest()) for row in self.rows})
        value['evidence']['CLIENTS']['client_provenance'] = dict(uid=1234,context='claude',ancestor_pid=456,
            accepted=dict(status='ACCEPTED',run_id='a'*32),refused=dict(status='UNQUALIFIED',reason='QUALIFICATION_ONLY'),
            codex='unsupported (runner denies AF_UNIX)')
        for row in self.custody:
            value['evidence'][row]['custody'] = dict(row=row,passed=True,observations={'real':'offline decision fixture'})
        return identity,summary,value

    def test_acceptance_requires_each_custody_row_and_passing_evidence(self):
        identity,summary,value = self.evidence()
        check = installer()['acceptance_check']
        def accept(v): check(identity,v,dict(identity=identity,stage='qualifying'),lambda r: {'summary':summary})
        accept(value)
        for row in self.custody:
            for variant in ('missing','no-proof','failed','empty','wrong-row'):
                with self.subTest(row=row,variant=variant):
                    bad = copy.deepcopy(value)
                    if variant=='missing':
                        bad['passed'].remove(row); del bad['evidence'][row]
                    elif variant=='no-proof': del bad['evidence'][row]['custody']
                    elif variant=='failed': bad['evidence'][row]['custody']['passed'] = False
                    elif variant=='empty': bad['evidence'][row]['custody']['observations'] = {}
                    else: bad['evidence'][row]['custody']['row'] = 'C0'
                    with self.assertRaises(ValueError): accept(bad)
        legacy = copy.deepcopy(value)
        for row in self.custody:
            legacy['passed'].remove(row); del legacy['evidence'][row]
        with self.assertRaises(ValueError): accept(legacy)

    def test_probe_requires_custody_rows_and_refuses_unrun_proofs(self):
        probe = load('probe')
        self.assertEqual(set(getattr(probe,'REQUIRED_ROWS',())),self.rows)
        self.assertTrue(callable(getattr(probe,'custody_oracle',None)))
        identity,summary,_ = self.evidence()
        with tempfile.TemporaryDirectory() as tmp, ExitStack() as stack:
            stack.enter_context(patch.object(probe,'CONFIG',Path(tmp)))
            stack.enter_context(patch.object(probe,'require_installed'))
            stack.enter_context(patch.object(probe,'fixture',return_value=(summary,{})))
            stack.enter_context(patch.object(probe,'installed_identity',return_value=identity))
            stack.enter_context(patch.object(probe,'protected'))
            stack.enter_context(patch.object(probe,'config',side_effect=lambda n: identity if n=='identity.json' else
                dict(identity=identity,passed=[],evidence={})))
            write = stack.enter_context(patch.object(probe,'atomic_config'))
            for row in self.custody:
                result = dict(row=row,passed=True,observations={'real':'offline decision fixture'})
                with patch.object(probe,'custody_oracle',return_value=result) as run:
                    probe.qualify(row)
                    run.assert_called_once_with(row)
                    self.assertEqual(write.call_args.args[1]['evidence'][row]['custody'],result)
                write.reset_mock()
                with patch.object(probe,'custody_oracle',side_effect=ValueError('unavailable')):
                    with self.assertRaises(ValueError): probe.qualify(row)
                write.assert_not_called()
                for bad in (None,{},dict(result,passed=False),dict(result,observations={})):
                    with patch.object(probe,'custody_oracle',return_value=bad):
                        with self.assertRaises(ValueError): probe.qualify(row)
                    write.assert_not_called()

    def test_custody_runner_rejects_skips_and_failures(self):
        probe = load('probe')
        self.assertTrue(callable(getattr(probe,'custody_oracle',None)))
        # Inject an unavailable installed test through the runner's suite boundary.
        class Unavailable(unittest.TestCase):
            def test_unavailable(self): self.skipTest('not installed')
        for row in self.custody:
            with patch.object(unittest.defaultTestLoader,'loadTestsFromName',
                    return_value=unittest.defaultTestLoader.loadTestsFromTestCase(Unavailable)):
                with self.assertRaises(ValueError): probe.custody_oracle(row)

    def test_units_publish_only_after_readonly_absence_precheck(self):
        scope = installer()
        self.assertIn('publish_units',scope)
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); control = root/'control'; control.mkdir()
            pairs = []
            for i in range(3):
                source = root/('stage'+str(i)); source.write_bytes(b'unit')
                pairs.append((source,root/('unit'+str(i))))
            with patch.dict(scope,CONTROL=control,protected=Mock(),run=Mock()):
                with self.assertRaisesRegex(ValueError,'reboot'): scope['publish_units'](pairs)
                scope['run'].assert_not_called()
                self.assertTrue(all(s.exists() and not t.exists() for s,t in pairs))
                control.rmdir()
                self.assertEqual(scope['publish_units'](pairs),[t for s,t in pairs])
                scope['run'].assert_called_once_with(['/usr/bin/systemctl','daemon-reload'])
        tree = ast.parse(
            (HERE/'install.sh').read_text().split("<<'PY'\n",1)[1].rsplit('\nPY',1)[0])
        main = next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='main')
        calls = [ast.unparse(n.func) for n in ast.walk(main) if isinstance(n,ast.Call)]
        self.assertEqual(calls.count('publish_units'),1)
        # The remaining rename in main is only for staged sudoers/polkit.
        loops = [n for n in ast.walk(main) if isinstance(n,ast.For) and
                 any(isinstance(c,ast.Call) and ast.unparse(c.func)=='os.rename' for c in ast.walk(n))]
        self.assertEqual([ast.unparse(n.iter) for n in loops],['staged_security[3:]'])

    def test_offline_tempdirs_outside_installed_tree(self):
        for name in ('test_service.py','test_unit_core.py','test_custody.py'):
            tree = ast.parse((HERE/'tests'/name).read_text())
            for call in (n for n in ast.walk(tree) if isinstance(n,ast.Call) and
                         ast.unparse(n.func)=='tempfile.TemporaryDirectory'):
                for kw in call.keywords:
                    if kw.arg=='dir': self.assertNotIn('HERE',ast.unparse(kw.value),name)

    def test_created_accounts_exact_for_new_mixed_and_existing(self):
        scope = installer()
        entries = [SimpleNamespace(pw_name=n,pw_uid=i,pw_gid=i) for n,i in (('mothergate',991),('gaterun',992))]
        for flags,submit_exists in (((False,False),True),((True,False),True),((True,True),False)):
            calls = []
            with patch.dict(scope,run=calls.append), patch.object(scope['grp'],'getgrnam',
                    side_effect=None if submit_exists else KeyError('absent')):
                result = scope['apply_accounts'](list(zip(entries,flags)),'nexus')
            users = [e.pw_name for e,c in zip(entries,flags) if c]
            self.assertEqual(result,dict(users=users,groups=users+([] if submit_exists else ['gatesubmit'])))
        source = (HERE/'install.sh').read_text()
        self.assertIn("record(CONFIG/'created-accounts.json',created)",source)
        self.assertIn("CONFIG/'created-accounts.json'",source[source.index('outputs +='):])

    def test_polkit_hash_required_and_claims_narrowed(self):
        probe = load('probe'); rule = '/etc/polkit-1/rules.d/00-gatebox-deny.rules'
        self.assertTrue(callable(getattr(probe,'polkit_check',None)))
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)/'rule'; path.write_bytes(b'rule')
            with patch.object(probe,'POLKIT_RULE',path), patch.object(probe,'protected'), \
                    patch.object(probe,'config',return_value={str(path):hashlib.sha256(b'rule').hexdigest()}):
                probe.polkit_check(); path.write_bytes(b'changed')
                with self.assertRaises(ValueError): probe.polkit_check()
            with patch.object(probe,'POLKIT_RULE',path), patch.object(probe,'protected'), \
                    patch.object(probe,'config',return_value={}):
                with self.assertRaises(ValueError): probe.polkit_check()
        self.assertIn('polkit_check()',inspect.getsource(probe.authority))
        for name in ('README.md','MISSION.md'):
            source = (HERE/name).read_text()
            self.assertNotIn('including conflicting earlier policy',source)
            self.assertIn('B2 proves denial',source)
