#!/usr/bin/env python3
"""Integrity and source binding of the sealed proof documents (mission sections 4 and 10).

Mission P3-C11-DR01-D2A1-FINAL-SOURCE-PROOF-READINESS-01. Read only: it reads the sealed
evidence root (outside the repository) and git objects; it writes one JSON document.
Nothing is regenerated, rewritten or copied.

For each sealed origin directory: every SHA256SUMS line is re-hashed; files present
but not listed, and listed but missing, are reported; the directory's own mode is
recorded. For each proof document: path, sha256, size, git blob id; its line in the
origin's SHA256SUMS; the byte-identical transported copy at evidence commit 8693e3a7
(and of the origin's SHA256SUMS); the source identities its header claims, checked
against git; and the inventory of its theorem/lemma/item labels.
It also authenticates AD-1-R4, the Owner-adoption record and the ct-1 manifest at C.
"""
import argparse
import hashlib
import json
import os
import re
import stat
import sys

import common as K

ROOT = '/home/nexus/nexus-audits/claude-architect'
TRANSPORT = '8693e3a742ee4d3917cbfdb809b377bcd919d9f4'
TRANSPORT_REF = 'refs/heads/review/p3-c11-d2a1-ad1r2-audit-packet-01'
TRANSPORT_DIR = 'docs/evidence/p3-candidate11/ad1r2-review-packet-01/evidence/'
CONTRACT_SHA256 = 'cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c'
AD1R4 = 'docs/evidence/p3-candidate11/ad1r4-consolidation-01/ad1r4/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R4-DRAFT.md'
ADOPT = 'docs/evidence/p3-candidate11/ad1r4-adoption/OWNER-ADOPTION-2026-10-14.md'
CT1 = 'docs/evidence/p3-candidate11/current-tree-183/manifest.json'

ORIGINS = {
    'ADM-QUOTIENT-01': 'p3-c11-dr01-d2a1-adm-quotient-01',
    'GC-SYMMETRY-01': 'p3-c11-dr01-d2a1-gc-symmetry-01',
    'CLOSURE-PREP-01': 'p3-c11-dr01-d2a1-closure-prep-01',
    'GC-SYM-F3-01': 'p3-c11-dr01-d2a1-gc-sym-f3-01',
    'PT-FB-EXPERIMENT-01': 'p3-c11-dr01-d2a1-pt-fb-experiment-01',
    'F3-QUOTIENT-01': 'p3-c11-dr01-d2a1-f3-quotient-01',
    'AD1R1-MUT-READINESS-01': 'p3-c11-dr01-d2a1-ad1r1-mut-readiness-01',
}


def labels(*names):
    return list(names)


DOCS = [
    {'key': 'ADM-PROOF', 'origin': 'ADM-QUOTIENT-01', 'path': 'receipts/stage2/01-formal-argument.txt',
     'role': 'proof: admission observer quotient (Theorem 1, Corollary 1, L-A, L-B, Q1-Q15, AQ1-AQ6, facts F1-F10)',
     'claims': {'source_commit': K.SOURCE_R2, 'tree_prefix': '1e0c76d8',
                'blobs': {K.MODEL: 'beb48fe7', K.PROTO: 'a2df2841', K.UNIT: '55dc6640'},
                'sha256_ends': {K.MODEL: ('ede5ef7b', '1b64'), K.PROTO: ('a5454944', 'a296')}},
     'implementation_commit': K.ADM,
     'labels': (['Theorem 1', 'Corollary 1', 'L-A', 'L-B'] + ['Q%d' % i for i in range(1, 16)]
                + ['AQ%d' % i for i in range(1, 7)] + ['F%d' % i for i in range(1, 11)])},
    {'key': 'GC-PROOF', 'origin': 'GC-SYMMETRY-01', 'path': 'receipts/stage2/00-FORMAL-SYMMETRY-ANALYSIS.txt',
     'role': 'proof: gating-caller symmetry (Lemma E, S1-S15, ASYM-1..6, AS1-AS4, identity uses I1-I13)',
     'claims': {'source_commit': K.ADM, 'tree_full': '74a6a0e5add063dac4d24939b3a62c619847dd3a',
                'line_counts': {K.PROTO: 2083, K.MODEL: 6034}},
     'implementation_commit': K.GC,
     'labels': (['Lemma E'] + ['S%d' % i for i in range(1, 16)] + ['ASYM-%d' % i for i in range(1, 7)]
                + ['AS%d' % i for i in range(1, 5)] + ['I%d' % i for i in range(1, 14)])},
    {'key': 'EXT-PROOF', 'origin': 'CLOSURE-PREP-01', 'path': 'receipts/stage2/00-FPT-FB-SYMMETRY-PROOF.txt',
     'role': 'proof: F-PT/F-B extension (E-PT, E-B, E-R; F-PT and F-B matrices, items 1-17 each; C-1..C-6)',
     'claims': {'source_commit': K.GC, 'tree_full': '21818166ee10a8075ca364bd09f31c7aa64555dd',
                'blobs': {K.PROTO: 'a2df2841', K.MODEL: 'a80dd05c'}, 'line_counts': {K.MODEL: 7758}},
     'implementation_commit': K.EXT,
     'labels': ['E-PT', 'E-B', 'E-R'] + ['C-%d' % i for i in range(1, 7)]},
    {'key': 'GC-GREP', 'origin': 'GC-SYMMETRY-01', 'path': 'receipts/stage2/01-identity-grep.txt',
     'role': 'supporting: the exhaustive identity grep behind I1-I13', 'labels': []},
    {'key': 'EXT-GREP', 'origin': 'CLOSURE-PREP-01', 'path': 'receipts/stage2/01-grep.txt',
     'role': 'supporting: the grep behind E-PT/E-B', 'labels': []},
    {'key': 'ADM-REPORT', 'origin': 'ADM-QUOTIENT-01', 'path': 'REPORT.txt', 'role': 'report', 'labels': []},
    {'key': 'GC-REPORT', 'origin': 'GC-SYMMETRY-01', 'path': 'REPORT.txt', 'role': 'report', 'labels': []},
    {'key': 'EXT-REPORT', 'origin': 'CLOSURE-PREP-01', 'path': 'REPORT.txt', 'role': 'report', 'labels': []},
    {'key': 'GCSYMF3-REPORT', 'origin': 'GC-SYM-F3-01', 'path': 'REPORT.txt',
     'role': 'historical experimental execution (F-3 composed quotient); informational, not final evidence', 'labels': []},
    {'key': 'PTFB-REPORT', 'origin': 'PT-FB-EXPERIMENT-01', 'path': 'REPORT.txt',
     'role': 'historical experimental execution (F-PT, F-B composed quotient); informational, not final evidence',
     'labels': []},
    {'key': 'F3Q-REPORT', 'origin': 'F3-QUOTIENT-01', 'path': 'REPORT.txt',
     'role': 'historical experimental execution (F-3 admission quotient alone, INCOMPLETE); informational',
     'labels': []},
    {'key': 'GATE36', 'origin': 'AD1R1-MUT-READINESS-01', 'path': 'receipts/stage5/36-negatives-and-fail-closed.txt',
     'role': 'historical receipt naming the 24 negatives and fail-closed tests of AD-1-R4 D (N.8)', 'labels': []},
]
GATE36_NAMES = [
    'gc_symmetry_negative_p1_on_sym_pt', 'gc_symmetry_negative_p12_on_sym_b', 'gc_symmetry_negative_p1_on_sym_bi',
    'gc_symmetry_negative_e2_admission_views_twice', 'gc_symmetry_negative_e9_admission_ignores_the_latch',
    'gc_symmetry_negative_p1_admission_ignores_unapplied', 'gc_symmetry_allow_list_is_exact',
    'gc_symmetry_refuses_a_batch_under_f3s_name', 'gc_symmetry_refuses_an_asymmetric_step_on_sym_pt',
    'gc_symmetry_refuses_an_asymmetric_step_on_sym_b', 'gc_symmetry_refuses_an_asymmetric_step_on_sym_bi',
    'gc_symmetry_refuses_f_pt_with_a_lower_fault_budget', 'gc_symmetry_refuses_f_pt_with_a_raised_cap',
    'gc_symmetry_refuses_f_b_with_another_actor_change', 'gc_symmetry_refuses_f_b_with_a_raised_time_cap',
    'gc_symmetry_refuses_acknowledgements', 'gc_symmetry_refuses_a_configuration_not_shown_symmetric',
    'gc_symmetry_refuses_an_acknowledgement_row', 'gc_symmetry_refuses_an_asymmetric_step',
    'gc_symmetry_refuses_another_shape_under_a_known_name', 'gc_symmetry_refuses_a_raised_cap',
    'gc_symmetry_refuses_f_g', 'gc_symmetry_refuses_kept_bits', 'gc_symmetry_refuses_other_callers']
FAILURES = []


def expect(cond, what):
    if not cond:
        FAILURES.append(what)
    return bool(cond)


def sha_file(p):
    with open(p, 'rb') as f:
        return hashlib.sha256(f.read()).hexdigest()


def seal(origin_dir):
    base = os.path.join(ROOT, origin_dir)
    sums = os.path.join(base, 'SHA256SUMS')
    listed = {}
    for line in open(sums, 'rb').read().decode().splitlines():
        if not line.strip():
            continue
        h, p = line.split(None, 1)
        listed[p.strip().lstrip('*')] = h
    present = set()
    for dp, _, fns in os.walk(base):
        for fn in fns:
            rel = os.path.relpath(os.path.join(dp, fn), base)
            present.add(rel)
    bad = sorted(p for p, h in listed.items() if p not in present or sha_file(os.path.join(base, p)) != h)
    unlisted = sorted(p for p in present if p not in listed and p != 'SHA256SUMS')
    return {
        'dir': base, 'mode': stat.filemode(os.stat(base).st_mode), 'sha256sums_sha256': sha_file(sums),
        'listed': len(listed), 'present_files': len(present), 'mismatched_or_missing': bad,
        'present_but_unlisted': unlisted, 'listed_map': listed}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('repo')
    ap.add_argument('--out')
    args = ap.parse_args()
    repo = args.repo
    doc = {'tool': 'proof_docs.py', 'mission': 'P3-C11-DR01-D2A1-FINAL-SOURCE-PROOF-READINESS-01',
           'evidence_root': ROOT, 'transport_commit': TRANSPORT}
    t_info = K.commit_info(repo, TRANSPORT)
    doc['transport_commit_info'] = t_info
    expect(t_info['parents'] == [K.W1], 'transport commit 8693e3a7 has the single parent W1 (42dd1bd1)')
    seals = {}
    for name, d in ORIGINS.items():
        s = seal(d)
        seals[name] = {k: v for k, v in s.items() if k != 'listed_map'}
        expect(not s['mismatched_or_missing'], '%s: every SHA256SUMS line verifies' % name)
        tp = TRANSPORT_DIR + d + '/SHA256SUMS'
        try:
            t_bytes = K.blob(repo, TRANSPORT, tp)
            seals[name]['transported_sha256sums'] = {'path': '%s:%s' % (TRANSPORT[:8], tp),
                                                     'identical': K.sha256(t_bytes) == s['sha256sums_sha256']}
            expect(seals[name]['transported_sha256sums']['identical'], '%s: transported SHA256SUMS identical' % name)
        except SystemExit:
            seals[name]['transported_sha256sums'] = None
        seals[name]['_listed'] = s['listed_map']
        # the contract every origin carries
        c = os.path.join(ROOT, d, 'contract', 'P3-C11-D2A1-CONTRACT-02-R1.md')
        if os.path.exists(c):
            seals[name]['contract_sha256'] = sha_file(c)
            expect(sha_file(c) == CONTRACT_SHA256, '%s: contract sha256 cac65351...' % name)
    docs = []
    for spec in DOCS:
        d = ORIGINS[spec['origin']]
        p = os.path.join(ROOT, d, spec['path'])
        data = open(p, 'rb').read()
        ent = {'key': spec['key'], 'origin': spec['origin'], 'role': spec['role'], 'sealed_path': p,
               'sha256': K.sha256(data), 'bytes': len(data), 'git_blob': K.git_blob_hash(data),
               'mode': stat.filemode(os.stat(p).st_mode)}
        listed = seals[spec['origin']]['_listed'].get(spec['path'])
        ent['listed_in_origin_sha256sums'] = listed == ent['sha256']
        expect(ent['listed_in_origin_sha256sums'], '%s is listed in its origin SHA256SUMS with its sha256' % spec['key'])
        tp = TRANSPORT_DIR + d + '/' + spec['path']
        try:
            tb = K.blob(repo, TRANSPORT, tp)
            ent['transported'] = {'path': '%s:%s' % (TRANSPORT, tp), 'blob': K.blob_id(repo, TRANSPORT, tp),
                                  'identical': tb == data}
            expect(tb == data, '%s transported copy is byte-identical' % spec['key'])
        except SystemExit:
            ent['transported'] = None
            if spec['key'] in ('ADM-PROOF', 'GC-PROOF', 'EXT-PROOF'):
                expect(False, '%s has a transported copy' % spec['key'])
        text = data.decode()
        if spec.get('labels'):
            inv = {}
            for lab in spec['labels']:
                pat = re.compile(r'(^|\s|\(|/)' + re.escape(lab) + r'(?![0-9A-Za-z])', re.M)
                inv[lab] = len(pat.findall(text))
            ent['label_occurrences'] = inv
            missing = [k for k, v in inv.items() if v == 0]
            ent['labels_missing'] = missing
            expect(not missing, '%s defines every expected label' % spec['key'])
        claims = spec.get('claims')
        if claims:
            chk = {}
            src = claims['source_commit']
            chk['claimed_source_commit'] = src
            chk['source_commit_in_text'] = src[:8] in text
            info = K.commit_info(repo, src)
            if 'tree_prefix' in claims:
                chk['tree'] = info['tree']
                chk['tree_matches_claim'] = info['tree'].startswith(claims['tree_prefix']) and claims['tree_prefix'] in text
            if 'tree_full' in claims:
                chk['tree'] = info['tree']
                chk['tree_matches_claim'] = info['tree'] == claims['tree_full'] and claims['tree_full'] in text
            for path, pref in claims.get('blobs', {}).items():
                bid = K.blob_id(repo, src, path)
                chk['blob ' + path] = {'claimed': pref, 'actual': bid, 'match': bid.startswith(pref) and pref in text}
            for path, (a, z) in claims.get('sha256_ends', {}).items():
                h = K.sha256(K.blob(repo, src, path))
                chk['sha256 ' + path] = {'claimed': '%s...%s' % (a, z), 'actual': h,
                                         'match': h.startswith(a) and h.endswith(z)}
            for path, n in claims.get('line_counts', {}).items():
                lines = K.blob(repo, src, path).count(b'\n')
                chk['lines ' + path] = {'claimed': n, 'actual': lines, 'match': lines == n}
            chk['implementation_commit'] = spec['implementation_commit']
            impl = K.commit_info(repo, spec['implementation_commit'])
            chk['implementation_parent_is_source'] = impl['parents'] == [src]
            ok = (chk['source_commit_in_text'] and chk.get('tree_matches_claim', True)
                  and all(v['match'] for k, v in chk.items() if isinstance(v, dict))
                  and chk['implementation_parent_is_source'])
            chk['all_match'] = ok
            expect(ok, '%s source binding matches git' % spec['key'])
            ent['source_binding'] = chk
        docs.append(ent)
    # the ADM origin's source copies equal the git blobs at SOURCE-R2
    copies = {}
    for name, path in (('model_tests.rs', K.MODEL), ('protocol.rs', K.PROTO), ('unit_tests.rs', K.UNIT)):
        local = open(os.path.join(ROOT, ORIGINS['ADM-QUOTIENT-01'], 'source', name), 'rb').read()
        copies[name] = {'sha256': K.sha256(local), 'equals_git_at_SOURCE-R2': local == K.blob(repo, K.SOURCE_R2, path)}
        expect(copies[name]['equals_git_at_SOURCE-R2'], 'ADM source copy %s equals git at SOURCE-R2' % name)
    doc['adm_source_copies'] = copies
    # gate 36 names (AD-1-R4 D, N.8)
    g36 = open(os.path.join(ROOT, ORIGINS['AD1R1-MUT-READINESS-01'], 'receipts/stage5/36-negatives-and-fail-closed.txt')).read()
    cmd = [l for l in g36.splitlines() if l.startswith('command:')][0]
    names = [w.split('::')[-1] for w in cmd.split() if w.startswith('ledger::protocol::model_tests::')]
    doc['gate36_names'] = names
    expect(names == GATE36_NAMES, 'gate 36 names the 24 expected tests')
    for s in seals.values():
        s.pop('_listed', None)
    doc['seals'] = seals
    doc['documents'] = docs
    # AD-1-R4, adoption record and ct-1 at C
    at_c = {}
    for label, path, blob_id, sha in (
            ('AD-1-R4', AD1R4, '35d5074c2daa2ce9602fee3cd23dc0ccd6aef5b1',
             '00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693'),
            ('OWNER-ADOPTION', ADOPT, '54c956bc1324dd3a8fd97439faa815f0865b404d',
             '105e087137fed4ed7bf2baaab50b06812a1847ddbe9549809b85eecbdae379c6'),
            ('ct-1 manifest', CT1, '5d31cb45123403626f06c521dd2d53103676250e',
             '20bcc6fe221238f81246348cbbe94636a3c9b6d8acf89901d230e091f1a44279')):
        b = K.blob(repo, K.C, path)
        at_c[label] = {'path': path, 'blob': K.blob_id(repo, K.C, path), 'sha256': K.sha256(b),
                       'match': K.blob_id(repo, K.C, path) == blob_id and K.sha256(b) == sha}
        expect(at_c[label]['match'], '%s at C has blob %s and sha256 %s' % (label, blob_id[:8], sha[:8]))
    ct1 = json.loads(K.blob(repo, K.C, CT1))
    at_c['ct-1 manifest']['version'] = ct1.get('version')
    expect(ct1.get('version') == 'ct-1', 'ct-1 manifest version is ct-1')
    doc['at_C'] = at_c
    doc['expectation_failures'] = FAILURES
    doc['result'] = 'PASS' if not FAILURES else 'FAIL'
    text = json.dumps(doc, indent=1, sort_keys=True)
    if args.out:
        open(args.out, 'w').write(text + '\n')
    else:
        print(text)
    print('proof_docs: %s (%d expectation failures)' % (doc['result'], len(FAILURES)), file=sys.stderr)
    for f in FAILURES:
        print('  FAILED: ' + f, file=sys.stderr)
    sys.exit(0 if not FAILURES else 1)


if __name__ == '__main__':
    main()
