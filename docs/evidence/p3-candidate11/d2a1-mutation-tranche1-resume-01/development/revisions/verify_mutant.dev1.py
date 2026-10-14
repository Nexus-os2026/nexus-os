#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission sections 10 and 11: byte-level check that a disposable
worktree is exactly Candidate C plus one control's frozen patch. Repaired by P3-C11-DR01-D2A1-MUTATION-TRANCHE1-
RESUME-01 (sections 5-7): the status predicate is the path-aware policy below; nothing else changed.

  verify_mutant.py <worktree> <path> <mutant-blob> <mutant-sha256>
      prints a JSON summary; exit 0 only if the worktree is exactly C with that one file replaced by the mutant bytes

The checks of verify_tree.py (Stage 1, reused unchanged in its own file) with one difference: the named file must hold
the mutant bytes (git blob id computed in-process, never written to any object store; and sha256), and `git status`
may show only that file as modified (plus ignored entries beneath the worktree's own target directory). HEAD must
still be C, detached, the index must match HEAD (nothing staged), and every other tracked file must be byte-equal to
C's blob, mode included. No second control, overlay or untracked source can pass: an extra file is reported by `git
status`, and any other changed file by the byte comparison.

Tracked-source identity is established by the byte comparison, never by status strings. The status policy
(status_policy(), also used by control.py for restoration) reads `git status --porcelain --ignored
--untracked-files=all --no-renames -z` as NUL-delimited records and accepts only:
  - with a mutant file named: exactly one record ` M <that file>`; for a restored worktree: no tracked record;
  - zero or more `!!` (ignored) records whose path is exactly `target/` or begins with `target/`, with no empty, `.`
    or `..` component, each resolving (realpath) beneath the worktree's own `target`, which, when it exists, must be
    a real directory (not a symlink), exactly the worktree's own `target` entry (realpath <worktree>/target), and
    neither a mount point nor holding one (so it cannot be another worktree's target).
Everything else is rejected: any `??` record, any ignored record outside target, any second modified file, any
staged, deleted, renamed or type-changed record, a malformed record, a symlinked or foreign target, a path that
escapes. git runs with GIT_OPTIONAL_LOCKS=0, so the status listing never rewrites the worktree's index.
"""
import hashlib
import json
import os
import stat
import subprocess
import sys

C = 'd5964ee5326c2733177307db6f507328023d361a'
C_TREE = '51bac5fd335999cc995c0ab9f582a30244973ef0'
TARGET = 'target'
ENV = dict(os.environ, GIT_OPTIONAL_LOCKS='0')


def git(wt, *args):
    return subprocess.run(['git', '-C', wt, *args], capture_output=True, check=True, env=ENV).stdout


def blob_id(data):
    return hashlib.sha1(b'blob %d\0' % len(data) + data).hexdigest()


def status_records(wt):
    """The NUL-delimited porcelain v1 records: (raw bytes, [(XY, path)], [malformed fields]). A rename or copy record
    (impossible with --no-renames) carries its source path as the next field; both are kept as records, so they are
    rejected, never mis-parsed."""
    raw = git(wt, 'status', '--porcelain', '--ignored', '--untracked-files=all', '--no-renames', '-z')
    fields = raw.split(b'\0')
    malformed = []
    if fields[-1] != b'':
        malformed.append('listing not NUL-terminated: ' + fields[-1].decode(errors='replace'))
    fields = fields[:-1]
    records, i = [], 0
    while i < len(fields):
        f = fields[i]
        i += 1
        if len(f) < 4 or f[2:3] != b' ':
            malformed.append(f.decode(errors='replace'))
            continue
        xy, p = f[:2].decode(errors='replace'), f[3:].decode(errors='surrogateescape')
        records.append((xy, p))
        if xy[0] in 'RC' or xy[1] in 'RC':
            records.append(('<-', fields[i].decode(errors='surrogateescape') if i < len(fields) else ''))
            i += 1
    return raw, records, malformed


def mount_points():
    out = []
    with open('/proc/self/mountinfo') as f:
        for line in f:
            mp = line.split()[4]
            for esc, ch in (('\\040', ' '), ('\\011', '\t'), ('\\012', '\n'), ('\\134', '\\')):
                mp = mp.replace(esc, ch)
            out.append(mp)
    return out


def target_facts(wt):
    """The worktree's own target entry: absent, or a real directory that is exactly <realpath worktree>/target."""
    top = os.path.realpath(wt)
    t = os.path.join(wt, TARGET)
    f = {'path': t, 'worktree_is_own_realpath': top == os.path.abspath(wt), 'exists': os.path.lexists(t)}
    if f['exists']:
        st = os.lstat(t)
        f['is_symlink'] = stat.S_ISLNK(st.st_mode)
        f['is_dir'] = stat.S_ISDIR(st.st_mode)
        f['realpath'] = os.path.realpath(t)
        f['own_entry'] = f['realpath'] == os.path.join(top, TARGET)
        f['mounts'] = [m for m in mount_points() if m == f['realpath'] or m.startswith(f['realpath'] + '/')]
        f['ok'] = (f['worktree_is_own_realpath'] and f['is_dir'] and not f['is_symlink'] and f['own_entry']
                   and not f['mounts'])
    else:
        f['ok'] = f['worktree_is_own_realpath']
    return f


def target_entry_problem(wt, p, tf):
    """None if p is the worktree-local target or a normal path beneath it that resolves beneath it; else why not."""
    if p == TARGET + '/':
        rel = ''
    elif p.startswith(TARGET + '/'):
        rel = p[len(TARGET) + 1:]
    else:
        return 'ignored path outside the worktree-local target'
    body = rel[:-1] if rel.endswith('/') else rel
    if rel and any(c in ('', '.', '..') for c in body.split('/')):
        return 'path traversal or non-normal component'
    if not tf['exists'] or not tf['ok']:
        return 'the worktree-local target is absent, a symlink, foreign, or not a real directory'
    real = os.path.realpath(os.path.join(wt, p))
    if not (real == tf['realpath'] or real.startswith(tf['realpath'] + '/')):
        return 'resolves outside the worktree-local target'
    return None


def status_policy(wt, modified=None):
    """Resume mission section 7. modified: the one tracked file a mutant worktree may show as ` M`; None for a
    restored worktree (no tracked modification at all). Returns a JSON-able record with 'ok'."""
    raw, records, malformed = status_records(wt)
    tf = target_facts(wt)
    modified_seen, target_entries, rejected = [], 0, []
    for xy, p in records:
        if xy == ' M' and modified is not None and p == modified:
            modified_seen.append(p)
        elif xy == '!!':
            why = target_entry_problem(wt, p, tf)
            if why:
                rejected.append({'record': xy + ' ' + p, 'why': why})
            else:
                target_entries += 1
        else:
            rejected.append({'record': xy + ' ' + p, 'why': 'untracked' if xy == '??' else
                             'rename/copy source' if xy == '<-' else 'tracked change not allowed here'})
    rejected += [{'record': m, 'why': 'malformed status record'} for m in malformed]
    if not tf['ok']:
        rejected.append({'record': TARGET, 'why': 'the target entry is not the worktree\'s own real directory: '
                         + json.dumps({k: v for k, v in tf.items() if k != 'path'})})
    want = [modified] if modified is not None else []
    ok = not rejected and modified_seen == want
    shown = [xy + ' ' + p for xy, p in records if not (xy == '!!' and not target_entry_problem(wt, p, tf))]
    if target_entries:
        shown.append(f'!! target/... ({target_entries} ignored entries, each beneath the worktree-local real target)')
    return {'ok': ok, 'mode': 'mutant' if modified is not None else 'restored', 'allowed_modified': modified,
            'modified_seen': modified_seen, 'records': len(records), 'target_entries': target_entries,
            'status_sha256': hashlib.sha256(raw).hexdigest(), 'shown': shown, 'rejected': rejected, 'target': tf}


def main(wt, path, mutant_blob, mutant_sha):
    head = git(wt, 'rev-parse', 'HEAD').decode().strip()
    tree = git(wt, 'rev-parse', 'HEAD^{tree}').decode().strip()
    detached = subprocess.run(['git', '-C', wt, 'symbolic-ref', '-q', 'HEAD'], capture_output=True).returncode != 0
    diff_index = git(wt, 'diff-index', '--cached', '--name-only', 'HEAD').decode().split()
    pol = status_policy(wt, path)
    status, unexpected_status = pol['shown'], pol['rejected']
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
        'status_policy': pol, 'other_content_mismatches': mismatched, 'missing': missing, 'mode_mismatches': mode_wrong,
        'mutant_file': mutant, 'expected_mutant_blob': mutant_blob, 'expected_mutant_sha256': mutant_sha,
    }
    ok = (head == C and tree == C_TREE and detached and not diff_index and pol['ok']
          and not mismatched and not missing and not mode_wrong and mutant is not None
          and mutant['differs_from_baseline'] and mutant['git_blob'] == mutant_blob and mutant['sha256'] == mutant_sha)
    result['verdict'] = 'EXACTLY_C_PLUS_PATCH' if ok else 'NOT_THE_MUTANT'
    print(json.dumps(result, indent=1))
    return 0 if ok else 1


if __name__ == '__main__':
    sys.exit(main(*sys.argv[1:5]))
