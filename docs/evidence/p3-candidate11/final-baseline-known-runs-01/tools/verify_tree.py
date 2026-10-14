#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE1-01: byte-level source-freeze check of the mission worktree.

  verify_tree.py <worktree>      prints a JSON summary; exit 0 only if the worktree is exactly Candidate C

Checks: HEAD is C and its tree is C's tree; HEAD is detached; the index matches HEAD's tree; `git status
--porcelain --ignored` lists nothing but the mission-owned target directory; and every tracked file's bytes on disk
(read raw, no filter) equal its blob in C's tree, mode included. No replacement, overlay or generated source can
pass this: an extra file is reported by `git status`, and a changed one by the byte comparison.
"""
import hashlib
import json
import os
import subprocess
import sys

C = 'd5964ee5326c2733177307db6f507328023d361a'
C_TREE = '51bac5fd335999cc995c0ab9f582a30244973ef0'


def git(wt, *args, data=None):
    return subprocess.run(['git', '-C', wt, *args], input=data, capture_output=True, check=True).stdout


def main(wt):
    head = git(wt, 'rev-parse', 'HEAD').decode().strip()
    tree = git(wt, 'rev-parse', 'HEAD^{tree}').decode().strip()
    detached = subprocess.run(['git', '-C', wt, 'symbolic-ref', '-q', 'HEAD'], capture_output=True).returncode != 0
    diff_index = git(wt, 'diff-index', '--cached', '--name-only', 'HEAD').decode().split()
    status = [l for l in git(wt, 'status', '--porcelain', '--ignored', '-z').decode().split('\0') if l]
    allowed = {'!! target/'}
    unexpected_status = [s for s in status if s not in allowed]
    entries = []
    for e in git(wt, 'ls-tree', '-r', '-z', C).split(b'\0'):
        if e:
            meta, path = e.split(b'\t', 1)
            mode, kind, obj = meta.decode().split()
            entries.append((mode, kind, obj, path.decode()))
    mismatched, missing, mode_wrong = [], [], []
    blobs = [e for e in entries if e[1] == 'blob']
    proc = subprocess.Popen(['git', '-C', wt, 'cat-file', '--batch'], stdin=subprocess.PIPE, stdout=subprocess.PIPE)
    total = 0
    for mode, kind, obj, path in blobs:
        proc.stdin.write((obj + '\n').encode())
        proc.stdin.flush()
        header = proc.stdout.readline().split()
        size = int(header[2])
        content = proc.stdout.read(size)
        proc.stdout.read(1)
        full = os.path.join(wt, path)
        if mode == '120000':
            if not os.path.islink(full) or os.readlink(full).encode() != content:
                mismatched.append(path)
            continue
        if not os.path.isfile(full) or os.path.islink(full):
            missing.append(path)
            continue
        with open(full, 'rb') as f:
            disk = f.read()
        total += len(disk)
        if hashlib.sha256(disk).digest() != hashlib.sha256(content).digest():
            mismatched.append(path)
        executable = bool(os.stat(full).st_mode & 0o100)
        if executable != (mode == '100755'):
            mode_wrong.append(path)
    proc.stdin.close()
    proc.wait()
    gitlinks = [e[3] for e in entries if e[1] == 'commit']
    result = {
        'worktree': wt, 'head': head, 'tree': tree, 'detached': detached,
        'tracked_blobs_compared': len(blobs), 'bytes_compared': total, 'gitlinks': gitlinks,
        'index_differs_from_head': diff_index, 'status_entries': status, 'unexpected_status': unexpected_status,
        'content_mismatches': mismatched, 'missing': missing, 'mode_mismatches': mode_wrong,
    }
    ok = (head == C and tree == C_TREE and detached and not diff_index and not unexpected_status
          and not mismatched and not missing and not mode_wrong)
    result['verdict'] = 'EXACTLY_C' if ok else 'NOT_C'
    print(json.dumps(result, indent=1))
    return 0 if ok else 1


if __name__ == '__main__':
    sys.exit(main(sys.argv[1]))
