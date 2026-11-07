"""PR-2a offline oracles extracted under Q2 (2026-11-07).
Root fixtures use synthetic registered UID 1234; production still rejects UID 0.
InstalledAcceptance and CLIENTS provenance tests are saved in the PR-2b draft.
PR-2 frozen oracles. Offline success never asserts installed qualification.

Q1 oracle amendment, 2026-11-07 (supersedes original CLIENTS row):
CLIENTS | actual Claude client accepts frozen full qualification request from
registered UID, refuses non-qualification request while qualifying; Codex is
unsupported because runner denies AF_UNIX. | PASS S1-CLIENTS claude-qualification
"""
import hashlib
import errno
import importlib.util
import json
import os
from pathlib import Path
import socket
import stat
import struct
import tempfile
import threading
import time
import unittest
from unittest.mock import patch, Mock

HERE = Path(__file__).resolve().parents[1]
def load(name):
    spec = importlib.util.spec_from_file_location('gbx_'+name, HERE/(name+'.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module
supervisor, lifecycle, init, verify = [load(n) for n in
                                      ('supervisor','lifecycle','init','verify')]
RUN = 'a'*32
SHA = '899e3872c5b61223c87b905f63c66947a816259b'
REQUEST = {'repo_id':'mother','full_sha':SHA,'gate_set_id':'full'}
REGISTRY = {'mother':{'uid':os.getuid(),'object_format':'sha1'}}
ENV = {'PATH':'/opt/rust/bin:/usr/bin:/bin','HOME':'/work/home','PWD':'/work/src',
       'CARGO_HOME':'/work/cargo','CARGO_TARGET_DIR':'/work/target','CARGO_BUILD_JOBS':'4',
       'RUST_TEST_THREADS':'4','CARGO_TERM_COLOR':'never','CARGO_NET_OFFLINE':'true',
       'RUST_BACKTRACE':'1','TMPDIR':'/run/gate/tmp','XDG_RUNTIME_DIR':'/run/gate/xdg',
       'XDG_CACHE_HOME':'/work/cache','LANG':'C.UTF-8','LC_ALL':'C.UTF-8','TZ':'UTC','CI':'true'}

def frame(value):
    data = json.dumps(value).encode()
    return struct.pack('!I',len(data))+data

def start_event():
    return {'event':'start','id':'format','gate_id':'format','argv':['fixed'],
            'cwd':'/work/src','environment':ENV}

def done_event(code=0):
    return {'event':'end','id':'format','exit_code':code,
            'signal':-code if code < 0 else None}

class OfflineServiceTests(unittest.TestCase):
    def setUp(self):
        if self._testMethodName not in ('test_b1_socket_frame_and_peer',
                'test_b1_deadline_and_trailing_data','test_b2_capture_socket_and_request_types'):
            return
        try:
            if self._testMethodName.startswith('test_b1_'):
                left, right = socket.socketpair()
                with left, right: right.sendall(b'x')
            else:
                with tempfile.TemporaryDirectory() as tmp, socket.socket(socket.AF_UNIX) as sock:
                    sock.bind(str(Path(tmp)/'probe.sock'))
        except PermissionError as exc:
            if exc.errno != errno.EPERM: raise
            self.skipTest('sandbox_blocked: AF_UNIX operation denied with EPERM; Architect must run outside runner')
    def test_b1_socket_frame_and_peer(self):
        for payload in (frame(REQUEST),frame(dict(REQUEST,full_sha='1234567')),
                        frame(dict(REQUEST,path='/tmp')),b'\0\0\0\0',
                        b'\0\0\4\1',b'\0\0\0\x02\xff\xff'):
            left, right = socket.socketpair()
            with left, right:
                left.settimeout(2)
                right.sendall(payload)
                right.shutdown(socket.SHUT_WR)
                if payload == frame(REQUEST):
                    self.assertEqual(supervisor.receive_request(left,REGISTRY),REQUEST)
                else:
                    with self.assertRaises(ValueError): supervisor.receive_request(left,REGISTRY)
        left, right = socket.socketpair()
        with left, right:
            right.sendall(frame(REQUEST))
            right.shutdown(socket.SHUT_WR)
            with self.assertRaises(ValueError): supervisor.receive_request(left,{'mother':dict(REGISTRY['mother'],uid=-1)})
    def test_b1_deadline_and_trailing_data(self):
        for payload in (b'\0',frame(REQUEST)+b'x'):
            left, right = socket.socketpair()
            with left, right:
                right.sendall(payload)
                left.settimeout(.03)
                began = time.monotonic()
                with self.assertRaises(ValueError): supervisor.receive_request(left,REGISTRY)
                self.assertLess(time.monotonic()-began,1)
    def test_b1_admission_is_single_and_pending_closes_it(self):
        with tempfile.TemporaryDirectory() as tmp:
            pending = Path(tmp)
            admission = supervisor.Admission(pending)
            self.assertTrue(admission.acquire())
            self.assertFalse(admission.acquire())
            admission.release()
            (pending/RUN).mkdir()
            self.assertFalse(admission.acquire())
    def test_b2_fixed_shim_arguments(self):
        for verb in ('start','stop'):
            argv = lifecycle.command([verb,RUN])
            self.assertEqual(argv, ['/usr/bin/systemctl',verb]+(['--no-block'] if verb=='start' else [])+
                             ['mothergate-run@'+RUN+'.service'])
        for args in ([],['restart',RUN],['start','../x'],['start',RUN,'--property=x'],
                     ['start','mothergate.service'],['start',RUN.upper()]):
            with self.subTest(args=args), self.assertRaises(ValueError): lifecycle.command(args)
    def test_b2_capture_socket_and_request_types(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            stage, control = root/'staging',root/'control'
            stage.mkdir(mode=0o700)
            control.mkdir(mode=0o700)
            run = stage/RUN
            run.mkdir(mode=0o700)
            (run/'snapshot').mkdir(mode=0o555)
            (run/'request.json').write_text('{}')
            for mode in ('absent','regular','socket'):
                sockets = []
                try:
                    for suffix in ('log','status'):
                        path = control/(RUN+'.'+suffix+'.sock')
                        if mode == 'regular': path.touch()
                        if mode == 'socket':
                            sock = socket.socket(socket.AF_UNIX)
                            sock.bind(str(path))
                            sockets.append(sock)
                    if mode == 'socket': lifecycle.check_start(RUN,stage,control,os.getuid())
                    else:
                        with self.assertRaises(ValueError): lifecycle.check_start(RUN,stage,control,os.getuid())
                finally:
                    for sock in sockets: sock.close()
                    for path in control.iterdir(): path.unlink()
            (run/'request.json').unlink()
            (run/'request.json').symlink_to(root/'outside')
            with self.assertRaises(ValueError): lifecycle.check_start(RUN,stage,control,os.getuid())
    def test_b3_detached_and_remapped_receipt_unauthenticated(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            root.chmod(0o755)
            with self.assertRaises(ValueError): verify.host_ancestry(root)
        with patch.object(verify.os,'geteuid',return_value=1000):
            with self.assertRaises(ValueError): verify.host_context()
    def test_b4_missing_status_never_success(self):
        evidence = supervisor.Evidence()
        with self.assertRaises(ValueError): evidence.finish(True,False)
        evidence.accept(start_event())
        with self.assertRaises(ValueError): evidence.finish(True,False)
        evidence.accept(done_event())
        with self.assertRaises(ValueError): evidence.finish(True,False)
    def test_b4_nonzero_signal_timeout_and_collector_loss(self):
        for code, collector, timeout in ((1,True,False),(-9,True,False),(0,False,False),(0,True,True)):
            evidence = supervisor.Evidence()
            evidence.accept(start_event())
            evidence.accept(done_event(code))
            evidence.accept({'event':'complete'})
            with self.subTest(code=code,collector=collector,timeout=timeout), self.assertRaises(ValueError):
                evidence.finish(collector,timeout)
    def test_b4_status_protocol_and_log_bounds(self):
        for event in ({'event':'complete'},done_event(),{'event':'forged'},dict(start_event(),extra=True)):
            evidence = supervisor.Evidence()
            with self.subTest(event=event), self.assertRaises(ValueError): evidence.accept(event)
        evidence = supervisor.Evidence()
        evidence.accept(start_event())
        with self.assertRaises(ValueError): evidence.accept(start_event())
        evidence = supervisor.Evidence(log_limit=4)
        evidence.capture(b'abcd')
        with self.assertRaises(ValueError): evidence.capture(b'e')
    def test_b4_positive_status_and_private_fd(self):
        evidence = supervisor.Evidence()
        evidence.accept(start_event())
        evidence.accept(done_event())
        evidence.accept({'event':'complete'})
        self.assertEqual(evidence.finish(True,False)[0]['id'],'format')
        options = init.child_options(17)
        self.assertTrue(options['close_fds'])
        self.assertEqual(options.get('pass_fds',()),())
        self.assertEqual(options['stdin'],-3)  # subprocess.DEVNULL
        self.assertEqual(options['env'],ENV)
    def test_e2e_libtest_reconciliation_rejects_noop(self):
        names = ['alpha','beta']
        output = 'test alpha ... ok\ntest beta ... ok\n\ntest result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;\n'
        result = init.reconcile(names,output)
        self.assertEqual(result['passed'],names)
        for bad in ('',output.replace('beta ... ok','beta ... ignored'),output.replace('2 passed','1 passed'),
                    output+'test unknown ... ok\n'):
            with self.subTest(output=bad), self.assertRaises(ValueError): init.reconcile(names,bad)
    def test_policy_and_units_are_frozen(self):
        hashes = json.loads('{"mothergate-run@.service": "0bf915285dee3187950045ada9d1721e32f279fa57ba0248f021dff16f70a595", "mothergate.service": "15cf0f59f01706639fb723761b553404e3e7af28556c726c50196155bff4aacd", "mothergate.socket": "4a8bf90e4c2da9c84239c963fdd137dd25457f7e3fafb122998df8db5c837d6a"}')
        for name, expected in hashes.items():
            self.assertEqual(hashlib.sha256((HERE/'units'/name).read_bytes()).hexdigest(),expected,name)
        self.assertEqual((HERE/'policy/sudoers').read_text(),
                         'Defaults:mothergate env_reset\nmothergate ALL=(root) NOPASSWD: NOSETENV: /usr/libexec/gatebox/lifecycle.py\n')
        self.assertIn('polkit.Result.NO',(HERE/'policy/deny-systemd.rules').read_text())
    def test_q1_qualifying_admission(self):
        identity = {'bundle':'a'*64,'policy':'b'*64,'gates':'c'*64}
        registration = dict(REGISTRY['mother'],uid=1234,fixture_sha=SHA,qualification_gate_set='full')
        state = {'stage':'qualifying','identity':identity}
        decide = supervisor.admission_stage
        self.assertEqual(decide(REQUEST,1234,registration,identity,state,{}),'qualifying')
        for req, uid in ((dict(REQUEST,full_sha='b'*40),1234),
                         (dict(REQUEST,gate_set_id='other'),1234),
                         (REQUEST,-1),(REQUEST,0)):
            with self.subTest(request=req,uid=uid), self.assertRaisesRegex(ValueError,'QUALIFICATION_'):
                decide(req,uid,registration,identity,state,{})
        for bad in ({},dict(state,identity={}),dict(state,stage='unexpected')):
            with self.assertRaises(ValueError): decide(REQUEST,1234,registration,identity,bad,{})
        normal = dict(state,stage='normal')
        with self.assertRaises(ValueError): decide(REQUEST,1234,registration,identity,normal,{})
        evidence = {'identity':identity,'passed':['B1','B2','B3','B4','A1','A5','A6','A8','E2E','CLIENTS']}
        self.assertEqual(decide(REQUEST,1234,registration,identity,normal,evidence),'normal')
    def test_installer_admission_binding(self):
        identity = {'bundle':'a'*64,'policy':'b'*64,'gates':'c'*64}
        evidence = {'identity':identity,'passed':['B1','B2','B3','B4','A1','A5','A6','A8','E2E','CLIENTS']}
        self.assertTrue(supervisor.qualified(identity,evidence))
        for bad in ({},dict(evidence,identity={}),dict(evidence,passed=['E2E'])):
            self.assertFalse(supervisor.qualified(identity,bad))

    def test_b4_capture_preserved_in_failed_receipt(self):
        self.run_capture_fixture(fail=True)
    def test_b4_normalized_capture_reaches_reconciliation(self):
        self.run_capture_fixture(fail=False)
    def run_capture_fixture(self, fail):
        with tempfile.TemporaryDirectory() as tmp:
            state = Path(tmp)
            for name in ('staging','pending','receipts'): (state/name).mkdir(mode=0o700)
            marker = state/'pending'/('.'+RUN)
            marker.touch()
            def exported(source, sha, path):
                path.mkdir()
                return {'commit':sha}
            def captured(run, listeners, evidence, logs):
                evidence.accept(start_event())
                evidence.accept(done_event())
                logs['c1'] = bytearray(b'partial output')
                if fail: raise ValueError('collector lost')
                evidence.accept({'event':'complete'})
                return evidence, {'c1.log':b'partial output'}
            seen = []
            def reconciled(evidence, logs):
                seen.append(logs)
                raise ValueError('fixture stops before deferred qualification')
            def sibling(name):
                self.assertEqual(name,'lifecycle')
                return Mock(check_start=Mock())
            with patch.object(supervisor,'STATE',state), patch.object(supervisor,'sibling',side_effect=sibling), \
                    patch.object(supervisor.snapshot,'GitSource'), patch.object(supervisor.snapshot,'export',side_effect=exported), \
                    patch.object(supervisor.socket,'socket',return_value=Mock()), patch.object(supervisor,'shim'), \
                    patch.object(supervisor,'collect',side_effect=captured), patch.object(supervisor,'coverage',side_effect=reconciled), \
                    patch.object(supervisor,'stop_and_confirm',return_value={'complete':True}):
                supervisor.execute(RUN,REQUEST,1234,{'device':1,'inode':2},{},marker,'qualifying')
            item = supervisor.receipt.verify_integrity(state/'receipts'/RUN,os.getuid())['summary']
            self.assertEqual(item['status'],'FAILED')
            self.assertEqual(item['installed']['stage'],'qualifying')
            self.assertEqual(item['commands'][0]['id'],'format')
            self.assertEqual((state/'receipts'/RUN/'c1.log').read_bytes(),b'partial output')
            self.assertFalse(marker.exists())
            self.assertEqual(seen,[] if fail else [{'c1.log':b'partial output'}])
    def test_b1_slow_fragments_share_one_framing_deadline(self):
        # A virtual stream supplies one byte per 0.6 seconds and honours timeout.
        # This tests deadline arithmetic; the real AF_UNIX tests remain blocked.
        class SlowPeer:
            def __init__(self): self.now, self.timeout, self.data = 0., 2., bytearray(frame(REQUEST))
            def gettimeout(self): return self.timeout
            def settimeout(self, value): self.timeout = value
            def getsockopt(self, *args): return struct.pack('3i',1,os.getuid(),os.getgid())
            def recv(self, count):
                if self.timeout < .6: raise TimeoutError('slow peer')
                self.now += .6
                if not self.data: return b''
                data = bytes(self.data[:1])
                del self.data[:1]
                return data
        peer = SlowPeer()
        with patch.object(supervisor.time,'monotonic',side_effect=lambda:peer.now):
            with self.assertRaisesRegex(ValueError,'framing/reply deadline'):
                supervisor.receive_request(peer,REGISTRY)
        self.assertLessEqual(peer.now,2.)
