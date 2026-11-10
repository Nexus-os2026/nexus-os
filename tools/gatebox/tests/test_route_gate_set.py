"""GB1a controls (SB3E-R18, R19): the route-cells gate set's request, receipt identity, historical
policy, client, verifier and install/upgrade. Each rejection is its own check; `full` is unchanged."""
import copy
import fcntl
import hashlib
import io
import json
import os
import re
from pathlib import Path
import re
import struct
import subprocess
import sys
import shutil
import tarfile
import tempfile
import unittest
from contextlib import ExitStack
from unittest.mock import patch, Mock
import test_unit_core as core
from test_installed import HERE, INSTALLED, installer, load

receipt, snapshot = core.receipt, core.snapshot
ROUTE = json.loads((HERE/'policy/route-cells.json').read_text())
SHA, EXE = '1'*40, '/work/target-route/release/deps/route_cells-0123456789abcdef'
NAMES = ['cells::rc_r1_alpha_beta_one','cells::rc_r2_alpha_beta_two']
WRAPPER = 'd23fdea2e987259b7c07a8ab7fe039cbd0c330ecf40b317cb5ad4195c69a7762'
def sha(data): return hashlib.sha256(data).hexdigest()
INVENTORY = sha('\n'.join(NAMES).encode())

def route_request(**change):
    return dict(dict(repo_id='mother',full_sha=SHA,gate_set_id='route-cells',inventory_sha256=INVENTORY),**change)

def route_success():
    """A complete PASS_ROUTE_CELLS summary and its artifacts, as GB1b's supervisor publishes them."""
    env = ROUTE['environment']
    files = {'a7-'+rel.replace('/','-')+'.log':rel.encode() for rel in ROUTE['a7']['evidence']}
    files.update({'a7-verdict.json.log':b'{"verdict": "pass"}\n','c1.log':b'a7\n','c3.log':b'cells\n'})
    logs = [dict(file=n,sha256=sha(d)) for n,d in files.items()]
    def done(i, gate, argv, cwd, log):
        return dict(id='c%d'%i,gate_id=gate,argv=argv,cwd=cwd,environment=dict(env),exit_code=0,signal=None,
                    timed_out=False,oom=False,status_complete=True,collector_complete=True,log=log,log_sha256=sha(files[log]))
    a7 = done(1,'a7',ROUTE['gates'][0]['argv'],'/work/src','c1.log')
    cells = done(3,'route-cells',[EXE],'/work/src/mother/host','c3.log')
    cells.update(artifact_id='route_cells',inventory=dict(expected=list(NAMES),executed=list(NAMES),passed=list(NAMES),
                                                          failed=[],ignored=[],filtered=[]))
    listing = dict(id='c2',gate_id='route-cells-list',argv=[EXE,'--list','--format','terse'],cwd='/work/src/mother/host',
                   environment=dict(env),exit_code=0,signal=None)
    summary = {k:{'observed':True} for k in receipt.GROUPS}
    summary.update(schema_version=1,run_id='b'*32,peer_uid=os.getuid(),attempts=['b'*32],status='PASS_ROUTE_CELLS',
                   request=route_request(),commands=[a7,cells],logs=logs,missing_evidence=[],limitations=[receipt.MUTABLE],
                   environment=dict(env),cleanup={'complete':True})
    summary['snapshot'] = dict(commit=SHA,content_sha256='2'*64,lockfile_sha256='3'*64)
    summary['installed'] = dict(bundle_sha256='4'*64,policy_sha256='5'*64,gates_sha256=receipt.current_policy(),stage='normal')
    rust = ROUTE['toolchain']['manifest_sha256']
    summary['input_observations'] = {'before':{'rust':rust,'registry':'6'*64},'after':{'rust':rust,'registry':'6'*64}}
    summary['coverage'] = dict(expected=['a7','route-cells'],executed=['a7','route-cells'],passed=['a7','route-cells'],
                               failed=[],ignored=[],filtered=[],excluded=[],discovery=[listing],
                               artifacts=[dict(id='route_cells',adapter='libtest',executable=EXE,
                                               package='nexus-mother-host',target='route_cells')])
    summary['lane'] = dict(a7='PASS',verdict_sha256=sha(files['a7-verdict.json.log']),rustc_vv='rustc 1.94.0',cargo_v='cargo 1.94.0',
                           evidence={n:sha(d) for n,d in files.items() if n.startswith('a7-') and n != 'a7-verdict.json.log'},
                           lockfile_sha256='3'*64,wrapper_sha256=WRAPPER,harness_sha256='7'*64,inventory_sha256=INVENTORY,
                           names=list(NAMES),trust=ROUTE['trust'])
    return summary, files

def full_success():
    case = core.ReceiptTests('test_valid_scoped_success'); case.setUp()
    try: return case.success()
    finally: case.tmp.cleanup()

def frame(request): data = json.dumps(request).encode(); return struct.pack('!I',len(data))+data

class RouteRequestControls(unittest.TestCase):
    registry = {'mother':{'uid':1234,'object_format':'sha1'}}
    def parse(self, request): return snapshot.parse_request(frame(request),self.registry,1234)
    def test_route_request_exact(self):
        self.assertEqual(self.parse(route_request()),route_request())
    def test_route_request_requires_inventory(self):
        request = route_request(); del request['inventory_sha256']
        with self.assertRaisesRegex(ValueError,'request fields'): self.parse(request)
    def test_route_request_inventory_is_lowercase_sha256(self):
        for bad in (INVENTORY.upper(),INVENTORY[:63],INVENTORY+'0',12):
            with self.subTest(bad=bad), self.assertRaisesRegex(ValueError,'request types'):
                self.parse(route_request(inventory_sha256=bad))
    def test_full_request_refuses_inventory(self):
        with self.assertRaisesRegex(ValueError,'request fields'):
            self.parse(dict(repo_id='mother',full_sha=SHA,gate_set_id='full',inventory_sha256=INVENTORY))
    def test_unknown_gate_sets_refused(self):
        for name in ('smoke','Route-cells','route_cells',None):
            with self.subTest(name=name), self.assertRaisesRegex(ValueError,'unknown gate set'):
                self.parse(route_request(gate_set_id=name))

class RouteReceiptControls(unittest.TestCase):
    def setUp(self):
        self.summary, self.files = route_success()
    def refuse(self, pattern, summary=None):
        with self.assertRaisesRegex(ValueError,pattern): receipt.validate_summary(summary or self.summary)
    def test_route_success_publishes_and_verifies(self):
        receipt.validate_summary(self.summary)
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); (root/'pending').mkdir(mode=0o700); (root/'receipts').mkdir(mode=0o755)
            path = receipt.publish(root/'pending',root/'receipts','b'*32,self.summary,self.files)
            self.assertEqual(receipt.verify_integrity(path,os.getuid())['summary']['status'],'PASS_ROUTE_CELLS')
    def test_gate_set_table(self):
        self.assertEqual({k:(v['policy'],v['status'],v['extra']) for k,v in receipt.GATE_SETS.items()},
                         {'full':('gates.json','PASS_SCOPED',()),'route-cells':('route-cells.json','PASS_ROUTE_CELLS',('lane',))})
        self.assertEqual(receipt.GATE_SETS['route-cells']['cwd'],{'a7':'/work/src','route-cells':'/work/src/mother/host'})
    def test_route_request_with_full_status_refused(self):
        self.summary['status'] = 'PASS_SCOPED'; self.refuse('status/gate set')
    def test_full_request_with_route_status_refused(self):
        summary = full_success(); summary['status'] = 'PASS_ROUTE_CELLS'; self.refuse('status/gate set',summary)
    def test_full_receipt_never_carries_lane(self):
        summary = full_success(); receipt.validate_summary(summary)
        summary['lane'] = {}; self.refuse('receipt fields',summary)
    def test_route_receipt_requires_lane(self):
        del self.summary['lane']; self.refuse('receipt fields')
    def test_lane_keys_exact(self):
        self.summary['lane']['extra'] = 'x'; self.refuse('route lane keys')
    def test_lane_a7_must_pass(self):
        self.summary['lane']['a7'] = 'FAIL'; self.refuse('route A7 verdict')
    def test_lane_wrapper_is_reviewed_literal(self):
        self.summary['lane']['wrapper_sha256'] = '8'*64; self.refuse('route wrapper')
    def test_lane_inventory_equals_request(self):
        self.summary['lane']['inventory_sha256'] = '9'*64; self.refuse('route inventory commitment')
    def test_names_hash_to_commitment(self):
        names = NAMES+['cells::rc_r3_extra_x_y']
        self.summary['lane']['names'] = list(names)
        self.summary['commands'][1]['inventory'].update(expected=list(names),executed=list(names),passed=list(names))
        self.refuse('route inventory names')
    def test_toolchain_observation_is_pinned(self):
        for when in ('before','after'): self.summary['input_observations'][when]['rust'] = 'a'*64
        self.refuse('route toolchain pin')
    def test_environment_is_route_policy(self):
        self.summary['environment'] = dict(self.summary['environment'],CARGO_HOME='/work/cargo'); self.refuse('route environment')
    def test_harness_cwd_is_set_literal(self):
        self.summary['commands'][1]['cwd'] = '/work/src'; self.refuse('route invocation')
    def test_a7_argv_is_policy(self):
        self.summary['commands'][0]['argv'] = self.summary['commands'][0]['argv'][:-1]; self.refuse('route invocation')
    def test_harness_inventory_is_lane_names(self):
        self.summary['commands'][1]['inventory']['ignored'] = [NAMES[0]]; self.refuse('route test inventory')
    def test_listing_is_recorded(self):
        self.summary['coverage']['discovery'][0]['argv'] = [EXE,'--list']; self.refuse('route discovery')
    def test_evidence_bound_to_published_logs(self):
        name = sorted(self.summary['lane']['evidence'])[0]
        self.summary['lane']['evidence'][name] = 'c'*64; self.refuse('route A7 evidence')
    def strip_logs(self, names):
        self.summary['logs'] = [log for log in self.summary['logs'] if log['file'] not in names]
    def test_r21_null_evidence_with_omitted_logs_refused(self):  # review-gb1a P1, exactly
        names = set(self.summary['lane']['evidence'])
        self.summary['lane']['evidence'] = dict.fromkeys(names); self.summary['lane']['verdict_sha256'] = None
        self.strip_logs(names|{'a7-verdict.json.log'}); self.refuse('route A7 evidence')
    def test_r21_null_evidence_hash_refused(self):
        name = sorted(self.summary['lane']['evidence'])[0]
        self.summary['lane']['evidence'][name] = None; self.strip_logs({name}); self.refuse('route A7 evidence')
    def test_r21_null_verdict_hash_refused(self):
        self.summary['lane']['verdict_sha256'] = None; self.strip_logs({'a7-verdict.json.log'}); self.refuse('route A7 evidence')
    def test_r21_omitted_evidence_log_refused(self):
        self.strip_logs({sorted(self.summary['lane']['evidence'])[0]}); self.refuse('route A7 evidence')
    def test_r21_omitted_verdict_log_refused(self):
        self.strip_logs({'a7-verdict.json.log'}); self.refuse('route A7 evidence')
    def test_evidence_set_is_policy(self):
        self.summary['lane']['evidence'].pop(sorted(self.summary['lane']['evidence'])[0]); self.refuse('route A7 evidence')
    def test_trust_statement_is_policy(self):
        self.summary['lane']['trust'] = 'trusted'; self.refuse('route trust')
    def test_trust_statement_names_r19_and_r20(self):
        for phrase in ('cannot authenticate earlier writes','reviewed harness behaviour','replace its own pathname',
                       'commitment, not execution attestation','No unpinned candidate code runs before'):
            self.assertIn(phrase,ROUTE['trust'])
    def test_artifact_inside_route_target(self):
        for bad in ('/work/target/release/deps/route_cells-0123456789abcdef',EXE+'/../x',EXE+'x'):
            summary, _ = route_success(); summary['coverage']['artifacts'][0]['executable'] = bad
            for c in [summary['commands'][1],summary['coverage']['discovery'][0]]: c['argv'][0] = bad
            with self.subTest(bad=bad): self.refuse('route coverage',summary)
    def test_failed_route_receipt_keeps_structure(self):
        self.summary.update(status='FAILED',missing_evidence=['A7: fail'],lane={})
        receipt.validate_summary(self.summary)

class HistoricalPolicyControls(unittest.TestCase):
    """SB3E-R19.5: a receipt is validated against its own recorded policy, never the current one."""
    def setUp(self):
        self.stack = ExitStack(); self.addCleanup(self.stack.close)
        self.history = Path(self.stack.enter_context(tempfile.TemporaryDirectory()))/'history'
        self.history.mkdir(mode=0o755)
        self.stack.enter_context(patch.object(receipt,'HISTORY',self.history))
        self.stack.enter_context(patch.object(receipt,'HISTORY_OWNER',os.getuid()))
        exclusions = json.loads((HERE/'policy/exclusions.json').read_text())
        exclusions['reason'] = 'an older reviewed reason'
        self.old = {'/usr/libexec/gatebox/policy/gates.json':(HERE/'policy/gates.json').read_bytes(),
                    '/usr/libexec/gatebox/policy/exclusions.json':json.dumps(exclusions).encode()}
        self.digest = self.archive(self.old)
        self.summary = full_success()
        self.summary['installed']['gates_sha256'] = self.digest
        for item in self.summary['coverage']['excluded']: item['reason'] = exclusions['reason']
    def archive(self, files):
        manifest = {p:sha(d) for p,d in files.items()}
        digest = receipt.policy_digest(manifest)
        entry = self.history/digest; entry.mkdir(mode=0o755)
        for name,data in [('manifest.json',json.dumps(manifest).encode())]+[(Path(p).name,d) for p,d in files.items()]:
            (entry/name).write_bytes(data); (entry/name).chmod(0o444)
        return digest
    def test_historical_receipt_validates_against_its_own_policy(self):
        receipt.validate_summary(self.summary)
    def test_historical_receipt_is_not_validated_by_current_policy(self):
        summary = full_success()
        for item in summary['coverage']['excluded']: item['reason'] = 'an older reviewed reason'
        with self.assertRaisesRegex(ValueError,'exclusion inventory'): receipt.validate_summary(summary)
    def test_unknown_policy_digest_refused(self):
        summary = full_success(); summary['installed']['gates_sha256'] = 'd'*64  # valid under the current policy
        with self.assertRaisesRegex(ValueError,'unknown receipt policy'): receipt.validate_summary(summary)
    def test_tampered_history_bytes_refused(self):
        path = self.history/self.digest/'exclusions.json'; path.chmod(0o644)
        path.write_bytes(path.read_bytes()+b' '); path.chmod(0o444)
        with self.assertRaisesRegex(ValueError,'historical policy bytes'): receipt.validate_summary(self.summary)
    def test_history_entry_named_by_another_digest_refused(self):
        (self.history/self.digest).rename(self.history/('e'*64)); self.summary['installed']['gates_sha256'] = 'e'*64
        with self.assertRaisesRegex(ValueError,'historical policy inventory'): receipt.validate_summary(self.summary)
    def test_writable_history_refused(self):
        (self.history/self.digest/'gates.json').chmod(0o644)
        with self.assertRaisesRegex(ValueError,'unsafe receipt ownership'): receipt.validate_summary(self.summary)
    def test_old_route_receipt_needs_its_route_policy(self):
        summary, _ = route_success(); summary['installed']['gates_sha256'] = self.digest
        with self.assertRaisesRegex(ValueError,'route-cells policy missing'): receipt.validate_summary(summary)
    def test_r21_current_policy_hashed_and_parsed_from_same_bytes(self):
        original, reads = Path.read_bytes, {}
        policy = {Path(n).name:json.loads((HERE/n).read_bytes()) for n in receipt.GATE_POLICY}
        def replaced_after_first_read(path):
            if path.parent != HERE/'policy' or 'policy/'+path.name not in receipt.GATE_POLICY: return original(path)
            reads[path] = reads.get(path,0)+1
            return original(path) if reads[path] == 1 else b'{"replaced": true}'
        digest = receipt.current_policy()
        with patch.object(Path,'read_bytes',replaced_after_first_read):
            try: parsed = receipt.policy_set(digest)
            except ValueError: return  # a digest that no longer matches fails closed
        self.assertEqual(parsed,policy)
    def staged(self):
        staged = self.history.parent/'staged'; staged.mkdir(mode=0o755); (self.history/self.digest).rename(staged/self.digest)
        self.stack.enter_context(patch.object(receipt,'STAGED_HISTORY',staged))
        return staged/self.digest
    def test_r22_absent_retained_entry_falls_back_to_staged(self):
        self.staged(); receipt.validate_summary(self.summary)
        self.history.rmdir(); receipt.validate_summary(self.summary)  # first upgrade: no history directory yet
    def test_r22_ancestry_error_does_not_fall_back_to_staged(self):
        entry = self.staged(); target = self.history/self.digest
        relink = lambda: (self.history.unlink(),self.history.mkdir(mode=0o755))
        plants = {'symlinked entry':(lambda: target.symlink_to(entry),target.unlink),'file entry':(lambda: target.write_bytes(b''),target.unlink),
                  'dangling symlink':(lambda: target.symlink_to(self.history/'absent'),target.unlink),
                  'symlinked history':(lambda: (self.history.rmdir(),self.history.symlink_to(entry.parent)),relink),
                  'unsearchable history':(lambda: self.history.chmod(0),lambda: self.history.chmod(0o755))}  # ENOTDIR, EACCES
        for name,(plant,undo) in plants.items():
            with self.subTest(plant=name):
                plant()
                try:
                    with self.assertRaisesRegex(ValueError,'unsafe receipt ancestry'): receipt.validate_summary(self.summary)
                finally: undo()
                receipt.validate_summary(self.summary)  # restored: the absent entry falls back again
    def test_current_digest_matches_installed_identity_formula(self):
        pairs = sorted((str(HERE/n),sha((HERE/n).read_bytes())) for n in receipt.GATE_POLICY)
        self.assertEqual(receipt.current_policy(),sha(json.dumps(pairs,separators=(',',':')).encode()))
        self.assertEqual(sorted(receipt.GATE_POLICY),['policy/exclusions.json','policy/gates.json','policy/route-cells.json'])

class SupervisorRouteFailure(unittest.TestCase):
    def test_route_failure_publishes_with_lane_group(self):
        supervisor = load('supervisor'); run = 'b'*32
        with tempfile.TemporaryDirectory() as tmp, ExitStack() as stack:
            root = Path(tmp)
            for name in ('staging','pending','receipts','incoming'): (root/name).mkdir(mode=0o700)
            (root/'receipts').chmod(0o755); marker = root/'pending'/('.'+run); marker.touch()
            for name, value in (('STATE',root),('CONTROL',root),('stop_and_confirm',lambda r: {'complete':True})):
                stack.enter_context(patch.object(supervisor,name,value))
            stack.enter_context(patch.object(supervisor.snapshot,'GitSource'))
            stack.enter_context(patch.object(supervisor.snapshot,'export',side_effect=ValueError('fixture export failure')))
            supervisor.execute(run,route_request(),1234,{'device':1,'inode':2},{'gates_sha256':'0'*64},marker,'normal')
            summary = supervisor.receipt.verify_integrity(root/'receipts'/run,os.getuid())['summary']
        self.assertEqual((summary['status'],summary['lane']),('FAILED',{}))

class ClientVerifyControls(unittest.TestCase):
    def client(self, argv, reply=None, summary=None):
        client = load('client'); sent = []
        def submit(request): sent.append(request); return reply or {'status':'ACCEPTED','run_id':'b'*32}
        with patch.object(client,'submit',side_effect=submit), patch.object(sys,'argv',['client.py',*argv]), \
                patch.object(client,'read_summary',return_value=summary), patch('builtins.print'):
            try: client.main(); code = 0
            except SystemExit as exc: code = exc.code
        return sent, code
    def test_client_submits_inventory_for_route_cells(self):
        sent, _ = self.client(['--repo','mother','--sha',SHA,'--gateset','route-cells','--inventory-sha256',INVENTORY])
        self.assertEqual(sent,[route_request()])
    def test_client_refuses_route_cells_without_inventory(self):
        with self.assertRaisesRegex(ValueError,'request fields'): self.client(['--repo','mother','--sha',SHA,'--gateset','route-cells'])
    def test_client_refuses_inventory_for_full(self):
        with self.assertRaisesRegex(ValueError,'request fields'):
            self.client(['--repo','mother','--sha',SHA,'--gateset','full','--inventory-sha256',INVENTORY])
    def test_client_success_status_is_per_gate_set(self):
        args = ['--repo','mother','--sha',SHA,'--gateset','route-cells','--inventory-sha256',INVENTORY,'--wait']
        self.assertEqual(self.client(args,summary={'status':'PASS_ROUTE_CELLS'})[1],0)
        self.assertEqual(self.client(args,summary={'status':'PASS_SCOPED'})[1],1)
    def verify_latest(self, argv, receipts):
        verify = load('verify'); out = []
        entries = []
        for i,n in enumerate(receipts):
            entry = Mock(**{'stat.return_value':Mock(st_mtime_ns=i)}); entry.name = n; entries.append(entry)
        root = Mock(**{'iterdir.return_value':entries})
        with patch.object(verify,'host_context'), patch.object(verify,'host_ancestry'), patch.object(verify,'ROOT',root), \
                patch.object(verify,'authenticate',side_effect=lambda n: receipts[n]), \
                patch.object(sys,'argv',['verify.py','--latest',*argv]), patch('builtins.print',side_effect=out.append):
            verify.main()
        return json.loads(out[0])
    def test_verify_latest_discovers_route_request_by_inventory(self):
        other = route_request(inventory_sha256='f'*64)
        receipts = {'a'*32:{'summary':{'request':route_request(),'status':'PASS_ROUTE_CELLS'}},
                    'c'*32:{'summary':{'request':other,'status':'PASS_ROUTE_CELLS'}}}
        found = self.verify_latest(['--repo','mother','--sha',SHA,'--gateset','route-cells','--inventory-sha256',INVENTORY],receipts)
        self.assertEqual(found['summary']['request'],route_request())
    def test_verify_identity_current_or_historical(self):
        verify = load('verify'); summary = full_success()
        identity = {k:summary['installed'][k] for k in ('bundle_sha256','policy_sha256','gates_sha256')}
        with patch.object(verify,'host_context'), patch.object(verify,'host_ancestry'), \
                patch.object(verify,'installed_identity',return_value=identity), \
                patch.object(verify.receipt,'verify_integrity',return_value={'authentication':'UNAUTHENTICATED','summary':summary}), \
                patch.object(verify.pwd,'getpwnam',return_value=Mock(pw_uid=979)):
            self.assertEqual(verify.authenticate('a'*32)['identity'],'current')
            summary['installed']['bundle_sha256'] = '0'*64
            self.assertEqual(verify.authenticate('a'*32)['identity'],'historical')
    def test_verifier_is_the_least_privilege_authentication_path(self):
        self.assertTrue((HERE/'verify.py').read_text().startswith('#!/usr/bin/python3 -IB\n'))
        rules = [l for l in (HERE/'policy/sudoers').read_text().splitlines() if 'verify.py' in l]
        self.assertEqual(rules,['nexus ALL=(root) NOPASSWD: NOSETENV: /usr/libexec/gatebox/verify.py --authenticate-stdin'])
    def test_r21_verifier_import_writes_no_bytecode(self):
        with tempfile.TemporaryDirectory(dir=os.environ.get('TMPDIR')) as tmp:
            for name in ('verify.py','receipt.py'): (Path(tmp)/name).write_bytes((HERE/name).read_bytes())
            (Path(tmp)/'verify.py').chmod(0o755)
            proc = subprocess.run([str(Path(tmp)/'verify.py'),'--unknown-option'],env={'PATH':'/usr/bin:/bin','LANG':'C.UTF-8'},
                                  stdin=subprocess.DEVNULL,capture_output=True,timeout=60)  # executed by its own shebang
            self.assertNotEqual(proc.returncode,0)
            self.assertEqual(sorted(p.name for p in Path(tmp).iterdir()),['receipt.py','verify.py'])
    def stdin_mode(self, argv, data):
        verify, out = load('verify'), []
        with patch.object(verify,'authenticate',side_effect=lambda run: {'authentication':'AUTHENTICATED','run':run}) as auth, \
                patch.object(sys,'argv',['verify.py',*argv]), patch.object(sys,'stdin',Mock(buffer=io.BytesIO(data))), \
                patch('builtins.print',side_effect=out.append):
            verify.main()
        return auth, out
    def test_r21_authenticate_stdin_reads_one_run_id(self):
        for data in (b'a'*32,b'a'*32+b'\n'):
            auth, out = self.stdin_mode(['--authenticate-stdin'],data)
            auth.assert_called_once_with('a'*32)
            self.assertEqual([json.loads(o) for o in out],[{'authentication':'AUTHENTICATED','run':'a'*32}])
    def test_r21_authenticate_stdin_refuses_other_arguments(self):
        for argv in (['--authenticate-stdin','x'],['--authenticate-stdin','--run-id','a'*32],['--help','--authenticate-stdin'],
                     ['--authenticate-stdin','--authenticate-stdin'],['--authenticate-stdin','--require-status','PASS_SCOPED']):
            with self.subTest(argv=argv), self.assertRaises(SystemExit) as raised:
                self.stdin_mode(argv,b'a'*32)
            self.assertNotIn(raised.exception.code,(0,None))
    def test_r21_authenticate_stdin_refuses_malformed_oversized_or_multiple_ids(self):
        for data in (b'',b'\n',b'A'*32,b'a'*31,b'a'*33,b'a'*32+b'\n\n',b'a'*32+b'\r\n',b'a'*32+b' ',b'a'*32+b'\n'+b'b'*32,
                     b'a'*4096,b'../'+b'a'*29,b'../../etc/passwd',b'a'*16+b'/'+b'a'*15,b'\xff'*32):
            with self.subTest(data=data[:40]), self.assertRaisesRegex(ValueError,'^invalid receipt ID$'):
                self.stdin_mode(['--authenticate-stdin'],data)
    def test_r21_symlinked_or_traversing_run_id_refused(self):
        verify = load('verify')
        with tempfile.TemporaryDirectory() as tmp, patch.object(verify,'host_context'), patch.object(verify,'host_ancestry'), \
                patch.object(verify,'ROOT',Path(tmp)), patch.object(verify.pwd,'getpwnam',return_value=Mock(pw_uid=os.getuid())):
            real = Path(tmp)/('b'*32); real.mkdir()
            (Path(tmp)/('a'*32)).symlink_to(real)
            with self.assertRaisesRegex(ValueError,'unsafe receipt directory'): verify.authenticate('a'*32)
            for run in ('../'+'a'*29,'a'*32+'/..','.'):
                with self.subTest(run=run), self.assertRaisesRegex(ValueError,'invalid receipt ID'): verify.authenticate(run)

class InstallControls(unittest.TestCase):
    def test_files_are_the_tracked_tree(self):
        files = installer()['FILES']
        self.assertTrue({'policy/route-cells.json','route/route_a7.py','route/route_a7_graph.py','route/route_rustc.c'} <= set(files))
        if not INSTALLED:
            tracked = subprocess.run(['git','ls-files','--','.'],cwd=HERE,capture_output=True,text=True,check=True).stdout.split()
            self.assertEqual(sorted(files),sorted(tracked))
    def bundle(self, root, wrapper):
        scope = installer(); archive = root/'bundle.tar'
        harness = scope['HARNESS']; source = core.HERE.parents[1]/harness if not INSTALLED else HERE/'policy/phase2_live_sandbox.rs'
        with tarfile.open(archive,'w') as tar:
            members = [('tools/gatebox/'+n,(HERE/n).read_bytes()) for n in scope['FILES']]+[(harness,source.read_bytes())]
            if wrapper is not None: members.append(('tools/gatebox/'+scope['WRAPPER'],wrapper))
            for name,data in members:
                item = tarfile.TarInfo(name); item.size = len(data); tar.addfile(item,io.BytesIO(data))
        return scope, archive, sha(archive.read_bytes())
    @unittest.skipIf(INSTALLED or not Path('/usr/bin/cc').exists(),'the PIN step builds the wrapper in the checkout')
    def test_wrapper_rebuilds_to_reviewed_literal(self):
        argv = ['/usr/bin/cc','-std=c11','-O2','-Wall','-Wextra','-Werror','-static','-o','tools/gatebox/route/route_rustc',
                'tools/gatebox/route/route_rustc.c']
        self.assertIn("['/usr/bin/cc','-std=c11','-O2','-Wall','-Wextra','-Werror','-static','-o',wrapper,",(HERE/'README.md').read_text())
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)/'tools/gatebox/route'; out.mkdir(parents=True)
            (out/'route_rustc.c').write_bytes((HERE/'route/route_rustc.c').read_bytes())
            subprocess.run(argv,cwd=tmp,env={'PATH':'/usr/bin:/bin','LANG':'C'},check=True)
            self.assertEqual(sha((out/'route_rustc').read_bytes()),installer()['WRAPPER_SHA256'])
    def test_bundle_requires_wrapper_member(self):
        with tempfile.TemporaryDirectory() as tmp:
            scope, archive, digest = self.bundle(Path(tmp),None)
            with self.assertRaisesRegex(ValueError,'incomplete bundle'): scope['bundle_files'](archive,digest)
    def test_bundle_wrapper_is_reviewed_hash(self):
        with tempfile.TemporaryDirectory() as tmp:
            scope, archive, digest = self.bundle(Path(tmp),b'\x7fELF not the reviewed wrapper')
            self.assertEqual(scope['WRAPPER_SHA256'],WRAPPER)
            with self.assertRaisesRegex(ValueError,'wrapper SHA-256'): scope['bundle_files'](archive,digest)
    def test_payload_policies_include_route_cells(self):
        files = {'tools/gatebox/policy/'+n:(HERE/'policy'/n).read_bytes() for n in ('gates.json','exclusions.json','route-cells.json')}
        self.assertEqual(set(installer()['payload_policies'](files)),{'gates.json','exclusions.json','route-cells.json'})
    def test_toolchain_pin_checked_at_install(self):
        scope = installer(); files = {'tools/gatebox/'+n:(HERE/n).read_bytes() for n in ('snapshot.py','policy/route-cells.json')}
        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp)/'bin').mkdir(); (Path(tmp)/'bin/rustc').write_bytes(b'not the pinned toolchain')
            with self.assertRaisesRegex(ValueError,'reviewed /opt/rust pin'): scope['toolchain_check'](files,Path(tmp))
    def test_identity_covers_route_policy_and_code(self):
        text = (HERE/'probe.py').read_text()
        for prefix in ("'/etc/gatebox/route-cells.json'","str(HERE/'route')","str(HERE/'policy/route-cells.json')"):
            self.assertIn(prefix,text)
    def test_run_unit_core_limit_is_one_byte(self):
        unit = (HERE/'units/mothergate-run@.service').read_text()
        self.assertIn('\nLimitCORE=1\n',unit); self.assertNotIn('LimitCORE=0',unit)
        self.assertIn(['LimitCORE=0\n','LimitCORE=1\n'],json.loads((HERE/'policy/custody.json').read_text())['units']['mothergate-run@.service'])
        self.assertIn('\nLimitCORE=0\n',(HERE/'units/mothergate.service').read_text())
    def test_verify_installed_executable(self):
        self.assertIn('verify.py',installer()['EXECUTABLE'])
        self.assertIn("0o755 if name in EXECUTABLE else 0o644",(HERE/'install.sh').read_text().split('def main():',1)[1])

class UpgradeControls(unittest.TestCase):
    """SB3E-R19.5 prerequisites, each refused on its own; nothing under the test root is repaired or removed."""
    OLD, NEW = 'a'*64, 'b'*64
    def setUp(self):
        self.stack = ExitStack(); self.addCleanup(self.stack.close)
        self.root = root = Path(self.stack.enter_context(tempfile.TemporaryDirectory()))
        self.scope = scope = installer()
        for name in ('verify_old_install','require_settled','require_no_custody','state_ownership','receipt_inventory'):
            self.assertIn(name,scope,'upgrade lacks '+name)
        self.paths = dict(DEST=root/'dest',CONFIG=root/'config',STATE=root/'state',CONTROL=root/'control')
        for path in self.paths.values(): path.mkdir()
        for name in ('staging','pending','receipts'): (root/'state'/name).mkdir()
        for name in ('incoming','sealed'): (root/'control'/name).mkdir()
        (root/'dest/probe.py').write_bytes(b'old')
        inventory = {str(root/'dest/probe.py'):sha(b'old')}
        (root/'config/installed-files.json').write_text(json.dumps(inventory))
        (root/'config/bundle.json').write_text(json.dumps({'sha256':self.OLD}))
        self.identity = {'bundle_sha256':self.OLD,'policy_sha256':'c'*64,'gates_sha256':'d'*64}
        (root/'config/identity.json').write_text(json.dumps(self.identity))
        self.calls = []
        self.stack.enter_context(patch.dict(scope,protected=lambda p: None,**self.paths))
    def old(self, identity=None):
        return self.scope['verify_old_install'](self.OLD,lambda: identity or self.identity)
    def test_verified_old_install(self):
        self.assertEqual(self.old()[0],self.identity)
    def test_old_bundle_must_be_recorded(self):
        with self.assertRaisesRegex(ValueError,'not the reviewed old bundle'): self.scope['verify_old_install'](self.NEW,lambda: self.identity)
    def test_installed_bytes_must_be_recorded(self):
        (self.root/'dest/probe.py').write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError,'installed bytes changed'): self.old()
    def test_unrecorded_installed_file_refused(self):
        (self.root/'dest/extra.py').write_bytes(b'')
        with self.assertRaisesRegex(ValueError,'unrecorded installed file'): self.old()
    def test_identity_must_be_recomputed_record(self):
        with self.assertRaisesRegex(ValueError,'identity is not its record'): self.old(dict(self.identity,policy_sha256='0'*64))
    RUN_UNIT, ROOT_UNIT = 'mothergate-run@'+'a'*32+'.service', 'mothergate-root@'+'a'*32+'.service'
    def settled(self, units='', **states):
        cgroup = self.root/'cgroup'; (cgroup/'system.slice').mkdir(parents=True,exist_ok=True)
        def run(argv):
            self.calls.append(argv)
            if argv[1] == 'list-units': return units.encode()
            if argv[1] != 'show': return b''
            unit = argv[-1]
            state = dict(Id=unit,Job='',ActiveState='inactive',ControlGroup=''); state.update(states.get(unit,{}))
            return ''.join(f'{k}={v}\n' for k,v in state.items() if v is not None).encode()
        with patch.dict(self.scope,run=run,CGROUP=cgroup): self.scope['require_settled']()
    # R22.1: this host's layout (systemctl show Slice/ControlGroup, /sys/fs/cgroup); see the conformance test below.
    SLICE = {RUN_UNIT:'system-mothergate\\x2drun.slice',ROOT_UNIT:'system-mothergate\\x2droot.slice'}
    def group(self, unit): return '/system.slice/'+(self.SLICE[unit]+'/' if unit in self.SLICE else '')+unit
    def populated(self, group, value):
        path = self.root/'cgroup'/group.lstrip('/'); path.mkdir(parents=True,exist_ok=True)
        for level in [path]+[p for p in path.parents if self.root/'cgroup/system.slice' in p.parents]:  # recursive, as cgroupfs
            (level/'cgroup.events').write_text(f'populated {value}\nfrozen 0\n')
    def test_services_stopped_and_settled(self):
        listed = self.RUN_UNIT+' loaded failed failed run\n'+self.ROOT_UNIT+' loaded inactive dead root\n'
        self.settled(listed)
        self.assertEqual(self.calls[0],['/usr/bin/systemctl','stop','mothergate.socket','mothergate.service'])
        self.assertEqual([c[-1] for c in self.calls if c[1] == 'show'],['mothergate.socket','mothergate.service',self.RUN_UNIT,self.ROOT_UNIT])
        for unit in ('mothergate.socket','mothergate.service'):
            for bad in (dict(ActiveState='active'),dict(Job='12'),dict(Id='other.service'),dict(ControlGroup=None)):
                with self.subTest(unit=unit,bad=bad), self.assertRaisesRegex(ValueError,'unit not settled: '+unit): self.settled(**{unit:bad})
        with self.assertRaisesRegex(ValueError,'unit not settled'): self.settled(self.ROOT_UNIT+' loaded active running r\n',**{self.ROOT_UNIT:dict(ActiveState='active')})
        with self.assertRaisesRegex(ValueError,'unexpected run unit'): self.settled('mothergate-run@x.service loaded failed failed run\n')
    def test_r21_every_instance_job_state_checked(self):
        for unit in (self.RUN_UNIT,self.ROOT_UNIT):
            with self.subTest(unit=unit), self.assertRaisesRegex(ValueError,'unit not settled: '+unit):
                self.settled(unit+' loaded inactive dead x\n',**{unit:dict(Job='41')})
    def test_r21_every_instance_cgroup_empty(self):
        for unit in (self.RUN_UNIT,self.ROOT_UNIT):
            with self.subTest(unit=unit):
                listed, nested = unit+' loaded failed failed x\n', {unit:dict(ControlGroup=self.group(unit))}
                self.populated(self.group(unit),1)
                with self.assertRaisesRegex(ValueError,'cgroup populated: '+unit): self.settled(listed,**nested)
                with self.assertRaisesRegex(ValueError,'cgroup populated: '+unit): self.settled()  # an unlisted instance's cgroup
                self.populated(self.group(unit),0); self.settled(listed,**nested)
                self.populated('/system.slice/'+unit,1)  # a stray flat group of an instance's name is still scanned
                with self.assertRaisesRegex(ValueError,'cgroup populated: '+unit): self.settled()
                self.populated('/system.slice/'+unit,0)
        for unit in ('mothergate.socket','mothergate.service'):
            self.populated(self.group(unit),1)
            with self.subTest(unit=unit), self.assertRaisesRegex(ValueError,'cgroup populated: '+unit): self.settled(**{unit:dict(ControlGroup=self.group(unit))})
            self.populated(self.group(unit),0)
        for group in ('/system.slice/other.service','/system.slice/'+self.RUN_UNIT,self.group(self.ROOT_UNIT)):
            with self.subTest(group=group), self.assertRaisesRegex(ValueError,'unit not settled'):
                self.settled(self.RUN_UNIT+' loaded failed failed x\n',**{self.RUN_UNIT:dict(ControlGroup=group)})
    def test_r22_template_slice_trees_scanned(self):
        for slice_ in self.SLICE.values():
            for child in ('mothergate-run@'+'b'*32+'.service','stray.scope','unrelated'):  # unlisted or foreign groups
                with self.subTest(slice_=slice_,child=child):
                    self.populated('/system.slice/'+slice_+'/'+child,1)
                    with self.assertRaisesRegex(ValueError,'cgroup populated: '+re.escape(child)): self.settled()
                    self.populated('/system.slice/'+slice_+'/'+child,0)
            with self.subTest(slice_=slice_):
                self.settled()
                self.populated('/system.slice/'+slice_,1)  # populated is recursive: any deeper group in the tree
                with self.assertRaisesRegex(ValueError,'cgroup populated: '+re.escape(slice_)): self.settled()
                (self.root/'cgroup/system.slice'/slice_/'cgroup.events').unlink()
                with self.assertRaisesRegex(ValueError,'unresolved cgroup evidence: '+re.escape(slice_)): self.settled()
                self.populated('/system.slice/'+slice_,0); self.settled()
    def test_r22_fixture_is_this_hosts_layout(self):
        # Read-only conformance of the fixture above with systemd on a host that has the Gate Box installed.
        if not shutil.which('systemctl') or not all(Path('/sys/fs/cgroup/system.slice',s).is_dir() for s in self.SLICE.values()):
            self.skipTest('no installed Gate Box')
        show = lambda unit,prop: subprocess.run(['systemctl','show','-P',prop,unit],check=True,capture_output=True,text=True).stdout.strip()
        for unit,slice_ in self.SLICE.items():
            self.assertEqual(show(slice_,'ControlGroup'),'/system.slice/'+slice_)
            self.assertEqual(show(unit,'Slice'),slice_)
            self.assertEqual(self.group(unit).rpartition('/')[0],'/system.slice/'+slice_)
        for unit in ('mothergate.service','mothergate.socket'): self.assertEqual(show(unit,'Slice'),'system.slice')
    def used(self, kind='file', mode=0o600):
        path = self.root/'control/sealed'/('.used-'+'a'*32)
        if kind == 'file': path.write_bytes(b''); path.chmod(mode)
        elif kind == 'directory': path.mkdir()
        elif kind == 'symlink': (self.root/'target').write_bytes(b''); (self.root/'target').chmod(0o600); path.symlink_to(self.root/'target')
        elif kind == 'fifo': os.mkfifo(path,0o600)
        elif kind == 'hardlink': path.write_bytes(b''); path.chmod(0o600); os.link(path,self.root/'second')
        return path
    def test_r21_used_marker_is_a_strict_root_custody_file(self):
        with patch.dict(self.scope,CUSTODY_OWNER=(os.getuid(),os.getgid())):
            self.used(); self.scope['require_no_custody']()
            for kind, mode in (('directory',0o600),('symlink',0o600),('fifo',0o600),('hardlink',0o600),('file',0o644),('file',0o400)):
                with self.subTest(kind=kind,mode=oct(mode)):
                    for p in (self.root/'control/sealed'/('.used-'+'a'*32),self.root/'second',self.root/'target'):
                        if p.is_dir() and not p.is_symlink(): p.rmdir()
                        elif p.exists() or p.is_symlink(): p.unlink()
                    path = self.used(kind,mode)
                    with self.assertRaisesRegex(ValueError,'unsafe custody marker'): self.scope['require_no_custody']()
                    self.assertTrue(path.exists() or path.is_symlink())
        (self.root/'control/sealed'/('.used-'+'a'*32)).unlink(); self.used()
        with self.assertRaisesRegex(ValueError,'unsafe custody marker'): self.scope['require_no_custody']()  # owner must be root
    def test_pending_and_custody_state_refused_and_retained(self):
        self.scope['require_no_custody']()
        for rel in ('state/pending/.'+'a'*32,'state/staging/'+'a'*32,'control/incoming/'+'a'*32+'.log.sock',
                    'control/sealed/.pending-'+'a'*32,'control/sealed/'+'a'*32):
            with self.subTest(rel=rel):
                (self.root/rel).write_bytes(b'retain')
                with self.assertRaisesRegex(ValueError,'retained custody or pending state'): self.scope['require_no_custody']()
                self.assertEqual((self.root/rel).read_bytes(),b'retain'); (self.root/rel).unlink()
    def test_missing_control_is_not_absence_of_custody(self):
        (self.root/'control/sealed').rmdir()
        with self.assertRaises(OSError): self.scope['require_no_custody']()
    def test_state_ownership_unchanged(self):
        uid, gid = os.getuid(), os.getgid()
        for path,mode in ((self.root/'state',0o755),(self.root/'state/staging',0o700),(self.root/'state/pending',0o700),(self.root/'state/receipts',0o755)):
            path.chmod(mode)
        with patch.dict(self.scope,pwd=Mock(getpwnam=lambda n: Mock(pw_uid=uid,pw_gid=gid))):
            with self.assertRaisesRegex(ValueError,'state ownership changed'): self.scope['state_ownership']()  # root not 0:0
            with patch.dict(self.scope,STATE_ROOT_OWNER=(uid,gid)):
                before = self.scope['state_ownership']()
                (self.root/'state/receipts').chmod(0o775)
                with self.assertRaisesRegex(ValueError,'state ownership changed'): self.scope['state_ownership']()
        self.assertTrue(before)
    def test_receipt_inventory_detects_byte_and_seal_change(self):
        run = self.root/'state/receipts'/('a'*32); run.mkdir()
        (run/'seal.json').write_bytes(b'{}'); (run/'summary.json').write_bytes(b'{}')
        before = self.scope['receipt_inventory'](self.root/'state/receipts')
        (run/'seal.json').write_bytes(b'{ }')
        self.assertNotEqual(self.scope['receipt_inventory'](self.root/'state/receipts'),before)
    def test_upgrade_is_separate_and_install_still_refuses_outputs(self):
        text = (HERE/'install.sh').read_text()
        self.assertIn("choices=['install','enable-qualified','upgrade']",text)
        self.assertIn("raise ValueError('existing output needs review: '+str(path))",text)
        upgrade = text.split('def upgrade_locked(',1)[1].split('\ndef ',1)[0]
        for step in ('verify_old_install(','require_settled()','require_no_custody()','state_ownership()','receipt_inventory(',
                     'authenticate_retained(','PLACEHOLDER',"'stage':'qualifying'"):
            self.assertIn(step,upgrade)
        self.assertLess(upgrade.index('authenticate_retained('),upgrade.index('replace('))

class UpgradeComposition(unittest.TestCase):
    """The upgrade action end to end on a temporary layout: host commands, accounts, unit staging and bundle
    verification are mocked here (each has its own control); custody, receipts and history are real inodes."""
    OLD, NEW = 'a'*64, 'b'*64
    def setUp(self):
        self.stack = ExitStack(); self.addCleanup(self.stack.close)
        root = self.root = Path(self.stack.enter_context(tempfile.TemporaryDirectory()))
        uid, gid = os.getuid(), os.getgid()
        self.scope = scope = installer()
        dest, config, state, control, boot = (root/n for n in ('dest','config','state','control','boot'))
        for path in (dest/'policy',dest/'units',config,root/'etc',root/'tmpfiles',control/'incoming',control/'sealed'): path.mkdir(parents=True)
        for name,mode in (('',0o755),('staging',0o700),('pending',0o700),('receipts',0o755)):
            (state/name).mkdir(exist_ok=True); (state/name).chmod(mode)
        (control/'sealed'/('.used-'+'c'*32)).write_bytes(b'')
        exclusions = json.loads((HERE/'policy/exclusions.json').read_text()); exclusions['reason'] = 'an older reviewed reason'
        old = {'probe.py':b'old probe','policy/gates.json':(HERE/'policy/gates.json').read_bytes(),
               'policy/exclusions.json':json.dumps(exclusions).encode(),'policy/phase2_live_sandbox.rs':b'harness',
               'units/mothergate-run@.service':(HERE/'units/mothergate-run@.service').read_bytes()}
        for name,data in old.items(): (dest/name).write_bytes(data)
        render = scope['render']((dest/'units/mothergate-run@.service').read_text(),uid,gid)
        (root/'etc/run.service').write_text(render)
        inventory = {str(dest/n):sha(d) for n,d in old.items()}
        self.gates = receipt.policy_digest({p:h for p,h in inventory.items() if p.endswith(('gates.json','exclusions.json'))})
        self.identity = dict(bundle_sha256=self.OLD,policy_sha256='c'*64,gates_sha256=self.gates)
        for name,value in (('bundle.json',{'sha256':self.OLD}),('installed-files.json',inventory),('identity.json',self.identity),
                           ('qualification.json',{'identity':self.identity,'passed':['B1']}),('acceptance-'+'d'*32+'.json',{}),
                           ('admission.json',{'stage':'normal','identity':self.identity}),('skeleton.json',{}),('registry.json',{}),
                           ('created-accounts.json',{})):
            (config/name).write_text(json.dumps(value))
        (root/'tmpfiles/mothergate-control.conf').write_bytes(b'old')
        self.files = {'tools/gatebox/'+n:(HERE/n).read_bytes() for n in scope['FILES']}
        self.files.update({'tools/gatebox/'+scope['WRAPPER']:b'wrapper',scope['HARNESS']:b'harness'})
        for name in scope['FILES']+[scope['WRAPPER']]:
            (boot/'tools/gatebox'/name).parent.mkdir(parents=True,exist_ok=True)
            (boot/'tools/gatebox'/name).write_bytes(self.files['tools/gatebox/'+name])
        self.publish_old(exclusions['reason'])
        self.receipts = scope['receipt_inventory'](state/'receipts')
        accounts = Mock(getpwnam=lambda n: Mock(pw_uid=uid,pw_gid=gid))
        entry = Mock(pw_uid=uid,pw_gid=gid)
        self.new_identity = dict(bundle_sha256=self.NEW,policy_sha256='e'*64,gates_sha256='f'*64)
        def module(path, name):
            if name.endswith('probe'):
                value = self.identity if name == 'gbx_old_probe' else self.new_identity
                return Mock(installed_identity=lambda: value)
            loaded = load('receipt'); loaded.HERE = Path(path).parent; loaded.HISTORY_OWNER = uid
            if name == 'gbx_installed_receipt': loaded.HISTORY = config/'history'
            return loaded
        self.run = Mock(side_effect=lambda argv: f'Id={argv[-1]}\nJob=\nActiveState=inactive\nControlGroup=\n'.encode()
                        if argv[1] == 'show' else b'')
        self.stack.enter_context(patch.dict(scope,DEST=dest,CONFIG=config,STATE=state,CONTROL=control,UPGRADE_BOOT=boot,
            HISTORY=config/'history',ETC=root/'etc',RUN_UNIT=root/'etc/run.service',CONTROL_POLICY=root/'tmpfiles/mothergate-control.conf',
            STATE_ROOT_OWNER=(uid,gid),protected=lambda p: None,run=self.run,pwd=accounts,toolchain_check=lambda f: None,
            bundle_files=lambda path,digest: self.files,plan_accounts=lambda: [(entry,False),(entry,False)],
            stage_security=lambda *a: [],load_module=module,LOCK_OWNER=(uid,gid),CUSTODY_OWNER=(uid,gid),
            MAINTENANCE_TIMEOUT=.2,CGROUP=root/'cgroup'))
        (control/'sealed'/('.used-'+'c'*32)).chmod(0o600)
    def publish_old(self, reason):
        old = load('receipt'); old.HISTORY_OWNER = os.getuid()
        history = self.root/'oldhistory'; entry = history/self.gates; entry.mkdir(parents=True); entry.chmod(0o755)
        manifest = {str(self.root/'dest'/n):sha((self.root/'dest'/n).read_bytes()) for n in ('policy/gates.json','policy/exclusions.json')}
        for name,data in [('manifest.json',json.dumps(manifest).encode())]+[(Path(p).name,Path(p).read_bytes()) for p in manifest]:
            (entry/name).write_bytes(data); (entry/name).chmod(0o444)
        old.HISTORY = history
        summary = full_success(); summary['installed'] = dict(self.identity,stage='normal')
        for item in summary['coverage']['excluded']: item['reason'] = reason
        pending = self.root/'oldpending'; pending.mkdir(mode=0o700)
        old.publish(pending,self.root/'state/receipts','a'*32,summary,{'run.log':b'failure\n'})
    def held(self, lock='control/lifecycle.lock'):
        fd = os.open(self.root/lock,os.O_RDWR|os.O_CREAT,0o600); self.addCleanup(os.close,fd)
        os.chmod(self.root/lock,0o600)
        return fd
    def lock_is_held(self):
        held = []
        for lock in ('control/lifecycle.lock','config/qualification.lock'):
            fd = os.open(self.root/lock,os.O_RDWR)
            try: fcntl.flock(fd,fcntl.LOCK_EX|fcntl.LOCK_NB); held.append(False)
            except BlockingIOError: held.append(True)
            finally: os.close(fd)
        return all(held) if any(held) else False
    def upgrade(self):
        with patch('builtins.print'): self.scope['upgrade'](Mock(bundle_sha256=self.NEW,old_bundle_sha256=self.OLD))
    def test_upgrade_preserves_receipts_and_requires_requalification(self):
        self.upgrade()
        config, dest = self.root/'config', self.root/'dest'
        self.assertEqual(json.loads((config/'admission.json').read_text()),{'stage':'qualifying','identity':self.new_identity})
        self.assertEqual(json.loads((config/'qualification.json').read_text()),self.scope['PLACEHOLDER'])
        archive = config/'history'/('install-'+self.OLD)
        self.assertEqual(json.loads((archive/'qualification.json').read_text())['passed'],['B1'])
        self.assertEqual(sorted(p.name for p in (config/'history'/self.gates).iterdir()),['exclusions.json','gates.json','manifest.json'])
        self.assertEqual(self.scope['receipt_inventory'](self.root/'state/receipts'),self.receipts)
        installed = json.loads((config/'installed-files.json').read_text())
        self.assertEqual((dest/'route/route_rustc').stat().st_mode & 0o777,0o755)
        self.assertIn(str(dest/'route/route_rustc'),installed); self.assertIn(str(config/'route-cells.json'),installed)
        self.assertEqual((dest/'route/route_a7.py').read_bytes(),(HERE/'route/route_a7.py').read_bytes())
        self.assertEqual(self.run.call_args_list[0].args[0],['/usr/bin/systemctl','stop','mothergate.socket','mothergate.service'])
    def test_r21_upgrade_refused_while_lifecycle_or_qualification_holds_the_lock(self):
        for lock in ('control/lifecycle.lock','config/qualification.lock'):
            with self.subTest(lock=lock):
                fd = self.held(lock); fcntl.flock(fd,fcntl.LOCK_EX)
                with self.assertRaisesRegex(TimeoutError,'maintenance lock held: '+Path(lock).name): self.upgrade()
                fcntl.flock(fd,fcntl.LOCK_UN)
                self.run.assert_not_called()
                self.assertEqual(json.loads((self.root/'config/bundle.json').read_text()),{'sha256':self.OLD})
    def test_r21_lock_held_from_first_check_through_final_recheck(self):
        seen, run, module = [], self.run.side_effect, self.scope['load_module']
        def observed_run(argv): seen.append((argv[1],self.lock_is_held())); return run(argv)
        def observed_module(path, name): seen.append((name,self.lock_is_held())); return module(path,name)
        with patch.dict(self.scope,run=observed_run,load_module=observed_module): self.upgrade()
        self.assertEqual(seen[0],('gbx_old_probe',True))  # verify_old_install: the first check
        self.assertIn(('daemon-reload',True),seen); self.assertEqual(seen[-1],('gbx_installed_receipt',True))
        self.assertTrue(all(held for _,held in seen),seen)
        self.assertFalse(self.lock_is_held())
    def test_r21_enable_qualified_takes_the_maintenance_lock(self):
        for lock, before in (('control/lifecycle.lock',['control']),('config/qualification.lock',['control','config'])):
            with self.subTest(lock=lock):
                fd = self.held(lock); fcntl.flock(fd,fcntl.LOCK_EX)
                protected = Mock()
                with patch.dict(self.scope,protected=protected), self.assertRaisesRegex(TimeoutError,'maintenance lock held: '+Path(lock).name):
                    self.scope['enable_qualified']()
                fcntl.flock(fd,fcntl.LOCK_UN)
                self.assertEqual(protected.call_args_list,[((self.root/d,),) for d in before])  # nothing read before the locks
    def test_r21_unsafe_lifecycle_lock_refused(self):
        for lock in ('control/lifecycle.lock','config/qualification.lock'):
            with self.subTest(lock=lock):
                self.held(lock); (self.root/lock).chmod(0o644)
                with self.assertRaisesRegex(ValueError,'unsafe maintenance lock: '+Path(lock).name): self.upgrade()
                (self.root/lock).chmod(0o600)
                self.run.assert_not_called()
    def second_generation(self):
        self.upgrade()
        config = self.root/'config'
        installed = json.loads((config/'installed-files.json').read_text())
        policy = {str(self.root/'dest'/n) for n in receipt.GATE_POLICY}
        gates = receipt.policy_digest({p:h for p,h in installed.items() if p in policy})
        self.identity = dict(self.new_identity,gates_sha256=gates)
        (config/'identity.json').write_text(json.dumps(self.identity))
        self.new_identity = dict(bundle_sha256='c'*64,policy_sha256='e'*64,gates_sha256='0'*64)
        return gates
    def test_r21_receipts_from_two_installations_back_still_authenticate(self):
        gates = self.second_generation()
        with patch('builtins.print'): self.scope['upgrade'](Mock(bundle_sha256='c'*64,old_bundle_sha256=self.NEW))
        history = self.root/'config/history'
        self.assertEqual(sorted(p.name for p in history.iterdir()),sorted([self.gates,gates,'install-'+self.OLD,'install-'+self.NEW]))
        self.assertEqual(self.scope['receipt_inventory'](self.root/'state/receipts'),self.receipts)
        retained = load('receipt'); retained.HISTORY, retained.HISTORY_OWNER = history, os.getuid()
        self.assertEqual(retained.verify_integrity(self.root/'state/receipts'/('a'*32),os.getuid())['summary']['installed']['gates_sha256'],self.gates)
    def retained_copy(self):
        history = self.root/'config/history'; history.mkdir(mode=0o755)
        shutil.copytree(self.root/'oldhistory'/self.gates,history/self.gates)
        return history/self.gates
    def test_r21_identical_retained_history_entry_is_reused(self):
        entry = self.retained_copy(); before = {p.name:(p.stat().st_ino,p.read_bytes()) for p in entry.iterdir()}
        self.upgrade()
        self.assertEqual({p.name:(p.stat().st_ino,p.read_bytes()) for p in entry.iterdir()},before)
    def test_r21_differing_retained_history_entry_refused_before_mutation(self):
        entry = self.retained_copy(); (entry/'exclusions.json').chmod(0o644)
        (entry/'exclusions.json').write_text('{}'); (entry/'exclusions.json').chmod(0o444)
        with self.assertRaisesRegex(ValueError,'historical policy bytes'): self.upgrade()
        self.assertEqual((self.root/'dest/probe.py').read_bytes(),b'old probe')
        self.assertEqual(json.loads((self.root/'config/admission.json').read_text())['stage'],'normal')
    def test_r21_differing_unreferenced_history_entry_refused_before_mutation(self):
        shutil.rmtree(self.root/'state/receipts'/('a'*32))  # no receipt reads the entry: only the reuse comparison sees it
        entry = self.retained_copy(); (entry/'exclusions.json').chmod(0o644)
        (entry/'exclusions.json').write_text('{}'); (entry/'exclusions.json').chmod(0o444)
        with self.assertRaisesRegex(ValueError,'retained historical policy differs'): self.upgrade()
        self.assertEqual((self.root/'dest/probe.py').read_bytes(),b'old probe')
        self.assertFalse((self.root/'config/history'/('install-'+self.OLD)).exists())
    def unsafe_retained_entry(self, plant):
        source, history = self.root/'oldhistory'/self.gates, self.root/'config/history'
        shutil.rmtree(self.root/'state/receipts'/('a'*32))  # no receipt reads the entry: only the reuse validation sees it
        history.mkdir(mode=0o755); entry = history/self.gates; shutil.copytree(source,entry)
        plant(entry,source); security = Mock(return_value=[])  # identical bytes in every plant
        with patch.dict(self.scope,stage_security=security), self.assertRaisesRegex(ValueError,'unsafe retained historical policy: '+self.gates):
            self.upgrade()
        security.assert_not_called()  # preflight: before staging and before any mutation
        self.assertEqual(((self.root/'dest/probe.py').read_bytes(),(history/('install-'+self.OLD)).exists()),(b'old probe',False))
        self.assertTrue(os.path.lexists(entry))  # never replaced or repaired
    RETAINED_PLANTS = {  # R22.2 sweep: each class of unsafe retained entry, each a generated control
        'symlinked_entry':lambda e,s: (shutil.rmtree(e),e.symlink_to(s)),
        'file_entry':lambda e,s: (shutil.rmtree(e),e.write_bytes(b'')),
        'symlinked_member':lambda e,s: ((e/'gates.json').unlink(),(e/'gates.json').symlink_to(s/'gates.json')),
        'hardlinked_member':lambda e,s: ((e/'gates.json').unlink(),os.link(s/'gates.json',e/'gates.json')),
        'fifo_member':lambda e,s: ((e/'gates.json').unlink(),os.mkfifo(e/'gates.json',0o444)),
        'writable_member':lambda e,s: (e/'gates.json').chmod(0o644),
        'writable_entry':lambda e,s: e.chmod(0o775)}
    for _n in RETAINED_PLANTS: locals()['test_r22_unsafe_retained_entry_refused_at_preflight_'+_n] = lambda self,_n=_n: self.unsafe_retained_entry(self.RETAINED_PLANTS[_n])
    del _n
    def test_r22_symlinked_identical_referenced_entry_refused_at_preflight(self):
        history = self.root/'config/history'; history.mkdir(mode=0o755)
        (history/self.gates).symlink_to(self.root/'oldhistory'/self.gates); security = Mock(return_value=[])
        with patch.dict(self.scope,stage_security=security), self.assertRaisesRegex(ValueError,'unsafe receipt ancestry'): self.upgrade()
        security.assert_not_called()
        self.assertEqual((self.root/'dest/probe.py').read_bytes(),b'old probe')
        self.assertEqual(json.loads((self.root/'config/admission.json').read_text())['stage'],'normal')
    def test_receipt_failing_its_own_policy_refuses_before_mutation(self):
        (self.root/'dest/policy/exclusions.json').chmod(0o644)
        inventory = json.loads((self.root/'config/installed-files.json').read_text())
        bad = json.loads((HERE/'policy/exclusions.json').read_text()); bad['reason'] = 'not the receipt policy'
        (self.root/'dest/policy/exclusions.json').write_text(json.dumps(bad))
        inventory[str(self.root/'dest/policy/exclusions.json')] = sha((self.root/'dest/policy/exclusions.json').read_bytes())
        (self.root/'config/installed-files.json').write_text(json.dumps(inventory))
        with self.assertRaisesRegex(ValueError,'old gate policy is not its recorded digest'): self.upgrade()
        self.identity['gates_sha256'] = receipt.policy_digest({p:h for p,h in inventory.items() if p.endswith(('gates.json','exclusions.json'))})
        (self.root/'config/identity.json').write_text(json.dumps(self.identity))
        with self.assertRaisesRegex(ValueError,'unknown receipt policy'): self.upgrade()
        self.assertEqual((self.root/'dest/probe.py').read_bytes(),b'old probe')
        self.assertFalse((self.root/'config/history').exists())
        self.assertEqual(json.loads((self.root/'config/admission.json').read_text())['stage'],'normal')

if __name__ == '__main__': unittest.main()

# ---- GB1b: the payload route sequence with real processes (SB3E-R18 P1, R19, R20) ----------------
# Each scenario runs init's Runner.route in a fresh child Python made a child subreaper, so every
# orphan is adopted by it exactly as by payload PID 1; A7 and the harness are scripted stand-ins.
DETACH = r'''
def detach(pidfile):
    """setsid + double fork; the survivor closes the captured pipes, so the command itself ends."""
    import os, time
    r, w = os.pipe()
    first = os.fork()
    if first == 0:
        os.setsid()
        if os.fork() == 0:
            null = os.open(os.devnull, os.O_RDWR)
            for fd in (0, 1, 2): os.dup2(null, fd)
            open(pidfile, 'w').write(str(os.getpid())); os.write(w, b'x')
            time.sleep(60); os._exit(0)
        os.read(r, 1); os._exit(0)
    os.waitpid(first, 0); os.close(r); os.close(w)
'''
FAKE_A7 = DETACH + r'''
import hashlib, json, os, shutil, sys
cfg = json.load(open(sys.argv[1]))
ev, exe = cfg['evidence_dir'], cfg['target'] + '/release/deps/route_cells-0123456789abcdef'
os.makedirs(os.path.dirname(exe)); shutil.copyfile(cfg['harness'], exe); os.chmod(exe, 0o755)
digest = '0'*64 if cfg['mode'] == 'wrong-hash' else hashlib.sha256(open(exe, 'rb').read()).hexdigest()
for rel in cfg['evidence']:
    os.makedirs(os.path.join(ev, os.path.dirname(rel)), exist_ok=True)
    body = {'lane/inventory.json': json.dumps({'files': {'release/deps/route_cells-0123456789abcdef': digest}}),
            'lane/meta.json': json.dumps({'rustc': 'rustc 1.94.0 -vV', 'cargo': 'cargo 1.94.0'})}.get(rel, rel)
    body = '' if rel.endswith('/start') else body  # A7's start markers are empty files
    open(os.path.join(ev, rel), 'w').write(body)
if cfg['mode'] == 'symlink':
    os.unlink(ev + '/lane/start'); os.symlink(ev + '/ship/start', ev + '/lane/start')
if cfg['mode'] == 'extra': open(ev + '/lane/extra', 'w').write('x')
if cfg['mode'] == 'fifo': os.unlink(ev + '/lane/start'); os.mkfifo(ev + '/lane/start')
open(cfg['verdict'], 'w').write(json.dumps({'verdict': 'fail' if cfg['mode'] == 'fail' else 'pass'}))
if cfg['mode'] == 'stray': detach(cfg['pidfile'])
sys.exit(1 if cfg['mode'] == 'fail' else 0)
'''
FAKE_HARNESS = DETACH + r'''
import os, sys
MODE, PIDFILE = %r, %r
if sys.argv[1:] == ['--list', '--format', 'terse']:
    print('cells::rc_r1_alpha_beta_one: test'); print('cells::rc_r2_alpha_beta_two: test')
    if MODE == 'replace':  # R20 (i): a constructor that replaces its own pathname during --list
        me = sys.argv[0]; text = open(me).read().replace("MODE, PIDFILE = 'replace'", "MODE, PIDFILE = 'replaced'")
        open(me + '.new', 'w').write(text); os.chmod(me + '.new', 0o755); os.rename(me + '.new', me)
    if MODE == 'spawn': detach(PIDFILE)  # R20 (ii): a process spawned during --list that survives
    sys.exit(0)
print('\nrunning 2 tests\ntest cells::rc_r1_alpha_beta_one ... ok\ntest cells::rc_r2_alpha_beta_two ... ok\n')
print('test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s')
'''
DRIVER = DETACH + r'''
import ctypes, json, os, sys
sys.path.insert(0, sys.argv[1]); import init
cfg = json.loads(sys.argv[2])
if ctypes.CDLL(None, use_errno=True).prctl(36, 1, 0, 0, 0) != 0: raise OSError('PR_SET_CHILD_SUBREAPER')
init.ROUTE_CWD = cfg['cwd']
log = os.open(cfg['log'], os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
runner = init.Runner(log, os.open(os.devnull, os.O_WRONLY))
if cfg['pre_stray']: detach(cfg['pre_stray'])  # a process alive before A7 starts
try: runner.route(cfg['policy'], cfg['request']); out = {'error': None}
except Exception as exc: out = {'error': str(exc)}
out.update(quiescence=runner.quiescence, left=sorted(runner.descendants()))
print(json.dumps(out))
'''

class RoutePayloadControls(unittest.TestCase):
    """init.Runner.route: quiescence, sealing, harness identity (R20) and the inventory commitment."""
    def scenario(self, a7='pass', harness='normal', inventory=INVENTORY, pre_stray=False):
        tmp = Path(tempfile.mkdtemp(dir=os.environ.get('TMPDIR')))
        self.addCleanup(lambda: subprocess.run(['rm','-rf',str(tmp)]))
        (tmp/'src/mother/host').mkdir(parents=True); (tmp/'run').mkdir(); (tmp/'home').mkdir()
        (tmp/'a7.py').write_text(FAKE_A7)
        (tmp/'harness').write_text('#!/usr/bin/python3 -I\n'+FAKE_HARNESS % (harness,str(tmp/'harness.pid')))
        a7cfg = dict(ROUTE['a7'],target=str(tmp/'target-route'),evidence_dir=str(tmp/'run/a7'),verdict=str(tmp/'run/a7-verdict.json'))
        (tmp/'a7.json').write_text(json.dumps(dict(a7cfg,mode=a7,harness=str(tmp/'harness'),pidfile=str(tmp/'a7.pid'))))
        env = dict(ROUTE['environment'],HOME=str(tmp/'home'),TMPDIR=str(tmp))
        gates = [dict(id='a7',argv=[sys.executable,'-I','-B',str(tmp/'a7.py'),str(tmp/'a7.json')]),
                 dict(ROUTE['gates'][1],executable=re.escape(a7cfg['target'])+'/release/deps/route_cells-[0-9a-f]{16}')]
        cfg = dict(policy=dict(ROUTE,a7=a7cfg,environment=env,gates=gates),request=route_request(inventory_sha256=inventory),
                   log=str(tmp/'log'),cwd={'a7':str(tmp/'src'),'route-cells':str(tmp/'src/mother/host')},
                   pre_stray=str(tmp/'pre.pid') if pre_stray else None)
        proc = subprocess.run([sys.executable,'-I','-B','-c',DRIVER,str(HERE),json.dumps(cfg)],capture_output=True,text=True,timeout=120)
        self.assertEqual(proc.returncode,0,proc.stderr)
        out = json.loads(proc.stdout)
        out['outputs'] = outputs(tmp/'log')
        for name in ('a7.pid','harness.pid','pre.pid'):
            if (tmp/name).exists():
                with self.assertRaises(ProcessLookupError,msg='stray survived quiescence'): os.kill(int((tmp/name).read_text()),0)
        self.assertEqual(out['left'],[])
        return out
    def test_scripted_route_passes_and_seals_exact_evidence(self):
        out = self.scenario()
        self.assertIsNone(out['error'])
        self.assertEqual(out['quiescence'],{'before-a7':True,'after-a7':True,'after-list':True})
        sealed = {'a7-'+rel.replace('/','-') for rel in ROUTE['a7']['evidence']}|{'a7-verdict.json'}
        self.assertLessEqual(sealed,set(out['outputs']))  # empty evidence (the start markers) is still a sealed log
        self.assertEqual((out['outputs']['a7-ship-start'],out['outputs']['a7-lane-start']),(b'',b''))
        lane = json.loads(out['outputs']['route-lane'])
        self.assertEqual((lane['names'],lane['harness_sha256']),(NAMES,json.loads(out['outputs']['a7-lane-inventory.json'])['files']
                                                               ['release/deps/route_cells-0123456789abcdef']))
        self.assertEqual(json.loads(out['outputs']['c3-inventory'])['inventory']['passed'],NAMES)
    def test_stray_that_detaches_and_closes_pipes_fails_quiescence(self):
        out = self.scenario(a7='stray')
        self.assertRegex(out['error'],'^QUIESCENCE: after-a7: process survived and was killed')
        self.assertNotIn('a7-verdict.json',out['outputs'])  # nothing sealed after a stray
    def test_process_alive_before_a7_fails_quiescence(self):
        out = self.scenario(pre_stray=True)
        self.assertRegex(out['error'],'^QUIESCENCE: before-a7: process survived and was killed')
        self.assertNotIn('c1',out['outputs'])  # A7 never started
    def test_r20_harness_replacing_its_pathname_fails_rehash(self):
        out = self.scenario(harness='replace')
        self.assertEqual(out['error'],'HARNESS: bytes differ from A7 lane inventory')
        self.assertTrue(out['quiescence'].get('after-list'))  # killed at the re-hash, not by quiescence
        self.assertNotIn('c3',out['outputs'])
    def test_r20_process_surviving_list_fails_quiescence(self):
        out = self.scenario(harness='spawn')
        self.assertRegex(out['error'],'^QUIESCENCE: after-list: process survived and was killed')
        self.assertNotIn('c3',out['outputs'])
    def test_symlinked_evidence_refused(self):
        self.assertRegex(self.scenario(a7='symlink')['error'],'^ROUTE_EVIDENCE: missing/linked .*/run/a7/lane/start$')
    def test_non_regular_evidence_refused(self):
        self.assertRegex(self.scenario(a7='fifo')['error'],'^ROUTE_EVIDENCE: not a bounded regular file .*/run/a7/lane/start$')
    def test_unexpected_evidence_file_refused(self):
        self.assertRegex(self.scenario(a7='extra')['error'],'^ROUTE_EVIDENCE: unexpected evidence set in .*/run/a7/lane$')
    def test_harness_differing_from_a7_inventory_refused_before_listing(self):
        out = self.scenario(a7='wrong-hash')
        self.assertEqual(out['error'],'HARNESS: bytes differ from A7 lane inventory')
        self.assertNotIn('c2',out['outputs'])  # never executed
    def test_listed_names_must_match_submitted_inventory(self):
        out = self.scenario(inventory=sha(b'cells::rc_r1_alpha_beta_one'))
        self.assertEqual(out['error'],'INVENTORY: listed names differ from the submitted inventory_sha256')
        self.assertNotIn('c3',out['outputs'])
    def test_a7_fail_is_reported_as_itself(self):
        out = self.scenario(a7='fail')
        self.assertEqual(out['error'],'A7: run verdict is not pass')
        self.assertEqual(json.loads(out['outputs']['a7-verdict.json'])['verdict'],'fail')

def outputs(log):
    data, named = Path(log).read_bytes(), {}
    while data:
        size = struct.unpack('!I',data[:4])[0]
        frame, data = json.loads(data[4:4+size]), data[4+size:]
        named[frame['id']] = named.get(frame['id'],b'')+__import__('base64').b64decode(frame['data'])
    return named

class RouteMainControls(unittest.TestCase):
    def main(self, rust):
        init = load('init')
        runner = Mock(quiescence={'before-a7':True})
        with ExitStack() as stack:
            for name, value in (('Path',Mock()),('route_policy',lambda: ROUTE),('observations',lambda: {'rust':rust,'registry':'6'*64}),
                                ('probe_payload',Mock(side_effect=AssertionError('probe ran'))),('packet',Mock()),('resources',lambda: {})):
                stack.enter_context(patch.object(init,name,value))
            stack.enter_context(patch.object(init.shutil,'copytree')); stack.enter_context(patch.object(init.os,'walk',return_value=[]))
            init.route_main(runner,route_request())
        return runner
    def test_toolchain_pin_checked_before_a7(self):
        with self.assertRaisesRegex(ValueError,'^TOOLCHAIN_PIN: /opt/rust differs from the reviewed manifest$'): self.main('0'*64)
    def test_route_runs_no_probe(self):
        runner = self.main(ROUTE['toolchain']['manifest_sha256'])
        runner.route.assert_called_once()
        self.assertIn('not run: route-cells',json.loads(runner.output.call_args_list[0].args[1])['probe'])

class SupervisorRouteSummary(unittest.TestCase):
    def test_command_cwd_is_a_gate_set_literal(self):
        supervisor = load('supervisor')
        start = dict(event='start',id='c1',gate_id='route-cells',argv=[EXE],environment=dict(ROUTE['environment']))
        for cwd in ('/work/src','/work/src/mother/host'): supervisor.Evidence().accept(dict(start,cwd=cwd))
        for cwd in ('/work/src/mother','/work/target-route','/run/gate'):
            with self.subTest(cwd=cwd), self.assertRaisesRegex(ValueError,'invalid command start'):
                supervisor.Evidence().accept(dict(start,cwd=cwd))
    def test_route_summary_from_init_outputs_validates(self):
        supervisor, (expected, files) = load('supervisor'), route_success()
        start = lambda c: {k:c[k] for k in ('id','gate_id','argv','cwd','environment','exit_code','signal')}
        commands = [start(expected['commands'][0]),expected['coverage']['discovery'][0],start(expected['commands'][1])]
        logs = dict(files)
        logs['c3-inventory.log'] = json.dumps(dict(artifact=expected['coverage']['artifacts'][0],
                                                   inventory=expected['commands'][1]['inventory'])).encode()
        logs['route-lane.log'] = json.dumps(dict(harness_sha256='7'*64,names=NAMES,rustc_vv='rustc 1.94.0',cargo_v='cargo 1.94.0')).encode()
        logs['coverage.log'] = json.dumps(dict(artifacts=expected['coverage']['artifacts'],failures=[])).encode()
        summary = {k:copy.deepcopy(v) for k,v in expected.items() if k not in ('commands','coverage','lane','environment')}
        supervisor.route_summary(summary,Mock(commands=commands),logs,summary['installed'])
        for key in ('commands','coverage','lane','environment'): self.assertEqual(summary[key],expected[key],key)
        summary['logs'] = [dict(file=n,sha256=sha(d)) for n,d in logs.items()]
        receipt.validate_summary(summary)
