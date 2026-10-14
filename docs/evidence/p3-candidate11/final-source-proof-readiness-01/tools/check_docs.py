#!/usr/bin/env python3
"""Checks the authored review documents against the gate's own receipts.

Mission P3-C11-DR01-D2A1-FINAL-SOURCE-PROOF-READINESS-01. Read only. For each document
it requires the identities and findings the receipts compute (commits, trees, blobs,
sha256s, byte ranges, verdicts, counts) to appear in it, and it requires that no
document uses a forbidden acceptance phrase. Usage:
  check_docs.py <packet-dir> <receipts-dir>
Exit 0 only if every required string is present and no forbidden phrase is.
"""
import json
import os
import sys

FORBIDDEN = ['proof accepted', 'proofs accepted', 'theorem proved', 'final baseline frozen', 'd2a-1 accepted',
             'phase three complete', 'baseline is frozen']


def main():
    pkt, rc = sys.argv[1:3]
    load = lambda n: json.load(open(os.path.join(rc, n)))
    sc, wc, bi, ci, pd, mx = (load('10-source-closure.json'), load('11-witness-check.json'),
                              load('12-build-inputs.json'), load('13-config-identity.json'),
                              load('14-proof-docs.json'), load('15-matrix.json'))
    need = {}

    def req(doc, *strings):
        need.setdefault(doc, []).extend(str(s) for s in strings)

    # SOURCE-CLOSURE.md
    for c in sc['lineage']:
        req('SOURCE-CLOSURE.md', c['commit'], c['tree'])
    for p, v in sc['files']['C'].items():
        req('SOURCE-CLOSURE.md', v['blob'], v['sha256'])
    for k, b in sc['model_blobs'].items():
        req('SOURCE-CLOSURE.md', b)
    for name, (lo, hi) in sc['regions_at_C'].items():
        req('SOURCE-CLOSURE.md', '%d' % lo)
    for k, v in sc['item_mapping'].items():
        req('SOURCE-CLOSURE.md', '| %s | %d |' % (k, v['items']))
    for r in sc['sections']:
        if r['section'] in ('C', 'D'):
            req('SOURCE-CLOSURE.md', r['c_decl_sha256'][:16], '`%s`' % r['item'])
            if r['baseline_decl_sha256'] != r['c_decl_sha256']:
                req('SOURCE-CLOSURE.md', r['baseline_decl_sha256'][:16])
    req('SOURCE-CLOSURE.md', sc['protocol_hook']['decl_sha256'], '%d named items' % sc['B_named'],
        '%d items' % len(sc['B_closure']))
    for e in sc['layer_isolation']:
        req('SOURCE-CLOSURE.md', '[%d, %d)' % tuple(e['bytes_at_C']))
    # WITNESS-NONINTERFERENCE.md
    for a in wc['additions']:
        req('WITNESS-NONINTERFERENCE.md', a['witness_commit'], a['witness_model_blob'],
            [c for c in a['checks'] if c['check'].startswith('N01')][0]['evidence']['appended_sha256'],
            '[%d, %d)' % tuple(a['region_at_C_bytes']))
        for d in [c for c in a['checks'] if c['check'].startswith('N04')][0]['evidence']['defined_top_level']:
            req('WITNESS-NONINTERFERENCE.md', d.split(' ', 1)[1])
        if a['verdict'] != 'NON_INTERFERING':
            req('WITNESS-NONINTERFERENCE.md', 'PROOF_REVIEW_BLOCKER')
    scanned = [c for c in wc['additions'][0]['checks'] if c['check'].startswith('N08')][0]['evidence']['scanned_identifiers']
    req('WITNESS-NONINTERFERENCE.md', '%d identifiers scanned' % len(scanned))
    verdicts = wc['verdicts']
    req('WITNESS-NONINTERFERENCE.md', '| **Verdict** | **%s** | **%s** |' % (verdicts['C1/C25'], verdicts['C4']))
    # BUILD-INPUT-CLOSURE.md
    for r in bi['inputs']:
        req('BUILD-INPUT-CLOSURE.md', r['identities']['C']['blob'], r['identities']['C']['sha256'], r['classification'])
    req('BUILD-INPUT-CLOSURE.md', bi['toolchain']['toolchain']['channel'],
        *[v['sha256'] for v in bi['build_script_reads'].values()], *bi['path_dependency_closure'])
    if bi['changed_inputs']:
        req('BUILD-INPUT-CLOSURE.md', *bi['changed_inputs'])
    # CONFIGURATION-IDENTITY.md
    for k, v in ci['bound_items'].items():
        req('CONFIGURATION-IDENTITY.md', v['c_decl_sha256'])
    req('CONFIGURATION-IDENTITY.md', '[%s]' % ', '.join(ci['symmetric']['members']),
        ', '.join(ci['symmetric']['names']))
    for r in ci['admission_table']:
        req('CONFIGURATION-IDENTITY.md', '| %s | %s |' % (r['const'], r['name'] or '(empty)'))
    for n, v in ci['n10'].items():
        req('CONFIGURATION-IDENTITY.md', '[%s]' % ', '.join(v['derived']))
    # PROOF-DOCUMENT-INTEGRITY.md
    for d in pd['documents']:
        req('PROOF-DOCUMENT-INTEGRITY.md', d['sha256'])
        if d['key'] in ('ADM-PROOF', 'GC-PROOF', 'EXT-PROOF'):
            req('PROOF-DOCUMENT-INTEGRITY.md', d['git_blob'])
    for s in pd['seals'].values():
        req('PROOF-DOCUMENT-INTEGRITY.md', s['sha256sums_sha256'])
    for v in pd['at_C'].values():
        req('PROOF-DOCUMENT-INTEGRITY.md', v['blob'], v['sha256'])
    req('PROOF-DOCUMENT-INTEGRITY.md', pd['transport_commit'])
    # INDEX.md
    counts = mx['status_counts']
    req('INDEX.md', '%d rows' % mx['rows'], *['%d %s' % (n, s) for s, n in counts.items()],
        'C1/C25: **%s**; C4: **%s**' % (verdicts['C1/C25'], verdicts['C4']), sc['lineage'][-2]['commit'],
        sc['lineage'][-2]['tree'])
    for rid in mx['changed_rows'] + mx['not_established_rows']:
        req('INDEX.md', rid if not rid.startswith('N.') else rid)
    failures = []
    docs = ['INDEX.md', 'SOURCE-CLOSURE.md', 'WITNESS-NONINTERFERENCE.md', 'BUILD-INPUT-CLOSURE.md',
            'CONFIGURATION-IDENTITY.md', 'PROOF-DOCUMENT-INTEGRITY.md', 'PROOF-PRECONDITION-MATRIX.md']
    for doc in docs:
        text = open(os.path.join(pkt, doc)).read()
        missing = [s for s in need.get(doc, []) if s and s not in text]
        low = text.lower()
        bad = [f for f in FORBIDDEN if f in low]
        print('%s: %d required strings, %d missing; forbidden phrases: %s' % (
            doc, len(need.get(doc, [])), len(missing), bad or 'none'))
        for m in missing:
            print('  MISSING: %s' % m)
        if missing or bad:
            failures.append(doc)
    # the matrix TSV and the authored status vocabulary
    tsv = open(os.path.join(pkt, 'PROOF-PRECONDITION-MATRIX.tsv')).read().lower()
    bad = [f for f in FORBIDDEN if f in tsv]
    print('PROOF-PRECONDITION-MATRIX.tsv: forbidden phrases: %s' % (bad or 'none'))
    if bad:
        failures.append('tsv')
    print('check_docs: %s' % ('PASS' if not failures else 'FAIL ' + ', '.join(failures)))
    sys.exit(0 if not failures else 1)


if __name__ == '__main__':
    main()
