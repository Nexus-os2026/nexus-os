#!/usr/bin/env python3
"""Independent all-183 alignment checker for the current-tree manifest ct-1.

Mission P3-C11-DR01-D2A1-CTM01-CANDIDATE-BASELINE-01; contract basis AD-1-R4 H.3-H.5.

Written separately from gen-ct1.py and imports nothing from it. It reads the
manifests from git objects of --commit, and every source file from a filesystem
extraction of that commit's tree (git archive), so the two tools take different
read paths. Anchors are counted with a regular expression rather than
bytes.count, and preservation is proven by rebuilding the whole historical
manifest from ct-1 and requiring byte equality with it.

Every check prints one line. Exit 0 only when every check passes (fail closed).
"""
import argparse
import hashlib
import io
import json
import os
import re
import subprocess
import sys
import tarfile

HIST_PATH = 'docs/evidence/p3-candidate10/original-183/manifest.json'
HIST_SHA256 = '2c4adc84121c1efbbfe84aa4422444407c4d1eb4b556fbf3a0ab8e22d4782e14'
HIST_BLOB = '65fa69dcc0742e04658a3360c42e051e38ebbf5b'
R4_PATH = ('docs/evidence/p3-candidate11/ad1r4-consolidation-01/ad1r4/'
           'P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R4-DRAFT.md')
R4_SHA256 = '00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693'
R4_BLOB = '35d5074c2daa2ce9602fee3cd23dc0ccd6aef5b1'
CT_PATH = 'docs/evidence/p3-candidate11/current-tree-183/manifest.json'

P3 = 'app/src-tauri/src/phase3_tests.rs'
GC = 'crates/nexus-governed-control/src/'
# The 22 previously identified stale pairs (mission section 7; AD-1-R4 H.4).
EXPECTED_STALE = {
    'M32': P3, 'M78': P3, 'M79': P3, 'M80': P3, 'M87': GC + 'lib.rs', 'M89': P3,
    'M112': P3, 'M113': P3, 'M141': P3, 'M143': P3, 'M153': P3, 'M177': P3,
    'M178': P3, 'M179': P3, 'M180': P3,
    'M18': GC + 'broker.rs', 'M19': GC + 'broker.rs', 'M39': GC + 'broker.rs',
    'M84': GC + 'authority/approval.rs', 'M05': GC + 'authority/mod.rs',
    'M45': GC + 'authority/run.rs', 'M129': GC + 'authority/run.rs',
}
DEF = ['id', 'category', 'historical_meaning', 'candidate10_meaning', 'historical',
       'edits', 'command', 'cwd', 'expected_failure_tests']
TOP = ['name', 'version', 'status', 'authority', 'derived_from', 'contract_basis',
       'derivation', 'historical_header', 'current_tree_summary', 'controls']
CT_KEYS = ['files', 'anchors', 'anchor_cardinality_matches', 'definition_equals_historical',
           'source_hash_status', 'source_hash_review', 'review_status']
MARKER = ('HISTORICAL: copied unchanged from derived_from (version 1-r1). '
          'Not current-tree approval.')
HEX40 = re.compile(r'(?<![0-9a-f])[0-9a-f]{40}(?![0-9a-f])')
SECRET = re.compile(r'(ghp_[A-Za-z0-9]{20,}|github_pat_|gho_[A-Za-z0-9]{20,}|AKIA[0-9A-Z]{16}'
                    r'|-----BEGIN [A-Z ]*PRIVATE KEY|xox[abpr]-|sk-[A-Za-z0-9]{20,}'
                    r'|glpat-|AIza[0-9A-Za-z_-]{30,})')
APPROVAL = re.compile(r'SAME PROPERTY|TEXT UPDATE|\bACCEPTED\b|\bAPPROVED\b')

results = []


def record(name, ok, detail=''):
    results.append(ok)
    print('%s %s%s' % ('PASS' if ok else 'FAIL', name, (' :: ' + detail) if detail else ''))
    sys.stdout.flush()


def git(repo, *args):
    return subprocess.run(['git', '-C', repo, *args], capture_output=True, check=True).stdout


def blob_id(data):
    return hashlib.sha1(b'blob ' + str(len(data)).encode() + b'\x00' + data).hexdigest()


def strict_pairs(pairs):
    keys = [k for k, _ in pairs]
    if len(keys) != len(set(keys)):
        raise ValueError('duplicate key in JSON object: %r' % keys)
    return dict(pairs)


def extract(repo, commit, directory):
    os.makedirs(directory, exist_ok=False)
    data = git(repo, 'archive', '--format=tar', commit)
    with tarfile.open(fileobj=io.BytesIO(data)) as tar:
        for member in tar.getmembers():
            if member.issym() or member.islnk():
                continue
            target = os.path.realpath(os.path.join(directory, member.name))
            if not target.startswith(os.path.realpath(directory) + os.sep):
                raise SystemExit('unsafe archive member ' + member.name)
        tar.extractall(directory, filter='data')


def read_tree_file(root, relative):
    path = os.path.join(root, relative)
    if os.path.isabs(relative) or '..' in relative.split('/'):
        return None
    if os.path.islink(path) or not os.path.isfile(path):
        return None
    if not os.path.realpath(path).startswith(os.path.realpath(root) + os.sep):
        return None
    with open(path, 'rb') as handle:
        return handle.read()


def regex_positions(data, anchor):
    return [m.start() for m in re.finditer(re.escape(anchor), data)]


def line_of(data, offset):
    return len(data[:offset].split(b'\n'))


def strip_historical(obj):
    """The parts of ct-1 written by the generator rather than copied from history."""
    new = {k: v for k, v in obj.items() if k not in ('historical_header', 'controls')}
    new['controls'] = [{k: v for k, v in c.items() if k not in DEF and k != 'historical_review'}
                       for c in obj['controls']]
    return new


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    parser.add_argument('--repo', required=True)
    parser.add_argument('--commit', required=True)
    parser.add_argument('--extract-dir', required=True, help='new empty scratch directory')
    parser.add_argument('--manifest-file', help='validate this file instead of the commit\'s copy')
    args = parser.parse_args()

    commit = git(args.repo, 'rev-parse', '--verify', args.commit + '^{commit}').decode().strip()
    tree = git(args.repo, 'rev-parse', commit + '^{tree}').decode().strip()
    print('checking commit %s tree %s' % (commit, tree))

    hist_raw = git(args.repo, 'cat-file', 'blob', commit + ':' + HIST_PATH)
    record('C01 historical manifest sha256 and blob unchanged',
           hashlib.sha256(hist_raw).hexdigest() == HIST_SHA256 and blob_id(hist_raw) == HIST_BLOB
           and git(args.repo, 'rev-parse', commit + ':' + HIST_PATH).decode().strip() == HIST_BLOB,
           hashlib.sha256(hist_raw).hexdigest())
    r4_raw = git(args.repo, 'cat-file', 'blob', commit + ':' + R4_PATH)
    record('C02 AD-1-R4 sha256 and blob unchanged',
           hashlib.sha256(r4_raw).hexdigest() == R4_SHA256 and blob_id(r4_raw) == R4_BLOB,
           hashlib.sha256(r4_raw).hexdigest())

    if args.manifest_file:
        with open(args.manifest_file, 'rb') as handle:
            ct_raw = handle.read()
    else:
        ct_raw = git(args.repo, 'cat-file', 'blob', commit + ':' + CT_PATH)
    print('ct-1 bytes %d sha256 %s blob %s' % (len(ct_raw), hashlib.sha256(ct_raw).hexdigest(),
                                                blob_id(ct_raw)))
    try:
        ct = json.loads(ct_raw.decode('utf-8'), object_pairs_hook=strict_pairs)
        parsed = True
    except ValueError as error:
        parsed = False
        record('C03 ct-1 parses as UTF-8 JSON without duplicate keys', False, str(error))
        return finish()
    record('C03 ct-1 parses as UTF-8 JSON without duplicate keys', parsed)
    canonical = (json.dumps(ct, indent=2, ensure_ascii=False) + '\n').encode('utf-8')
    record('C04 ct-1 is in canonical rendering (parse then render is byte-identical)', canonical == ct_raw)

    hist = json.loads(hist_raw.decode('utf-8'))
    record('C05 top-level keys and order', list(ct) == TOP, str(list(ct)))
    record('C06 name and version', ct['name'] == 'Candidate 11 current-tree 183-control manifest'
           and ct['version'] == 'ct-1')
    record('C07 derived_from binds the historical manifest',
           ct['derived_from'] == {'path': HIST_PATH, 'sha256': HIST_SHA256, 'git_blob': HIST_BLOB,
                                  'version': hist['version'], 'controls': 183})
    record('C08 contract_basis binds AD-1-R4',
           ct['contract_basis']['path'] == R4_PATH and ct['contract_basis']['sha256'] == R4_SHA256
           and ct['contract_basis']['git_blob'] == R4_BLOB)

    # Lossless preservation: rebuild the historical manifest from ct-1 alone.
    header = ct['historical_header']
    rebuilt = dict(header['fields'])
    rebuilt['controls'] = []
    for c in ct['controls']:
        original = {k: c[k] for k in DEF}
        original.update(c['historical_review']['fields'])
        rebuilt['controls'].append(original)
    rebuilt_raw = (json.dumps(rebuilt, indent=2, ensure_ascii=False) + '\n').encode('utf-8')
    record('C09 historical manifest rebuilt from ct-1 is byte-identical to the original '
           '(every id, category, meaning, edit, file, anchor, replacement, count, command, cwd, '
           'expected test, lineage and review field, in order)',
           rebuilt_raw == hist_raw and header['marker'] == MARKER,
           'rebuilt sha256 %s' % hashlib.sha256(rebuilt_raw).hexdigest())
    record('C10 183 controls, ids M01..M183 in historical order',
           [c['id'] for c in ct['controls']] == ['M%02d' % i for i in range(1, 184)]
           == [c['id'] for c in hist['controls']])

    extract(args.repo, commit, args.extract_dir)
    hist_by_id = {c['id']: c for c in hist['controls']}
    differing = []
    anchor_total = 0
    for c in ct['controls']:
        h = hist_by_id[c['id']]
        problems = []
        expected_keys = DEF + ['historical_review', 'source_sha256', 'current_tree']
        if list(c) != expected_keys:
            problems.append('control keys %s' % list(c))
        if c['historical_review']['marker'] != MARKER or list(c['historical_review']) != ['marker', 'fields']:
            problems.append('historical marker')
        for k in DEF:
            if c[k] != h[k]:
                problems.append('definition field %s changed' % k)
        t = c['current_tree']
        if list(t) != CT_KEYS:
            problems.append('current_tree keys %s' % list(t))
        if t['review_status'] != 'pending':
            problems.append('review_status %r' % t['review_status'])
        named = []
        for e in h['edits']:
            if e['file'] not in named:
                named.append(e['file'])
        if list(c['source_sha256']) != named or [r['file'] for r in t['files']] != named:
            problems.append('file list')
        data = {}
        stale_here = False
        for row in t['files']:
            f = row['file']
            content = read_tree_file(args.extract_dir, f)
            if content is None:
                problems.append('%s missing or not a regular file' % f)
                continue
            data[f] = content
            digest = hashlib.sha256(content).hexdigest()
            via_git = git(args.repo, 'rev-parse', commit + ':' + f).decode().strip()
            if not (digest == row['sha256'] == c['source_sha256'][f]):
                problems.append('%s sha256' % f)
            if not (blob_id(content) == row['git_blob'] == via_git):
                problems.append('%s git blob' % f)
            if row['historical_recorded_sha256'] != h['source_sha256'][f]:
                problems.append('%s historical recorded hash' % f)
            equal = digest == h['source_sha256'][f]
            if row['sha256_equals_historical_record'] != equal:
                problems.append('%s equality flag' % f)
            if not equal:
                stale_here = True
                differing.append((c['id'], f))
        want_status = 'DIFFERS_FROM_HISTORICAL_RECORD' if stale_here else 'EQUAL_TO_HISTORICAL_RECORD'
        want_review = 'ARCHITECT_REVIEW_PENDING' if stale_here else 'NONE_NEEDED_HASH_EQUAL'
        if t['source_hash_status'] != want_status or t['source_hash_review'] != want_review:
            problems.append('source hash status/review')
        if len(data) == len(named):
            working = dict(data)
            if len(t['anchors']) != len(h['edits']):
                problems.append('anchor row count')
            for index, (e, row) in enumerate(zip(h['edits'], t['anchors']), 1):
                anchor_total += 1
                anchor = e['anchor'].encode('utf-8')
                unedited = regex_positions(data[e['file']], anchor)
                applied = regex_positions(working[e['file']], anchor)
                lines = [line_of(data[e['file']], p) for p in unedited]
                want = {'edit': index, 'file': e['file'], 'expected_count': e['count'],
                        'count_in_unedited_file': len(unedited),
                        'count_at_application': len(applied),
                        'cardinality_matches': len(unedited) == e['count'] == len(applied),
                        'lines_in_unedited_file': lines}
                if row != want:
                    problems.append('anchor %d row %s != %s' % (index, row, want))
                if not (len(unedited) == e['count'] == len(applied)):
                    problems.append('anchor %d cardinality %d/%d vs %d'
                                    % (index, len(unedited), len(applied), e['count']))
                # Apply in order, leftmost first, at most count times (as the runner's edited()).
                text = working[e['file']]
                pieces = []
                cursor = 0
                for p in applied[:e['count']]:
                    pieces.append(text[cursor:p])
                    pieces.append(e['replacement'].encode('utf-8'))
                    cursor = p + len(anchor)
                pieces.append(text[cursor:])
                working[e['file']] = b''.join(pieces)
            if all(working[f] == data[f] for f in named):
                problems.append('replayed edits change nothing')
            if t['anchor_cardinality_matches'] is not True or t['definition_equals_historical'] is not True:
                problems.append('summary flags')
        line = '%s files=%d anchors=%d hash=%s' % (
            c['id'], len(named), len(h['edits']),
            'DIFFERS' if stale_here else 'EQUAL')
        record('A ' + line, not problems, '; '.join(problems))

    expected = sorted(EXPECTED_STALE.items())
    record('C11 the differing pairs are exactly the 22 previously identified stale pairs',
           sorted(differing) == expected, '%d found' % len(differing))
    summary = ct['current_tree_summary']
    record('C12 summary agrees with the controls',
           summary['controls'] == 183 and summary['recorded_control_file_pairs'] == 184
           and summary['pairs_differing_from_historical_record'] == 22
           and summary['pairs_equal_to_historical_record'] == 162
           and summary['differing_pairs'] == ['%s %s' % p for p in differing]
           and summary['differing_pairs_review'] == 'ARCHITECT_REVIEW_PENDING'
           and summary['anchor_edits'] == anchor_total == 191
           and summary['anchor_cardinality_mismatches'] == 0
           and summary['controls_with_review_status_pending'] == 183
           and summary['source_files'] == len({f for c in hist['controls'] for f in c['source_sha256']}))

    # No self-reference: no commit/tree identity of the validated commit, no final_tree key,
    # and every 40-hex token outside the copied historical text is a content address.
    generated = json.dumps(strip_historical(ct), ensure_ascii=False)
    all_text = ct_raw.decode('utf-8')
    record('C13 no identity of the validated commit or its tree anywhere in ct-1',
           commit not in all_text and tree not in all_text
           and commit[:12] not in all_text and tree[:12] not in all_text)
    keys = set()

    def walk(node):
        if isinstance(node, dict):
            for k, v in node.items():
                keys.add(k)
                walk(v)
        elif isinstance(node, list):
            for v in node:
                walk(v)
    walk(strip_historical(ct))
    record('C14 no final_tree or commit/tree field in any generated (non-historical) field',
           not ({'final_tree', 'commit', 'tree', 'final_commit', 'containing_commit'} & keys),
           ','.join(sorted({'final_tree', 'commit', 'tree'} & keys)))
    allowed = {HIST_BLOB, R4_BLOB}
    for c in ct['controls']:
        for row in c['current_tree']['files']:
            allowed.add(row['git_blob'])
    tokens = set(HEX40.findall(generated))
    record('C15 every 40-hex token in generated fields is a content (blob) address',
           tokens <= allowed, ','.join(sorted(tokens - allowed)))
    approvals = APPROVAL.findall(generated)
    record('C16 no approval wording in generated fields (SAME PROPERTY, TEXT UPDATE, ACCEPTED, APPROVED)',
           not approvals, ','.join(approvals))
    statuses = {c['current_tree']['review_status'] for c in ct['controls']}
    record('C17 every current review status is pending', statuses == {'pending'}, str(statuses))
    record('C18 no credential-like material in ct-1', not SECRET.search(all_text))
    return finish()


def finish():
    failed = results.count(False)
    print('SUMMARY checks=%d passed=%d failed=%d' % (len(results), results.count(True), failed))
    return 0 if failed == 0 and results else 1


if __name__ == '__main__':
    sys.exit(main())
