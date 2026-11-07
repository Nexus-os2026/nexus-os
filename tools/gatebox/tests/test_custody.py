"""Q9 Design A oracles, frozen before implementation. No host manager calls.
FIFO fixtures stand in for socket inodes ONLY at the type predicate: real
rename/dirfd/locking/cleanup run offline. AF_UNIX and privilege proof are separate.
"""
import errno
import json
import os
from pathlib import Path
import socket
import stat
import subprocess
import tempfile
import threading
import unittest
from contextlib import ExitStack
from unittest.mock import Mock, patch
from test_service import HERE, RUN, REQUEST, lifecycle, supervisor

OTHER = 'b'*32
LAYOUT = 'd /run/mothergate-control 0755 root root -\nd /run/mothergate-control/incoming 0700 mothergate mothergate -\nd /run/mothergate-control/sealed 0700 root root -\n'
UNITS = {
    'mothergate.service': [
        ['RuntimeDirectory=mothergate-control\nRuntimeDirectoryMode=0700\n',
         'ReadWritePaths=/run/mothergate-control\n']],
    'mothergate-run@.service': [
        ['file:/run/mothergate-control/%i.log.sock','file:/run/mothergate-control/sealed/%i/log.sock'],
        ['file:/run/mothergate-control/%i.status.sock','file:/run/mothergate-control/sealed/%i/status.sock']]}

class CustodyTests(unittest.TestCase):
    def setUp(self):
        self.stack = ExitStack()
        self.addCleanup(self.stack.close)
        # Q11: real protected ancestry, with a private test-owned 0700 base.
        # Never place custody evidence under world-writable /tmp or model it away.
        self.root = Path(self.stack.enter_context(tempfile.TemporaryDirectory(
            prefix='.custody-', dir=HERE/'tests')))
        self.control, self.stage = self.root/'control', self.root/'staging'
        self.control.mkdir(mode=0o755)
        self.stage.mkdir(mode=0o700)
        for name in ('incoming','sealed'): (self.control/name).mkdir(mode=0o700)
        self.state = {'Job':'0','ActiveState':'inactive','ControlGroup':''}
        self.calls = []
        self.runner = Mock(side_effect=self.manager)
        # No real socket operation is hidden: offline positive cases are FIFOs.
        is_socket = stat.S_ISSOCK
        self.stack.enter_context(patch.object(lifecycle.stat,'S_ISSOCK',side_effect=lambda m:is_socket(m) or stat.S_ISFIFO(m)))
        self.bridge = lifecycle.Custody(self.stage,self.control,os.getuid(),os.getuid(),self.runner)
        self.inputs(RUN)
    def manager(self, argv, **options):
        self.calls.append(argv)
        self.assertEqual(options['env'],lifecycle.ENV)
        self.assertTrue(options['close_fds'])
        self.assertLessEqual(options['timeout'],40)
        if argv[1] == 'start':
            for name in ('log','status'):
                self.assertTrue((self.control/'sealed'/RUN/(name+'.sock')).exists())
        return subprocess.CompletedProcess(argv,0,'\n'.join(k+'='+v for k,v in self.state.items()))
    def inputs(self, run):
        stage = self.stage/run
        stage.mkdir(mode=0o700)
        (stage/'snapshot').mkdir(mode=0o555)
        (stage/'request.json').write_text('{}')
        (stage/'request.json').chmod(0o444)
        for kind in ('log','status'):
            os.mkfifo(self.control/'incoming'/(run+'.'+kind+'.sock'),0o700)
    def start(self, run=RUN): self.bridge.run('start',run)
    def stop(self, run=RUN): self.bridge.run('stop',run)
    def sealed(self): return self.control/'sealed'/RUN
    def test_replacement_before_relocation_rejected_without_start(self):
        canary = self.root/'canary'
        canary.write_bytes(b'untouched')
        original = os.rename
        def replace(src, dst, **kwargs):
            if src.endswith('.log.sock'):
                path = self.control/'incoming'/src
                path.unlink()
                path.symlink_to(canary)
            return original(src,dst,**kwargs)
        with patch.object(lifecycle.os,'rename',side_effect=replace):
            with self.assertRaises(ValueError): self.start()
        self.assertFalse(any(c[1]=='start' for c in self.calls))
        self.assertEqual(canary.read_bytes(),b'untouched')
        self.assertTrue((self.sealed()/'log.sock').is_symlink())
    def bad_endpoint(self, variant):
        path = self.control/'incoming'/(RUN+'.status.sock')
        path.unlink()
        if variant=='regular': path.write_text('not socket')
        if variant=='dangling': path.symlink_to(self.root/'absent')
        if variant=='directory': path.mkdir()
        with self.assertRaises(ValueError): self.start()
        self.assertFalse(any(c[1]=='start' for c in self.calls))
    def test_missing_endpoint(self): self.bad_endpoint('missing')
    def test_regular_endpoint(self): self.bad_endpoint('regular')
    def test_dangling_endpoint(self): self.bad_endpoint('dangling')
    def test_directory_endpoint(self): self.bad_endpoint('directory')
    def test_post_relocation_validation_checks_both_endpoints(self):
        path = self.control/'incoming'/(RUN+'.status.sock')
        path.unlink()
        path.write_text('regular')
        with self.assertRaises(ValueError): self.start()
        self.assertFalse(any(c[1]=='start' for c in self.calls))
        self.assertTrue((self.sealed()/'log.sock').exists())
        self.assertTrue((self.sealed()/'status.sock').is_file())
    def test_unsafe_ancestor_and_acl_fail_closed(self):
        self.control.chmod(0o777)
        with self.assertRaises(ValueError): self.start()
        self.assertEqual(self.calls,[])
        self.control.chmod(0o755)
        with patch.object(lifecycle.os,'listxattr',return_value=['system.posix_acl_access']):
            with self.assertRaises(ValueError): self.start()
        self.assertEqual(self.calls,[])
    def test_endpoint_mode_fail_closed(self):
        path = self.control/'incoming'/(RUN+'.log.sock')
        path.chmod(0o666)
        with self.assertRaises(ValueError): self.start()
        self.assertFalse(any(c[1]=='start' for c in self.calls))
    def test_endpoint_owner_fail_closed(self):
        original = os.stat
        def wrong_owner(path, **options):
            value = original(path,**options)
            if path == 'status.sock':
                fields = list(value)
                fields[4] = os.getuid()+1
                return os.stat_result(fields)
            return value
        with patch.object(lifecycle.os,'stat',side_effect=wrong_owner):
            with self.assertRaises(ValueError): self.start()
        self.assertFalse(any(c[1]=='start' for c in self.calls))
    def test_endpoint_acl_fail_closed(self):
        original = os.listxattr
        def acl(path, **options):
            if str(path).endswith('/status.sock'): return ['system.posix_acl_access']
            return original(path,**options)
        with patch.object(lifecycle.os,'listxattr',side_effect=acl):
            with self.assertRaises(ValueError): self.start()
        self.assertFalse(any(c[1]=='start' for c in self.calls))
    def test_duplicate_id_and_normal_completion_cleanup(self):
        self.start()
        self.stop()
        self.assertFalse(self.sealed().exists())
        with self.assertRaises(ValueError): self.start()
        self.assertEqual(sum(c[1]=='start' for c in self.calls),1)
    def test_ambiguous_completion_retains_and_blocks_new_admission(self):
        self.start()
        self.state['Job'] = '73'
        with self.assertRaises(ValueError): self.stop()
        self.assertTrue((self.sealed()/'log.sock').exists())
        self.inputs(OTHER)
        with self.assertRaises(ValueError): self.start(OTHER)
        self.assertEqual(sum(c[1]=='start' for c in self.calls),1)
    def test_active_and_populated_cgroup_never_cleanup(self):
        self.start()
        self.state['ActiveState'] = 'active'
        with self.assertRaises(ValueError): self.stop()
        self.assertTrue(self.sealed().exists())
        self.state.update(ActiveState='inactive',ControlGroup='/other')
        with self.assertRaises(ValueError): self.stop()
        self.assertTrue(self.sealed().exists())
    def test_populated_and_missing_cgroup_evidence_block_cleanup(self):
        self.start()
        cgroups = self.root/'cgroups'
        group = cgroups/'system.slice'/('mothergate-run@'+RUN+'.service')
        cgroups.mkdir(mode=0o700)
        (cgroups/'system.slice').mkdir(mode=0o700)
        group.mkdir(mode=0o700)
        self.state['ControlGroup'] = '/system.slice/mothergate-run@'+RUN+'.service'
        with patch.object(lifecycle,'CGROUP',cgroups):
            for data in ('populated 1\n','invalid\n',None):
                events = group/'cgroup.events'
                if data is not None: events.write_text(data)
                else: events.unlink()
                with self.assertRaises(ValueError): self.stop()
                self.assertTrue(self.sealed().exists())
            events.write_text('populated 0\n')
            self.stop()
        self.assertFalse(self.sealed().exists())
    def test_timeout_and_collector_death_keep_custody_across_restart(self):
        self.runner.side_effect = subprocess.TimeoutExpired('systemctl',40)
        with self.assertRaises(subprocess.TimeoutExpired): self.start()
        self.assertTrue((self.sealed()/'log.sock').exists())
        # A dead collector cannot clean root entries; a new shim sees root state.
        restarted = lifecycle.Custody(self.stage,self.control,os.getuid(),os.getuid(),self.runner)
        self.inputs(OTHER)
        with self.assertRaises(ValueError): restarted.run('start',OTHER)
        self.assertTrue(self.sealed().exists())
    def test_concurrent_stop_cannot_cleanup_during_start(self):
        entered, release = threading.Event(), threading.Event()
        errors = []
        def pending(argv, **options):
            if argv[1]=='start':
                entered.set()
                if not release.wait(2): raise TimeoutError('test synchronization')
            return self.manager(argv,**options)
        self.runner.side_effect = pending
        def start():
            try: self.start()
            except Exception as exc: errors.append(exc)
        thread = threading.Thread(target=start)
        thread.start()
        try:
            self.assertTrue(entered.wait(2))
            with patch.object(lifecycle,'LOCK_TIMEOUT',0):
                with self.assertRaises(TimeoutError): self.stop()
            self.assertTrue((self.sealed()/'log.sock').exists())
        finally:
            release.set()
            thread.join(3)
        self.assertFalse(thread.is_alive())
        self.assertEqual(errors,[])
        self.stop()
        self.assertFalse(self.sealed().exists())
    def test_unexpected_directory_is_never_recursively_deleted(self):
        path = self.control/'incoming'/(RUN+'.log.sock')
        path.unlink()
        path.mkdir()
        (path/'canary').write_text('retain')
        with self.assertRaises(ValueError): self.start()
        with self.assertRaises(ValueError): self.stop()
        self.assertEqual((self.sealed()/'log.sock/canary').read_text(),'retain')
    def test_supervisor_closes_listeners_but_never_unlinks_sealed_entries(self):
        # Exercise collector death, not just the root helper in isolation.
        state = self.root/'state'
        for name in ('staging','pending','receipts'): (state/name).mkdir(parents=True)
        marker = state/'pending'/('.'+RUN)
        marker.touch()
        sockets = [Mock(),Mock()]
        with patch.object(supervisor,'STATE',state), patch.object(supervisor,'CONTROL',self.control), \
                patch.object(supervisor.snapshot,'GitSource'), patch.object(supervisor.snapshot,'export',return_value={}), \
                patch.object(supervisor.socket,'socket',side_effect=sockets), patch.object(supervisor,'shim'), \
                patch.object(supervisor,'collect',side_effect=RuntimeError('collector died')), \
                patch.object(supervisor,'stop_and_confirm',side_effect=TimeoutError('manager pending')), \
                patch.object(supervisor.receipt,'validate_summary'), patch.object(supervisor.receipt,'publish') as publish, \
                patch.object(supervisor.Path,'unlink') as unlink:
            supervisor.execute(RUN,REQUEST,1234,{'device':1,'inode':2},{},marker,'qualifying')
        unlink.assert_not_called()
        self.assertTrue(marker.exists())
        self.assertFalse(publish.call_args.args[3]['cleanup']['complete'])
        for sock in sockets: sock.close.assert_called_once()
    def test_frozen_unit_and_tmpfiles_amendment(self):
        amendment = json.loads((HERE/'policy/custody.json').read_text())
        self.assertEqual(amendment,{'units':UNITS,'tmpfiles':LAYOUT})
        self.assertEqual((HERE/'policy/mothergate-control.conf').read_text(),LAYOUT)
        unit = (HERE/'units/mothergate-run@.service').read_text()
        self.assertIn('StandardOutput=file:/run/mothergate-control/sealed/%i/log.sock',unit)
        self.assertIn('StandardError=file:/run/mothergate-control/sealed/%i/status.sock',unit)
        self.assertNotIn('RuntimeDirectory=',(HERE/'units/mothergate.service').read_text())

class ListenerRenameTests(unittest.TestCase):
    def test_listener_keeps_accepting_after_rename(self):
        try: listener = socket.socket(socket.AF_UNIX,socket.SOCK_STREAM)
        except PermissionError as exc:
            if exc.errno!=errno.EPERM: raise
            self.skipTest('sandbox_blocked: AF_UNIX EPERM; run unchanged outside runner')
        with listener, tempfile.TemporaryDirectory(
                prefix='.listener-', dir=HERE/'tests') as tmp:
            before, after = Path(tmp)/'incoming.sock', Path(tmp)/'sealed.sock'
            try: listener.bind(str(before))
            except PermissionError as exc:
                if exc.errno!=errno.EPERM: raise
                self.skipTest('sandbox_blocked: AF_UNIX bind EPERM; run unchanged outside runner')
            listener.listen(1)
            listener.settimeout(1)
            before.rename(after)
            with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as client:
                client.settimeout(1)
                client.connect(str(after))
                connection, _ = listener.accept()
                with connection:
                    client.sendall(b'renamed')
                    self.assertEqual(connection.recv(7),b'renamed')

class InstalledCustodyTests(unittest.TestCase):
    @unittest.skip('installed_only: root/mothergate identities and real OS DAC required; no host changes authorized')
    def test_mothergate_cannot_unlink_rename_or_symlink_after_relocation(self):
        """Owner: as mothergate try all three operations on both sealed endpoints; expect EACCES."""
    @unittest.skip('installed_only: disposable root-owned canary and systemd executor required; installation HOLD Q8')
    def test_systemd_output_never_writes_disposable_root_canary(self):
        """Owner: pre-handoff replacement rejected; no start and canary bytes unchanged."""
    @unittest.skip('installed_only: actual manager jobs/cgroups, service death/restart and A8 descriptor accounting required')
    def test_pending_manager_job_survives_collector_death_and_supervisor_restart(self):
        """Owner: race start/stop, kill collector, restart; retain custody until exact unit settled."""
