"""Frozen PR-1 behavioral oracles. No installed-qualification PASS lines."""
import hashlib
import copy
import importlib.util
import inspect
import json
import os
from pathlib import Path
import shutil
import stat
import tempfile
import unittest
from contextlib import contextmanager
from unittest.mock import patch
import zlib

HERE = Path(__file__).resolve().parents[1]
def load(name):
    spec = importlib.util.spec_from_file_location(name, HERE / (name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

snapshot, receipt = load('snapshot'), load('receipt')
def oid(kind, data):
    return hashlib.sha1(kind.encode() + b' ' + str(len(data)).encode() + b'\0' + data).hexdigest()

class Objects:
    def __init__(self):
        self.objects = {}
    def put(self, kind, data):
        key = oid(kind, data)
        self.objects[key] = (kind, data)
        return key
    def tree(self, entries):
        return self.put('tree', b''.join(mode.encode()+b' '+name.encode()+b'\0'+bytes.fromhex(key)
                                       for mode, name, key in entries))
    def commit(self, entries):
        tree = self.tree(entries)
        return self.put('commit', b'tree '+tree.encode()+b'\nparent '+b'1'*40+b'\n\nfixture\n')
    def read(self, key, kind, limit, deadline):
        actual, data = self.objects[key]
        if actual != kind or len(data) > limit:
            raise ValueError('object type/size')
        return data

def fixture():
    source = Objects()
    lock = source.put('blob', b'lock\n')
    content = source.put('blob', b'$Format:%H$\nexact\n')
    attr = source.put('blob', b'file export-ignore export-subst\n')
    link = source.put('blob', b'file')
    sha = source.commit([('100644','.gitattributes',attr),('100644','Cargo.lock',lock),
                         ('100755','file',content),('120000','link',link)])
    return source, sha

# SB3E-R26 (GB2): the source identity is the ext2/3/4 superblock's UUID-derived f_fsid plus the inode.
EXT4, TMPFS, VFAT = 0xEF53, 0x01021994, 0x4D44
FSID = 0x39847550afb00c35  # this host's root filesystem 9ac90fd2-...: le64(uuid[:8]) ^ le64(uuid[8:])
REGISTERED = {'type':'ext4','fsid':'%016x' % FSID}

def administration(admin):
    """A minimal bare Git administration directory; Python writes it, no Git mutation command."""
    (admin/'objects/info').mkdir(parents=True)
    (admin/'refs').mkdir()
    (admin/'HEAD').write_text('ref: refs/heads/main\n')
    (admin/'config').write_text('[core]\nrepositoryformatversion = 0\nbare = true\n')
    return admin

@contextmanager
def filesystem(module=snapshot, kind=EXT4, fsid=FSID, error=None):
    """Stands in for fstatfs(2) only (TMPDIR may be tmpfs); descriptors, inodes and Git stay real."""
    def statfs(fd):
        os.fstat(fd)  # a real open descriptor is still required
        if error is not None: raise error
        return kind, fsid
    with patch.object(module,'statfs',statfs,create=True): yield

@contextmanager
def renumbered(inode, shift=0x100):
    """A reboot that renumbers block devices: st_dev of the same inode changes in os.stat and os.fstat."""
    real_stat, real_fstat = os.stat, os.fstat
    def moved(result):
        if result.st_ino != inode: return result
        fields = list(result)
        fields[2] = result.st_dev+shift
        return os.stat_result(fields)
    with patch('os.stat',lambda *a,**k: moved(real_stat(*a,**k))), patch('os.fstat',lambda *a,**k: moved(real_fstat(*a,**k))):
        yield

def registered_source(path, row):
    """supervisor.execute's construction from a registry row. GB1's GitSource took (device, inode), GB2's takes
    (filesystem, inode): the parameter name selects the field, so this oracle runs unchanged against both."""
    return snapshot.GitSource(path, row.get(list(inspect.signature(snapshot.GitSource).parameters)[1]), row.get('inode'))

def boot_a_row(admin):
    """A registry row recorded at boot A with both pins: GB1's st_dev and GB2's filesystem identity."""
    info = os.stat(admin)
    return dict(device=info.st_dev, inode=info.st_ino, filesystem=dict(REGISTERED))

class SnapshotTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.addCleanup(self.tmp.cleanup)
    def test_exact_snapshot(self):
        source, sha = fixture()
        result = snapshot.export(source, sha, self.root/'out')
        self.assertEqual((self.root/'out/file').read_bytes(), b'$Format:%H$\nexact\n')
        self.assertEqual(os.readlink(self.root/'out/link'), 'file')
        self.assertEqual(stat.S_IMODE((self.root/'out/file').stat().st_mode), 0o555)
        self.assertEqual(stat.S_IMODE((self.root/'out').stat().st_mode), 0o555)
        self.assertFalse((self.root/'out/.git').exists())
        self.assertEqual(result['commit'], sha)
        self.assertEqual(result['lockfile_sha256'], hashlib.sha256(b'lock\n').hexdigest())
        self.assertEqual(result['content_sha256'], 'b7b1f881eb82ce334f3b644e69c0bf3858eb8bcbfcd2495ee2e2ddaf0a80c773')
    def test_wrong_hash_rejected(self):
        source, sha = fixture()
        key = next(k for k, v in source.objects.items() if v == ('blob', b'lock\n'))
        source.objects[key] = ('blob', b'fake\n')
        with self.assertRaisesRegex(ValueError, 'hash'):
            snapshot.export(source, sha, self.root/'out')
    def test_wrong_commit_tree_blob_types_and_missing(self):
        for kind in ('commit','tree','blob'):
            source, sha = fixture()
            key = next(k for k, v in source.objects.items() if v[0] == kind)
            source.objects[key] = ('tag', source.objects[key][1])
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                snapshot.export(source, sha, self.root/kind)
        source, sha = fixture()
        del source.objects[sha]
        with self.assertRaises(ValueError):
            snapshot.export(source, sha, self.root/'missing')
    def test_unsafe_trees_and_links(self):
        for mode, name, data in [('160000','sub',b'x'),('100664','bad',b'x'),
                                 ('100644','../escape',b'x'),('100644','.git',b'x'),
                                 ('100644','a\\b',b'x'),('120000','link',b'../escape'),
                                 ('120000','link',b'/etc/passwd'),('120000','link',b'.git/config')]:
            source = Objects()
            key = source.put('blob', data)
            sha = source.commit([('100644','Cargo.lock',source.put('blob',b'lock')), (mode,name,key)])
            with self.subTest(mode=mode,name=name,data=data), self.assertRaises(ValueError):
                snapshot.export(source, sha, self.root/'bad')
        source, sha = fixture()
        tree = source.tree([('100644','same',source.put('blob',b'1')),('120000','same',source.put('blob',b'same'))])
        sha = source.put('commit', b'tree '+tree.encode()+b'\n\nx')
        with self.assertRaises(ValueError):
            snapshot.export(source, sha, self.root/'duplicate')
    def test_composed_link_escape(self):
        source = Objects()
        subtree = source.tree([('120000','l',source.put('blob',b'..'))])
        sha = source.commit([('100644','Cargo.lock',source.put('blob',b'lock')),
                             ('40000','d',subtree),('120000','x',source.put('blob',b'd/l/..'))])
        with self.assertRaisesRegex(ValueError,'composed link escape'):
            snapshot.export(source,sha,self.root/'out')
        self.assertFalse((self.root/'out').exists())
    def test_composed_link_deadline(self):
        source = Objects()
        sha = source.commit([('100644','Cargo.lock',source.put('blob',b'lock')),
                             ('120000','l',source.put('blob',b'./'*50+b'Cargo.lock'))])
        clock = {'resolving':False,'calls':0}
        original = snapshot.validate_link
        def validate(*args):
            original(*args)
            clock['resolving'] = True
        def now():
            if clock['resolving']: clock['calls'] += 1
            return 301 if clock['calls'] > 1 else 0
        with patch.object(snapshot,'validate_link',side_effect=validate), patch.object(snapshot.time,'monotonic',side_effect=now):
            with self.assertRaisesRegex(ValueError,'deadline'): snapshot.export(source,sha,self.root/'out')
        self.assertFalse((self.root/'out').exists(),'deadline must stop resolution before staging')
    def test_bounds_before_export(self):
        for field, value in [('blob',3),('tree',3),('commit',3),('entries',1),
                             ('objects',1),('bytes',3),('export',3),('path',3),('component',3),('seconds',0)]:
            source, sha = fixture()
            with self.subTest(field=field), patch.dict(snapshot.LIMITS, {field:value}), self.assertRaises(ValueError):
                snapshot.export(source, sha, self.root/field)
        source = Objects()
        key = source.put('blob', b'lock')
        tree = source.tree([('100644','Cargo.lock',key)])
        for _ in range(3):
            tree = source.tree([('40000','dir',tree)])
        sha = source.put('commit', b'tree '+tree.encode()+b'\n\nx')
        with patch.dict(snapshot.LIMITS, {'depth':2}), self.assertRaises(ValueError):
            snapshot.export(source, sha, self.root/'deep')
    def test_git_administration_indirection(self):
        admin, common = self.root/'admin', self.root/'common'
        for directory in (admin,common):
            administration(directory)
        (common/'objects/info/alternates').write_text(str(admin/'objects')+'\n')
        info = admin.stat()
        with filesystem():
            snapshot.GitSource(admin,REGISTERED,info.st_ino)
            for name in ('commondir','gitdir'):
                marker = admin/name
                marker.write_text(str(common)+'\n')
                try:
                    with self.subTest(name=name), self.assertRaisesRegex(ValueError,'indirection'):
                        snapshot.GitSource(admin,REGISTERED,info.st_ino)
                finally: marker.unlink()
    def test_raw_git_source_and_administration(self):
        # Synthetic loose-object fixture: Python writes data; no Git mutation command.
        source, sha = fixture()
        admin = self.root/'objects-fixture'
        (admin/'objects/info').mkdir(parents=True)
        (admin/'refs').mkdir()
        (admin/'HEAD').write_text('ref: refs/heads/main\n')
        (admin/'config').write_text('[core]\nrepositoryformatversion = 0\nbare = true\n')
        for key, (kind, data) in source.objects.items():
            dest = admin/'objects'/key[:2]/key[2:]
            dest.parent.mkdir(exist_ok=True)
            dest.write_bytes(zlib.compress(kind.encode()+b' '+str(len(data)).encode()+b'\0'+data))
        info = admin.stat()
        with filesystem():
            raw = snapshot.GitSource(admin, REGISTERED, info.st_ino)
            snapshot.export(raw, sha, self.root/'real')
            self.assertEqual((self.root/'real/file').read_bytes(), b'$Format:%H$\nexact\n')
            with self.assertRaises(ValueError):
                snapshot.GitSource(admin, REGISTERED, info.st_ino+1)
            for path in ['objects/info/alternates','objects/pack/test.promisor','refs/replace/'+sha]:
                marker = admin/path
                marker.parent.mkdir(parents=True, exist_ok=True)
                marker.write_text(sha+'\n')
                with self.subTest(path=path), self.assertRaises(ValueError):
                    snapshot.GitSource(admin, REGISTERED, info.st_ino)
                marker.unlink()
    # ---- GB2 (SB3E-R26): boot-stable source identity. st_dev follows kernel device naming; never a pass condition.
    def test_gb2_registry_row_admits_source_without_renumbering(self):
        # Control for the oracle below: passes on GB1 and GB2 alike, so the oracle's RED is the renumbering alone.
        admin = administration(self.root/'source.git'); row = boot_a_row(admin)
        with filesystem():
            source = registered_source(admin, row)
            source.check(source.deadline)
    def test_gb2_same_filesystem_and_inode_pass_after_device_renumbering(self):
        # RED on GB1: `renumbered: source identity changed`. GREEN on GB2: st_dev is never a pass condition.
        admin = administration(self.root/'source.git'); row = boot_a_row(admin)
        with filesystem():
            source = registered_source(admin, row)  # boot A
            with renumbered(row['inode']):
                self.assertNotEqual(os.stat(admin).st_dev, os.lstat(admin).st_dev)  # the renumbering is observable
                try:
                    source.check(source.deadline)  # boot B, the same source
                    registered_source(admin, row)  # boot B, from the boot-A registry row
                except ValueError as exc: self.fail('renumbered: '+str(exc))
    def test_gb2_other_filesystem_identity_refused(self):
        admin = administration(self.root/'source.git'); inode = admin.stat().st_ino
        for fsid in (FSID ^ 1, FSID ^ (1 << 63), 0x29ed1b591aefb877):  # the last: this host's other ext4 filesystem
            with self.subTest(fsid=hex(fsid)), filesystem(fsid=fsid), self.assertRaisesRegex(ValueError,'^source identity changed$'):
                snapshot.GitSource(admin, REGISTERED, inode)
    def test_gb2_other_inode_refused(self):
        admin = administration(self.root/'source.git'); inode = admin.stat().st_ino
        other = administration(self.root/'other.git')
        with filesystem():
            for path, registered in ((admin, inode+1), (other, inode)):
                with self.subTest(path=path.name), renumbered(inode, 0), self.assertRaisesRegex(ValueError,'^source identity changed$'):
                    snapshot.GitSource(path, REGISTERED, registered)
    def test_gb2_unavailable_or_unsupported_identity_refused(self):
        admin = administration(self.root/'source.git'); inode = admin.stat().st_ino
        for change, message in ((dict(error=OSError(38,'fstatfs')),'^source filesystem identity unavailable$'),
                                (dict(fsid=0),'^source filesystem identity unavailable$'),
                                (dict(kind=TMPFS),'^unsupported source filesystem$'),
                                (dict(kind=VFAT,fsid=0x10304),'^unsupported source filesystem$')):  # vfat: a device-derived f_fsid
            with self.subTest(change=change), filesystem(**change):
                with self.assertRaisesRegex(ValueError,message): snapshot.observe(admin)
                with self.assertRaisesRegex(ValueError,message): snapshot.GitSource(admin, REGISTERED, inode)
        with filesystem(), self.assertRaisesRegex(ValueError,'^source identity unavailable$'): snapshot.observe(self.root/'absent.git')
    def test_gb2_layout_disagreement_is_unavailable(self):
        fd = os.open(self.root, os.O_RDONLY | os.O_DIRECTORY)
        self.addCleanup(os.close, fd)
        real = os.fstatvfs(fd)
        for field in ('f_fsid','f_bsize','f_blocks'):
            values = {k:getattr(real,k) for k in ('f_fsid','f_bsize','f_blocks')}
            values[field] ^= 1
            with self.subTest(field=field), patch.object(snapshot.os,'fstatvfs',lambda fd: type('V',(),values)):
                with self.assertRaisesRegex(OSError,'fstatfs layout'): snapshot.statfs(fd)
                with self.assertRaisesRegex(ValueError,'^source filesystem identity unavailable$'): snapshot.filesystem_identity(fd)
    def test_gb2_registered_identity_missing_or_invalid_refused(self):
        admin = administration(self.root/'source.git'); inode = admin.stat().st_ino
        bad_filesystems = (None, {}, {'type':'ext4'}, {'fsid':REGISTERED['fsid']}, dict(REGISTERED,type='xfs'),
                           dict(REGISTERED,fsid='0'*16), dict(REGISTERED,fsid=REGISTERED['fsid'].upper()),
                           dict(REGISTERED,fsid=REGISTERED['fsid'][1:]), dict(REGISTERED,fsid=FSID), dict(REGISTERED,device=1))
        with filesystem():
            for registered, number in [(f, inode) for f in bad_filesystems]+[(REGISTERED, n) for n in (None, 0, -1, True, str(inode), float(inode))]:
                with self.subTest(registered=registered, inode=number), \
                        self.assertRaisesRegex(ValueError,'^registered source identity missing or invalid$'):
                    snapshot.GitSource(admin, registered, number)
    def test_gb2_noncanonical_source_refused(self):
        admin = administration(self.root/'source.git'); inode = admin.stat().st_ino
        (self.root/'link.git').symlink_to(admin)
        with filesystem():
            with self.assertRaisesRegex(ValueError,'^noncanonical registered source$'): snapshot.observe(self.root/'link.git')
            with self.assertRaisesRegex(ValueError,'^noncanonical registered source$'): snapshot.GitSource(self.root/'link.git', REGISTERED, inode)
            with self.assertRaisesRegex(ValueError,'^noncanonical registered source$'): snapshot.observe(Path('source.git'))
    def test_gb2_real_filesystem_identity(self):
        # Unpatched: fstatfs(2) agrees with glibc fstatvfs; ext2/3/4 is identified, anything else refused.
        fd = os.open(HERE, os.O_RDONLY | os.O_DIRECTORY)
        self.addCleanup(os.close, fd)
        kind, fsid = snapshot.statfs(fd)
        self.assertEqual(fsid, os.fstatvfs(fd).f_fsid)
        if kind == EXT4:
            self.assertEqual(snapshot.observe(HERE), {'filesystem':{'type':'ext4','fsid':'%016x' % fsid},'inode':HERE.stat().st_ino})
        else:
            with self.assertRaisesRegex(ValueError,'^unsupported source filesystem$'): snapshot.observe(HERE)

class ReceiptTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.pending, self.receipts = self.root/'pending', self.root/'receipts'
        self.pending.mkdir(mode=0o700)
        self.receipts.mkdir(mode=0o755)
        self.run_id = 'a'*32
        self.summary = {key:{} for key in ['request','source','snapshot','installed','host_inputs',
                        'versions','input_observations','environment','coverage','resources','isolation']}
        self.summary.update(schema_version=1, run_id=self.run_id, peer_uid=os.getuid(), attempts=[],
                            status='FAILED', commands=[], logs=[], cleanup={'complete':True},
                            missing_evidence=['fixture failure'], limitations=[
                            'Host toolchain and registry are mutable trusted inputs; inventories do not defeat change-and-restore attacks.'])
    def success(self):
        gates, exclusions = receipt.policy()
        summary = copy.deepcopy(self.summary)
        for key in receipt.GROUPS: summary[key] = {'fixture':'observed'}
        summary.update(status='PASS_SCOPED', missing_evidence=[], cleanup={'complete':True})
        summary['request'] = {'repo_id':'mother','full_sha':'1'*40,'gate_set_id':'full'}
        summary['snapshot'] = dict(commit='1'*40, content_sha256='2'*64, lockfile_sha256='3'*64)
        summary['installed'] = {key:'4'*64 for key in ('bundle_sha256','policy_sha256')}
        summary['installed']['gates_sha256'] = receipt.current_policy()  # SB3E-R19.5: its own recorded policy
        summary['input_observations'] = {'before':{'rust':'5'*64},'after':{'rust':'5'*64}}
        summary['environment'] = {'PATH':'/opt/rust/bin:/usr/bin:/bin','HOME':'/work/home','PWD':'/work/src',
                                  'CARGO_HOME':'/work/cargo','CARGO_TARGET_DIR':'/work/target','CARGO_BUILD_JOBS':'4',
                                  'RUST_TEST_THREADS':'4','CARGO_TERM_COLOR':'never','CARGO_NET_OFFLINE':'true',
                                  'RUST_BACKTRACE':'1','TMPDIR':'/run/gate/tmp','XDG_RUNTIME_DIR':'/run/gate/xdg',
                                  'XDG_CACHE_HOME':'/work/cache','LANG':'C.UTF-8','LC_ALL':'C.UTF-8','TZ':'UTC','CI':'true'}
        artifacts = [dict(id='host-bin',adapter='libtest',executable='/work/target/host-bin',
                          package='nexus-mother-host',target='host'),
                     dict(id='desktop-bin',adapter='libtest',executable='/work/target/desktop-bin',
                          package='nexus-mother-desktop',target='live')]
        coverage = {key:[g['id'] for g in gates['gates']] for key in ('expected','executed','passed')}
        coverage.update(failed=[],ignored=[],filtered=[],artifacts=artifacts,host_artifacts=['host-bin'],
                        excluded=[dict(name=n,status='excluded',reason=exclusions['reason']) for n in exclusions['cases']])
        summary['coverage'] = coverage
        summary['logs'] = [dict(file='run.log',sha256=hashlib.sha256(b'failure\n').hexdigest())]
        desktop = gates['gates'][6]['expected_names']
        for gate in gates['gates']:
            for artifact in (artifacts if gate['id'] == 'workspace' else [artifacts[1] if gate['id'] == 'desktop-live' else None]):
                command = dict(id=gate['id']+('-'+artifact['id'] if artifact else ''),gate_id=gate['id'],
                               argv=gate.get('argv',[artifact['executable']] if artifact else
                                             ['/usr/bin/python3','-I','-B','/gate/bin/init.py','--gate',gate['id']]),
                               cwd='/work/src',environment=dict(summary['environment']),exit_code=0,signal=None,
                               timed_out=False,oom=False,status_complete=True,collector_complete=True,
                               log='run.log',log_sha256=summary['logs'][0]['sha256'])
                if gate['id'] in ('host','whitespace'): command.update(argv=None,internal=True)
                if artifact: command['artifact_id'] = artifact['id']
                if gate['id'] == 'host': command['artifact_ids'] = ['host-bin']
                if 'adapter' in gate and gate['id'] != 'whitespace':
                    names = list(desktop) if artifact == artifacts[1] else ['host_test']
                    command['inventory'] = {key:list(names) for key in ('expected','executed','passed')}
                    command['inventory'].update(failed=[],ignored=[],filtered=[])
                summary['commands'].append(command)
        return summary
    def test_valid_scoped_success(self):
        self.summary = self.success()
        receipt.validate_summary(self.summary)
        self.assertEqual(receipt.verify_integrity(self.publish(),os.getuid())['summary']['status'],'PASS_SCOPED')
    def test_unknown_log_without_digest(self):
        summary = self.success()
        summary['commands'][0]['log'] = 'never-published.log'
        del summary['commands'][0]['log_sha256']
        with self.assertRaisesRegex(ValueError,'command'): receipt.validate_summary(summary)
    def publish(self):
        return receipt.publish(self.pending, self.receipts, self.run_id, self.summary, {'run.log':b'failure\n'})
    def test_publish_and_inventory(self):
        path = self.publish()
        result = receipt.verify_integrity(path, os.getuid())
        self.assertEqual(result['authentication'], 'UNAUTHENTICATED')
        self.assertEqual(result['summary']['status'], 'FAILED')
        self.assertFalse((self.pending/self.run_id).exists())
        self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o755)
        self.assertEqual(stat.S_IMODE((path/'summary.json').stat().st_mode), 0o444)
        with self.assertRaises((ValueError,FileExistsError)):
            self.publish()
    def test_artifact_tamper_missing_symlink_and_write_modes(self):
        path = self.publish()
        artifact = path/'run.log'
        for mutation in ('changed','missing','symlink','group-write','other-write','parent-write'):
            data = artifact.read_bytes()
            artifact.chmod(0o644)
            if mutation == 'changed': artifact.write_bytes(b'forged\n')
            if mutation in ('missing','symlink'): artifact.unlink()
            if mutation == 'symlink': artifact.symlink_to(self.root/'outside')
            if mutation == 'group-write': artifact.chmod(0o464)
            elif mutation == 'other-write': artifact.chmod(0o446)
            elif mutation == 'parent-write': self.receipts.chmod(0o777)
            elif artifact.exists(): artifact.chmod(0o444)
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                receipt.verify_integrity(path, os.getuid())
            if artifact.exists() or artifact.is_symlink(): artifact.unlink()
            artifact.write_bytes(data)
            artifact.chmod(0o444)
            self.receipts.chmod(0o755)
        alias = self.root/'alias'
        alias.symlink_to(self.receipts, target_is_directory=True)
        with self.assertRaises(ValueError):
            receipt.verify_integrity(alias/self.run_id, os.getuid())
    def test_success_single_change_negatives(self):
        for change in ('omit-exclusion','reorder-exclusion','excluded-passed','excluded-artifact',
                       'missing-artifact','ignored','filtered','argv','missing-log','invocation','commands'):
            summary = self.success()
            coverage = summary['coverage']
            if change == 'omit-exclusion': coverage['excluded'].pop()
            if change == 'reorder-exclusion': coverage['excluded'].reverse()
            if change == 'excluded-passed': coverage['passed'].append(coverage['excluded'][0]['name'])
            if change == 'excluded-artifact':
                coverage['artifacts'][0].update(package='nexus-verifier-sandbox',target='phase2_live_sandbox')
            if change == 'missing-artifact': summary['commands'].pop(3)
            if change in ('ignored','filtered'): summary['commands'][3]['inventory'][change] = ['host_test']
            if change == 'argv': summary['commands'][0]['argv'] = ['/bin/true']
            if change == 'missing-log': summary['logs'] = []
            if change == 'invocation': summary['commands'].pop(0)
            if change == 'commands': summary['commands'] = []
            with self.subTest(change=change), self.assertRaises(ValueError): receipt.validate_summary(summary)
    def test_exclusion_identity_is_package_and_target(self):
        # Q4: the same target in another package remains required coverage.
        summary = self.success()
        summary['coverage']['artifacts'][0]['target'] = 'phase2_live_sandbox'
        receipt.validate_summary(summary)
        summary['commands'] = [c for c in summary['commands']
                               if not (c['gate_id'] == 'workspace' and c.get('artifact_id') == 'host-bin')]
        with self.assertRaisesRegex(ValueError,'missing artifact execution'):
            receipt.validate_summary(summary)
    def test_pinned_invocations_and_host_inventory(self):
        for change in ('host-argv','whitespace-argv','command-env','summary-env','empty-host','wrong-host','empty-host-workspace'):
            summary = self.success()
            host = next(c for c in summary['commands'] if c['gate_id'] == 'host')
            if change.endswith('-argv'):
                next(c for c in summary['commands'] if c['gate_id'] == change[:-5])['argv'] = ['/bin/true']
            if change == 'command-env': summary['commands'][0]['environment']['RUSTC_WRAPPER'] = '/bin/true'
            if change == 'summary-env': summary['environment']['RUSTC_WRAPPER'] = '/bin/true'
            if change in ('empty-host','empty-host-workspace','wrong-host'):
                names = ['invented'] if change == 'wrong-host' else []
                for key in ('expected','executed','passed'): host['inventory'][key] = list(names)
            if change == 'empty-host-workspace':
                for key in ('expected','executed','passed'): summary['commands'][3]['inventory'][key] = []
            with self.subTest(change=change), self.assertRaises(ValueError): receipt.validate_summary(summary)
        summary = self.success()
        extra = dict(summary['coverage']['artifacts'][0],id='host-extra',executable='/work/target/host-extra')
        summary['coverage']['artifacts'].append(extra)
        summary['coverage']['host_artifacts'].append(extra['id'])
        command = copy.deepcopy(summary['commands'][3])
        command.update(id='workspace-extra',artifact_id=extra['id'],argv=[extra['executable']])
        for key in ('expected','executed','passed'): command['inventory'][key] = ['second_test']
        summary['commands'].append(command)
        host = next(c for c in summary['commands'] if c['gate_id'] == 'host')
        host['artifact_ids'] = ['host-bin','host-extra']
        with self.assertRaises(ValueError): receipt.validate_summary(summary)
        for key in ('expected','executed','passed'): host['inventory'][key] = ['host_test','second_test']
        receipt.validate_summary(summary)
    def test_malformed_summary_types(self):
        cases = [(('commands',0,'log'),[]),(('commands',0,'log_sha256'),[]),
                 (('commands',3,'inventory'),[]),(('commands',3,'inventory','expected'),[{}]),
                 (('commands',3,'artifact_id'),[]),(('commands',0,'gate_id'),[]),
                 (('commands',0,'id'),[]),(('commands',0,'argv'),[{}]),
                 (('commands',0,'environment'),[]),(('coverage','host_artifacts'),17),
                 (('coverage','host_artifacts'),[{}]),(('coverage','host_artifacts'),['host-bin','host-bin']),
                 (('coverage','artifacts',0,'id'),[]),(('coverage','artifacts',0,'package'),[]),
                 (('coverage','artifacts',0,'target'),{}),(('coverage','artifacts',0,'executable'),[]),
                 (('snapshot','content_sha256'),[]),(('installed','bundle_sha256'),{}),
                 (('request','repo_id'),[]),(('logs',0,'file'),[]),(('logs',0,'sha256'),[])]
        for path, value in cases:
            summary = self.success()
            parent = summary
            for key in path[:-1]: parent = parent[key]
            parent[path[-1]] = value
            with self.subTest(path=path,value=value), self.assertRaises(ValueError): receipt.validate_summary(summary)
    def test_success_missing_evidence(self):
        for change in ('artifact','command-log','desktop-names','status_complete','collector_complete',
                       'exit_code','signal','timed_out','oom'):
            summary = self.success()
            command = summary['commands'][0]
            if change == 'artifact': summary['coverage']['artifacts'].pop(0)
            elif change == 'command-log': del command['log']
            elif change == 'desktop-names':
                desktop = next(c for c in summary['commands'] if c['gate_id'] == 'desktop-live')
                for key in ('expected','executed','passed'): desktop['inventory'][key] = []
            else: command[change] = {'exit_code':1,'signal':9,'timed_out':True,'oom':True}.get(change,False)
            with self.subTest(change=change), self.assertRaises(ValueError): receipt.validate_summary(summary)
    def test_q33_ignored_and_zero_test_inventories(self):
        # Q33: a workspace artifact may record #[ignore] tests (never as passed) and a zero-test target
        # may record empty lists; desktop-live stays exact with nothing ignored; failed/filtered stay [].
        def ignored_summary():
            summary = self.success()
            workspace = summary['commands'][3]
            workspace['inventory'] = dict(expected=['host_test','child_entry'],executed=['host_test'],passed=['host_test'],
                                          failed=[],ignored=['child_entry'],filtered=[])
            host = next(c for c in summary['commands'] if c['gate_id'] == 'host')
            host['inventory'] = copy.deepcopy(workspace['inventory'])
            return summary
        receipt.validate_summary(ignored_summary())
        summary = self.success()
        extra = dict(summary['coverage']['artifacts'][0],id='empty-bin',executable='/work/target/empty-bin',
                     package='nexus-mother',target='nexus-mother')
        summary['coverage']['artifacts'].append(extra)
        command = copy.deepcopy(summary['commands'][3])
        command.update(id='workspace-empty',artifact_id='empty-bin',argv=[extra['executable']],
                       inventory=dict(expected=[],executed=[],passed=[],failed=[],ignored=[],filtered=[]))
        summary['commands'].append(command)
        receipt.validate_summary(summary)
        for change in ('ignored-also-passed','ignored-not-expected','passed-missing','duplicate-ignored',
                       'failed','filtered','desktop-ignored','host-empty-expected','ignored-not-list',
                       'duplicate-ignored-only','ignored-empty-string','host-partition',
                       'nonhost-ignored-also-passed','nonhost-ignored-empty-string'):
            summary = ignored_summary()
            inv = summary['commands'][3]['inventory']
            if change == 'ignored-also-passed': inv['passed'] = inv['executed'] = ['host_test','child_entry']
            if change == 'ignored-not-expected': inv['ignored'] = ['invented']
            if change == 'passed-missing': inv['passed'] = inv['executed'] = []
            if change == 'duplicate-ignored': inv.update(expected=['host_test','child_entry','child_entry'],ignored=['child_entry','child_entry'])
            if change == 'failed': inv['failed'] = ['child_entry']
            if change == 'filtered': inv['filtered'] = ['child_entry']
            if change == 'ignored-not-list': inv['ignored'] = 'child_entry'
            if change == 'duplicate-ignored-only': inv['ignored'] = ['child_entry','child_entry']
            if change.startswith('nonhost-'):
                # Per-command consistency also holds for a workspace artifact outside the host aggregate.
                other = next(c for c in summary['commands'] if c['gate_id'] == 'workspace' and c['artifact_id'] != 'host-bin')['inventory']
                if change == 'nonhost-ignored-also-passed': other['ignored'] = [other['expected'][0]]
                else: other['ignored'] = ''
            if change == 'host-partition':
                # review F6-3 note 4: the host aggregate must carry the workspace partition exactly.
                host = next(c for c in summary['commands'] if c['gate_id'] == 'host')['inventory']
                host.update(passed=['host_test','child_entry'],executed=['host_test','child_entry'],ignored=[])
            if change == 'ignored-empty-string':
                inv.update(ignored='',passed=list(inv['expected']),executed=list(inv['expected']))
            if change == 'desktop-ignored':
                desktop = next(c for c in summary['commands'] if c['gate_id'] == 'desktop-live')['inventory']
                desktop['ignored'] = [desktop['expected'][0]]
                desktop['passed'] = desktop['executed'] = desktop['expected'][1:]
            if change == 'host-empty-expected':
                for c in summary['commands']:
                    if c['gate_id'] in ('host',) or c.get('artifact_id') == 'host-bin':
                        c['inventory'] = dict(expected=[],executed=[],passed=[],failed=[],ignored=[],filtered=[])
            with self.subTest(change=change), self.assertRaises(ValueError): receipt.validate_summary(summary)
    def test_publication_destination_race(self):
        # A competing publisher wins after lexists; only NOREPLACE prevents clobbering it.
        destination = self.receipts/self.run_id
        def race(path):
            self.assertEqual(path,destination)
            destination.mkdir()
            return False
        with patch.object(receipt.os.path,'lexists',side_effect=race):
            with self.assertRaises(FileExistsError): self.publish()
        self.assertEqual(list(destination.iterdir()),[])
        self.assertTrue((self.pending/self.run_id/'summary.json').exists())
    def test_schema_and_fabricated_success(self):
        receipt.validate_summary(self.summary)
        for change in ({'status':'pass'},{'status':'PASS_SCOPED'},{'schema_version':2},
                       {'run_id':'../x'},{'peer_uid':True},{'limitations':[]}):
            with self.subTest(change=change), self.assertRaises(ValueError):
                receipt.validate_summary(dict(self.summary, **change))
        for key in self.summary:
            bad = dict(self.summary)
            del bad[key]
            with self.subTest(missing=key), self.assertRaises(ValueError):
                receipt.validate_summary(bad)
    def test_publication_failure_is_retained(self):
        with patch.object(receipt.os, 'rename', side_effect=OSError('fixture rename failure')):
            with self.assertRaises(OSError): self.publish()
        self.assertFalse((self.receipts/self.run_id).exists())
        self.assertTrue((self.pending/self.run_id/'summary.json').exists())
