"""Evidence-classification checks only; never apply repository mutations."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec=importlib.util.spec_from_file_location('campaign',Path(__file__).with_name('campaign.py'))
campaign=importlib.util.module_from_spec(spec)
spec.loader.exec_module(campaign)

class Classification(unittest.TestCase):
    def observed(self, output, code=101, rust=True):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)
            def proc(*args,**kwargs):
                kwargs['stdout'].write(output.encode())
                class Process:
                    returncode=code
                    def wait(self,timeout=None): return code
                return Process()
            control=dict(command=['cargo'] if rust else ['npx'],cwd='.',expected_failure_tests=['intended'])
            with patch.object(campaign.subprocess,'Popen',proc):
                return campaign.run(root,control,{},root/'run.log',1)

    def test_compiler_rejection_never_counts_as_kill(self):
        result=self.observed('error[E0123]: bad mutation\n')
        self.assertEqual(result['observation'],'COMPILE ERROR')

    def test_zero_selected_tests_never_passes_baseline(self):
        result=self.observed('Running unittests src/lib.rs\nrunning 0 tests\ntest result: ok. 0 passed\n',0)
        self.assertEqual(result['observation'],'INFRA ERROR')
        self.assertFalse(result['intended_baseline_passed'])

    def test_unrelated_failure_never_counts(self):
        result=self.observed('Running unittests src/lib.rs\ntest unrelated ... FAILED\n')
        self.assertEqual(result['observation'],'INFRA ERROR')

    def test_intended_failure_requires_semantic_review(self):
        result=self.observed('Running unittests src/lib.rs\ntest intended ... FAILED\n')
        self.assertEqual(result['observation'],'INTENDED TEST FAILURE')
        self.assertNotIn('verdict',result)

    def test_passing_intended_test_is_a_survivor(self):
        result=self.observed('Running unittests src/lib.rs\ntest intended ... ok\n',0)
        self.assertEqual(result['observation'],'SURVIVED')
        self.assertTrue(result['intended_baseline_passed'])

    def test_named_test_without_test_binary_is_infrastructure_error(self):
        self.assertEqual(self.observed('test intended ... FAILED\n')['observation'],'INFRA ERROR')

    def test_vitest_error_must_identify_intended_case(self):
        result=self.observed('Test Files 1 failed\nTests 1 failed\nFAIL file > unrelated\n',1,False)
        self.assertEqual(result['observation'],'INFRA ERROR')
        result=self.observed('Test Files 1 failed\nTests 1 failed\nFAIL file > intended\n',1,False)
        self.assertEqual(result['observation'],'INTENDED TEST FAILURE')

    def test_anchor_ambiguity_is_refused_before_writing(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp); (root/'a').write_bytes(b'one one')
            control=dict(id='M01',edits=[dict(file='a',anchor='one',replacement='two',count=1)])
            with self.assertRaises(AssertionError): campaign.edited(root,control)
            self.assertEqual((root/'a').read_bytes(),b'one one')

    def test_multifile_edits_are_prepared_in_memory(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp); (root/'a').write_bytes(b'one'); (root/'b').write_bytes(b'three')
            control=dict(id='M166',edits=[dict(file='a',anchor='one',replacement='two',count=1),dict(file='b',anchor=None,replacement=' four',count=1)])
            before,after=campaign.edited(root,control)
            self.assertEqual(before,{'a':b'one','b':b'three'})
            self.assertEqual(after,{'a':b'two','b':b'three four'})
            self.assertEqual((root/'a').read_bytes(),b'one')
            self.assertEqual((root/'b').read_bytes(),b'three')

if __name__=='__main__': unittest.main()
