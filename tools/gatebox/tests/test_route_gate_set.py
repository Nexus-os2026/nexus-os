"""GB1a controls (SB3E-R18, R19): the route-cells gate set's request, receipt identity, historical
policy, client, verifier and install/upgrade. Each rejection is its own check; `full` is unchanged."""
import copy
import hashlib
import io
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
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
    files = {'a7-'+rel.replace('/','-'):rel.encode() for rel in ROUTE['a7']['evidence']}
    files.update({'a7-verdict.json':b'{"verdict": "pass"}\n','c1.log':b'a7\n','c3.log':b'cells\n'})
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
    summary['lane'] = dict(a7='PASS',verdict_sha256=sha(files['a7-verdict.json']),rustc_vv='rustc 1.94.0',cargo_v='cargo 1.94.0',
                           evidence={n:sha(d) for n,d in files.items() if n.startswith('a7-') and n != 'a7-verdict.json'},
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
        self.assertTrue((HERE/'verify.py').read_text().startswith('#!/usr/bin/python3 -I\n'))
        self.assertIn('nexus ALL=(root) NOPASSWD: NOSETENV: /usr/libexec/gatebox/verify.py --run-id *\n',
                      (HERE/'policy/sudoers').read_text())

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
    def settled(self, show, units=''):
        replies = iter([b'',show.encode(),units.encode()])
        with patch.dict(self.scope,run=lambda argv: (self.calls.append(argv),next(replies))[1]):
            self.scope['require_settled']()
    def test_services_stopped_and_settled(self):
        good = 'Id=mothergate.socket\nActiveState=inactive\nJob=\n\nId=mothergate.service\nActiveState=inactive\nJob=\n'
        self.settled(good,'mothergate-run@x.service loaded failed failed run\n')
        self.assertEqual(self.calls[0],['/usr/bin/systemctl','stop','mothergate.socket','mothergate.service'])
        for bad in (good.replace('inactive','active',1),good.replace('Job=\n','Job=12\n',1),good.split('\n\n')[0]):
            with self.subTest(bad=bad), self.assertRaisesRegex(ValueError,'services not stopped and settled'): self.settled(bad)
        with self.assertRaisesRegex(ValueError,'run unit not settled'): self.settled(good,'mothergate-root@x.service loaded active running r\n')
    def test_pending_and_custody_state_refused_and_retained(self):
        self.scope['require_no_custody']()
        (self.root/'control/sealed'/('.used-'+'a'*32)).write_bytes(b'')
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
        upgrade = text.split('def upgrade(',1)[1].split('\ndef ',1)[0]
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
        self.run = Mock(side_effect=lambda argv: {'show':b'Id=mothergate.socket\nActiveState=inactive\nJob=\n\n'
                                                          b'Id=mothergate.service\nActiveState=inactive\nJob=\n'}.get(argv[1],b''))
        self.stack.enter_context(patch.dict(scope,DEST=dest,CONFIG=config,STATE=state,CONTROL=control,UPGRADE_BOOT=boot,
            HISTORY=config/'history',ETC=root/'etc',RUN_UNIT=root/'etc/run.service',CONTROL_POLICY=root/'tmpfiles/mothergate-control.conf',
            STATE_ROOT_OWNER=(uid,gid),protected=lambda p: None,run=self.run,pwd=accounts,toolchain_check=lambda f: None,
            bundle_files=lambda path,digest: self.files,plan_accounts=lambda: [(entry,False),(entry,False)],
            stage_security=lambda *a: [],load_module=module))
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
