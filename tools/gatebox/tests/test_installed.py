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
import re
from pathlib import Path
import tempfile
import sys
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
        scope = installer(); check = scope['bundle_files']
        wrapper = b'built wrapper bytes'  # GB1: the reviewed literal is a separate control (test_route_gate_set)
        scope['WRAPPER_SHA256'] = hashlib.sha256(wrapper).hexdigest()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive = root/'bundle.tar'
            harness = 'crates/nexus-verifier-sandbox/tests/phase2_live_sandbox.rs'
            names = scope['FILES']+[scope['WRAPPER']]
            with tarfile.open(archive,'w') as tar:
                for name in names+[harness]:
                    source = HARNESS if name == harness else HERE/name
                    data = wrapper if name == scope['WRAPPER'] else source.read_bytes()
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
        sha = '8e0d6ae293df999eb6530e875df1206deac872a8'
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
    def test_empty_command_has_an_authenticated_empty_capture(self):
        init = load('init'); events = []
        runner = init.Runner(111,222)
        options = init.child_options
        with tempfile.TemporaryDirectory() as directory, \
                patch.object(init,'child_options',side_effect=lambda fd: dict(options(fd),cwd=directory)), \
                patch.object(init,'packet',side_effect=lambda fd,value: events.append((fd,value))):
            name,code,output = runner.command('format',[os.sys.executable,'-I','-c','pass'])
        self.assertEqual((code,output),(0,''))
        self.assertIn((111,{'id':name,'data':''}),events)
    def test_host_process_identity_ignores_volatile_counters(self):
        identify = load('probe').process_identity
        fields = ['0']*30; fields[19]='1234'
        first = '456 (host canary) '+' '.join(fields)
        fields[10]='9999'
        self.assertEqual(identify(first),identify('456 (host canary) '+' '.join(fields)))
        fields[19]='5678'
        self.assertNotEqual(identify(first),identify('456 (host canary) '+' '.join(fields)))
    def test_payload_policy_outputs_match_readonly_bind(self):
        files = {'tools/gatebox/policy/'+n:(HERE/'policy'/n).read_bytes()
                 for n in ('gates.json','exclusions.json','route-cells.json')}
        outputs = installer()['payload_policies'](files)
        self.assertEqual(outputs,{n:files['tools/gatebox/policy/'+n] for n in ('gates.json','exclusions.json','route-cells.json')})
    def test_enable_requires_bound_clients_provenance(self):
        identity = {k:'a'*64 for k in ('bundle_sha256','policy_sha256','gates_sha256')}
        summary = dict(status='PASS_SCOPED',request=dict(repo_id='mother',full_sha=load('probe').FIXTURE,gate_set_id='full'),
                       installed=dict(identity,stage='qualifying'),run_id='a'*32,peer_uid=1234)
        provenance = dict(uid=1234,context='claude',ancestor_pid=456,accepted=dict(status='ACCEPTED',run_id='a'*32),
                          refused=dict(status='UNQUALIFIED',reason='QUALIFICATION_ONLY'),codex='unsupported (runner denies AF_UNIX)')
        rows = ['B1','B2','B3','B4','A1','A5','A6','A8','E2E','CLIENTS','C1-SEALED','C2-CANARY','C3-RESTART']
        value = dict(identity=identity,passed=rows,evidence={r:dict(run_id='a'*32,
                     summary_sha256=hashlib.sha256(json.dumps(summary,sort_keys=True).encode()).hexdigest()) for r in rows})
        value['evidence']['CLIENTS']['client_provenance'] = provenance
        for row in ('C1-SEALED','C2-CANARY','C3-RESTART'):
            from test_install_composition import q15_proof
            value['evidence'][row]['custody'] = q15_proof(row)
        sensors=load('probe')
        from test_install_composition import BOOT_AFTER
        sensors.boot_id=lambda:BOOT_AFTER; sensors.clean_custody=lambda:True
        raw = installer()['acceptance_check']; state = dict(identity=identity,stage='qualifying')
        def check(*args): return raw(*args,sensors=sensors)
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
    def test_c1(self): self.check('C1-SEALED','sealed-DAC')
    def test_zz_c2(self): self.check('C2-CANARY','pre-handoff-canary-reboot-required')
    def test_c3(self): self.check('C3-RESTART','pending-restart')
    def test_e2e(self): self.check('E2E','8e0d6ae293df999eb6530e875df1206deac872a8 full PASS_SCOPED')

class Q8F1Oracles(unittest.TestCase):
    def test_canary_open_denial(self):
        import errno
        probe = load('probe')
        for error in (errno.EACCES,errno.EPERM,errno.ENOENT):
            with patch.object(probe.os,'open',side_effect=OSError(error,'denied')) as opened:
                probe.host_pty_denied('/dev/pts/99')
                opened.assert_called_once_with('/dev/pts/99',os.O_RDWR|os.O_NOCTTY)
        with patch.object(probe.os,'open',return_value=987), patch.object(probe.os,'close') as closed:
            with self.assertRaisesRegex(ValueError,'host PTY'): probe.host_pty_denied('/dev/pts/99')
            closed.assert_called_once_with(987)
        with patch.object(probe.os,'open',side_effect=OSError(errno.EIO,'broken')):
            with self.assertRaises(ValueError): probe.host_pty_denied('/dev/pts/99')
        self.assertIn("host_pty_denied(context['pty']['path'])",(HERE/'probe.py').read_text())

class Q8F2Oracles(unittest.TestCase):
    def test_api_mount_skeleton_inventory(self):
        tree = ast.parse((HERE/'install.sh').read_text().split("<<'PY'\n",1)[1].rsplit('\nPY',1)[0])
        loops = [n for n in ast.walk(tree) if isinstance(n,ast.For) and isinstance(n.iter,ast.Tuple)
                 and all(isinstance(e,ast.Constant) and isinstance(e.value,str) for e in n.iter.elts)]
        directories = next(ast.literal_eval(n.iter) for n in loops if 'dev/shm' in ast.literal_eval(n.iter))
        self.assertTrue({'proc','sys'} <= set(directories))
        text = (HERE/'install.sh').read_text()
        self.assertLess(text.index("'proc'"),text.index("record(CONFIG/'skeleton.json',inventory)"))

class Q20Oracles(unittest.TestCase):
    def test_skeleton_ships_manager_created_root_mountpoint(self):
        # Q20 evidence: systemd 259 created RootDirectory/root (root:root 0750)
        # while setting up the first payload's namespace, after skeleton.json was
        # recorded, so installed identity could never re-verify. The manager may
        # only find it already present; the inventory check is never relaxed.
        tree = ast.parse((HERE/'install.sh').read_text().split("<<'PY'\n",1)[1].rsplit('\nPY',1)[0])
        loops = [n for n in ast.walk(tree) if isinstance(n,ast.For) and isinstance(n.iter,ast.Tuple)
                 and all(isinstance(e,ast.Constant) and isinstance(e.value,str) for e in n.iter.elts)]
        directories = next(ast.literal_eval(n.iter) for n in loops if 'dev/shm' in ast.literal_eval(n.iter))
        self.assertIn('root',directories)
        # Q30: the skeleton stays at /usr/lib/gatebox/root; each payload root is a manifest-checked
        # tmpfs copy of it, so the manager-created 'root' must still ship in the skeleton.
        self.assertIn("SKELETON = Path('/usr/lib/gatebox/root')",(HERE/'rootfs.py').read_text())
        self.assertIn('RootDirectory=/run/mothergate-root/%i\n',(HERE/'units/mothergate-run@.service').read_text())
        self.assertIn("require(actual==expected,'skeleton inventory changed')",(HERE/'probe.py').read_text())

class Q23Oracles(unittest.TestCase):
    """Q23 live evidence (diag rounds 1-3): payload environment defects on systemd 259/NVIDIA hosts."""
    def unit(self): return (HERE/'units/mothergate-run@.service').read_text()
    def test_usr_bind_is_not_recursive(self):
        # The skeleton lives under /usr; a recursive /usr bind copied the payload's own
        # mounts (e.g. /run/gate twice), defeating the whole-tmpfs isolation check.
        unit = self.unit()
        self.assertIn('BindReadOnlyPaths=/usr:/usr:norbind\n',unit)
        self.assertNotIn('BindReadOnlyPaths=/usr\n',unit)
        # Q30: the skeleton source remains under /usr; norbind still keeps host submounts out.
        self.assertIn("SKELETON = Path('/usr/lib/gatebox/root')",(HERE/'rootfs.py').read_text())
    def test_vendor_gpu_egl_icd_hidden(self):
        # No GPU devices exist in the payload; the NVIDIA EGL ICD segfaults Xvfb.
        self.assertIn('InaccessiblePaths=-+/usr/share/glvnd/egl_vendor.d/10_nvidia.json\n',self.unit())
    def test_inaccessible_paths_are_root_relative(self):
        # Q24 (systemd v259 namespace.c): with RootDirectory=, an InaccessiblePaths entry without '+'
        # is host-absolute and silently dropped by drop_outside_root(); '+' makes it root-relative.
        unit = self.unit()
        entries = [e for line in unit.splitlines() if line.startswith('InaccessiblePaths=')
                   for e in line.split('=',1)[1].split()]
        self.assertEqual(sorted(entries),sorted(['+/usr/share/ollama',
                                                 '-+/usr/share/glvnd/egl_vendor.d/10_nvidia.json']))
        for entry in entries:
            self.assertTrue(entry.lstrip('-').startswith('+/'),entry)
    def test_a5_evidence_is_kernel_observed_not_headless_sandbox_page(self):
        # Q25: chrome://sandbox renders the New Tab page under --headless=new; A5 keeps local render,
        # version, label and the kernel-observed renderer (Seccomp=2, nested PID ns, lineage).
        source = (HERE/'probe.py').read_text()
        self.assertNotIn('chrome://sandbox',source)
        self.assertNotIn('sandbox_html',source)
        self.assertIn("'A5':('local_html','version','renderer','private_desktop')",source)
        self.assertIn("require('gatebox-local' in local,'Chrome local rendering')",source)
    def test_cmdline_tokens_see_rewritten_process_titles(self):
        tokens = load('probe').cmdline_tokens
        rewritten = b'/opt/google/chrome/chrome --type=renderer --no-sandbox --lang=en\0\0\0'
        self.assertIn(b'--type=renderer',tokens(rewritten))
        self.assertIn(b'--no-sandbox',tokens(rewritten))
        self.assertIn(b'--disable-setuid-sandbox',tokens(b'chrome\0--type=renderer\0--disable-setuid-sandbox\0'))
        self.assertNotIn(b'',tokens(rewritten))
        self.assertEqual(tokens(b''),[])
    def test_chrome_observation_uses_tokens_for_type_and_unsafe_flags(self):
        tree = ast.parse((HERE/'probe.py').read_text())
        function = next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='chrome_observation')
        source = ast.unparse(function)
        self.assertIn("chrome_process_is_renderer((path / 'cmdline').read_bytes())",source)
        self.assertNotIn("split(b'\\x00')",source)

class Q26Oracles(unittest.TestCase):
    """Q26 (Codex review-1 of GBX-F3): behavioural negative controls for the A5 Chrome checks."""
    def setUp(self): self.probe = load('probe')
    def test_switch_parsing_denies_unsafe_flags_in_every_chromium_form(self):
        is_renderer = self.probe.chrome_process_is_renderer
        for raw in (b'chrome\0--type=renderer\0--no-sandbox\0', b'chrome --type=renderer --no-sandbox\0\0',
                    b'chrome\0--disable-setuid-sandbox\0', b'chrome\0--no-sandbox=1\0', b'chrome\0-no-sandbox\0',
                    b'chrome\0--disable-setuid-sandbox=true\0'):
            with self.subTest(raw=raw), self.assertRaisesRegex(ValueError,'unsafe Chrome flag'): is_renderer(raw)
    def test_only_renderer_type_is_a_renderer(self):
        is_renderer = self.probe.chrome_process_is_renderer
        self.assertTrue(is_renderer(b'/opt/google/chrome/chrome --type=renderer --lang=en\0\0'))
        self.assertTrue(is_renderer(b'chrome\0--type=renderer\0'))
        for raw in (b'chrome\0--type=gpu-process\0', b'chrome\0--type=zygote\0', b'chrome\0', b'',
                    b'chrome\0--lang=--type=renderer\0', b'chrome\0--type=rendererX\0'):
            with self.subTest(raw=raw): self.assertFalse(is_renderer(raw))
    def status(self, seccomp='2', nspid='71\t4\t1', filters='3'):
        return {'Seccomp':'\t'+seccomp,'NSpid':'\t'+nspid,'Seccomp_filters':'\t'+filters}
    def test_renderer_isolation_requires_chrome_own_seccomp_filter_and_nested_pid_ns(self):
        isolated = self.probe.renderer_isolated
        browser = self.status(filters='2')
        self.assertEqual(isolated(self.status(),browser),
                         dict(seccomp=2,pid_levels=3,seccomp_filters=3,browser_seccomp_filters=2))
        for bad in (self.status(seccomp='0'), self.status(nspid='71\t4'), self.status(filters='2'),
                    self.status(filters='1')):
            with self.subTest(bad=bad), self.assertRaisesRegex(ValueError,'renderer isolation'): isolated(bad,browser)
    def evidence(self, **renderer):
        base = dict(pid=71,seccomp=2,pid_levels=3,seccomp_filters=3,browser_seccomp_filters=2,parents={'71':15,'15':2,'2':1})
        base.update(renderer)
        return dict(argv=['/opt/google/chrome/chrome','--headless=new'],browser_pid=2,renderer=base,browser_label='chrome',
                    local_html='<p>gatebox-local</p>',version='Google Chrome 1',private_desktop={'display':':0'})
    def test_validate_sensor_rejects_weakened_a5_evidence(self):
        validate = self.probe.validate_sensor
        validate('A5',self.evidence())
        for bad in (dict(seccomp=0), dict(pid_levels=2), dict(seccomp_filters=2), dict(browser_seccomp_filters=None),
                    dict(parents={'71':15,'15':9,'9':1})):
            with self.subTest(bad=bad), self.assertRaises((ValueError,TypeError)): validate('A5',self.evidence(**bad))
        unsafe = self.evidence(); unsafe['argv'] = unsafe['argv']+['--no-sandbox']
        with self.assertRaisesRegex(ValueError,'unsafe flags'): validate('A5',unsafe)
        for key in ('local_html','version','renderer','private_desktop'):
            missing = self.evidence(); missing.pop(key)
            with self.subTest(missing=key), self.assertRaisesRegex(ValueError,'missing A5'): validate('A5',missing)
    def test_chrome_observation_uses_the_shared_helpers(self):
        tree = ast.parse((HERE/'probe.py').read_text())
        source = ast.unparse(next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='chrome_observation'))
        self.assertIn("chrome_process_is_renderer((path / 'cmdline').read_bytes())",source)
        self.assertIn('select_renderer(candidates, parents, proc.pid, status(proc.pid))',source)

class Q27Oracles(unittest.TestCase):
    """Q27 (Codex review-2 of GBX-F3): denial over-approximates every Chromium spelling;
    renderer classification follows Chromium's parser exactly (NUL boundaries, one prefix, '--' terminator)."""
    def setUp(self): self.is_renderer = load('probe').chrome_process_is_renderer
    def test_whitespace_and_prefix_variants_are_denied(self):
        for raw in (b'chrome\0--type=renderer\0\t--no-sandbox\0', b'chrome\0--disable-setuid-sandbox\t\0',
                    b'chrome\0 --no-sandbox \0', b'chrome\0\n--no-sandbox\r\0', b'chrome\0---no-sandbox\0',
                    b'chrome\0--\0--no-sandbox\0', b'chrome\0--lang=en --no-sandbox\0',
                    b'chrome --type=renderer\t--no-sandbox\0\0'):
            with self.subTest(raw=raw), self.assertRaisesRegex(ValueError,'unsafe Chrome flag'): self.is_renderer(raw)
    def test_renderer_classification_matches_chromium_parser(self):
        for raw in (b'chrome\0---type=renderer\0', b'chrome\0--\0--type=renderer\0',
                    b'chrome\0--lang=en --type=renderer\0',
                    b'chrome\0--type= renderer\0', b'chrome\0--Type=renderer\0'):
            with self.subTest(raw=raw): self.assertFalse(self.is_renderer(raw))
        for raw in (b'chrome\0--type=renderer\0', b'chrome\0-type=renderer\0', b'chrome\0 --type=renderer\t\0',
                    b'/opt/google/chrome/chrome --type=renderer --lang=en\0\0\0'):
            with self.subTest(raw=raw): self.assertTrue(self.is_renderer(raw))

class Q28Oracles(unittest.TestCase):
    """Q28 (Codex review-3 of GBX-F3): empty NUL fields keep positions; last duplicate switch wins;
    the renderer start-up window (title before Chrome's filter) waits within the deadline."""
    def setUp(self): self.probe = load('probe')
    def test_empty_nul_fields_keep_argument_positions(self):
        is_renderer = self.probe.chrome_process_is_renderer
        self.assertFalse(is_renderer(b'\0--\0--type=renderer\0'))
        self.assertTrue(is_renderer(b'\0--type=renderer\0'))   # empty argv[0] keeps its position
        self.assertTrue(is_renderer(b'chrome\0\0--type=renderer\0'))
    def test_last_duplicate_type_wins_like_chromium(self):
        is_renderer = self.probe.chrome_process_is_renderer
        self.assertTrue(is_renderer(b'chrome\0--type=gpu-process\0--type=renderer\0'))
        self.assertTrue(is_renderer(b'chrome\0--type=renderer\0--type=renderer\0'))
        self.assertFalse(is_renderer(b'chrome\0--type=renderer\0--type=gpu-process\0'))
        self.assertFalse(is_renderer(b'chrome\0--type=renderer\0--type\0'))
    def status(self, filters='18', nspid='71\t4\t1'):
        return {'Seccomp':'\t2','NSpid':'\t'+nspid,'Seccomp_filters':'\t'+filters}
    def test_select_renderer_waits_for_every_sampled_renderer_to_isolate(self):
        select = self.probe.select_renderer
        parents = {71:15,72:15,15:2,2:1,90:1}
        browser = self.status(filters='17')
        self.assertIsNone(select({},parents,2,browser))
        self.assertIsNone(select({71:self.status(filters='17')},parents,2,browser))      # filter not yet installed
        self.assertIsNone(select({71:self.status(),72:self.status(filters='17')},parents,2,browser))
        self.assertIsNone(select({71:self.status(nspid='71\t4')},parents,2,browser))
        self.assertIsNone(select({90:self.status()},parents,2,browser))                  # not a descendant
        chosen = select({71:self.status(),72:self.status(),90:self.status(filters='1')},parents,2,browser)
        self.assertEqual(chosen,dict(seccomp=2,pid_levels=3,seccomp_filters=18,browser_seccomp_filters=17,pid=71,parents=parents))

class Q29Oracles(unittest.TestCase):
    """Q29 (qualification attempt 4): '+/usr/local' as InaccessiblePaths made /usr/local a mode-000
    node; GCC's default /usr/local/include then fails with EACCES ('cc1: error: Permission denied')
    and every C build script (ring) broke. Hide /usr/local with an EMPTY read-only tmpfs instead."""
    def unit(self): return (HERE/'units/mothergate-run@.service').read_text()
    def test_usr_local_is_an_empty_readonly_tmpfs(self):
        self.assertIn('TemporaryFileSystem=/usr/local:ro,nodev,nosuid,noexec,mode=0755,uid=0,gid=0,size=1M,nr_inodes=16\n',self.unit())
    def test_no_inaccessible_entry_covers_toolchain_default_search_paths(self):
        entries = [e.lstrip('-+') for line in self.unit().splitlines() if line.startswith('InaccessiblePaths=')
                   for e in line.split('=',1)[1].split()]
        for searched in ('/usr/local/include','/usr/local/lib','/usr/include','/usr/lib','/usr/libexec/gcc','/usr/bin'):
            for entry in entries:
                with self.subTest(entry=entry,searched=searched):
                    self.assertFalse(searched==entry or searched.startswith(entry.rstrip('/')+'/'),
                                     'inaccessible node on a compiler/linker default search path')

class Q30Oracles(unittest.TestCase):
    """Q30 (qualification attempt 5): the product admits a workspace only when "/", the workspace,
    HOME and the identity home each sit on a WHOLE filesystem mount (device listed once, root "/",
    ext4/xfs/tmpfs). RootDirectory=/usr/lib/gatebox/root made the payload "/" a bind of a subdirectory
    of 259:6, so every governed workspace was refused (unsupported_mount_topology). systemd v259
    namespace.c only self-binds RootDirectory when it is NOT already a mount point, so a per-run
    tmpfs mounted at the root path becomes the payload "/" unchanged."""
    RUN = 'c'*32
    def unit(self, name='mothergate-run@.service'): return (HERE/'units'/name).read_text()
    def test_payload_root_is_a_per_run_mount_owned_by_a_bound_root_unit(self):
        unit = self.unit()
        self.assertIn('RootDirectory=/run/mothergate-root/%i\n',unit)
        self.assertNotIn('RootDirectory=/usr/lib/gatebox/root',unit)
        head = unit.split('[Service]',1)[0]
        self.assertIn('BindsTo=mothergate-root@%i.service\n',head)
        self.assertIn('After=mothergate-root@%i.service\n',head)
    def test_root_unit_mounts_in_the_host_namespace_and_stops_with_the_payload(self):
        unit = self.unit('mothergate-root@.service')
        sections = {}
        for line in unit.splitlines():
            if line.startswith('['): current = sections.setdefault(line,[])
            elif line and not line.startswith('#'): current.append(line)
        self.assertEqual(sections['[Unit]'],['Description=Gate Box payload root %i','StopWhenUnneeded=yes',
                                             'RefuseManualStart=yes','AllowIsolate=no'])
        # Review F6-1 B1: an ALLOWLIST. Any namespacing option (PrivateNetwork= implies PrivateMounts=
        # on systemd 259) would mount the tmpfs in a private namespace the payload never sees.
        # ExecStopPost (not ExecStop) also unwinds a failed or cancelled start.
        self.assertEqual(sections['[Service]'],[
            'Type=oneshot','RemainAfterExit=yes',
            'ExecStart=/usr/bin/python3 -I -B /usr/libexec/gatebox/rootfs.py up %i',
            'ExecStopPost=/usr/bin/python3 -I -B /usr/libexec/gatebox/rootfs.py down %i',
            'PrivateMounts=no','NoNewPrivileges=yes','CapabilityBoundingSet=CAP_SYS_ADMIN','UMask=0077',
            'TimeoutStartSec=30s','TimeoutStopSec=30s',
            'StandardInput=null','StandardOutput=journal','StandardError=journal'])
        self.assertEqual(set(sections),{'[Unit]','[Service]'})
    def test_root_is_its_own_tmpfs_so_no_alias_of_the_root_device(self):
        # Review F6-1 B2 (A1 evidence: '259:6 /usr/lib/gatebox/root/root /root'): ProtectSystem=strict
        # adds /root as a READ_WRITE_IMPLICIT self-bind of the root filesystem, listing the "/" device
        # twice. An explicit tmpfs at /root outranks the implicit entry (namespace.c drop_duplicates).
        unit = self.unit()
        self.assertIn('ProtectSystem=strict\n',unit)
        self.assertIn('TemporaryFileSystem=/root:ro,nodev,nosuid,noexec,mode=0700,uid=0,gid=0,size=1M,nr_inodes=16\n',unit)
    def test_installer_ships_and_publishes_the_root_unit(self):
        scope = installer()
        self.assertTrue({'rootfs.py','units/mothergate-root@.service'} <= set(scope['FILES']))
        files = {'tools/gatebox/'+n:(HERE/n).read_bytes() for n in scope['FILES']}
        calls = []
        scope['run'] = lambda argv: calls.append(argv) or ''
        scope['protected'] = lambda path: None
        with tempfile.TemporaryDirectory() as tmp:
            stage = Path(tmp); stage.chmod(0o700)
            outputs = scope['stage_security'](files,1234,5678,stage)
        units = ['mothergate.socket','mothergate.service','mothergate-run@.service','mothergate-root@.service']
        self.assertEqual([t for s,t in outputs[:4]],[Path('/etc/systemd/system')/n for n in units])
        self.assertEqual(calls[-1],['/usr/bin/systemd-analyze','verify',*[str(stage/n) for n in units]])
        text = (HERE/'install.sh').read_text()
        for fragment in ('publish_units(staged_security[:4])','for staged,target in staged_security[4:]:',
                         "for n in ('mothergate.socket','mothergate.service','mothergate-run@.service','mothergate-root@.service')"):
            self.assertIn(fragment,text)
        doc = (HERE/'README.md').read_text()
        for fragment in ('/etc/systemd/system/mothergate-root@.service',"'mothergate-root@*.service'",'/run/mothergate-root'):
            self.assertIn(fragment,doc)

    # rootfs.py behaviour, offline: a fake skeleton, manifest and mount runner (no real mount).
    def fixture(self, tmp):
        tmp = Path(tmp); skeleton, base = tmp/'skeleton', tmp/'base'
        skeleton.mkdir(mode=0o755); base.mkdir(mode=0o700)
        (skeleton/'etc').mkdir(mode=0o755); (skeleton/'gate').mkdir(mode=0o755); (skeleton/'root').mkdir(mode=0o755)
        (skeleton/'etc/passwd').write_bytes(b'root:x:0:0::/nonexistent:/usr/sbin/nologin\n'); (skeleton/'etc/passwd').chmod(0o644)
        (skeleton/'gate/request.json').write_bytes(b''); (skeleton/'gate/request.json').chmod(0o444)
        (skeleton/'bin').symlink_to('usr/bin')
        manifest = {'bin':{'link':'usr/bin'},'etc':{'directory':True},'etc/passwd':{'sha256':hashlib.sha256((skeleton/'etc/passwd').read_bytes()).hexdigest()},
                    'gate':{'directory':True},'gate/request.json':{'sha256':hashlib.sha256(b'').hexdigest()},'root':{'directory':True}}
        (tmp/'skeleton.json').write_text(json.dumps(manifest)); (tmp/'skeleton.json').chmod(0o644)
        return skeleton, tmp/'skeleton.json', base, manifest
    @staticmethod
    def fake_mount(calls):
        # Offline stand-in for mount(8): unmounting a tmpfs discards everything copied into it.
        def mount(argv):
            calls.append(argv)
            if argv[0] == '/usr/bin/umount':
                for child in Path(argv[1]).iterdir():
                    if child.is_dir() and not child.is_symlink():
                        for base, dirs, files in os.walk(child): os.chmod(base,0o700)
                        import shutil; shutil.rmtree(child)
                    else: child.unlink()
        return mount
    def up(self, rootfs, skeleton, manifest, base, mounted=None):
        calls = []
        def mount(argv): calls.append(argv)
        rootfs.up(self.RUN,skeleton=skeleton,manifest=manifest,base=base,owner=os.getuid(),mount=mount,
                  ismount=(lambda path: True) if mounted is None else mounted)
        return calls
    def test_rootfs_copies_exactly_the_manifest_then_seals_read_only(self):
        rootfs = load('rootfs')
        with tempfile.TemporaryDirectory() as tmp:
            skeleton, manifest, base, expected = self.fixture(tmp)
            calls = self.up(rootfs,skeleton,manifest,base)
            target = base/self.RUN
            self.assertEqual(calls,[['/usr/bin/mount','-t','tmpfs','-o',rootfs.OPTIONS,'gatebox-root',str(target)],
                                    ['/usr/bin/mount','-o','remount,ro,'+rootfs.OPTIONS,str(target)]])
            self.assertIn('mode=0755',rootfs.OPTIONS); self.assertIn('nodev',rootfs.OPTIONS); self.assertIn('nosuid',rootfs.OPTIONS)
            self.assertEqual(rootfs.inventory(target),expected)
            self.assertEqual(os.readlink(target/'bin'),'usr/bin')
            self.assertEqual(oct((target/'gate/request.json').stat().st_mode & 0o777),'0o444')
            self.assertEqual(oct((target/'etc').stat().st_mode & 0o777),'0o755')
            self.assertEqual(oct(target.stat().st_mode & 0o777),'0o755')
    def test_rootfs_refuses_any_skeleton_drift_and_unwinds(self):
        rootfs = load('rootfs')
        def changed(s): (s/'etc/passwd').write_bytes(b'root::0:0::/:/bin/sh\n')
        def extra(s): (s/'etc/shadow').write_bytes(b'x')
        def relinked(s): (s/'bin').unlink(); (s/'bin').symlink_to('/tmp')
        def retyped(s): (s/'bin').unlink(); (s/'bin').mkdir()
        def writable(s): (s/'etc').chmod(0o775)
        def missing(s): (s/'gate/request.json').unlink()
        for tamper in (changed,extra,relinked,retyped,writable,missing):
            with self.subTest(tamper=tamper.__name__), tempfile.TemporaryDirectory() as tmp:
                skeleton, manifest, base, _ = self.fixture(tmp)
                tamper(skeleton)
                calls = []
                with self.assertRaises(ValueError):
                    rootfs.up(self.RUN,skeleton=skeleton,manifest=manifest,base=base,owner=os.getuid(),
                              mount=self.fake_mount(calls),ismount=lambda path: True)
                self.assertEqual(calls[-1],['/usr/bin/umount',str(base/self.RUN)])
                self.assertFalse((base/self.RUN).exists())
    def test_rootfs_refuses_a_root_that_is_not_exactly_the_manifest(self):
        # A mount that does not yield an empty fresh tmpfs (stray entry) must never become "/".
        rootfs = load('rootfs')
        with tempfile.TemporaryDirectory() as tmp:
            skeleton, manifest, base, _ = self.fixture(tmp)
            calls = []
            inner = self.fake_mount(calls)
            def mount(argv):
                inner(argv)
                if argv[:3] == ['/usr/bin/mount','-t','tmpfs']: (Path(argv[-1])/'stray').write_bytes(b'x')
            with self.assertRaisesRegex(ValueError,'differs from manifest'):
                rootfs.up(self.RUN,skeleton=skeleton,manifest=manifest,base=base,owner=os.getuid(),
                          mount=mount,ismount=lambda path: True)
            self.assertEqual(calls[-1],['/usr/bin/umount',str(base/self.RUN)])
            self.assertFalse((base/self.RUN).exists())
    def test_rootfs_refuses_bad_identity_layout_and_reuse(self):
        rootfs = load('rootfs')
        for run in ('C'*32,'c'*31,'../'+'c'*29,'c'*32+'\n'):
            with self.subTest(run=run), tempfile.TemporaryDirectory() as tmp:
                skeleton, manifest, base, _ = self.fixture(tmp)
                with self.assertRaises(ValueError):
                    rootfs.up(run,skeleton=skeleton,manifest=manifest,base=base,owner=os.getuid(),mount=self.fail,ismount=lambda p: True)
        for layout in ('reused','open-base','linked-base'):
            with self.subTest(layout=layout), tempfile.TemporaryDirectory() as tmp:
                skeleton, manifest, base, _ = self.fixture(tmp)
                if layout == 'reused': (base/self.RUN).mkdir()
                elif layout == 'open-base': base.chmod(0o755)
                else: base.rmdir(); base.symlink_to(skeleton)
                with self.assertRaises((ValueError,OSError)):
                    rootfs.up(self.RUN,skeleton=skeleton,manifest=manifest,base=base,owner=os.getuid(),mount=self.fail,ismount=lambda p: True)
    def test_rootfs_down_unmounts_then_removes_and_is_idempotent(self):
        rootfs = load('rootfs')
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)/'base'; base.mkdir(mode=0o700); (base/self.RUN).mkdir(mode=0o755)
            calls = []
            rootfs.down(self.RUN,base=base,owner=os.getuid(),mount=calls.append,ismount=lambda path: True)
            self.assertEqual(calls,[['/usr/bin/umount',str(base/self.RUN)]])
            self.assertFalse((base/self.RUN).exists())
            rootfs.down(self.RUN,base=base,owner=os.getuid(),mount=self.fail,ismount=lambda path: False)
            with self.assertRaises(ValueError): rootfs.down('x',base=base,owner=os.getuid(),mount=self.fail,ismount=lambda p: False)

    # A1 sensor: the product's whole-filesystem rule, enforced on the live payload.
    def mount(self, device, root, point, kind='tmpfs', options=('ro','nosuid','nodev')):
        return dict(device=device,root=root,point=point,options=list(options),type=kind,super=['rw'])
    def test_a1_requires_whole_filesystem_root_work_and_tmp(self):
        probe = load('probe')
        good = [self.mount('0:80','/','/'),self.mount('259:6','/usr','/usr','ext4'),
                self.mount('0:81','/','/work',options=('rw','nosuid','nodev')),self.mount('0:82','/','/tmp',options=('rw','nosuid','nodev'))]
        probe.whole_mount(good,'/',readonly=True)
        probe.whole_mount(good,'/work'); probe.whole_mount(good,'/tmp')
        bind = [self.mount('259:6','/usr/lib/gatebox/root','/','ext4'),self.mount('259:6','/usr','/usr','ext4')]
        stacked = [self.mount('259:6','/usr/lib/gatebox/root','/','ext4'),self.mount('0:80','/','/')]
        shared = good+[self.mount('0:80','/etc','/gate/policy')]
        writable = [self.mount('0:80','/','/',options=('rw',))]+good[1:]
        subtree = [self.mount('0:80','/sub','/')]+good[1:]
        for records in (bind,stacked,shared,writable,subtree):
            with self.subTest(records=records), self.assertRaises(ValueError): probe.whole_mount(records,'/',readonly=True)
        with self.assertRaises(ValueError): probe.whole_mount(good[:1],'/work')
        text = (HERE/'probe.py').read_text()
        body = text.split('def isolation(context):',1)[1].split('\ndef ',1)[0]
        for call in ("whole_mount(records,'/',readonly=True)","whole_mount(records,'/work')","whole_mount(records,'/tmp')"):
            self.assertIn(call,body)

class Q31Oracles(unittest.TestCase):
    """Q31 (qualification attempt 5, kernel desktop_tripwire_symlinks g0_s7): payload PID 1 reaped
    orphans only in its final block, so a killed grandchild stayed a zombie and `kill -0` still saw
    it. A host init reaps adopted orphans promptly; Popen keeps sole ownership of its own children."""
    def test_adopted_orphan_is_reaped_while_the_command_runs(self):
        init = load('init')
        options = init.child_options
        read, write = os.pipe()
        pid = os.fork()
        if pid == 0:
            result = 91
            try:
                os.close(read)
                import ctypes
                if ctypes.CDLL(None,use_errno=True).prctl(36,1,0,0,0) != 0: os._exit(90)  # PR_SET_CHILD_SUBREAPER
                runner = init.Runner(-1,-1)
                with tempfile.TemporaryDirectory() as directory, \
                        patch.object(init,'child_options',side_effect=lambda fd: dict(options(fd),cwd=directory)), \
                        patch.object(init,'packet'):
                    _,code,output = runner.command('format',['/bin/sh','-c','( sleep 0.2 & echo $! ); sleep 1.5; exit 7'])
                orphan = int(output.split()[0])
                try: state = Path('/proc',str(orphan),'stat').read_text().rsplit(')',1)[1].split()[0]
                except FileNotFoundError: state = 'reaped'
                os.write(write,json.dumps([code,state]).encode()); result = 0
            finally: os._exit(result)
        os.close(write)
        with os.fdopen(read,'rb') as stream: data = stream.read()
        _, status = os.waitpid(pid,0)
        self.assertEqual(status,0)
        self.assertEqual(json.loads(data),[7,'reaped'])
    def test_owned_children_are_never_waited_by_the_reaper(self):
        init = load('init'); runner = init.Runner(-1,-1)
        owned = type('Owned',(),{'pid':4242})(); runner.children.add(owned)
        calls = []
        with patch.object(init.os,'listdir',return_value=[str(os.getpid())]), \
                patch.object(init,'children',return_value='4242 5151\n'), \
                patch.object(init.os,'waitpid',side_effect=lambda pid,flags: calls.append((pid,flags)) or (0,0)):
            runner.reap_orphans()
        self.assertEqual(calls,[(5151,os.WNOHANG)])
    def test_unreadable_child_list_fails_closed(self):
        init = load('init'); runner = init.Runner(-1,-1)
        with patch.object(init.os,'listdir',return_value=[str(os.getpid())]), \
                patch.object(init,'children',side_effect=FileNotFoundError('children')), \
                self.assertRaises(OSError):
            runner.reap_orphans()
    def test_command_loop_reaps(self):
        text = (HERE/'init.py').read_text()
        loop = text.split('while selector.get_map():',1)[1].split('code = proc.wait(',1)[0]
        self.assertIn('self.reap_orphans()',loop)

# Q32 (review F6-1 B3) pinned 0c8cb6d3 = 899e3872 + the o18_d/o18_g fake-/sys commit. Q34 moved the
# pin forward to a descendant (8e0d6ae2); Q34Oracles now holds the single-SHA oracle, and also
# requires both earlier pins to be absent.

LIBTEST_TOTAL = 'test result: ok. {} passed; 0 failed; {} ignored; 0 measured; 0 filtered out; finished in 0.01s\n'

def libtest_run(lines, passed, ignored, running=None):
    count = passed+ignored if running is None else running
    return ('\nrunning %d test%s\n' % (count,'' if count == 1 else 's')+''.join(l+'\n' for l in lines)
            +'\n'+LIBTEST_TOTAL.format(passed,ignored))

class Q33Oracles(unittest.TestCase):
    """Q33 (diag15 evidence): the frozen libtest adapter rejected three standard libtest forms the
    fixture uses: #[ignore] tests ("test x ... ignored[, reason]", counted in totals), the
    #[should_panic] marker ("test x - should panic ... ok") and test targets with zero tests.
    Ruling: reconcile them truthfully. Ignored names come from the run, must be listed names, are
    recorded as ignored (never as passed) and must match the totals; every other listed test must
    pass. A zero-test target must prove libtest's own zero-test run. Anything else stays refused."""
    def setUp(self): self.init = load('init')
    def test_should_panic_marker_is_the_listed_name(self):
        out = libtest_run(['test a ... ok','test b - should panic ... ok'],2,0)
        self.assertEqual(self.init.reconcile(['a','b'],out)['passed'],['a','b'])
        for bad in (out.replace('b - should panic','b - should panicX'),
                    out.replace('test b - should panic','test c - should panic')):
            with self.subTest(bad=bad), self.assertRaises(ValueError): self.init.reconcile(['a','b'],bad)
    def test_ignored_tests_are_recorded_not_passed(self):
        out = libtest_run(['test a ... ok','test b ... ignored','test c ... ignored, child entry; run by the parent','test d ... ok'],2,2)
        inventory = self.init.reconcile(['a','b','c','d'],out)
        self.assertEqual(inventory,{'expected':['a','b','c','d'],'executed':['a','d'],'passed':['a','d'],
                                    'failed':[],'ignored':['b','c'],'filtered':[]})
        out = libtest_run(['test a - should panic ... ignored','test b ... ok'],1,1)
        self.assertEqual(self.init.reconcile(['a','b'],out)['ignored'],['a'])
    def test_ignored_negatives(self):
        names = ['a','b','c']
        good = ['test a ... ok','test b ... ok','test c ... ignored']
        cases = {'ignored-not-listed':libtest_run(good[:2]+['test z ... ignored'],2,1,running=3),
                 'totals-ignored-count':libtest_run(good,2,0,running=3),
                 'totals-passed-count':libtest_run(good,3,1,running=3),
                 'missing-ignored-line':libtest_run(good[:2],2,1,running=3),
                 'duplicate-line':libtest_run(good+['test c ... ignored'],2,2,running=3),
                 'ok-and-ignored':libtest_run(good+['test a ... ignored'],2,2,running=3),
                 'failed':libtest_run(good[:1]+['test b ... FAILED','test c ... ignored'],1,1,running=3),
                 'unknown-result':libtest_run(good[:2]+['test c ... bench'],2,0,running=3),
                 'running-count':libtest_run(good,2,1,running=2),
                 'two-totals':libtest_run(good,2,1)+LIBTEST_TOTAL.format(2,1),
                 'no-running':libtest_run(good,2,1).replace('running 3 tests\n',''),
                 'all-ignored-allowed-but-filtered':libtest_run(good,2,1).replace('0 filtered out','1 filtered out')}
        for case, out in cases.items():
            with self.subTest(case=case), self.assertRaises(ValueError): self.init.reconcile(names,out)
        with self.assertRaises(ValueError): self.init.reconcile(['a','a'],libtest_run(['test a ... ok','test a ... ok'],2,0))
    def test_zero_test_target_needs_libtest_zero_run(self):
        empty = libtest_run([],0,0)
        self.assertEqual(self.init.reconcile([],empty),{'expected':[],'executed':[],'passed':[],'failed':[],'ignored':[],'filtered':[]})
        for bad in ('','custom harness ran 12 checks\n',empty.replace('running 0 tests\n',''),
                    empty.replace('0 passed','1 passed'),libtest_run(['test a ... ok'],1,0)):
            with self.subTest(bad=bad), self.assertRaises(ValueError): self.init.reconcile([],bad)
    def run_libtest(self, listing, output, list_code=0, run_code=0, expected=None):
        calls = []
        def command(gate, argv):
            calls.append((gate,argv))
            return ('c%d' % len(calls),list_code,listing) if '--list' in argv else ('c%d' % len(calls),run_code,output)
        runner = self.init.Runner(-1,-1)
        with patch.object(runner,'command',side_effect=command), patch.object(runner,'output'):
            result = runner.libtest('workspace',{'executable':'/work/target/t'},expected)
        return result, calls
    def test_runner_runs_zero_test_targets_and_ignored_targets(self):
        names, calls = self.run_libtest('',libtest_run([],0,0))
        self.assertEqual(names,[])
        self.assertEqual([argv for _, argv in calls],[['/work/target/t','--list','--format','terse'],['/work/target/t']])
        names, _ = self.run_libtest('a: test\nb: test\n',libtest_run(['test a ... ok','test b ... ignored'],1,1))
        self.assertEqual(names,['a','b'])
    def test_runner_still_refuses_unknown_adapters(self):
        for listing, code in (('a: test\n',2),('a: benchmark\n',0),('a: test\nnot a listing line\n',0)):
            with self.subTest(listing=listing,code=code), self.assertRaises(ValueError):
                self.run_libtest(listing,libtest_run(['test a ... ok'],1,0),list_code=code)
        with self.assertRaises(ValueError): self.run_libtest('a: test\n',libtest_run(['test a ... ok'],1,0),run_code=101)
    def test_malformed_listing_is_never_an_empty_inventory(self):
        # review F6-3 B1: validation and extraction must agree; a CR line or a nameless entry is
        # not a zero-test target even when the run then prints libtest's zero-test transcript.
        for listing in ('hidden_test: test\r\n',': test\n','a: test\r\nb: test\n',' : test\n','a b: test\n'):
            with self.subTest(listing=listing), self.assertRaises(ValueError):
                self.run_libtest(listing,libtest_run([],0,0))
    def test_ignore_reason_may_contain_the_delimiter(self):
        out = libtest_run(['test a ... ok','test b ... ignored, waits ... for the parent'],1,1)
        self.assertEqual(self.init.reconcile(['a','b'],out)['ignored'],['b'])
    def test_desktop_live_keeps_strict_names_and_no_ignored(self):
        expected = ['a','b']
        self.run_libtest('a: test\nb: test\n',libtest_run(['test a ... ok','test b ... ok'],2,0),expected=expected)
        with self.assertRaises(ValueError):
            self.run_libtest('a: test\nb: test\n',libtest_run(['test a ... ok','test b ... ignored'],1,1),expected=expected)
    def test_libtest_results_are_parsed_from_stdout_only(self):
        # diag15 c75: a test's helper child panicked on the shared stderr while libtest had written
        # "test x ... " but not yet "ok", splitting the result line. libtest reports results on stdout
        # only; stderr still reaches the log but never the parsed output (as for compile-tests).
        options = self.init.child_options
        script = ('import os,sys,time\nos.write(1,b"test a ... ")\ntime.sleep(.05)\n'
                  'os.write(2,b"child panicked\\n")\ntime.sleep(.05)\nos.write(1,b"ok\\n")\n')
        for gate in ('workspace','workspace-list','desktop-live','desktop-live-list'):
            runner = self.init.Runner(-1,-1)
            logged = []
            with tempfile.TemporaryDirectory() as directory, \
                    patch.object(self.init,'child_options',side_effect=lambda fd: dict(options(fd),cwd=directory)), \
                    patch.object(self.init,'packet'), patch.object(runner,'output',side_effect=lambda n,b: logged.append(b)):
                _, code, output = runner.command(gate,[sys.executable,'-I','-c',script])
            with self.subTest(gate=gate):
                self.assertEqual((code,output),(0,'test a ... ok\n'))
                self.assertIn(b'child panicked\n',b''.join(logged))
    def test_supervisor_host_inventory_keeps_ignored_separate(self):
        supervisor = load('supervisor')
        artifacts = [dict(id='a0',adapter='libtest',package='nexus-mother-host',target='h1',executable='/work/target/h1'),
                     dict(id='a1',adapter='libtest',package='nexus-mother-host',target='h2',executable='/work/target/h2'),
                     dict(id='a2',adapter='libtest',package='nexus-mother-host',target='h3',executable='/work/target/h3')]
        inventories = [dict(expected=['x','y'],executed=['x'],passed=['x'],failed=[],ignored=['y'],filtered=[]),
                       dict(expected=['z'],executed=['z'],passed=['z'],failed=[],ignored=[],filtered=[]),
                       dict(expected=[],executed=[],passed=[],failed=[],ignored=[],filtered=[])]
        evidence = type('E',(),{})()
        evidence.commands = [dict(id='c%d' % i,gate_id='workspace') for i in range(3)]+[dict(id='c3',gate_id='host')]
        logs = {'coverage.log':json.dumps(dict(artifacts=artifacts,failures=[])).encode()}
        for i in range(4): logs['c%d.log' % i] = b'x'
        for i in range(3): logs['c%d-inventory.log' % i] = json.dumps(dict(artifact=artifacts[i],inventory=inventories[i])).encode()
        commands, _ = supervisor.coverage(evidence,logs)
        self.assertEqual(commands[3]['inventory'],dict(expected=['x','y','z'],executed=['x','z'],passed=['x','z'],
                                                       failed=[],ignored=['y'],filtered=[]))

class Q34Oracles(unittest.TestCase):
    """Q34 (diag15 evidence): fixture test decision9_host_port_does_not_adopt_legacy_replay asked
    DNS for example.org inside the network-less payload. The fixture is re-pinned to 8e0d6ae2 =
    0c8cb6d3 + one test-only commit using a public address literal. One SHA everywhere."""
    FIXTURE = '8e0d6ae293df999eb6530e875df1206deac872a8'
    def test_one_fixture_sha_everywhere(self):
        self.assertEqual(installer()['FIXTURE'],self.FIXTURE)
        self.assertIn("FIXTURE = '"+self.FIXTURE+"'",(HERE/'probe.py').read_text())
        self.assertEqual(json.loads((HERE/'policy/registry.json').read_text())['mother']['fixture_sha'],self.FIXTURE)
        doc = (HERE/'README.md').read_text()
        self.assertIn('--fixture-sha '+self.FIXTURE,doc); self.assertIn('--sha '+self.FIXTURE,doc)
        for old in ('899e3872c5b61223c87b905f63c66947a816259b','0c8cb6d360b1296262b7a5c478261e5e47e5c7f5'):
            for name in ('install.sh','probe.py','policy/registry.json','README.md','init.py','supervisor.py','client.py'):
                self.assertNotIn(old,(HERE/name).read_text(),name)

class Q35Oracles(unittest.TestCase):
    """Q35 (post-merge, F6): F6 added rootfs.py and the root unit to FILES (29), but the README PIN
    recipe still refused any inventory other than 27, so the reviewed bundle could not be built.
    The recipe's count is the installer's FILES length, always."""
    def test_readme_pin_count_is_the_installer_inventory(self):
        counts = re.findall(r"len\(values\['FILES'\]\)!=(\d+)",(HERE/'README.md').read_text())
        self.assertEqual(counts,[str(len(installer()['FILES']))])

class Q36Oracles(unittest.TestCase):
    """Q36 (review Q35-1, inherited from F6): the qualification probe checked the effective
    configuration of only the original three units, so a drop-in broadening mothergate-root@
    (root, CAP_SYS_ADMIN) would escape it. The root template is checked like the others."""
    RUN = 'a'*32
    def values(self):
        return {'mothergate.socket':dict(FragmentPath='/etc/systemd/system/mothergate.socket'),
                'mothergate.service':dict(FragmentPath='/etc/systemd/system/mothergate.service'),
                'mothergate-run@'+self.RUN+'.service':dict(FragmentPath='/etc/systemd/system/mothergate-run@.service',
                    User='gaterun',PrivatePIDs='yes',PrivateNetwork='yes',PrivateIPC='yes',NoNewPrivileges='yes',
                    BindLogSockets='no',SystemCallArchitectures='native'),
                'mothergate-root@'+self.RUN+'.service':dict(FragmentPath='/etc/systemd/system/mothergate-root@.service',
                    User='',NoNewPrivileges='yes',PrivateMounts='no',PrivateNetwork='no',CapabilityBoundingSet='cap_sys_admin')}
    def run_probe(self, values):
        probe = load('probe'); asked = []
        def capture(argv, **_):
            asked.append(argv[2])
            unit = dict(values[argv[2]]); unit.setdefault('DropInPaths','')
            return ''.join(k+'='+v+'\n' for k,v in unit.items())
        with patch.object(probe,'capture',side_effect=capture): probe.effective_units()
        return asked
    def test_root_template_is_checked(self):
        self.assertIn('mothergate-root@'+self.RUN+'.service',self.run_probe(self.values()))
    def test_root_template_overrides_are_refused(self):
        name = 'mothergate-root@'+self.RUN+'.service'
        for key, value in (('DropInPaths','/etc/systemd/system/mothergate-root@.service.d/x.conf'),
                           ('FragmentPath','/run/systemd/system/mothergate-root@.service'),
                           ('CapabilityBoundingSet','cap_sys_admin cap_dac_override'),('NoNewPrivileges','no'),
                           ('PrivateMounts','yes'),('PrivateNetwork','yes'),('User','gaterun')):
            values = self.values(); values[name][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): self.run_probe(values)

class Q21Oracles(unittest.TestCase):
    def test_publication_dirs_share_one_supervisor_mount(self):
        # Q21 D1 evidence: separate ReadWritePaths for pending/ and receipts/
        # became two bind mounts of one device in the supervisor namespace, so
        # receipt.publish's renameat2 failed with EXDEV and no receipt (PASS or
        # FAIL) could ever publish. One mount at the root-owned parent keeps
        # DAC unchanged (parent root 0755) and makes the rename same-mount.
        unit = (HERE/'units/mothergate.service').read_text().splitlines()
        state = [l for l in unit if l.startswith('ReadWritePaths=') and 'mothergate-control' not in l]
        self.assertEqual(state,['ReadWritePaths=/var/lib/mothergate'])
        receipt = (HERE/'receipt.py').read_text()
        self.assertIn('rename(source,run_id.encode(),target,run_id.encode(),1)',receipt)
        self.assertIn("for name in ('staging','pending','receipts'):",(HERE/'install.sh').read_text())

class Q8F3Oracles(unittest.TestCase):
    def test_private_stage_validates_before_publish(self):
        scope = installer()
        files = {'tools/gatebox/'+n:(HERE/n).read_bytes() for n in scope['FILES']}
        with tempfile.TemporaryDirectory() as tmp, patch.dict(scope,protected=lambda p: None):
            stage = Path(tmp); os.chmod(stage,0o700); calls = []
            def validate(argv):
                paths = argv[3:] if argv[1]=='verify' else argv[2:]
                self.assertTrue(paths)
                for p in paths: self.assertEqual(Path(p).parent,stage); self.assertTrue(Path(p).is_file())
                calls.append(argv)
            with patch.dict(scope,run=validate):
                staged = scope['stage_security'](files,1234,5678,stage)
            self.assertEqual(len(staged),6)  # Q30: four units (root unit added) + sudoers + polkit.
            self.assertEqual({c[0] for c in calls},{'/usr/sbin/visudo','/usr/bin/systemd-analyze'})
            self.assertEqual(os.stat(stage).st_mode & 0o777,0o700)
            for source,target in staged: self.assertEqual(source.parent,stage)
        text = (HERE/'install.sh').read_text()
        self.assertIn('os.rename(staged,target)',text)

class Q8F4Oracles(unittest.TestCase):
    def test_preflight_before_mutation_and_manual_removal(self):
        text = (HERE/'install.sh').read_text()
        self.assertLess(text.index('preflight_accounts(args.submit_user)'),text.index('apply_accounts(' ,text.index('def main():')))
        self.assertLess(text.index('stage_security(files,' ,text.index('def main():')),text.index('apply_accounts(',text.index('def main():')))
        doc = (HERE/'README.md').read_text()
        for fragment in ('Manual removal','systemctl disable --now mothergate.socket',
                         'systemctl stop mothergate.service',"if group is not None and 'nexus' in group.gr_mem:",
                         "subprocess.run(['/usr/bin/gpasswd','-d','nexus','gatesubmit'],check=True)",
                         '/etc/gatebox/created-accounts.json',"subprocess.run(['/usr/sbin/userdel',name],check=True)",'/var/lib/gatebox-source',
                         '/usr/lib/gatebox/root','/etc/polkit-1/rules.d/00-gatebox-deny.rules'):
            self.assertIn(fragment,doc)

class Q8F6Oracles(unittest.TestCase):
    def test_active_non_native_abi_and_sensor_binding(self):
        import signal
        probe = load('probe')
        for status,expected in ((signal.SIGSYS,'SIGSYS'),(0,'EPERM'),(256,None)):
            with patch.object(probe.os,'fork',return_value=777), patch.object(probe.os,'waitpid',return_value=(777,status)):
                if expected: self.assertEqual(probe.non_native_abi(),expected)
                else:
                    with self.assertRaises(ValueError): probe.non_native_abi()
        text = (HERE/'probe.py').read_text()
        self.assertIn('int 0x80',text)
        self.assertIn('non_native_abi()',text[text.index('def syscall_probe'):text.index('def process_identity')])
        value = dict(tiocsti_denied=True,host_hidden=True,reaped=True,syscalls=dict(openat2=True,outer={'p':'EPERM'},inner={'p':'ENOSYS'}))
        with self.assertRaises(ValueError): probe.validate_sensor('A6',value)
        value['syscalls']['non_native_abi']='SIGSYS'; probe.validate_sensor('A6',value)

class Q8F7Oracles(unittest.TestCase):
    def test_clients_heuristic_and_claude_shell_prefix(self):
        doc = (HERE/'README.md').read_text()
        self.assertIn('process-ancestry heuristic',doc)
        self.assertIn('!sudo /usr/bin/python3 -I -B -m unittest discover',doc)
        self.assertIn('not an attestation',doc)

class Q8F8Oracles(unittest.TestCase):
    def test_archive_uses_hashed_buffer(self):
        import io
        scope = installer(); path = unittest.mock.Mock()
        path.stat.return_value.st_size = 0; path.read_bytes.return_value = b'checked bytes'
        class Stop(Exception): pass
        def opened(*args,**kwargs):
            self.assertEqual(args,())
            self.assertIsInstance(kwargs['fileobj'],io.BytesIO)
            self.assertEqual(kwargs['fileobj'].getvalue(),b'checked bytes')
            raise Stop
        with patch.object(scope['tarfile'],'open',side_effect=opened):
            with self.assertRaises(Stop): scope['bundle_files'](path,hashlib.sha256(b'checked bytes').hexdigest())

class Q8F9Oracles(unittest.TestCase):
    def test_reproducible_bundle_and_extraction_contract(self):
        doc = (HERE/'README.md').read_text()
        for fragment in ("tar --exclude='__pycache__'",'--files-from=-','members=members',
                         "for parent in path.parents",'§10','needs `--bundle-sha256'):
            self.assertIn(fragment,doc)

class Q8F10Oracles(unittest.TestCase):
    def test_synthetic_machine_identity(self):
        text = (HERE/'install.sh').read_text()
        self.assertIn("write(ROOT/'etc/machine-id',b'0123456789abcdef0123456789abcdef\\n')",text)
        self.assertNotIn("('ld.so.cache','machine-id'",text)

class Q8F12Oracles(unittest.TestCase):
    def test_account_collisions_and_existing_submit_members(self):
        from types import SimpleNamespace as NS
        scope = installer()
        users = [NS(pw_name=n,pw_uid=u,pw_gid=g,pw_shell='/usr/sbin/nologin') for n,u,g in
                 [('nexus',1000,1000),('mothergate',901,901),('gaterun',902,902)]]
        groups = [NS(gr_name=n,gr_gid=g,gr_mem=m) for n,g,m in
                  [('nexus',1000,[]),('mothergate',901,[]),('gaterun',902,[]),('gatesubmit',903,['nexus'])]]
        def check(): return scope['preflight_accounts']('nexus')
        with patch.object(scope['pwd'],'getpwall',return_value=users), patch.object(scope['grp'],'getgrall',return_value=groups), \
                patch.object(scope['os'],'getgrouplist',side_effect=lambda n,g:[g]), patch.dict(scope,run=lambda argv: b'name L'):
            check()
            for extra in (NS(pw_name='alias',pw_uid=901,pw_gid=999),NS(pw_name='alias',pw_uid=1000,pw_gid=999),
                          NS(pw_name='intruder',pw_uid=1111,pw_gid=903)):
                users.append(extra)
                with self.assertRaises(ValueError): check()
                users.pop()
            groups[-1].gr_mem.append('intruder')
            with self.assertRaises(ValueError): check()

class Q8F13Oracles(unittest.TestCase):
    def test_documented_host_effects_and_prerequisites(self):
        doc = (HERE/'README.md').read_text()
        for fragment in ("bash -euo pipefail <<'OWNER'",'persists at boot','Host changes',
                         '/etc/systemd/system/mothergate-run@.service','/run/mothergate-control',
                         '/var/lib/mothergate',"subprocess.run(['/usr/sbin/groupdel',name],check=True)","('groups',{'mothergate','gaterun','gatesubmit'})"):
            self.assertIn(fragment,doc)
        text = (HERE/'install.sh').read_text()
        tree = ast.parse(text.split("<<'PY'\n",1)[1].rsplit('\nPY',1)[0])
        prerequisites = next(ast.literal_eval(n.iter) for n in ast.walk(tree) if isinstance(n,ast.For)
                             and isinstance(n.target,ast.Name) and n.target.id=='executable')
        self.assertIn('xdotool',prerequisites)
