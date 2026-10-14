#!/usr/bin/env python3
"""Negative controls for check-ct1.py: each injects one defect into a copy of a
ct-1 manifest and requires the checker to exit non-zero and name the expected
failing check. Writes only inside --work. The manifest under test is not changed.
"""
import argparse
import json
import os
import shutil
import subprocess
import sys


def render(obj):
    return (json.dumps(obj, indent=2, ensure_ascii=False) + '\n').encode('utf-8')


def ctl(m, cid):
    return next(c for c in m['controls'] if c['id'] == cid)


def build_cases(base_commit):
    def anchor(m): ctl(m, 'M32')['edits'][0]['anchor'] += 'x'
    def count(m): ctl(m, 'M01')['edits'][1]['count'] = 2
    def replacement(m): ctl(m, 'M18')['edits'][0]['replacement'] = 'if true {'
    def command(m): ctl(m, 'M05')['command'][-1] = 'governed::tests::other'
    def expected(m): ctl(m, 'M84')['expected_failure_tests'].append('extra::test')
    def cwd(m): ctl(m, 'M100')['cwd'] = 'app'
    def hist_status(m): ctl(m, 'M45')['historical_review']['fields']['review_status'] = 'RE-ANCHOR NEEDED'
    def hist_hash(m):
        f = ctl(m, 'M87')['historical_review']['fields']['source_sha256']
        f[next(iter(f))] = '0' * 64
    def current_status(m): ctl(m, 'M19')['current_tree']['review_status'] = 'accepted'
    def final_tree(m): ctl(m, 'M02')['current_tree'] = dict(ctl(m, 'M02')['current_tree'], final_tree='x')
    def self_ref(m): m['status'] += ' ' + base_commit
    def order(m): m['controls'][3], m['controls'][4] = m['controls'][4], m['controls'][3]
    def source_hash(m):
        c = ctl(m, 'M150')
        f = next(iter(c['source_sha256']))
        c['source_sha256'][f] = 'f' * 64
    def blob(m): ctl(m, 'M151')['current_tree']['files'][0]['git_blob'] = '0' * 40
    def line(m): ctl(m, 'M152')['current_tree']['anchors'][0]['lines_in_unedited_file'][0] += 1
    def stale_flag(m):
        c = ctl(m, 'M141')
        c['current_tree']['files'][0]['sha256_equals_historical_record'] = True
    def approval(m): ctl(m, 'M129')['current_tree']['source_hash_review'] = 'TEXT UPDATE, SAME PROPERTY'
    def drop(m): del m['controls'][182]
    def secret(m): m['authority'] += ' ghp_' + 'A' * 36
    cases = [
        ('N01 changed anchor', anchor, 'C09'),
        ('N02 changed count', count, 'C09'),
        ('N03 changed replacement', replacement, 'C09'),
        ('N04 changed command', command, 'C09'),
        ('N05 added expected test', expected, 'C09'),
        ('N06 changed cwd', cwd, 'C09'),
        ('N07 changed historical review status', hist_status, 'C09'),
        ('N08 changed historical recorded hash', hist_hash, 'C09'),
        ('N09 current review status not pending', current_status, 'C17'),
        ('N10 final_tree field', final_tree, 'C14'),
        ('N11 self-reference to the validated commit', self_ref, 'C13'),
        ('N12 control order', order, 'C10'),
        ('N13 wrong current source hash', source_hash, 'A M150'),
        ('N14 wrong git blob', blob, 'A M151'),
        ('N15 wrong anchor line', line, 'A M152'),
        ('N16 stale pair marked equal', stale_flag, 'A M141'),
        ('N17 approval wording', approval, 'C16'),
        ('N18 control removed', drop, 'C10'),
        ('N19 credential-like token', secret, 'C18'),
    ]
    return cases


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--repo', required=True)
    parser.add_argument('--commit', required=True)
    parser.add_argument('--manifest-file', required=True)
    parser.add_argument('--checker', required=True)
    parser.add_argument('--work', required=True)
    args = parser.parse_args()
    commit = subprocess.run(['git', '-C', args.repo, 'rev-parse', args.commit + '^{commit}'],
                            capture_output=True, check=True, text=True).stdout.strip()
    os.makedirs(args.work, exist_ok=False)
    base = json.loads(open(args.manifest_file, 'rb').read())
    cases = build_cases(commit)
    variants = [(name, mutate, expect, None) for name, mutate, expect in cases]
    # Rendering-level defects, applied to bytes rather than to the parsed object.
    raw = open(args.manifest_file, 'rb').read()
    variants.append(('N20 non-canonical rendering', None, 'C04',
                     json.dumps(base, indent=1, ensure_ascii=False).encode() + b'\n'))
    variants.append(('N21 duplicate key', None, 'C03',
                     raw.replace(b'{\n  "name":', b'{\n  "name": "dup",\n  "name":', 1)))
    detected = 0
    for index, (name, mutate, expect, data) in enumerate(variants, 1):
        if data is None:
            m = json.loads(json.dumps(base))
            mutate(m)
            data = render(m)
        path = os.path.join(args.work, 'variant-%02d.json' % index)
        with open(path, 'wb') as handle:
            handle.write(data)
        extract = os.path.join(args.work, 'extract-%02d' % index)
        done = subprocess.run([sys.executable, args.checker, '--repo', args.repo, '--commit', commit,
                               '--extract-dir', extract, '--manifest-file', path],
                              capture_output=True, text=True)
        shutil.rmtree(extract, ignore_errors=True)
        fails = [l for l in done.stdout.splitlines() if l.startswith('FAIL ')]
        named = any(l.startswith('FAIL ' + expect) for l in fails)
        ok = done.returncode != 0 and named
        detected += ok
        print('%s %s :: exit=%d expected=%s failing=[%s]' % (
            'DETECTED' if ok else 'MISSED', name, done.returncode, expect,
            '; '.join(l.split(' :: ')[0][5:60] for l in fails)))
    print('NEGATIVE-CONTROLS %d/%d detected' % (detected, len(variants)))
    return 0 if detected == len(variants) else 1


if __name__ == '__main__':
    sys.exit(main())
