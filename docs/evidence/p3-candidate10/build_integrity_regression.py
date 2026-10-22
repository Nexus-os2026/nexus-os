#!/usr/bin/env python3
"""Isolated M70 stale-build regression; run only in an expendable clean checkout.

This is not a campaign.  It tests one corrected mutation and the build identity
barrier, then repeats the formerly contaminated pristine judge ten times.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE / 'original-183'))
import campaign


def git(root, *args):
    return subprocess.check_output(['git', '--no-optional-locks', '-C', str(root), *args]).decode().strip()


def latest_binary(target):
    files = [p for p in (target/'debug/deps').glob('nexus_governed_control-*')
             if p.is_file() and os.access(p, os.X_OK)]
    assert files, 'no governed-control test executable'
    return max(files, key=lambda p:p.stat().st_mtime_ns)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--root', required=True, type=Path)
    parser.add_argument('--sha', required=True)
    parser.add_argument('--environment', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    args = parser.parse_args()
    root = args.root.resolve()
    out = args.out.resolve()
    assert not out.is_relative_to(root)
    assert git(root, 'rev-parse', 'HEAD') == args.sha
    assert git(root, 'status', '--porcelain=v1', '--untracked-files=all') == ''
    env = json.loads(args.environment.read_text())
    target = Path(env['CARGO_TARGET_DIR']).resolve()
    out.mkdir(parents=True, exist_ok=False)
    guard = campaign.BuildIntegrity(root, target, env, out/'build-integrity')
    control = json.loads(campaign.MANIFEST.read_text())['controls'][69]
    assert control['id'] == 'M70' and len(control['edits']) == 2
    original, mutant = campaign.edited(root, control)
    package = {'nexus-governed-control'}
    assert guard.packages_for(original) == package
    result = {'sha':args.sha, 'control':'M70', 'historical_oracle': {
        'stale_mutant_sha256':'4a9e3996c7fea84a18ba4a7b7fd208033646dd44a3b8a20cf40ca3348cf75aaf',
        'rebuilt_pristine_sha256':'3ac0f2aeada14dea881878c6cc631e35cb1a55fc6a8d694539886b137a383933',
        'classification':'stale M70 mutant reused as Fresh on R1 before package clean'}}
    changed = False
    try:
        guard.refresh_tracked(package, 'regression pristine entry')
        baseline = campaign.run(root, control, env, out/'m70-pristine-before.log', 900, guard, package)
        result['m70_baseline'] = baseline
        assert baseline['observation'] == 'SURVIVED' and baseline['returncode'] == 0, baseline
        for name, content in mutant.items():
            (root/name).write_bytes(content)
        changed = True
        guard.barrier(mutant, package, 'regression corrected M70 mutant')
        failed = campaign.run(root, control, env, out/'m70-mutant.log', 900, guard, package)
        result['m70_mutant'] = failed
        assert failed['observation'] == 'INTENDED TEST FAILURE', failed
        mutant_binary = latest_binary(target)
        result['mutant_executable'] = str(mutant_binary)
        result['mutant_sha256'] = sha(mutant_binary)
        # The R1 forensic archive is the reproduction oracle.  Restoring
        # bytes and clean Git state here is intentionally *not* accepted as
        # proof that the warm executable is pristine.
        for name, content in original.items():
            (root/name).write_bytes(content)
        result['bytes_restored_before_guard'] = all((root/n).read_bytes() == v for n,v in original.items())
        assert result['bytes_restored_before_guard']
        guard.restore(original, package, 'regression original bytes and strict mtime')
        guard.prove_pending('regression pristine package build proof')
        changed = False
        pristine_binary = latest_binary(target)
        result['pristine_executable'] = str(pristine_binary)
        result['pristine_sha256'] = sha(pristine_binary)
        assert result['pristine_sha256'] != result['mutant_sha256'], 'stale mutant executable retained'
        test = 'governed::tests::a_display_start_is_recorded_with_no_lock_held_and_a_stop_overtakes_it'
        command = ['cargo','test','-p','nexus-governed-control','--locked','--lib','--',test,'--exact']
        repetitions = []
        for number in range(11):
            execution, proof = guard.run_cargo(command, root, f'governed-pristine-{number:02d}', 900)
            passed = execution['returncode'] == 0 and f'test {test} ... ok' in execution['text'] and '1 passed; 0 failed' in execution['text']
            repetitions.append({'index':number, 'passed':passed, 'build_identity':proof,
                                'log':execution['log']})
            assert passed, f'governed pristine test failed on repeat {number}'
        result['governed_repeats'] = repetitions
        assert git(root,'status','--porcelain=v1','--untracked-files=all') == ''
        result['scratch_clean'] = True
        result['status'] = 'PASS'
    finally:
        if changed:
            guard.restore(original,package,'regression exception restoration')
            guard.prove_pending('regression exception pristine proof')
        result['final_git_status'] = git(root,'status','--porcelain=v1','--untracked-files=all')
        (out/'result.json').write_text(json.dumps(result,indent=2,sort_keys=True)+'\n')
    print('M70 and stale-build regression: PASS; governed pristine 11/11; scratch clean')

if __name__ == '__main__':
    main()
