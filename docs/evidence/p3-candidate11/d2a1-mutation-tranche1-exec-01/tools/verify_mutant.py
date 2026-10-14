#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission sections 10 and 11: byte-level check that a disposable
worktree is exactly Candidate C plus one control's frozen patch.

  verify_mutant.py <worktree> <path> <mutant-blob> <mutant-sha256>
      prints a JSON summary; exit 0 only if the worktree is exactly C with that one file replaced by the mutant bytes

The checks of verify_tree.py (Stage 1, reused unchanged in its own file) with one difference: the named file must hold
the mutant bytes (git blob id computed in-process, never written to any object store; and sha256), and `git status`
may show only that file as modified (plus the worktree-local target directory). HEAD must still be C, detached, the
index must match HEAD (nothing staged), and every other tracked file must be byte-equal to C's blob, mode included.
No second control, overlay or untracked source can pass: an extra file is reported by `git status`, and any other
changed file by the byte comparison.
"""
import hashlib
import json
import os
import subprocess
import sys

C = 'd5964ee5326c2733177307db6f507328023d361a'
C_TREE = '51bac5fd335999cc995c0ab9f582a30244973ef0'


def git(wt, *args):
    return subprocess.run(['git', '-C', wt, *args], capture_output=True, check=True).stdout


def blob_id(data):
    return hashlib.sha1(b'blob %d\0' % len(data) + data).hexdigest()


def main(wt, path, mutant_blob, mutant_sha):
    head = git(wt, 'rev-parse', 'HEAD').decode().strip()
    tree = git(wt, 'rev-parse', 'HEAD^{tree}').decode().strip()
    detached = subprocess.run(['git', '-C', wt, 'symbolic-ref', '-q', 'HEAD'], capture_output=True).returncode != 0
    diff_index = git(wt, 'diff-index', '--cached', '--name-only', 'HEAD').decode().split()
    status = [l for l in git(wt, 'status', '--porcelain', '--ignored', '--untracked-files=all', '-z').decode().split('\0')
              if l]
    allowed = {' M ' + path, '!! target/'}
    unexpected_status = [s for s in status if s not in allowed]
    entries = []
    for e in git(wt, 'ls-tree', '-r', '-z', C).split(b'\0'):
        if e:
            meta, p = e.split(b'\t', 1)
            mode, kind, obj = meta.decode().split()
            entries.append((mode, kind, obj, p.decode()))
    mismatched, missing, mode_wrong = [], [], []
    blobs = [e for e in entries if e[1] == 'blob']
    proc = subprocess.Popen(['git', '-C', wt, 'cat-file', '--batch'], stdin=subprocess.PIPE, stdout=subprocess.PIPE)
    total, mutant = 0, None
    for mode, kind, obj, p in blobs:
        proc.stdin.write((obj + '\n').encode())
        proc.stdin.flush()
        header = proc.stdout.readline().split()
        size = int(header[2])
        content = proc.stdout.read(size)
        proc.stdout.read(1)
        full = os.path.join(wt, p)
        if mode == '120000':
            if not os.path.islink(full) or os.readlink(full).encode() != content:
                mismatched.append(p)
            continue
        if not os.path.isfile(full) or os.path.islink(full):
            missing.append(p)
            continue
        with open(full, 'rb') as f:
            disk = f.read()
        total += len(disk)
        if p == path:
            mutant = {'path': p, 'baseline_blob': obj, 'git_blob': blob_id(disk),
                      'sha256': hashlib.sha256(disk).hexdigest(), 'bytes': len(disk),
                      'differs_from_baseline': disk != content}
        elif hashlib.sha256(disk).digest() != hashlib.sha256(content).digest():
            mismatched.append(p)
        executable = bool(os.stat(full).st_mode & 0o100)
        if executable != (mode == '100755'):
            mode_wrong.append(p)
    proc.stdin.close()
    proc.wait()
    result = {
        'worktree': wt, 'head': head, 'tree': tree, 'detached': detached,
        'tracked_blobs_compared': len(blobs), 'bytes_compared': total,
        'index_differs_from_head': diff_index, 'status_entries': status, 'unexpected_status': unexpected_status,
        'other_content_mismatches': mismatched, 'missing': missing, 'mode_mismatches': mode_wrong,
        'mutant_file': mutant, 'expected_mutant_blob': mutant_blob, 'expected_mutant_sha256': mutant_sha,
    }
    ok = (head == C and tree == C_TREE and detached and not diff_index and not unexpected_status
          and ' M ' + path in status and not mismatched and not missing and not mode_wrong and mutant is not None
          and mutant['differs_from_baseline'] and mutant['git_blob'] == mutant_blob and mutant['sha256'] == mutant_sha)
    result['verdict'] = 'EXACTLY_C_PLUS_PATCH' if ok else 'NOT_THE_MUTANT'
    print(json.dumps(result, indent=1))
    return 0 if ok else 1


if __name__ == '__main__':
    sys.exit(main(*sys.argv[1:5]))
