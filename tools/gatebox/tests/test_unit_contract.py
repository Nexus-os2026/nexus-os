"""Frozen request, policy and exclusion contract; static policy is not execution."""
import hashlib
import json
from pathlib import Path
import struct
import unittest
from test_unit_core import snapshot, HERE

SHA = '0c8cb6d360b1296262b7a5c478261e5e47e5c7f5'
CARGO = '/opt/rust/bin/cargo'
class ContractTests(unittest.TestCase):
    def setUp(self):
        self.registry = {'mother': {'uid':1234, 'object_format':'sha1'}}
        self.request = {'repo_id':'mother','full_sha':SHA,'gate_set_id':'full'}
    def frame(self, data):
        return struct.pack('!I',len(data))+data
    def test_request_exact(self):
        self.assertEqual(snapshot.parse_request(self.frame(json.dumps(self.request).encode()), self.registry,1234), self.request)
    def test_request_abbreviation(self):
        self.request['full_sha'] = SHA[:8]
        with self.assertRaises(ValueError):
            snapshot.parse_request(self.frame(json.dumps(self.request).encode()), self.registry,1234)
    def test_request_unknown_key(self):
        self.request['command'] = 'true'
        with self.assertRaises(ValueError):
            snapshot.parse_request(self.frame(json.dumps(self.request).encode()), self.registry,1234)
    def test_request_rejections(self):
        good = json.dumps(self.request).encode()
        cases = [b'',b'\x00'*4,self.frame(b'x'*1025),self.frame(good)+b'x',self.frame(good)[:-1],
                 self.frame(b'\xff'),self.frame(b'[]'),self.frame(good+b' {}'),
                 self.frame(b'{"repo_id":"mother","repo_id":"mother","full_sha":"'+SHA.encode()+b'","gate_set_id":"full"}')]
        for key, value in [('repo_id','unknown'),('full_sha',SHA.upper()),('full_sha',12),
                           ('gate_set_id','smoke'),('repo_id',None)]:
            cases.append(self.frame(json.dumps(dict(self.request,**{key:value})).encode()))
        for frame in cases:
            with self.subTest(frame=frame), self.assertRaises(ValueError):
                snapshot.parse_request(frame,self.registry,1234)
        with self.assertRaises(ValueError):
            snapshot.parse_request(self.frame(good),self.registry,1235)
        self.registry['mother']['object_format'] = 'sha256'
        with self.assertRaises(ValueError):
            snapshot.parse_request(self.frame(good),self.registry,1234)
    def test_fixed_gates_and_desktop(self):
        policy = json.loads((HERE/'policy/gates.json').read_text())
        self.assertEqual((policy['schema_version'],policy['gate_set_id'],policy['success_status']), (1,'full','PASS_SCOPED'))
        self.assertIs(policy['no_fail_fast'],True)
        self.assertEqual(policy['environment'],{'PATH':'/opt/rust/bin:/usr/bin:/bin','HOME':'/work/home','PWD':'/work/src',
                         'CARGO_HOME':'/work/cargo','CARGO_TARGET_DIR':'/work/target','CARGO_BUILD_JOBS':'4',
                         'RUST_TEST_THREADS':'4','CARGO_TERM_COLOR':'never','CARGO_NET_OFFLINE':'true',
                         'RUST_BACKTRACE':'1','TMPDIR':'/run/gate/tmp','XDG_RUNTIME_DIR':'/run/gate/xdg',
                         'XDG_CACHE_HOME':'/work/cache','LANG':'C.UTF-8','LC_ALL':'C.UTF-8','TZ':'UTC','CI':'true'})  # sheet §5: exactly 17 variables
        gates = policy['gates']
        self.assertEqual([g['id'] for g in gates], ['format','clippy','compile-tests','workspace','doctests','host','desktop-live','walk','whitespace'])
        expected = {
            'format':['fmt','--all','--','--check'],
            'clippy':['clippy','--workspace','--all-targets','--all-features','--locked','--','-D','warnings'],
            'compile-tests':['test','--workspace','--locked','--no-run','--message-format=json'],
            'doctests':['test','--workspace','--locked','--doc','--no-fail-fast'],
            'walk':['build','-p','nexus-mother-host','--locked','--example','walk']}
        for g in gates:
            if g['id'] in expected: self.assertEqual(g['argv'],[CARGO,'--offline']+expected[g['id']])
        self.assertEqual(gates[3],{'id':'workspace','adapter':'libtest','artifacts':'every eligible emitted test artifact','list_before_execute':True})
        self.assertEqual(gates[5],{'id':'host','adapter':'reconcile-all-artifacts','package':'nexus-mother-host',
                                        'argv':None,'internal':True})
        desktop = gates[6]
        self.assertEqual((desktop['adapter'],desktop['package'],desktop['target'],desktop['minimum']), ('libtest-repeat','nexus-mother-desktop','live',14))
        self.assertEqual(len(desktop['expected_names']),23)
        self.assertEqual(len(set(desktop['expected_names'])),23)
        self.assertEqual(hashlib.sha256('\n'.join(desktop['expected_names']).encode()).hexdigest(), '326ab2d036bb6c0c114507d81724817272c6474cf61f08b5b7862cc2ab79bb91')
        self.assertEqual(gates[8],{'id':'whitespace','adapter':'snapshot-whitespace','label':'approximation to git diff --check',
                                        'argv':None,'internal':True})
        self.assertEqual(policy['coverage'],{'reject_unknown_harness':True,'reconcile_names_and_results':True,'missing_artifact_fails':True,'compile_excluded_target':True,'execute_excluded_target':False,'excluded_target':'phase2_live_sandbox','exclusions_file':'exclusions.json'})
    def test_exact_exclusions(self):
        exclusions = json.loads((HERE/'policy/exclusions.json').read_text())
        self.assertEqual(set(exclusions),{'target','reason','cases','source_blob'})
        self.assertEqual(exclusions['source_blob'],'6cf914ba5ae1eeffc1b61376ba6b768be8a0ffdb')
        self.assertEqual(exclusions['target'],'phase2_live_sandbox')
        self.assertEqual(exclusions['reason'],'needs nested namespaces (D-E2-10)')
        self.assertEqual(len(exclusions['cases']),39)
        self.assertEqual(len(set(exclusions['cases'])),39)
        self.assertEqual(hashlib.sha256('\n'.join(exclusions['cases']).encode()).hexdigest(), '5047e9d2fe438d6844f68d780dc79d49e59a577326f88f1462bd0581af3aa4d1')
