#!/usr/bin/env python3
"""Generate the Candidate 11 current-tree 183-control manifest, version ct-1.

Mission P3-C11-DR01-D2A1-CTM01-CANDIDATE-BASELINE-01; contract basis AD-1-R4 section H.

Reads only git objects of one commit: the immutable historical manifest and every
source file its controls name. Applies no mutation to any file: the edits are
replayed in memory only, exactly as the historical runner's edited() does, to
count each anchor at its point of application. Writes only --out.

The output records no commit or tree identity of any commit (AD-1-R4 H.3 item 6).
It confers no acceptance: every current review status is "pending".
"""
import argparse
import hashlib
import json
import subprocess
import sys

HIST_PATH = 'docs/evidence/p3-candidate10/original-183/manifest.json'
HIST_SHA256 = '2c4adc84121c1efbbfe84aa4422444407c4d1eb4b556fbf3a0ab8e22d4782e14'
HIST_BLOB = '65fa69dcc0742e04658a3360c42e051e38ebbf5b'
HIST_VERSION = '1-r1'
HIST_NAME = 'adapted Candidate-10 canonical campaign'

R4_PATH = ('docs/evidence/p3-candidate11/ad1r4-consolidation-01/ad1r4/'
           'P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R4-DRAFT.md')
R4_SHA256 = '00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693'
R4_BLOB = '35d5074c2daa2ce9602fee3cd23dc0ccd6aef5b1'

# Definition fields: copied from the historical control, never changed, kept at
# the control's top level under their historical names (AD-1-R4 H.3 item 2).
DEFINITION_KEYS = ('id', 'category', 'historical_meaning', 'candidate10_meaning',
                   'historical', 'edits', 'command', 'cwd', 'expected_failure_tests')
# Historical review fields: copied unchanged, nested and marked historical
# (AD-1-R4 H.3 item 3). They are not current-tree approval.
HISTORICAL_REVIEW_KEYS = ('review_status', 'lineage', 'review', 'application_review',
                          'test_review', 'runner_note', 'r1_semantic_reconstruction',
                          'semantic_review_resolution', 'source_sha256')
EDIT_KEYS = ('file', 'anchor', 'replacement', 'count')

HISTORICAL_MARKER = ('HISTORICAL: copied unchanged from derived_from (version 1-r1). '
                     'Not current-tree approval.')
PENDING = 'pending'


def fail(message):
    sys.stderr.write('GEN-CT1 FAIL: ' + message + '\n')
    raise SystemExit(2)


def git(repo, *args):
    done = subprocess.run(['git', '-C', repo, *args], capture_output=True)
    if done.returncode != 0:
        fail('git %s: %s' % (' '.join(args), done.stderr.decode(errors='replace').strip()))
    return done.stdout


def tree_entry(repo, commit, path):
    out = git(repo, 'ls-tree', '-z', commit, '--', path).split(b'\0')
    out = [x for x in out if x]
    if len(out) != 1:
        fail('%s: not exactly one tree entry' % path)
    meta, name = out[0].split(b'\t', 1)
    mode, kind, oid = meta.decode().split(' ')
    if name.decode() != path or kind != 'blob' or mode not in ('100644', '100755'):
        fail('%s: not a regular file blob (mode %s, type %s)' % (path, mode, kind))
    return oid


def read_blob(repo, commit, path):
    oid = tree_entry(repo, commit, path)
    data = git(repo, 'cat-file', 'blob', oid)
    computed = hashlib.sha1(b'blob %d\0' % len(data) + data).hexdigest()
    if computed != oid:
        fail('%s: blob id %s does not match its bytes (%s)' % (path, oid, computed))
    return oid, data


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def occurrence_lines(data, anchor):
    lines = []
    start = 0
    while True:
        at = data.find(anchor, start)
        if at < 0:
            return lines
        lines.append(data.count(b'\n', 0, at) + 1)
        start = at + len(anchor)


def render(obj):
    return (json.dumps(obj, indent=2, ensure_ascii=False) + '\n').encode('utf-8')


def build(repo, commit):
    hist_oid, hist_raw = read_blob(repo, commit, HIST_PATH)
    if sha256(hist_raw) != HIST_SHA256 or hist_oid != HIST_BLOB:
        fail('historical manifest identity mismatch: sha256 %s blob %s' % (sha256(hist_raw), hist_oid))
    r4_oid, r4_raw = read_blob(repo, commit, R4_PATH)
    if sha256(r4_raw) != R4_SHA256 or r4_oid != R4_BLOB:
        fail('AD-1-R4 identity mismatch: sha256 %s blob %s' % (sha256(r4_raw), r4_oid))
    hist = json.loads(hist_raw)
    if render(hist) != hist_raw:
        fail('historical manifest is not in its canonical rendering')
    if hist.get('name') != HIST_NAME or hist.get('version') != HIST_VERSION:
        fail('historical manifest name/version mismatch')
    controls = hist['controls']
    if [c['id'] for c in controls] != ['M%02d' % i for i in range(1, 184)]:
        fail('historical control identities/order are not M01..M183')

    files = {}
    for c in controls:
        for e in c['edits']:
            if e['file'] not in files:
                files[e['file']] = read_blob(repo, commit, e['file'])

    out_controls = []
    differing = []
    anchor_edits = 0
    for c in controls:
        unknown = set(c) - set(DEFINITION_KEYS) - set(HISTORICAL_REVIEW_KEYS)
        if unknown:
            fail('%s: unclassified historical field(s) %s' % (c['id'], sorted(unknown)))
        missing = [k for k in DEFINITION_KEYS if k not in c]
        if missing or 'source_sha256' not in c:
            fail('%s: missing field(s) %s' % (c['id'], missing))
        for e in c['edits']:
            if tuple(e) != EDIT_KEYS:
                fail('%s: edit fields %s' % (c['id'], list(e)))
            if not isinstance(e['anchor'], str) or not e['anchor']:
                fail('%s: non-string or empty anchor' % c['id'])

        named = []
        for e in c['edits']:
            if e['file'] not in named:
                named.append(e['file'])
        if sorted(named) != sorted(c['source_sha256']):
            fail('%s: recorded source_sha256 files differ from edited files' % c['id'])

        before = {f: files[f][1] for f in named}
        after = dict(before)
        anchors = []
        for index, e in enumerate(c['edits'], 1):
            f = e['file']
            anchor = e['anchor'].encode('utf-8')
            replacement = e['replacement'].encode('utf-8')
            in_unedited = before[f].count(anchor)
            at_application = after[f].count(anchor)
            matches = in_unedited == e['count'] and at_application == e['count']
            if not matches:
                fail('%s edit %d on %s: anchor count %d / %d at application, manifest count %d'
                     % (c['id'], index, f, in_unedited, at_application, e['count']))
            anchors.append({
                'edit': index,
                'file': f,
                'expected_count': e['count'],
                'count_in_unedited_file': in_unedited,
                'count_at_application': at_application,
                'cardinality_matches': matches,
                'lines_in_unedited_file': occurrence_lines(before[f], anchor),
            })
            after[f] = after[f].replace(anchor, replacement, e['count'])
            anchor_edits += 1
        if not any(after[f] != before[f] for f in named):
            fail('%s: the replayed edits change nothing' % c['id'])

        file_rows = []
        current = {}
        status = 'EQUAL_TO_HISTORICAL_RECORD'
        for f in named:
            oid, data = files[f]
            digest = sha256(data)
            current[f] = digest
            equal = digest == c['source_sha256'][f]
            if not equal:
                status = 'DIFFERS_FROM_HISTORICAL_RECORD'
                differing.append('%s %s' % (c['id'], f))
            file_rows.append({
                'file': f,
                'sha256': digest,
                'git_blob': oid,
                'historical_recorded_sha256': c['source_sha256'][f],
                'sha256_equals_historical_record': equal,
            })

        entry = {k: c[k] for k in DEFINITION_KEYS}
        entry['historical_review'] = {
            'marker': HISTORICAL_MARKER,
            'fields': {k: c[k] for k in c if k in HISTORICAL_REVIEW_KEYS},
        }
        entry['source_sha256'] = current
        entry['current_tree'] = {
            'files': file_rows,
            'anchors': anchors,
            'anchor_cardinality_matches': all(a['cardinality_matches'] for a in anchors),
            'definition_equals_historical': True,
            'source_hash_status': status,
            'source_hash_review': ('ARCHITECT_REVIEW_PENDING'
                                   if status == 'DIFFERS_FROM_HISTORICAL_RECORD' else 'NONE_NEEDED_HASH_EQUAL'),
            'review_status': PENDING,
        }
        out_controls.append(entry)

    header = {k: v for k, v in hist.items() if k != 'controls'}
    manifest = {
        'name': 'Candidate 11 current-tree 183-control manifest',
        'version': 'ct-1',
        'status': ('CANDIDATE. Every current review status is "pending". '
                   'Nothing in this file confers acceptance.'),
        'authority': ('This file is evidence, not authority. A string is not authority. '
                      'Acceptance exists only as the Sole Architect\'s own recorded decision, '
                      'bound by the detached attestation of AD-1-R4 H.6.'),
        'derived_from': {
            'path': HIST_PATH,
            'sha256': HIST_SHA256,
            'git_blob': HIST_BLOB,
            'version': HIST_VERSION,
            'controls': len(controls),
        },
        'contract_basis': {
            'document': 'AD-1-R4',
            'path': R4_PATH,
            'sha256': R4_SHA256,
            'git_blob': R4_BLOB,
            'sections': ['H.2', 'H.3', 'H.4', 'H.5', 'H.6'],
        },
        'derivation': [
            'Controls are the historical manifest\'s 183, in its order.',
            'Definition fields are copied unchanged at each control\'s top level: '
            + ', '.join(DEFINITION_KEYS) + '.',
            'Historical review fields are copied unchanged under historical_review.fields and are '
            'marked historical. They are not current-tree approval. They include the historical '
            'review_status and the historically recorded source_sha256.',
            'source_sha256 is the sha256 of each file the control names, in the source tree this '
            'manifest is validated against.',
            'Anchor counts follow the historical runner\'s edited(): edits applied in order, '
            'in memory; each anchor counted (non-overlapping) in the text as edited so far '
            '(count_at_application) and in the unedited file (count_in_unedited_file). '
            'lines_in_unedited_file are 1-based start lines of each occurrence.',
            'current_tree.review_status starts "pending" for every control. Neither this generator '
            'nor any executor sets it otherwise.',
            'No self-reference: this file records no commit or tree identity of the commit that '
            'contains it, and its correctness requires none. AD-1-R1\'s final_tree is not used.',
        ],
        'historical_header': {
            'marker': HISTORICAL_MARKER,
            'fields': header,
        },
        'current_tree_summary': {
            'controls': len(out_controls),
            'source_files': len(files),
            'recorded_control_file_pairs': sum(len(c['source_sha256']) for c in controls),
            'pairs_equal_to_historical_record':
                sum(len(c['source_sha256']) for c in controls) - len(differing),
            'pairs_differing_from_historical_record': len(differing),
            'differing_pairs': differing,
            'differing_pairs_review': 'ARCHITECT_REVIEW_PENDING',
            'anchor_edits': anchor_edits,
            'anchor_cardinality_mismatches': 0,
            'controls_with_review_status_pending': len(out_controls),
        },
        'controls': out_controls,
    }
    return render(manifest)


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    parser.add_argument('--repo', required=True)
    parser.add_argument('--commit', required=True, help='commit whose tree supplies every input')
    parser.add_argument('--out', required=True)
    args = parser.parse_args()
    data = build(args.repo, args.commit)
    with open(args.out, 'wb') as handle:
        handle.write(data)
    print('GEN-CT1 OK bytes=%d sha256=%s' % (len(data), sha256(data)))


if __name__ == '__main__':
    main()
