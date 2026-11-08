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
from test_installed import load
import copy
import hashlib
import json
import inspect
from types import SimpleNamespace

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

class Q13Oracles(unittest.TestCase):
    """Frozen Q13 offline decisions; never execute a host command."""
    custody = {'C1-SEALED','C2-CANARY','C3-RESTART'}
    rows = {'B1','B2','B3','B4','A1','A5','A6','A8','E2E','CLIENTS'} | custody

    def evidence(self):
        identity = {'bundle_sha256':'a'*64,'policy_sha256':'b'*64,'gates_sha256':'c'*64}
        summary = dict(status='PASS_SCOPED',request=dict(repo_id='mother',
            full_sha=load('probe').FIXTURE,gate_set_id='full'),installed=dict(identity,stage='qualifying'),
            run_id='a'*32,peer_uid=1234)
        value = dict(identity=identity,passed=sorted(self.rows),evidence={row:dict(run_id='a'*32,
            summary_sha256=hashlib.sha256(json.dumps(summary,sort_keys=True).encode()).hexdigest()) for row in self.rows})
        value['evidence']['CLIENTS']['client_provenance'] = dict(uid=1234,context='claude',ancestor_pid=456,
            accepted=dict(status='ACCEPTED',run_id='a'*32),refused=dict(status='UNQUALIFIED',reason='QUALIFICATION_ONLY'),
            codex='unsupported (runner denies AF_UNIX)')
        for row in self.custody:
            value['evidence'][row]['custody'] = q15_proof(row)
        return identity,summary,value

    def test_acceptance_requires_each_custody_row_and_passing_evidence(self):
        identity,summary,value = self.evidence()
        check = installer()['acceptance_check']; sensors=load('probe')
        sensors.boot_id=Mock(return_value=BOOT_AFTER); sensors.clean_custody=Mock(return_value=True)
        def accept(v): check(identity,v,dict(identity=identity,stage='qualifying'),lambda r: {'summary':summary},sensors=sensors)
        accept(value)
        for row in self.custody:
            for variant in ('missing','no-proof','failed','empty','wrong-row'):
                with self.subTest(row=row,variant=variant):
                    bad = copy.deepcopy(value)
                    if variant=='missing':
                        bad['passed'].remove(row); del bad['evidence'][row]
                    elif variant=='no-proof': del bad['evidence'][row]['custody']
                    elif variant=='failed': bad['evidence'][row]['custody']['passed'] = False
                    elif variant=='empty': bad['evidence'][row]['custody']['observations'] = {}
                    else: bad['evidence'][row]['custody']['row'] = 'C0'
                    with self.assertRaises(ValueError): accept(bad)
        legacy = copy.deepcopy(value)
        for row in self.custody:
            legacy['passed'].remove(row); del legacy['evidence'][row]
        with self.assertRaises(ValueError): accept(legacy)

    def test_probe_requires_custody_rows_and_refuses_unrun_proofs(self):
        probe = load('probe')
        self.assertEqual(set(getattr(probe,'REQUIRED_ROWS',())),self.rows)
        self.assertTrue(callable(getattr(probe,'custody_oracle',None)))
        identity,summary,_ = self.evidence()
        with tempfile.TemporaryDirectory() as tmp, ExitStack() as stack:
            stack.enter_context(patch.object(probe,'CONFIG',Path(tmp)))
            stack.enter_context(patch.object(probe,'require_installed'))
            stack.enter_context(patch.object(probe,'fixture',return_value=(summary,{})))
            stack.enter_context(patch.object(probe,'installed_identity',return_value=identity))
            stack.enter_context(patch.object(probe,'protected'))
            stack.enter_context(patch.object(probe,'boot_id',return_value=BOOT_BEFORE))
            stack.enter_context(patch.object(probe,'clean_custody',return_value=True))
            stack.enter_context(patch.object(probe,'config',side_effect=lambda n: identity if n=='identity.json' else
                dict(identity=identity,passed=sorted(self.rows-{'C2-CANARY'}),evidence={})))
            write = stack.enter_context(patch.object(probe,'atomic_config'))
            for row in self.custody:
                result = q15_proof(row)
                with patch.object(probe,'custody_oracle',return_value=result) as run:
                    probe.qualify(row)
                    run.assert_called_once_with(row)
                    self.assertEqual(write.call_args.args[1]['evidence'][row]['custody'],result)
                write.reset_mock()
                with patch.object(probe,'custody_oracle',side_effect=ValueError('unavailable')):
                    with self.assertRaises(ValueError): probe.qualify(row)
                write.assert_not_called()
                for bad in (None,{},dict(result,passed=False),dict(result,observations={})):
                    with patch.object(probe,'custody_oracle',return_value=bad):
                        with self.assertRaises(ValueError): probe.qualify(row)
                    write.assert_not_called()

    def test_custody_runner_rejects_skips_and_failures(self):
        probe = load('probe')
        self.assertTrue(callable(getattr(probe,'custody_oracle',None)))
        # Inject an unavailable installed test through the runner's suite boundary.
        class Unavailable(unittest.TestCase):
            def test_unavailable(self): self.skipTest('not installed')
        for row in self.custody:
            with patch.object(unittest.defaultTestLoader,'loadTestsFromName',
                    return_value=unittest.defaultTestLoader.loadTestsFromTestCase(Unavailable)):
                with self.assertRaises(ValueError): probe.custody_oracle(row)

    def test_units_publish_only_after_readonly_absence_precheck(self):
        scope = installer()
        self.assertIn('publish_units',scope)
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); control = root/'control'; control.mkdir()
            pairs = []
            for i in range(3):
                source = root/('stage'+str(i)); source.write_bytes(b'unit')
                pairs.append((source,root/('unit'+str(i))))
            with patch.dict(scope,CONTROL=control,protected=Mock(),run=Mock()):
                with self.assertRaisesRegex(ValueError,'reboot'): scope['publish_units'](pairs)
                scope['run'].assert_not_called()
                self.assertTrue(all(s.exists() and not t.exists() for s,t in pairs))
                control.rmdir()
                self.assertEqual(scope['publish_units'](pairs),[t for s,t in pairs])
                scope['run'].assert_called_once_with(['/usr/bin/systemctl','daemon-reload'])
        tree = ast.parse(
            (HERE/'install.sh').read_text().split("<<'PY'\n",1)[1].rsplit('\nPY',1)[0])
        main = next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='main')
        calls = [ast.unparse(n.func) for n in ast.walk(main) if isinstance(n,ast.Call)]
        self.assertEqual(calls.count('publish_units'),1)
        # The remaining rename in main is only for staged sudoers/polkit.
        loops = [n for n in ast.walk(main) if isinstance(n,ast.For) and
                 any(isinstance(c,ast.Call) and ast.unparse(c.func)=='os.rename' for c in ast.walk(n))]
        # Q30: four staged units precede sudoers/polkit.
        self.assertEqual([ast.unparse(n.iter) for n in loops],['staged_security[4:]'])

    def test_offline_tempdirs_outside_installed_tree(self):
        for name in ('test_service.py','test_unit_core.py','test_custody.py'):
            tree = ast.parse((HERE/'tests'/name).read_text())
            for call in (n for n in ast.walk(tree) if isinstance(n,ast.Call) and
                         ast.unparse(n.func)=='tempfile.TemporaryDirectory'):
                for kw in call.keywords:
                    if kw.arg=='dir': self.assertNotIn('HERE',ast.unparse(kw.value),name)

    def test_created_accounts_exact_for_new_mixed_and_existing(self):
        scope = installer()
        entries = [SimpleNamespace(pw_name=n,pw_uid=i,pw_gid=i) for n,i in (('mothergate',991),('gaterun',992))]
        for flags,submit_exists in (((False,False),True),((True,False),True),((True,True),False)):
            calls = []
            with patch.dict(scope,run=calls.append), patch.object(scope['grp'],'getgrnam',
                    side_effect=None if submit_exists else KeyError('absent')):
                result = scope['apply_accounts'](list(zip(entries,flags)),'nexus')
            users = [e.pw_name for e,c in zip(entries,flags) if c]
            self.assertEqual(result,dict(users=users,groups=users+([] if submit_exists else ['gatesubmit'])))
        source = (HERE/'install.sh').read_text()
        self.assertIn("record(CONFIG/'created-accounts.json',created)",source)
        self.assertIn("CONFIG/'created-accounts.json'",source[source.index('outputs +='):])

    def test_polkit_hash_required_and_claims_narrowed(self):
        probe = load('probe'); rule = '/etc/polkit-1/rules.d/00-gatebox-deny.rules'
        self.assertTrue(callable(getattr(probe,'polkit_check',None)))
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)/'rule'; path.write_bytes(b'rule')
            with patch.object(probe,'POLKIT_RULE',path), patch.object(probe,'protected'), \
                    patch.object(probe,'config',return_value={str(path):hashlib.sha256(b'rule').hexdigest()}):
                probe.polkit_check(); path.write_bytes(b'changed')
                with self.assertRaises(ValueError): probe.polkit_check()
            with patch.object(probe,'POLKIT_RULE',path), patch.object(probe,'protected'), \
                    patch.object(probe,'config',return_value={}):
                with self.assertRaises(ValueError): probe.polkit_check()
        self.assertIn('polkit_check()',inspect.getsource(probe.authority))
        for name in ('README.md','MISSION.md'):
            source = (HERE/name).read_text()
            self.assertNotIn('including conflicting earlier policy',source)
            self.assertIn('B2 proves denial',source)

# Q15 pre-registered decision fixtures: no host manager or ownership modelling.
BOOT_BEFORE = '11111111-1111-4111-8111-111111111111'
BOOT_AFTER = '22222222-2222-4222-8222-222222222222'
def q15_proof(row):
    observations = dict(run_id='b'*32,boot_id=BOOT_BEFORE,row=row)
    if row=='C1-SEALED': observations['denied'] = [[k,a,13] for k in ('log','status') for a in ('unlink','rename','symlink')]
    elif row=='C2-CANARY': observations.update(rejected=True,no_invocation=True,canary_unchanged=True,
        dangling_absent=True,custody_held=True)
    else: observations.update(retained=True,supervisor_before={'MainPID':'12'},supervisor_after={'MainPID':'13'})
    return dict(row=row,passed=True,observations=observations)

class Q15Oracles(unittest.TestCase):
    def test_bind_absolute_short_from_root_and_deep_cwd(self):
        # Exercise the actual B2 suite and production temporary pin. Redirect
        # only /var/tmp into our authorized out TMPDIR; preserve its path length.
        probe = load('probe')
        real = tempfile.TemporaryDirectory
        bind = __import__('socket').socket.bind
        def bounded(sock,address):
            self.assertTrue(os.path.isabs(address),'cwd-relative bind')
            self.assertLessEqual(len(os.fsencode(address)),107)
            return bind(sock,address)
        with real() as temporary:
            deep = Path(temporary)/('d'*210); deep.mkdir()
            previous = Path.cwd()
            try:
                for cwd in (Path('/'),deep):
                    with self.subTest(cwd=str(cwd)):
                        self.b2_at_cwd(probe,cwd,temporary,real,bounded)
            finally: os.chdir(previous)

    def b2_at_cwd(self,probe,cwd,temporary,real,bounded):
        os.chdir(cwd)
        def pin(*args,**kwargs):
            if kwargs.get('dir')=='/var/tmp': kwargs['dir']=temporary
            return real(*args,**kwargs)
        with patch.object(probe.tempfile,'TemporaryDirectory',side_effect=pin), \
                patch.object(__import__('socket').socket,'bind',bounded):
            probe.offline_oracles('B2')

    def evidence(self):
        identity,summary,value = Q13Oracles().evidence()
        for row in Q13Oracles.custody: value['evidence'][row]['custody'] = q15_proof(row)
        return identity,summary,value

    def accept(self,value,boot=BOOT_AFTER,clean=True):
        probe = load('probe'); identity,summary,_ = self.evidence()
        scope = installer()
        with patch.object(probe,'boot_id',return_value=boot), \
                patch.object(probe,'clean_custody',return_value=clean):
            scope['acceptance_check'](identity,value,dict(identity=identity,stage='qualifying'),
                                     lambda r:{'summary':summary},sensors=probe)

    def test_acceptance_requires_changed_boot_and_clean_custody(self):
        _,_,value = self.evidence()
        self.accept(value)
        for boot,clean in ((BOOT_BEFORE,True),(BOOT_AFTER,False),('',True)):
            with self.subTest(boot=boot,clean=clean), self.assertRaises(ValueError): self.accept(value,boot,clean)

    def test_custody_evidence_content_run_boot_and_row(self):
        probe = load('probe')
        for row in Q13Oracles.custody:
            proof = q15_proof(row); probe.validate_custody(row,proof)
            for key in ('run_id','boot_id','row'):
                bad = copy.deepcopy(proof); bad['observations'].pop(key)
                with self.subTest(row=row,key=key), self.assertRaises(ValueError): probe.validate_custody(row,bad)
                _,_,value = self.evidence(); value['evidence'][row]['custody'] = bad
                with self.assertRaises(ValueError): self.accept(value)
            bad = copy.deepcopy(proof)
            key = {'C1-SEALED':'denied','C2-CANARY':'rejected','C3-RESTART':'retained'}[row]
            bad['observations'][key] = False
            with self.assertRaises(ValueError): probe.validate_custody(row,bad)

    def qualify(self,row,*,clean=True,proof=None,prior=None):
        probe = load('probe'); identity,summary,value = self.evidence()
        value['passed'].remove('C2-CANARY'); del value['evidence']['C2-CANARY']
        if prior is not None: value = prior
        with tempfile.TemporaryDirectory() as tmp, ExitStack() as stack:
            for name, replacement in dict(CONFIG=Path(tmp),require_installed=Mock(),
                    fixture=Mock(return_value=(summary,{})),installed_identity=Mock(return_value=identity),
                    protected=Mock(),boot_id=Mock(return_value=BOOT_BEFORE),
                    clean_custody=Mock(return_value=clean),custody_oracle=Mock(return_value=proof or q15_proof(row)),
                    config=Mock(side_effect=lambda n: identity if n=='identity.json' else value)).items():
                stack.enter_context(patch.object(probe,name,replacement))
            write = stack.enter_context(patch.object(probe,'atomic_config'))
            probe.qualify(row)
            return write.call_args.args[1]

    def test_qualification_preboot_binding_last_and_leftovers(self):
        value = self.qualify('C2-CANARY')
        self.assertEqual(value['evidence']['C2-CANARY']['custody']['observations']['boot_id'],BOOT_BEFORE)
        for variant in ('leftover','stale-boot','not-last','repeat'):
            with self.subTest(variant=variant), self.assertRaises(ValueError):
                if variant=='leftover': self.qualify('C1-SEALED',clean=False)
                elif variant=='stale-boot':
                    proof=q15_proof('C2-CANARY'); proof['observations']['boot_id']=BOOT_AFTER
                    self.qualify('C2-CANARY',proof=proof)
                elif variant=='not-last': self.qualify('C2-CANARY',prior=dict(identity=value['identity'],passed=[],evidence={}))
                else: self.qualify('C1-SEALED',prior=value)

    def test_setup_failure_cleanup_continues_after_failed_stop(self):
        from test_custody import InstalledCustodyTests
        case = InstalledCustodyTests('test_mothergate_cannot_unlink_rename_or_symlink_after_relocation')
        with tempfile.TemporaryDirectory() as tmp:
            case.run='a'*32; case.unit='mothergate-run@'+case.run+'.service'; case.blocker='gatebox-custody-'+case.run+'.service'
            case.root=Path(tmp)/'root'; case.root.mkdir(); case.fifo=case.root/'release'; case.fifo.touch()
            case.control=Path(tmp)/'control'; (case.control/'incoming').mkdir(parents=True)
            (case.control/'sealed').mkdir(); case.sealed=case.control/'sealed'/case.run
            case.stage=Path(tmp)/'stage'; case.marker=Path(tmp)/'marker'
            case.files={}; case.launch=Mock(); case.collector=None; case.hold_custody=False
            case.bridge=Mock(); case.bridge.run.return_value={'Job':''}
            case.owner=SimpleNamespace(pw_uid=os.getuid(),pw_gid=os.getgid())
            case.observations={}; case.probe=SimpleNamespace(protected=Mock())
            case._outcome=SimpleNamespace(result=SimpleNamespace(failures=[],errors=[]))
            calls=[]
            def manager(*args):
                calls.append(args)
                if args[0]=='stop': raise subprocess.CalledProcessError(5,args)
            case.manager=manager
            # No subprocess may execute. This is the persistent exact-name remover.
            case.remove_persistent=Mock()
            with patch.object(subprocess,'run',side_effect=AssertionError('unexpected subprocess')):
                with self.assertRaises(AssertionError): case.cleanup()
            case.bridge.run.assert_called_once_with('stop',case.run)
            case.remove_persistent.assert_called_once()
            self.assertFalse(case.fifo.exists(),'cleanup aborted before exact runtime leaves')

    def test_setup_registers_cleanup_before_first_fixture_write(self):
        from test_custody import InstalledCustodyTests
        source=inspect.getsource(InstalledCustodyTests.setUp)
        self.assertLess(source.index('self.addCleanup(self.cleanup)'),source.index('self.root.mkdir'))

    def test_removal_absent_identities_are_skipped(self):
        # Execute the reviewed README account-removal block twice, no host ops.
        source=(HERE/'README.md').read_text()
        start=source.index("for name in created['users']:")
        end=source.index('\nPY',start)
        code=compile(source[start:end],'README removal','exec')
        users={'mothergate'}; groups={'gatesubmit'}; calls=[]
        def lookup(names,name):
            if name not in names: raise KeyError(name)
            return SimpleNamespace(gr_mem=[])
        def remove(argv,**kwargs):
            calls.append(argv); (users if argv[0].endswith('userdel') else groups).remove(argv[-1])
        scope=dict(created=dict(users=['mothergate','gaterun'],groups=['gatesubmit']),
            pwd=SimpleNamespace(getpwnam=lambda n:lookup(users,n)),
            grp=SimpleNamespace(getgrnam=lambda n:lookup(groups,n)),subprocess=SimpleNamespace(run=remove))
        exec(code,scope); exec(code,scope)
        self.assertEqual(calls,[['/usr/sbin/userdel','mothergate'],['/usr/sbin/groupdel','gatesubmit']])

class Q16Oracles(unittest.TestCase):
    """Q16 decision oracles: fake time/host interfaces, real proof code."""
    def fixture_cleanup(self, delay, retained=()):
        probe = load('probe'); run = 'c'*32; elapsed = 0.0
        with tempfile.TemporaryDirectory() as tmp, ExitStack() as stack:
            root = Path(tmp)
            for name in ('pending','staging','receipts'): (root/name).mkdir()
            marker = root/'pending'/('.'+run); marker.touch()
            stage = root/'staging'/run; stage.mkdir()
            (root/'receipts'/run).mkdir()  # receipt precedes both cleanup leaves
            def path(value):
                value = str(value)
                prefix = '/var/lib/mothergate/'
                return root/value[len(prefix):] if value.startswith(prefix) else Path(value)
            def sleep(seconds):
                nonlocal elapsed
                self.assertGreater(seconds,0,'poll must yield')
                elapsed += seconds
                self.assertLessEqual(elapsed,5.000001,'cleanup exceeded five-second bound')
                if elapsed >= delay:
                    if 'pending' not in retained: marker.unlink(missing_ok=True)
                    if 'staging' not in retained and stage.exists(): stage.rmdir()
            summary = dict(status='PASS_SCOPED',missing_evidence=[])
            sock = Mock(); sock.getsockname.return_value = ('127.0.0.1',12345)
            replacements = dict(Path=path,require_installed=Mock(),
                config=Mock(return_value={'mother':{'uid':1234}}),claude_context=Mock(return_value={}),
                atomic_config=Mock(),client_request=Mock(side_effect=[
                    {'status':'UNQUALIFIED','reason':'QUALIFICATION_ONLY'},
                    {'status':'ACCEPTED','run_id':run}]),
                sibling=Mock(return_value=SimpleNamespace(authenticate=Mock(return_value={'summary':summary}))),
                time=SimpleNamespace(monotonic=lambda:elapsed,sleep=sleep),
                socket=SimpleNamespace(AF_INET=2,AF_UNIX=1,socket=Mock(return_value=sock)),
                pty=SimpleNamespace(openpty=lambda:(501,502)),
                pwd=SimpleNamespace(getpwnam=lambda n:SimpleNamespace(pw_uid=1235)),
                os=SimpleNamespace(getpid=lambda:42,ttyname=lambda n:'/dev/pts/1',
                    readlink=lambda n:'namespace',close=Mock()))
            for name,value in replacements.items(): stack.enter_context(patch.object(probe,name,value))
            if retained:
                with self.assertRaisesRegex(ValueError,'custody leftovers after proof; reviewed recovery required'):
                    probe.fixture()
                self.assertIsNone(probe.SESSION,'timeout must not cache success')
            else:
                self.assertEqual(probe.fixture()[0],summary)
                self.assertFalse(marker.exists() or stage.exists(),'receipt returned before cleanup')
            self.assertGreaterEqual(elapsed,.062,'no post-receipt wait')
            self.assertLessEqual(elapsed,5.000001)

    def test_fixture_delayed_cleanup_62ms(self):
        self.fixture_cleanup(.062)

    def test_fixture_cleanup_bound(self):
        for retained in (('pending',),('staging',),('pending','staging')):
            with self.subTest(retained=retained): self.fixture_cleanup(5.062,retained)

    def test_c2_rejection_message(self):
        from test_custody import InstalledCustodyTests
        method = 'test_systemd_output_never_writes_disposable_root_canary'
        for message in ('unsafe sealed endpoint','unrelated start dependency failed'):
            with self.subTest(message=message), tempfile.TemporaryDirectory() as tmp:
                case = InstalledCustodyTests(method); case.root = Path(tmp)
                case.run = 'c'*32; case.unit = 'mothergate-run@'+case.run+'.service'
                case.control = case.root/'control'; (case.control/'sealed').mkdir(parents=True)
                case.sealed = case.control/'sealed'/case.run; case.sealed.mkdir()
                case.observations = {}
                case.state = Mock(return_value=dict(Job='',InvocationID='',ActiveEnterTimestampMonotonic='0',MainPID='0'))
                def command(argv,**kwargs):
                    if argv[0]=='/usr/sbin/runuser':
                        for kind,target in zip(('log','status'),argv[-2:]): (case.sealed/(kind+'.sock')).symlink_to(target)
                        (case.control/'sealed'/('.pending-'+case.run)).touch()
                    return SimpleNamespace(returncode=1,stderr=message)
                case.command = command
                if message=='unsafe sealed endpoint': getattr(case,method)()
                else:
                    with self.assertRaises(AssertionError): getattr(case,method)()

    def test_early_setup_no_launcher_cleanup(self):
        from test_custody import InstalledCustodyTests
        for settle_fails in (False,True):
            with self.subTest(settle_fails=settle_fails), tempfile.TemporaryDirectory() as tmp:
                case = InstalledCustodyTests('test_mothergate_cannot_unlink_rename_or_symlink_after_relocation')
                case.run='c'*32; case.unit='mothergate-run@'+case.run+'.service'; case.blocker='gatebox-custody-'+case.run+'.service'
                case.root=Path(tmp)/'root'; case.root.mkdir(); case.fifo=case.root/'release'; case.fifo.touch()
                case.control=Path(tmp)/'control'; (case.control/'incoming').mkdir(parents=True)
                (case.control/'sealed').mkdir(); case.sealed=case.control/'sealed'/case.run
                case.stage=Path(tmp)/'stage'; case.marker=Path(tmp)/'marker'
                case.files={}; case.launch=None; case.collector=None; case.hold_custody=False
                case.owner=SimpleNamespace(pw_uid=os.getuid(),pw_gid=os.getgid())
                case.observations={}; case.probe=SimpleNamespace(protected=Mock())
                case._outcome=SimpleNamespace(result=SimpleNamespace(failures=[],errors=[]))
                case.manager=Mock(side_effect=AssertionError('no unit stop without launcher'))
                case.bridge=Mock()
                case.bridge.run.side_effect=subprocess.CalledProcessError(5,['stop'])
                if settle_fails: case.bridge.settled.side_effect=ValueError('unsettled')
                def remove(): case.bridge.settled.assert_called_once_with(case.run)
                case.remove_persistent=Mock(side_effect=remove)
                with patch.object(subprocess,'run',side_effect=AssertionError('unexpected subprocess')):
                    with self.assertRaises(AssertionError): case.cleanup()  # original error remains observable
                case.manager.assert_not_called()
                case.bridge.run.assert_called_once_with('stop',case.run)
                case.bridge.settled.assert_called_once_with(case.run)
                self.assertEqual(case.remove_persistent.call_count,0 if settle_fails else 1)
                self.assertEqual(case.fifo.exists(),settle_fails)

    def test_readme_reset_procedure(self):
        source=(HERE/'README.md').read_text()
        self.assertIn("<<'RESET'",source)
        block=source.split("<<'RESET'",1)[1].split('\nRESET',1)[0]
        tree=ast.parse(block)
        calls=[ast.unparse(n) for n in ast.walk(tree) if isinstance(n,ast.Call)]
        self.assertIn("probe.atomic_config('qualification.json', {'identity': None, 'passed': []})",calls)
        for call in ('probe.require_installed()','probe.clean_custody()',"probe.config('admission.json')"):
            self.assertIn(call,calls)
        self.assertIn('qualification-reset-',block)  # retain the failed evidence
        self.assertIn('LOCK_EX',block)

    def test_readme_pin_object_integrity(self):
        source=(HERE/'README.md').read_text()
        block=source.split("<<'PIN'",1)[1].split('\nPIN',1)[0]
        tree=ast.parse(block)
        argv=[ast.literal_eval(n) for n in ast.walk(tree) if isinstance(n,ast.List)
              and n.elts and all(isinstance(e,ast.Constant) for e in n.elts) and n.elts[0].value=='git']
        self.assertIn(['git','--no-replace-objects','for-each-ref','--format=%(refname)','refs/replace'],argv)
        self.assertIn("['git', '--no-replace-objects', 'fsck'",ast.unparse(tree))
        self.assertLess(block.index("'fsck'"),block.index("tarfile.open"))
        self.assertIn("['git','--no-replace-objects','show'",block)
        self.assertIn("['git','--no-replace-objects','ls-tree'",block)
