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
import subprocess
import sys
import re
from contextlib import ExitStack
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
            # Explicit modes: the fixture must not depend on the caller's umask.
            (run/'request.json').chmod(0o600)
            for mode in ('absent','regular','socket'):
                sockets = []
                try:
                    for suffix in ('log','status'):
                        path = control/(RUN+'.'+suffix+'.sock')
                        if mode == 'regular': path.touch(mode=0o600)
                        if mode == 'socket':
                            sock = socket.socket(socket.AF_UNIX)
                            sock.bind(str(path))
                            path.chmod(0o600)
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
        register = (HERE/'MISSION.md').read_text()
        sheet = Path(re.search(r'^Frozen sheet: (.+)\.$',register,re.M)[1]).read_text()
        units = re.findall(r'### `(mothergate[^`]+)`\n\n```ini\n(.*?)```',sheet,re.S)
        self.assertEqual(len(units),3)
        for name, expected in units:
            self.assertEqual((HERE/'units'/name).read_text(),expected,name)
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

    def test_review1_full_run_publishes_validated_pass(self):
        self.full_run(False)
    def test_review1_validation_failure_publishes_and_reopens(self):
        self.full_run(True)
    def full_run(self, invalid):
        # Pipes replace only denied AF_UNIX transport; collect/coverage/publish are real.
        class Pipe:
            def __init__(self, fd): self.fd = fd
            def fileno(self): return self.fd
            def getsockopt(self, *args): return struct.pack('3i',1,0,0)
            def recv(self, count): return os.read(self.fd,count)
            def setblocking(self, value): os.set_blocking(self.fd,value)
            def bind(self, path): pass
            def listen(self, count): pass
            def close(self): os.close(self.fd)
        class Listener(Pipe):
            def __init__(self):
                ready, trigger = os.pipe()
                super().__init__(ready)
                os.write(trigger,b'x')
                os.close(trigger)
                read, self.write = os.pipe()
                self.stream = Pipe(read)
            def accept(self): return self.stream, None
        with tempfile.TemporaryDirectory() as tmp, ExitStack() as stack:
            root = Path(tmp)
            for name in ('staging','pending','receipts','src'): (root/name).mkdir(mode=0o700)
            marker = root/'pending'/('.'+RUN)
            marker.touch()
            desktop = json.loads((HERE/'policy/gates.json').read_bytes())['gates'][6]['expected_names']
            cases = json.loads((HERE/'policy/exclusions.json').read_bytes())['cases']
            specs = [('nexus-mother-host','host',['host_test']),
                     ('nexus-mother-desktop','live',desktop),
                     ('nexus-verifier-sandbox','phase2_live_sandbox',cases)]
            artifacts = [dict(reason='compiler-artifact',profile={'test':True},
                              package_id='path+file:///work/src/'+p+'#0.1.0',
                              manifest_path='/work/src/'+p+'/Cargo.toml',target={'name':t},
                              executable='/work/target/'+t) for p,t,names in specs]
            for p,t,names in specs:
                directory = root/'src'/p
                directory.mkdir()
                (directory/'Cargo.toml').write_text('[package]\nname = '+json.dumps(p)+'\nversion = "0.1.0"\n')
            routed = []
            popen = subprocess.Popen
            def spawn(argv, **options):
                routed.append(argv)
                if argv[0] == '/opt/rust/bin/cargo':
                    output = '\n'.join(json.dumps(a) for a in artifacts) if '--no-run' in argv else 'done\n'
                else:
                    names = next(n for p,t,n in specs if argv[0] == '/work/target/'+t)
                    if '--list' in argv: output = ''.join(n+': test\n' for n in names)
                    else:
                        self.assertNotEqual(argv[0],'/work/target/phase2_live_sandbox')
                        output = ''.join('test '+n+' ... ok\n' for n in names)
                        output += f'test result: ok. {len(names)} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;\n'
                options['cwd'] = str(root/'src')
                return popen([sys.executable,'-c','import sys; sys.stdout.write(sys.argv[1]); sys.stderr.write("Finished test build\\n")',output],**options)
            def payload_path(path):
                value = str(path)
                if value.startswith('/gate/policy/'): return HERE/'policy'/Path(value).name
                if value.startswith('/work/src'): return root/'src'/value.removeprefix('/work/src').lstrip('/')
                return Path(value)
            listeners = [Listener(),Listener()]
            errors, workers = [], []
            def payload():
                runner = init.Runner(listeners[0].write,listeners[1].write)
                try:
                    runner.gates()
                    for name, value in {'inputs':{'before':{'rust':'same'},'after':{'rust':'same'}},
                                        'resources':{'fixture':True},'isolation':{'fixture':True},
                                        'versions':{'fixture':True}}.items():
                        runner.output(name,json.dumps(value).encode())
                    init.packet(runner.status,{'event':'complete'})
                except Exception as exc: errors.append(str(exc))
                finally:
                    for listener in listeners: os.close(listener.write)
            def launch(verb, run):
                worker = threading.Thread(target=payload)
                workers.append(worker)
                worker.start()
            def exported(source, sha, path):
                path.mkdir()
                return dict(commit=sha,content_sha256='invalid' if invalid else 'a'*64,lockfile_sha256='b'*64)
            for obj, name, value in [(init,'Path',payload_path),(init.subprocess,'Popen',spawn),
                    (supervisor,'STATE',root),(supervisor,'CONTROL',root),(supervisor,'shim',launch),
                    (supervisor,'unit_state',lambda run:{'ActiveState':'active'}),
                    (supervisor,'stop_and_confirm',lambda run:{'complete':True}),
                    (supervisor.socket,'socket',Mock(side_effect=listeners)),
                    (supervisor.snapshot,'GitSource',Mock()),(supervisor.snapshot,'export',exported)]:
                stack.enter_context(patch.object(obj,name,value))
            stack.enter_context(patch.object(lifecycle,'check_start'))
            stack.enter_context(patch.object(supervisor,'sibling',side_effect=lambda name: lifecycle if name=='lifecycle' else init))
            identity = {k:'c'*64 for k in ('bundle_sha256','policy_sha256','gates_sha256')}
            supervisor.execute(RUN,REQUEST,1234,{'device':1,'inode':2},identity,marker,'qualifying')
            for worker in workers: worker.join(5)
            item = supervisor.receipt.verify_integrity(root/'receipts'/RUN,os.getuid())['summary']
            self.assertEqual(item['status'],'FAILED' if invalid else 'PASS_SCOPED',item['missing_evidence'])
            if invalid: self.assertIn('snapshot identity','; '.join(item['missing_evidence']))
            else: self.assertEqual(errors,[])
            self.assertFalse(marker.exists())
            admission = supervisor.Admission(root/'pending')
            self.assertTrue(admission.acquire())
            admission.release()

    def test_review3_manifest_package_identity(self):
        with tempfile.TemporaryDirectory() as tmp:
            manifest = Path(tmp)/'Cargo.toml'
            manifest.write_text('[package]\nname = "nexus-verifier-sandbox"\nversion = "0.1.0"\n')
            self.assertEqual(init.artifact_package({'manifest_path':str(manifest),
                              'package_id':'path+file:///work/src/cj#0.1.0'}),'nexus-verifier-sandbox')

    def test_review2_exact_excluded_artifact_and_listing(self):
        policy, exclusions = supervisor.receipt.policy()
        policy['gates'] = [g for g in policy['gates'] if g['id'] in ('compile-tests','workspace')]
        def artifact(package, executable):
            return dict(reason='compiler-artifact',profile={'test':True},package=package,
                        target={'name':exclusions['target']},executable='/work/target/'+executable)
        excluded = artifact('nexus-verifier-sandbox','excluded')
        other = artifact('other-crate','included')
        for variant in ('valid','missing','duplicate','names-changed'):
            items = [other]+([] if variant=='missing' else [excluded]*(2 if variant=='duplicate' else 1))
            names = exclusions['cases'][:-1] if variant=='names-changed' else exclusions['cases']
            command = Mock(side_effect=lambda gate,argv: ('c1',0,
                           '\n'.join(json.dumps(a) for a in items) if gate=='compile-tests'
                           else ''.join(n+': test\n' for n in names)))
            runner = init.Runner(-1,-1)
            with patch.object(init.Path,'read_bytes',side_effect=[json.dumps(policy).encode(),json.dumps(exclusions).encode()]), \
                    patch.object(init,'artifact_package',side_effect=lambda a:a['package']), \
                    patch.object(runner,'command',command), patch.object(runner,'output'), patch.object(runner,'libtest') as execute:
                if variant=='valid':
                    runner.gates()
                    self.assertEqual([a.args[1]['package'] for a in execute.call_args_list],['other-crate'])
                    self.assertIn(['/work/target/excluded','--list','--format','terse'],[a.args[1] for a in command.call_args_list])
                else:
                    with self.subTest(variant=variant), self.assertRaises(ValueError): runner.gates()

    def test_review4_collect_detects_failed_start(self):
        selector = Mock()
        selector.select.return_value = []
        with patch.object(supervisor.selectors,'DefaultSelector',return_value=selector), \
                patch.object(supervisor.time,'monotonic',side_effect=[0,0,14431]), \
                patch.object(supervisor,'unit_state',return_value={'ActiveState':'failed','Result':'resources'}):
            with self.assertRaisesRegex(ValueError,'unit exited'): supervisor.collect(RUN,{},supervisor.Evidence(),{})

    def test_review5_installer_placeholder_contract(self):
        self.assertEqual(supervisor.QUALIFICATION_PLACEHOLDER,{'identity':None,'passed':[]})
        self.assertFalse(supervisor.qualified({'bundle':'installed'},supervisor.QUALIFICATION_PLACEHOLDER))

    def test_review6_main_connection_delivery(self):
        for lost in (False,True):
            with self.subTest(lost=lost), tempfile.TemporaryDirectory() as tmp:
                state = Path(tmp)
                (state/'pending').mkdir()
                peer = Mock()
                peer.getsockopt.return_value = struct.pack('3i',7,1234,1234)
                if lost: peer.sendall.side_effect = [OSError('closed'),None]
                listener = Mock(family=socket.AF_UNIX)
                listener.getsockname.return_value = '/run/mothergate-submit.sock'
                listener.accept.side_effect = [(peer,None),StopIteration]
                registration = dict(uid=1234,fixture_sha=SHA,qualification_gate_set='full')
                configs = {'registry.json':{'mother':registration},'identity.json':{},
                           'admission.json':{'stage':'qualifying','identity':{}},
                           'qualification.json':{'identity':None,'passed':[]}}
                def thread(target,args=(),**kwargs): return Mock(start=lambda:target(*args))
                with patch.dict(os.environ,LISTEN_PID=str(os.getpid()),LISTEN_FDS='1'), \
                        patch.object(supervisor,'STATE',state), patch.object(supervisor,'read_config',side_effect=configs.__getitem__), \
                        patch.object(supervisor.socket,'socket',return_value=listener), \
                        patch.object(supervisor,'receive_request',return_value=REQUEST), \
                        patch.object(supervisor.threading,'Thread',side_effect=thread), \
                        patch.object(supervisor,'execute',side_effect=lambda *args:args[5].unlink()) as execute:
                    with self.assertRaises(StopIteration): supervisor.main()
                self.assertEqual(execute.call_count,1)
                self.assertEqual(execute.call_args.args[-1],'qualifying')
                self.assertEqual(peer.sendall.call_count,1)
                reply = json.loads(peer.sendall.call_args_list[0].args[0][4:])
                self.assertEqual(reply,{'status':'ACCEPTED','run_id':execute.call_args.args[0]})

    def test_review7_init_main_status_descriptor_closed_to_child(self):
        popen = subprocess.Popen
        def spawn(argv, **options):
            options['cwd'] = '/tmp'
            return popen(argv,**options)
        def gates(runner):
            self.assertFalse(os.get_inheritable(runner.status))
            script = 'import os,sys\ntry: os.fstat(int(sys.argv[1]))\nexcept OSError: sys.exit(0)\nsys.exit(9)'
            _, code, _ = runner.command('format',[sys.executable,'-c',script,str(runner.status)])
            self.assertEqual(code,0,'candidate inherited private status FD')
        open_fd = os.open
        with patch.object(init.os,'getpid',return_value=1), patch.object(init.os,'getuid',return_value=1234), \
                patch.object(init.ctypes,'CDLL',return_value=Mock(prctl=Mock(return_value=0))), \
                patch.object(init.os,'dup',side_effect=lambda fd:open_fd('/dev/null',os.O_RDWR)), \
                patch.object(init.os,'dup2'), patch.object(init.signal,'signal'), patch.object(init,'Path'), \
                patch.object(init.shutil,'copytree'), patch.object(init.os,'walk',return_value=[]), \
                patch.object(init,'observations',return_value={'fixture':True}), patch.object(init,'resources',return_value={}), \
                patch.object(init.Runner,'gates',gates), patch.object(init.subprocess,'Popen',spawn):
            init.main()

    def test_review7_stop_requires_empty_owned_cgroup(self):
        group = '/system.slice/mothergate-run@'+RUN+'.service'
        for state, events in [({'ActiveState':'active'},''),
                ({'ActiveState':'inactive','ControlGroup':'/other'},''),
                ({'ActiveState':'inactive','ControlGroup':group},'populated 1\n'),
                ({'ActiveState':'inactive','ControlGroup':group},'populated 0\n')]:
            with patch.object(supervisor,'shim') as shim, patch.object(supervisor,'unit_state',return_value=state), \
                    patch.object(supervisor.Path,'read_text',return_value=events):
                if events == 'populated 0\n': self.assertTrue(supervisor.stop_and_confirm(RUN)['complete'])
                else:
                    with self.assertRaises(ValueError): supervisor.stop_and_confirm(RUN)
                shim.assert_called_once_with('stop',RUN)

    def test_review2_other_package_target_receipt_is_included(self):
        from test_unit_core import ReceiptTests
        fixture = ReceiptTests()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        summary = fixture.success()
        summary['coverage']['artifacts'][0]['target'] = 'phase2_live_sandbox'
        supervisor.receipt.validate_summary(summary)
