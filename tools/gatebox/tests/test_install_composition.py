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
