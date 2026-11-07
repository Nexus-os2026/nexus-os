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
import hashlib
import select
import time
import uuid
import pwd
import unittest
from contextlib import ExitStack, nullcontext
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
        # The base comes from TMPDIR, which must have protected ancestry
        # (no group/world-writable ancestor); the repo checkout and /tmp do not.
        base = Path(tempfile.gettempdir()).resolve()
        for part in [base, *base.parents]:
            info = part.stat()
            if info.st_mode & 0o022 or info.st_uid not in (0, os.getuid()):
                self.fail('custody fixture requires TMPDIR with protected ancestry; '
                          'unsafe ancestor: ' + str(part))
        self.root = Path(self.stack.enter_context(tempfile.TemporaryDirectory(
            prefix='custody-', dir=base)))
        self.control, self.stage = self.root/'control', self.root/'staging'
        self.control.mkdir(mode=0o755)
        self.stage.mkdir(mode=0o700)
        for name in ('incoming','sealed'): (self.control/name).mkdir(mode=0o700)
        self.state = {'Job':'','ActiveState':'inactive','ControlGroup':''}
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
            self.assertEqual(sum(c[1]=='stop' for c in self.calls),1)
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
                patch.object(supervisor.Path,'unlink') as unlink, \
                patch.object(supervisor.os,'unlink') as os_unlink:
            supervisor.execute(RUN,REQUEST,1234,{'device':1,'inode':2},{},marker,'qualifying')
        unlink.assert_not_called()
        os_unlink.assert_not_called()
        self.assertTrue(marker.exists())
        self.assertFalse(publish.call_args.args[3]['cleanup']['complete'])
        for sock in sockets: sock.close.assert_called_once()
    def test_nonempty_job_including_zero_keeps_custody(self):
        self.start()
        for job in ('0','73','0 ','unknown'):
            with self.subTest(job=job):
                self.state['Job'] = job
                with self.assertRaisesRegex(ValueError,'manager not settled'): self.stop()
                self.assertEqual(set(p.name for p in self.sealed().iterdir()),{'log.sock','status.sock'})
                self.assertTrue((self.control/'sealed'/('.pending-'+RUN)).exists())
    def test_endpoint_hardlink_fail_closed(self):
        path = self.control/'incoming'/(RUN+'.status.sock')
        os.link(path,self.root/'extra-link')
        with self.assertRaisesRegex(ValueError,'unsafe sealed endpoint'): self.start()
        self.assertFalse(any(c[1]=='start' for c in self.calls))
    def test_endpoint_gid_fail_closed(self):
        original = os.stat
        def wrong_gid(path, **options):
            value = original(path,**options)
            if path == 'status.sock':
                fields = list(value)
                fields[5] = self.bridge.owner_gid+1
                return os.stat_result(fields)
            return value
        with patch.object(lifecycle.os,'stat',side_effect=wrong_gid):
            with self.assertRaisesRegex(ValueError,'unsafe sealed endpoint'): self.start()
        self.assertFalse(any(c[1]=='start' for c in self.calls))
    def directory_metadata(self, target, field, value):
        original = os.fstat
        identity = target.stat()
        def swapped(fd):
            info = original(fd)
            if (info.st_dev,info.st_ino) == (identity.st_dev,identity.st_ino):
                fields = list(info)
                fields[field] = value
                return os.stat_result(fields)
            return info
        return patch.object(lifecycle.os,'fstat',side_effect=swapped)
    def test_directory_ancestor_owner_swap(self):
        with self.directory_metadata(self.control,4,self.bridge.root_owner+1):
            with self.assertRaisesRegex(ValueError,'unsafe protected ancestry'):
                with self.bridge.directory(self.control/'sealed',mode=0o700): pass
    def test_directory_leaf_root_swap(self):
        # UID 0 is allowed by the ancestry predicate, but cannot replace mothergate.
        with self.directory_metadata(self.control/'incoming',4,0):
            with self.assertRaisesRegex(ValueError,'unsafe custody directory'):
                with self.bridge.directory(self.control/'incoming',self.bridge.owner,0o700): pass
    def test_directory_leaf_mothergate_cannot_replace_root(self):
        # Reverse of the incoming/root swap: isolate the UID check, retaining
        # the valid root GID/mode so a swapped UID rule cannot hide behind GID.
        bridge = lifecycle.Custody(self.stage,self.control,owner=os.getuid()+1,
                                   root_owner=os.getuid(),runner=self.runner)
        with self.directory_metadata(self.control/'sealed',4,bridge.owner):
            with self.assertRaises(ValueError):
                with bridge.directory(self.control/'sealed',mode=0o700): pass
        self.assertEqual(self.calls,[])
    def test_directory_gid_fail_closed(self):
        with self.directory_metadata(self.control/'incoming',5,self.bridge.owner_gid+1):
            with self.assertRaisesRegex(ValueError,'unsafe custody directory'):
                with self.bridge.directory(self.control/'incoming',self.bridge.owner,0o700): pass
    def test_lock_mode_fail_closed(self):
        lock = self.control/'lifecycle.lock'
        lock.touch(mode=0o600)
        lock.chmod(0o644)
        with self.assertRaisesRegex(ValueError,'unsafe root custody state'): self.start()
        self.assertEqual(self.calls,[])
    def test_lock_symlink_fail_closed(self):
        target = self.root/'lock-canary'
        target.write_bytes(b'retain')
        target.chmod(0o600)
        (self.control/'lifecycle.lock').symlink_to(target)
        with self.assertRaises(OSError): self.start()
        self.assertEqual(target.read_bytes(),b'retain')
        self.assertEqual(self.calls,[])
    def test_lock_owner_fail_closed(self):
        lock = self.control/'lifecycle.lock'
        lock.touch(mode=0o600)
        with self.directory_metadata(lock,4,self.bridge.root_owner+1):
            with self.assertRaisesRegex(ValueError,'unsafe root custody state'): self.start()
        self.assertEqual(self.calls,[])
    def test_lock_gid_fail_closed(self):
        lock = self.control/'lifecycle.lock'
        lock.touch(mode=0o600)
        with self.directory_metadata(lock,5,self.bridge.root_owner+1):
            with self.assertRaisesRegex(ValueError,'unsafe root custody state'): self.start()
        self.assertEqual(self.calls,[])
    def test_lock_hardlink_fail_closed(self):
        lock = self.control/'lifecycle.lock'
        lock.touch(mode=0o600)
        os.link(lock,self.root/'extra-lock')
        with self.assertRaisesRegex(ValueError,'unsafe root custody state'): self.start()
        self.assertEqual(self.calls,[])
    def test_lock_type_fail_closed(self):
        os.mkfifo(self.control/'lifecycle.lock',0o600)
        with self.assertRaisesRegex(ValueError,'unsafe root custody state'): self.start()
        self.assertEqual(self.calls,[])
    def test_missing_run_with_pending_retains_marker(self):
        pending = self.control/'sealed'/('.pending-'+RUN)
        pending.touch(mode=0o600)
        with self.assertRaisesRegex(ValueError,'custody disappeared'): self.stop()
        self.assertTrue(pending.exists())
        self.assertEqual([c[1] for c in self.calls],['stop','show'])
    def test_frozen_unit_and_tmpfiles_amendment(self):
        amendment = json.loads((HERE/'policy/custody.json').read_text())
        self.assertEqual(amendment,{'units':UNITS,'tmpfiles':LAYOUT})
        self.assertEqual((HERE/'policy/mothergate-control.conf').read_text(),LAYOUT)
        unit = (HERE/'units/mothergate-run@.service').read_text()
        self.assertIn('StandardOutput=file:/run/mothergate-control/sealed/%i/log.sock',unit)
        self.assertIn('StandardError=file:/run/mothergate-control/sealed/%i/status.sock',unit)
        self.assertNotIn('RuntimeDirectory=',(HERE/'units/mothergate.service').read_text())

class Q12DecisionTests(unittest.TestCase):
    """Decision/ordering oracles; no custody ownership or real manager claims."""
    def test_only_empty_job_is_settled(self):
        runner = Mock(return_value=subprocess.CompletedProcess([],0,'Job=\nActiveState=inactive\nControlGroup=\n'))
        bridge = lifecycle.Custody(owner=os.getuid(),runner=runner)
        self.assertEqual(bridge.settled(RUN)['Job'],'')
        for job in ('0','73','0 ','unknown'):
            runner.return_value.stdout = 'Job='+job+'\nActiveState=inactive\nControlGroup=\n'
            with self.subTest(job=job), self.assertRaisesRegex(ValueError,'manager not settled'):
                bridge.settled(RUN)
    def test_stop_precedes_layout_validation_failure(self):
        self.stop_before_failure('directory',ValueError('unsafe layout'))
    def test_stop_precedes_lock_failure(self):
        self.stop_before_failure('locked',TimeoutError('lock deadline'))
    def stop_before_failure(self, method, error):
        runner = Mock(return_value=subprocess.CompletedProcess([],0,''))
        bridge = lifecycle.Custody(owner=os.getuid(),runner=runner)
        def reject(*args, **kwargs):
            runner.assert_called_once()
            self.assertEqual(runner.call_args.args[0],lifecycle.command(['stop',RUN]))
            raise error
        with patch.object(bridge,'directory',side_effect=lambda *a,**k:nullcontext(7)), \
                patch.object(bridge,method,side_effect=reject), \
                patch.object(bridge,'settled') as settled, patch.object(bridge,'cleanup') as cleanup:
            with self.assertRaises(type(error)): bridge.run('stop',RUN)
        settled.assert_not_called()
        cleanup.assert_not_called()

class Q12SupervisorTests(unittest.TestCase):
    def test_start_failure_unlinks_own_incoming_leaves(self):
        self.failed_start()
    def test_partial_bind_failure_preserves_unowned_incoming_leaf(self):
        self.failed_start(partial=True)
    def failed_start(self, partial=False):
        with tempfile.TemporaryDirectory() as tmp, ExitStack() as stack:
            root = Path(tmp)
            for name in ('staging','pending','receipts','control/incoming','control/sealed'):
                (root/name).mkdir(parents=True)
            incoming = root/'control/incoming'
            sealed = root/'control/sealed'
            canary = sealed/'canary'
            canary.write_bytes(b'retain sealed')
            marker = root/'pending'/('.'+RUN)
            marker.touch()
            unowned = incoming/(RUN+'.status.sock')
            if partial: unowned.write_bytes(b'not ours')
            sockets = [Mock(),Mock()]
            def bind(path):
                # Real leaf creation; never replace an already-existing leaf.
                with open(path,'xb') as stream: stream.write(b'owned')
            for sock in sockets: sock.bind.side_effect = bind
            for obj,name,value in [(supervisor,'STATE',root),(supervisor,'CONTROL',root/'control'),
                    (supervisor.snapshot,'GitSource',Mock()),(supervisor.snapshot,'export',Mock(return_value={})),
                    (supervisor.socket,'socket',Mock(side_effect=sockets)),
                    (supervisor,'shim',Mock(side_effect=ValueError('start refused before relocation'))),
                    (supervisor,'collect',Mock()),
                    (supervisor,'stop_and_confirm',Mock(side_effect=ValueError('custody uncertain'))),
                    (supervisor.receipt,'validate_summary',Mock()),(supervisor.receipt,'publish',Mock())]:
                stack.enter_context(patch.object(obj,name,value))
            supervisor.execute(RUN,REQUEST,1234,{'device':1,'inode':2},{},marker,'qualifying')
            self.assertFalse((incoming/(RUN+'.log.sock')).exists())
            if partial: self.assertEqual(unowned.read_bytes(),b'not ours')
            else: self.assertFalse(unowned.exists())
            self.assertEqual(canary.read_bytes(),b'retain sealed')
            self.assertTrue(marker.exists())
            supervisor.collect.assert_not_called()
            for sock in sockets: sock.close.assert_called_once()

class ListenerRenameTests(unittest.TestCase):
    def test_listener_keeps_accepting_after_rename(self):
        try: listener = socket.socket(socket.AF_UNIX,socket.SOCK_STREAM)
        except PermissionError as exc:
            if exc.errno!=errno.EPERM: raise
            self.skipTest('sandbox_blocked: AF_UNIX EPERM; run unchanged outside runner')
        with listener, tempfile.TemporaryDirectory(
                prefix='.listener-') as tmp:
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

@unittest.skipUnless(HERE == Path('/usr/libexec/gatebox') and os.geteuid() == 0,
                     'installed_only: host root at /usr/libexec/gatebox required')
class InstalledCustodyTests(unittest.TestCase):
    """Real DAC, systemd executor, queued job and restart proofs. No mocks.

    An exact-instance runtime drop-in ADDS a dependency on a bounded FIFO
    barrier. It changes no payload restriction or output path. An empty snapshot
    prevents candidate execution; init can run installed sensors before failing.
    The disposable collector runs the installed supervisor.collect function.
    """
    def command(self, argv, check=True):
        return subprocess.run(argv,env=lifecycle.ENV,stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,stderr=subprocess.PIPE,encoding='utf-8',
            close_fds=True,timeout=35,check=check)

    def manager(self, *args):
        return self.command(['/usr/bin/systemctl',*args])

    def wait_for(self, predicate, label, seconds=10):
        deadline = time.monotonic()+seconds
        while time.monotonic()<deadline:
            value = predicate()
            if value: return value
            time.sleep(.05)
        self.fail('deadline: '+label)

    def state(self, unit):
        result = self.manager('show',unit,'--property=Job,ActiveState,MainPID,ControlGroup')
        return dict(line.split('=',1) for line in result.stdout.splitlines())

    def line(self):
        ready,_,_ = select.select([self.collector.stdout],[],[],10)
        self.assertTrue(ready,'collector observation deadline')
        line = self.collector.stdout.readline()
        self.assertTrue(line,'collector exited before observation')
        return json.loads(line)

    def setUp(self):
        from test_installed import load
        self.probe = load('probe'); self.probe.require_installed()
        self.control = Path('/run/mothergate-control')
        self.pending = Path('/var/lib/mothergate/pending')
        self.assertFalse(list(self.pending.iterdir()),'other pending work; refuse destructive proof')
        self.assertFalse(any(p.name.startswith('.pending-') for p in (self.control/'sealed').iterdir()))
        self.owner = pwd.getpwnam('mothergate')
        self.run = uuid.uuid4().hex
        self.unit = 'mothergate-run@'+self.run+'.service'
        self.blocker = 'gatebox-custody-'+self.run+'.service'
        self.root = Path('/run')/('gatebox-custody-'+self.run)
        self.root.mkdir(mode=0o700)
        self.probe.protected(self.root)
        self.fifo = self.root/'release'; os.mkfifo(self.fifo,0o600)
        self.stage = Path('/var/lib/mothergate/staging')/self.run
        self.marker = self.pending/('.'+self.run)
        # Never perform root writes/chown beneath supervisor-owned parents.
        script = r"""
import json,sys
from pathlib import Path
run=sys.argv[1]; stage=Path('/var/lib/mothergate/staging')/run
stage.mkdir(mode=0o700); (stage/'snapshot').mkdir(mode=0o555)
with (stage/'request.json').open('xb') as f: f.write(b'{}')
(stage/'request.json').chmod(0o444)
with (Path('/var/lib/mothergate/pending')/('.'+run)).open('xb') as f:
    f.write(json.dumps(dict(run_id=run,request=json.loads(sys.argv[2]))).encode())
"""
        subprocess.run(['/usr/bin/python3','-I','-B','-c',script,self.run,json.dumps(REQUEST)],
            user=self.owner.pw_uid,group=self.owner.pw_gid,extra_groups=[],umask=0o077,
            env=lifecycle.ENV,stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,close_fds=True,timeout=5,check=True)
        self.sealed = self.control/'sealed'/self.run
        self.bridge = lifecycle.Custody(owner=self.owner.pw_uid,owner_gid=self.owner.pw_gid)
        self.files = {}
        self.collector = self.launch = None
        # Cleanup is bounded, exact-name only, and requires the production stop
        # oracle. Unsettled or damaged custody is retained for Owner review.
        self.addCleanup(self.cleanup)
        self.probe.protected(Path('/run/systemd/system'))
        directory = Path('/run/systemd/system')/(self.unit+'.d')
        directory.mkdir(mode=0o755); self.dropin = directory
        self.install_fixture(directory/'custody.conf',
            '[Unit]\nRequires='+self.blocker+'\nAfter='+self.blocker+'\n')
        self.install_fixture(Path('/run/systemd/system')/self.blocker,
            '[Service]\nType=oneshot\nExecStart=/usr/bin/cat '+str(self.fifo)+
            '\nTimeoutStartSec=25s\nTimeoutStopSec=5s\nKillMode=control-group\nStandardOutput=null\nStandardError=null\n')
        self.manager('daemon-reload')
        # Instrument accept to report actual root executor credentials. All socket
        # operations and the installed production collector remain real.
        script = r"""
import importlib.util,json,os,socket,struct,sys
from pathlib import Path
spec=importlib.util.spec_from_file_location('custody_supervisor','/usr/libexec/gatebox/supervisor.py')
s=importlib.util.module_from_spec(spec); spec.loader.exec_module(s)
class Observed(socket.socket):
    def accept(self):
        conn,address=super().accept()
        pid,uid,gid=struct.unpack('3i',conn.getsockopt(socket.SOL_SOCKET,socket.SO_PEERCRED,12))
        print(json.dumps(dict(accepted=self.kind,pid=pid,uid=uid)),flush=True)
        return conn,address
listeners={}
for kind in ('log','status'):
    sock=Observed(socket.AF_UNIX,socket.SOCK_STREAM); sock.kind=kind
    path=Path('/run/mothergate-control/incoming')/(sys.argv[1]+'.'+kind+'.sock')
    sock.bind(str(path)); path.chmod(0o700); sock.listen(1); listeners[kind]=sock
print(json.dumps({'ready':True}),flush=True)
s.collect(sys.argv[1],listeners,s.Evidence(),{})
"""
        self.collector = subprocess.Popen(['/usr/bin/python3','-I','-B','-c',script,self.run],
            user=self.owner.pw_uid,group=self.owner.pw_gid,extra_groups=[],umask=0o077,
            env=lifecycle.ENV,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,close_fds=True,bufsize=0)
        self.assertEqual(self.line(),{'ready':True})
        self.launch = subprocess.Popen(['/usr/bin/python3','-I','-B',str(HERE/'lifecycle.py'),'start',self.run],
            env=lifecycle.ENV,stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,close_fds=True)
        self.wait_for(lambda:self.sealed.exists() and all((self.sealed/(k+'.sock')).exists()
            for k in ('log','status')),'sealed endpoints')
        self.job = self.wait_for(lambda:(v if v['Job'] and v['ActiveState']=='activating' else None)
            if (v:=self.state(self.blocker)) else None,'barrier activating')
        self.queued = self.wait_for(lambda:(v if v['Job'] else None)
            if (v:=self.state(self.unit)) else None,'real pending payload job')
        self.assertIsNone(self.collector.poll())
        self.identities = {k:((p:=self.sealed/(k+'.sock')).stat().st_dev,p.stat().st_ino)
                           for k in ('log','status')}
        self.observations = dict(run_id=self.run,pending_job=self.queued,barrier=self.job)

    def install_fixture(self, path, text):
        data = text.encode()
        fd = os.open(path,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o644)
        with os.fdopen(fd,'wb') as stream: stream.write(data)
        self.files[path] = hashlib.sha256(data).hexdigest()

    def release(self):
        def writer():
            try: return os.open(self.fifo,os.O_WRONLY|os.O_NONBLOCK|os.O_NOFOLLOW)
            except OSError as exc:
                if exc.errno!=errno.ENXIO: raise
                return None
        # fd may be zero; encode readiness separately from the descriptor.
        def ready():
            fd = writer()
            return (fd,) if fd is not None else None
        fd, = self.wait_for(ready,'FIFO reader ready')
        os.close(fd)  # EOF releases cat; no arbitrary fixed sleep.

    def intact(self):
        self.assertTrue((self.control/'sealed'/('.pending-'+self.run)).is_file())
        self.assertTrue(self.marker.is_file())
        for kind,identity in self.identities.items():
            info = (self.sealed/(kind+'.sock')).lstat()
            self.assertTrue(stat.S_ISSOCK(info.st_mode))
            self.assertEqual((info.st_dev,info.st_ino),identity)

    def denied_operations(self, canary):
        script = r"""
import errno,json,os,sys
base,target=sys.argv[1:]; results=[]
for kind in ('log','status'):
    path=base+'/'+kind+'.sock'
    for action in ('unlink','rename','symlink'):
        try:
            if action=='unlink': os.unlink(path)
            elif action=='rename': os.rename(path,path+'.moved')
            else: os.symlink(target,path)
        except OSError as e:
            if e.errno not in (errno.EACCES,errno.EPERM): raise
            results.append([kind,action,e.errno])
        else: raise RuntimeError('sealed namespace changed')
print(json.dumps(results))
"""
        result = self.command(['/usr/sbin/runuser','-u','mothergate','--','/usr/bin/python3',
                               '-I','-c',script,str(self.sealed),str(canary)])
        denied = json.loads(result.stdout); self.assertEqual(len(denied),6)
        self.intact(); return denied

    def test_mothergate_cannot_unlink_rename_or_symlink_after_relocation(self):
        self.observations['denied'] = self.denied_operations(self.root/'unused-target')

    def test_systemd_output_never_writes_disposable_root_canary(self):
        canary = self.root/'canary'
        with canary.open('xb') as stream: stream.write(os.urandom(64))
        canary.chmod(0o600)
        before = canary.read_bytes(); info = canary.stat()
        self.observations['denied'] = self.denied_operations(canary)
        # Replace BOTH original incoming names with canary symlinks as mothergate.
        # The systemd executor must connect to sealed inodes, not attacker names.
        script = 'import os,sys; [os.symlink(sys.argv[2],sys.argv[1]+"."+k+".sock") for k in ("log","status")]'
        self.command(['/usr/sbin/runuser','-u','mothergate','--','/usr/bin/python3','-I','-c',script,
                      str(self.control/'incoming'/self.run),str(canary)])
        self.release()
        observations = [self.line(),self.line()]
        self.assertEqual({v['accepted'] for v in observations},{'log','status'})
        self.assertTrue(all(v['pid']>0 and v['uid']==0 for v in observations),'actual systemd executor required')
        self.assertEqual(canary.read_bytes(),before)
        after = canary.stat()
        self.assertEqual((after.st_dev,after.st_ino,after.st_mtime_ns,after.st_ctime_ns),
                         (info.st_dev,info.st_ino,info.st_mtime_ns,info.st_ctime_ns))
        self.observations.update(executor=observations,canary_sha256=hashlib.sha256(before).hexdigest())

    def test_pending_manager_job_survives_collector_death_and_supervisor_restart(self):
        before = self.state('mothergate.service')
        self.assertGreater(int(before['MainPID']),1)
        self.collector.kill(); self.collector.wait(timeout=5)
        self.assertTrue(self.state(self.unit)['Job'],'job must be pending when collector dies')
        self.intact()
        self.manager('restart','mothergate.service')
        after = self.state('mothergate.service')
        self.assertEqual(after['ActiveState'],'active')
        self.assertGreater(int(after['MainPID']),1); self.assertNotEqual(after['MainPID'],before['MainPID'])
        # BindsTo may cancel the job on restart. That is NOT permission to
        # discard custody: only the explicit settled stop below can do so.
        self.intact()
        registration = self.probe.config('registry.json')['mother']
        self.assertEqual(self.probe.client_request(self.probe.FIXTURE,registration),{'status':'BUSY'})
        self.observations.update(collector_returncode=self.collector.returncode,
                                 supervisor_before=before,supervisor_after=after,retained=True)

    def cleanup(self):
        # Stop exact units first; never recursively delete or repair root custody.
        self.manager('stop',self.unit,self.blocker)
        if self.launch is not None: self.launch.wait(timeout=10)
        state = self.bridge.run('stop',self.run)
        self.assertEqual(state['Job'],'')
        self.assertFalse(self.sealed.exists())
        self.assertFalse((self.control/'sealed'/('.pending-'+self.run)).exists())
        if self.collector is not None:
            if self.collector.poll() is None: self.collector.kill()
            self.collector.wait(timeout=5); self.collector.stdout.close()
        result = self._outcome.result
        if any(test is self for test,_ in result.failures+result.errors):
            self.fail('proof failed; disposable evidence retained at '+str(self.root))
        for kind in ('log','status'):
            path = self.control/'incoming'/(self.run+'.'+kind+'.sock')
            try: info = path.lstat()
            except FileNotFoundError: continue
            self.assertEqual(info.st_uid,self.owner.pw_uid)
            self.assertTrue(stat.S_ISSOCK(info.st_mode) or stat.S_ISLNK(info.st_mode))
            path.unlink()
        script = r"""
import os,sys
from pathlib import Path
run=sys.argv[1]; stage=Path('/var/lib/mothergate/staging')/run
fd=os.open(stage,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW)
try:
    os.unlink('request.json',dir_fd=fd); os.rmdir('snapshot',dir_fd=fd)
finally: os.close(fd)
stage.rmdir(); (Path('/var/lib/mothergate/pending')/('.'+run)).unlink()
"""
        subprocess.run(['/usr/bin/python3','-I','-B','-c',script,self.run],
            user=self.owner.pw_uid,group=self.owner.pw_gid,extra_groups=[],
            env=lifecycle.ENV,stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,close_fds=True,timeout=5,check=True)
        for path,digest in self.files.items():
            self.probe.protected(path)
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),digest)
            path.unlink()
        if hasattr(self,'dropin'): self.dropin.rmdir()
        self.manager('daemon-reload')
        # Keep root canaries/observations on failure for review. Only known leaves
        # are removed, never globbed or recursively deleted.
        self.fifo.unlink()
        canary = self.root/'canary'
        if canary.exists(): canary.unlink()
        self.root.rmdir()
