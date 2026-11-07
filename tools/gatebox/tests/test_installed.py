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
        rows = ['B1','B2','B3','B4','A1','A5','A6','A8','E2E','CLIENTS']
        value = dict(identity=identity,passed=rows,evidence={r:dict(run_id='a'*32,
                     summary_sha256=hashlib.sha256(json.dumps(summary,sort_keys=True).encode()).hexdigest()) for r in rows})
        value['evidence']['CLIENTS']['client_provenance'] = provenance
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
    def test_e2e(self): self.check('E2E','899e3872c5b61223c87b905f63c66947a816259b full PASS_SCOPED')
