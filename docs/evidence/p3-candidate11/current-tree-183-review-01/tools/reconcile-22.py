#!/usr/bin/env python3
"""Mechanical facts for the 22 stale (control, file) pairs of the current-tree manifest ct-1.

Mission P3-C11-DR01-D2A1-CTM01-CANDIDATE-BASELINE-01; contract basis AD-1-R4 H.4.

Read only. For each pair it reports: the historically recorded hash; the
current sha256 and git blob; the first-known stale point from the existing
receipt; the transition commit derived from path-limited history (the first
commit after which the file no longer has the recorded hash) and the
historical commit H (its parent, where the file still has the recorded hash);
each anchor's count and lines at H and at the validated commit; whether the
definition is byte-equal to the historical one; a +/-20-line context
comparison around each anchor; the enclosing function's identity at H and now;
and each expected failing test's function identity at H and now.

It records facts only. It makes no SAME PROPERTY finding: that is the Sole
Architect's (AD-1-R4 H.4).
"""
import argparse
import hashlib
import json
import re
import subprocess
import sys

HIST_PATH = 'docs/evidence/p3-candidate10/original-183/manifest.json'
HIST_SHA256 = '2c4adc84121c1efbbfe84aa4422444407c4d1eb4b556fbf3a0ab8e22d4782e14'
CT_PATH = 'docs/evidence/p3-candidate11/current-tree-183/manifest.json'
DEF = ['id', 'category', 'historical_meaning', 'candidate10_meaning', 'historical',
       'edits', 'command', 'cwd', 'expected_failure_tests']
# First-known stale point, from the existing receipt (EXECUTOR-REPORTED):
# 8693e3a7:docs/evidence/p3-candidate11/ad1r2-review-packet-01/evidence/
#   p3-c11-dr01-d2a1-campaign-01/receipts/final/01-mutation-manifest-stale-hashes.txt
# (blob 8b0654b448f69a8a8ef84f60f664934b6c4e5170), which sampled five points.
RECEIPT = ('8693e3a7:docs/evidence/p3-candidate11/ad1r2-review-packet-01/evidence/'
           'p3-c11-dr01-d2a1-campaign-01/receipts/final/01-mutation-manifest-stale-hashes.txt')
RECEIPT_BLOB = '8b0654b448f69a8a8ef84f60f664934b6c4e5170'
FIRST_KNOWN = {
    'app/src-tauri/src/phase3_tests.rs': 'D2A-1 commit 22ebf9a5c5b1',
    'crates/nexus-governed-control/src/lib.rs': 'D2A-1 commit 22ebf9a5c5b1',
    'crates/nexus-governed-control/src/broker.rs': 'D2A-1 contract base 862a11cef284',
    'crates/nexus-governed-control/src/authority/approval.rs': 'Candidate 11 b1d535f1bb63',
    'crates/nexus-governed-control/src/authority/mod.rs': 'Candidate 11 b1d535f1bb63',
    'crates/nexus-governed-control/src/authority/run.rs': 'Candidate 11 b1d535f1bb63',
}
SET = {'M32': 'D2A-1', 'M78': 'D2A-1', 'M79': 'D2A-1', 'M80': 'D2A-1', 'M87': 'D2A-1',
       'M89': 'D2A-1', 'M112': 'D2A-1', 'M113': 'D2A-1', 'M141': 'D2A-1', 'M143': 'D2A-1',
       'M153': 'D2A-1', 'M177': 'D2A-1', 'M178': 'D2A-1', 'M179': 'D2A-1', 'M180': 'D2A-1',
       'M18': 'earlier Candidate 11', 'M19': 'earlier Candidate 11', 'M39': 'earlier Candidate 11',
       'M84': 'earlier Candidate 11', 'M05': 'earlier Candidate 11', 'M45': 'earlier Candidate 11',
       'M129': 'earlier Candidate 11'}
CRATE = {'nexus-governed-control': 'crates/nexus-governed-control/src/',
         'nexus-desktop-backend': 'app/src-tauri/src/'}
FN = re.compile(r'^(\s*)(pub(\([^)]*\))?\s+)?(const\s+)?(async\s+)?(unsafe\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)')
CONTEXT = 20


def git(repo, *args, ok=(0,)):
    done = subprocess.run(['git', '-C', repo, *args], capture_output=True)
    if done.returncode not in ok:
        raise SystemExit('git %s failed: %s' % (' '.join(args), done.stderr.decode()))
    return done.stdout


def show(repo, commit, path):
    return git(repo, 'cat-file', 'blob', '%s:%s' % (commit, path))


def sha(data):
    return hashlib.sha256(data).hexdigest()


def lines_of(data, anchor):
    out = []
    start = 0
    while True:
        at = data.find(anchor, start)
        if at < 0:
            return out
        out.append(data.count(b'\n', 0, at) + 1)
        start = at + len(anchor)


def fn_span(text_lines, start):
    """Lines [start, end] of the item beginning at start: up to the first later
    line that is exactly its indentation followed by '}' (rustfmt layout)."""
    indent = FN.match(text_lines[start]).group(1)
    for i in range(start + 1, len(text_lines)):
        if text_lines[i] == indent + '}':
            return start, i
        if text_lines[i].startswith(indent + '}') and text_lines[i].strip() == '}':
            return start, i
    return start, None


def fn_text(data, name):
    """The unique function named name in data, or a reason it is not unique."""
    text_lines = data.decode('utf-8').split('\n')
    starts = [i for i, line in enumerate(text_lines)
              if FN.match(line) and FN.match(line).group(7) == name]
    if len(starts) != 1:
        return None, '%d definitions' % len(starts)
    start, end = fn_span(text_lines, starts[0])
    if end is None:
        return None, 'no closing line'
    return '\n'.join(text_lines[start:end + 1]), 'lines %d-%d' % (start + 1, end + 1)


def enclosing_fn(data, line_no):
    text_lines = data.decode('utf-8').split('\n')
    for i in range(line_no - 1, -1, -1):
        m = FN.match(text_lines[i])
        if m:
            start, end = fn_span(text_lines, i)
            if end is not None and end >= line_no - 1:
                return m.group(7)
            if end is not None and end < line_no - 1:
                return None
    return None


def window(data, line_no, anchor):
    text_lines = data.decode('utf-8').split('\n')
    span = anchor.count(b'\n')
    lo = max(0, line_no - 1 - CONTEXT)
    hi = min(len(text_lines), line_no - 1 + span + CONTEXT + 1)
    return '\n'.join(text_lines[lo:hi])


def transition(repo, head, path, recorded):
    commits = git(repo, 'rev-list', '--reverse', head, '--', path).decode().split()
    states = []
    for c in commits:
        h = sha(show(repo, c, path))
        states.append((c, h == recorded))
    changes = [(c, s) for i, (c, s) in enumerate(states) if i == 0 or states[i - 1][1] != s]
    # The last change of state must be to "not recorded"; report that commit.
    last = changes[-1]
    if last[1] or len([1 for _, s in changes if s]) != 1:
        raise SystemExit('%s: unexpected hash history %s' % (path, changes))
    return last[0], [c[:12] + (' REC' if s else ' other') for c, s in changes]


def test_location(repo, commit, crate_dir, leaf):
    hits = git(repo, 'grep', '-l', '-E', r'fn\s+%s\b' % re.escape(leaf), commit, '--', crate_dir,
               ok=(0, 1)).decode().split()
    return sorted(h.split(':', 1)[1] for h in hits)


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    parser.add_argument('--repo', required=True)
    parser.add_argument('--commit', required=True, help='validated commit')
    parser.add_argument('--manifest-file', help='ct-1 file instead of the commit\'s copy')
    args = parser.parse_args()
    commit = git(args.repo, 'rev-parse', '--verify', args.commit + '^{commit}').decode().strip()
    hist_raw = show(args.repo, commit, HIST_PATH)
    assert sha(hist_raw) == HIST_SHA256
    receipt = git(args.repo, 'rev-parse', RECEIPT).decode().strip()
    assert receipt == RECEIPT_BLOB, receipt
    hist = {c['id']: c for c in json.loads(hist_raw)['controls']}
    if args.manifest_file:
        ct_raw = open(args.manifest_file, 'rb').read()
    else:
        ct_raw = show(args.repo, commit, CT_PATH)
    ct = json.loads(ct_raw)
    print('validated commit %s; ct-1 sha256 %s; first-known-stale receipt %s (blob %s)'
          % (commit, sha(ct_raw), RECEIPT, RECEIPT_BLOB))
    records = []
    for c in ct['controls']:
        for row in c['current_tree']['files']:
            if row['sha256_equals_historical_record']:
                continue
            f = row['file']
            h = hist[c['id']]
            trans, history = transition(args.repo, commit, f, h['source_sha256'][f])
            H = git(args.repo, 'rev-parse', trans + '^').decode().strip()
            old = show(args.repo, H, f)
            new = show(args.repo, commit, f)
            assert sha(old) == h['source_sha256'][f], (c['id'], 'H does not hold the recorded hash')
            assert sha(new) == row['sha256']
            rec = {
                'control': c['id'], 'set': SET[c['id']], 'file': f,
                'category': h['category'], 'meaning': h['candidate10_meaning'],
                'historical_recorded_sha256': h['source_sha256'][f],
                'current_sha256': row['sha256'], 'current_git_blob': row['git_blob'],
                'first_known_stale_from_existing_evidence': FIRST_KNOWN[f],
                'derived_transition_commit': trans,
                'derived_historical_commit_H': H,
                'path_history_states': history,
                'file_diff_H_to_current': git(args.repo, 'diff', '--shortstat', H, commit, '--', f
                                              ).decode().strip(),
                'definition_byte_equal': {k: json.dumps(c[k], ensure_ascii=False) ==
                                          json.dumps(h[k], ensure_ascii=False) for k in DEF},
                'anchors': [], 'expected_tests': [],
            }
            for i, e in enumerate(h['edits'], 1):
                if e['file'] != f:
                    continue
                a = e['anchor'].encode()
                old_lines = lines_of(old, a)
                new_lines = lines_of(new, a)
                enc_new = enclosing_fn(new, new_lines[0]) if new_lines else None
                enc_old = enclosing_fn(old, old_lines[0]) if old_lines else None
                fn_same = None
                if enc_new and enc_new == enc_old:
                    t_old, _ = fn_text(old, enc_new)
                    t_new, where = fn_text(new, enc_new)
                    fn_same = (t_old is not None and t_old == t_new)
                win_same = (len(old_lines) == len(new_lines) == 1 and
                            window(old, old_lines[0], a) == window(new, new_lines[0], a))
                rec['anchors'].append({
                    'edit': i, 'expected_count': e['count'],
                    'count_at_H': len(old_lines), 'lines_at_H': old_lines,
                    'count_now': len(new_lines), 'lines_now': new_lines,
                    'replacement_is_deletion': e['replacement'] == '',
                    'replacement_occurrences_now': (new.count(e['replacement'].encode())
                                                    if e['replacement'] else None),
                    'enclosing_fn_now': enc_new, 'enclosing_fn_at_H': enc_old,
                    'enclosing_fn_text_identical': fn_same,
                    'context_pm20_identical': win_same,
                })
            pkg = h['command'][h['command'].index('-p') + 1]
            crate_dir = CRATE[pkg]
            filters = h['command'][h['command'].index('--') + 1:]
            for t in h['expected_failure_tests']:
                leaf = t.split('::')[-1]
                where_now = test_location(args.repo, commit, crate_dir, leaf)
                where_H = test_location(args.repo, H, crate_dir, leaf)
                same = None
                if len(where_now) == 1 and where_now == where_H:
                    t_old, _ = fn_text(show(args.repo, H, where_now[0]), leaf)
                    t_new, span = fn_text(show(args.repo, commit, where_now[0]), leaf)
                    same = t_old is not None and t_old == t_new
                rec['expected_tests'].append({
                    'test': t, 'selected_by_command_filter': any(x in t for x in filters),
                    'defined_in_now': where_now, 'defined_in_at_H': where_H,
                    'test_fn_text_identical_H_to_now': same,
                })
            records.append(rec)
    print(json.dumps(records, indent=1, ensure_ascii=False))
    print('PAIRS %d' % len(records))


if __name__ == '__main__':
    main()
