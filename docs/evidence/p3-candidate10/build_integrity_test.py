"""Harness-only build-identity checks; no Nexus production code is run."""
import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec=importlib.util.spec_from_file_location('build_integrity',Path(__file__).with_name('build_integrity.py'))
module=importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
BuildIntegrity=module.BuildIntegrity

class Integrity(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.base=Path(self.temp.name)
        self.root=self.base/'source';self.root.mkdir()
        self.target=self.base/'target';self.target.mkdir()
        self.evidence=self.base/'evidence'
        src=self.root/'crates/example/src';src.mkdir(parents=True)
        (self.root/'crates/example/Cargo.toml').write_text('[package]\nname="example"\nversion="0.1.0"\n')
        self.source=src/'lib.rs';self.source.write_bytes(b'pristine\n')
        subprocess.run(['git','init','-q',str(self.root)],check=True)
        subprocess.run(['git','-C',str(self.root),'add','.'],check=True)
        subprocess.run(['git','-C',str(self.root),'-c','user.name=Harness','-c','user.email=harness@example.invalid','commit','-qm','fixture'],check=True)
        self.guard=BuildIntegrity(self.root,self.target,{'CARGO_TARGET_DIR':str(self.target)},self.evidence)
        self.relative='crates/example/src/lib.rs'

    def test_restoration_verifies_bytes_git_and_strict_mtime(self):
        artifact=self.target/'debug/deps/example-abc';artifact.parent.mkdir(parents=True)
        artifact.write_bytes(b'mutant executable')
        artifact_ns=artifact.stat().st_mtime_ns
        self.source.write_bytes(b'mutant\n')
        hashes=self.guard.restore({self.relative:b'pristine\n'},{'example'},'unit restore')
        self.assertEqual(hashes[0]['sha256'],module.sha256(b'pristine\n'))
        self.assertGreater(self.source.stat().st_mtime_ns,artifact_ns)
        self.assertLessEqual(self.source.stat().st_mtime_ns,__import__('time').time_ns())
        self.assertIn('example',self.guard.pending)
        self.assertEqual(subprocess.check_output(['git','-C',str(self.root),'status','--porcelain']),b'')

    def test_pending_mutant_blocks_unproven_cargo_run(self):
        self.guard.pending['example'] = {'source_hashes': [], 'barrier': {}}
        with self.assertRaises(AssertionError):
            self.guard.run_cargo(['cargo','test','-p','example'],self.root,'pending',10)

    def test_missing_compile_proof_requires_package_clean_and_rebuild(self):
        first={'text':'Fresh example v0.1.0\n','returncode':0,'timeout':False,'log':'first'}
        second={'text':'Compiling example v0.1.0\n','returncode':0,'timeout':False,'log':'second'}
        with patch.object(self.guard,'_execute',side_effect=[first,second]) as execute, patch.object(self.guard,'clean_package') as clean:
            result,proof=self.guard.run_cargo(['cargo','test','-p','example'],self.root,'unit',10,{'example'})
        self.assertEqual(execute.call_count,2)
        clean.assert_called_once()
        self.assertIs(result,second)
        self.assertTrue(proof['clean_fallback'])

if __name__=='__main__': unittest.main()
