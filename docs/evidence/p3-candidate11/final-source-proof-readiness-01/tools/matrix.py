#!/usr/bin/env python3
"""The proof-precondition matrix (mission section 8), computed from matrix_rows.py.

Mission P3-C11-DR01-D2A1-FINAL-SOURCE-PROOF-READINESS-01. Read only: git objects and
the JSON receipts of source_closure.py, witness_check.py and proof_docs.py in; the TSV
matrix and a JSON receipt out. No build, no test.

For each row it resolves every citation to items by byte range at the cited commit
(rust_items.py), unions the citations of the rows it relies on, compares every item
with Candidate C by bytes, and sets the row's static status from its kind:
  IDENTICAL, NON_INTERFERING_CHANGE, CHANGED_REVIEW_REQUIRED, EXECUTION_PENDING,
  OWNER_ALREADY_SATISFIED, NOT_ESTABLISHED.
A changed or unresolvable citation always wins over the kind. No row says a proof is
accepted or a theorem proved.
"""
import argparse
import bisect
import json
import sys

import common as K
import rust_items as R
import matrix_rows as MR

COMMITS = {'SOURCE-R2': K.SOURCE_R2, 'ADM': K.ADM, 'GC': K.GC, 'EXT': K.EXT, 'W1': K.W1, 'C': K.C}
FILES = {'M': K.MODEL, 'P': K.PROTO, 'U': K.UNIT}
RC = {'source': 'receipts/10-source-closure.json', 'witness': 'receipts/11-witness-check.json',
      'build': 'receipts/12-build-inputs.json', 'config': 'receipts/13-config-identity.json',
      'proof': 'receipts/14-proof-docs.json', 'matrix': 'receipts/15-matrix.json'}
COLUMNS = ['id', 'group', 'kind', 'statement', 'proof_citation', 'depends_on', 'source_items',
           'identity_at_proof_commit', 'identity_at_C', 'static_status', 'evidence', 'note']
ADOPT = 'docs/evidence/p3-candidate11/ad1r4-adoption/OWNER-ADOPTION-2026-10-14.md'
ADOPT_BLOB = '54c956bc1324dd3a8fd97439faa815f0865b404d'


class Repo:
    def __init__(self, repo):
        self.repo = repo
        self.cache = {}
        self.items = {}

    def data(self, f, c):
        key = (f, c)
        if key not in self.cache:
            self.cache[key] = K.blob(self.repo, COMMITS[c], FILES.get(f, f))
        return self.cache[key]

    def its(self, f, c):
        key = (f, c)
        if key not in self.items:
            data = self.data(f, c)
            its, toks, offs = R.extract(data)
            flat = list(R.flatten(its))
            starts = [0]
            for i, b in enumerate(data):
                if b == 0x0A:
                    starts.append(i + 1)
            for it in flat:
                it['span_line'] = bisect.bisect_right(starts, it['span'][0])
                it['depth'] = it['path'].count('/')
            self.items[key] = (its, flat, toks, offs)
        return self.items[key]


def resolve(rp, cite):
    """A list of dependencies: ('item', f, c, item) or ('file', path, c) or ('region',) or ('whole', c);
    or ('unresolved', text)."""
    kind = cite[0]
    if kind == 'L':
        _, f, c, a, b = cite
        _, flat, _, _ = rp.its(f, c)
        cand = [it for it in flat if it['span_line'] <= b and it['lines'][1] >= a]
        keep = [it for it in cand if not any(o is not it and o['path'].startswith(it['path'] + '/') for o in cand)]
        if not keep:
            return [('unresolved', '%s@%s lines %d-%d' % (f, c, a, b))]
        return [('item', f, c, it) for it in keep]
    if kind == 'N':
        _, f, c, path = cite
        _, flat, _, _ = rp.its(f, c)
        hit = [it for it in flat if it['path'] == path]
        if not hit:
            return [('unresolved', '%s@%s %s' % (f, c, path))]
        return [('item', f, c, it) for it in hit]
    if kind == 'F':
        return [('file', cite[1], cite[2])]
    if kind == 'R':
        return [('region',)]
    if kind == 'W':
        return [('whole', cite[1])]
    if kind == 'STEPS':
        _, flat, _, _ = rp.its('M', cite[1])
        return [('item', 'M', cite[1], it) for it in flat if it['kind'] == 'fn' and it['depth'] == 0
                and (it['name'] == 'step' or it['name'].startswith('step_'))]
    if kind == 'CONFIGS':
        data = rp.data('M', cite[1])
        _, flat, _, _ = rp.its('M', cite[1])
        out = []
        for it in flat:
            if it['kind'] == 'const' and it['depth'] == 0:
                head = data[it['decl'][0]:it['decl'][1]].split(b'=')[0]
                if head.strip().endswith(b': Config') or it['name'] in ('DEFAULT_CAP', 'DEFAULT_TIME', 'FULL_CAP',
                                                                         'FULL_TIME', 'TICKET_LIMIT', 'REFUSAL_LIMIT'):
                    out.append(('item', 'M', cite[1], it))
        return out
    if kind == 'IMPLS_IO':
        _, flat, _, _ = rp.its('M', cite[1])
        return [('item', 'M', cite[1], it) for it in flat if it['kind'] == 'impl' and it['depth'] == 0
                and ('Io' in it['name'].replace('<', ' ').replace('>', ' ').split())]
    return [('unresolved', repr(cite))]


def full_path(f):
    return FILES.get(f, f)


def compare(rp, dep, ctx):
    """(source_item_text, identity_at_proof_commit, identity_at_C, status)."""
    if dep[0] == 'unresolved':
        return dep[1], 'unresolved', 'unresolved', 'UNRESOLVED'
    if dep[0] == 'item':
        _, f, c, it = dep
        _, cflat, _, _ = rp.its(f, 'C')
        match = [o for o in cflat if o['path'] == it['path'] and o['ordinal'] == it['ordinal']]
        a, b = it['decl']
        src = '%s@%s:%s#%d' % (f, c, it['path'], it['ordinal'])
        left = '%s#%d [%d,%d) %s' % (it['path'], it['ordinal'], a, b, it['decl_sha256'][:16])
        if not match:
            return src, left, 'absent', 'UNRESOLVED'
        o = match[0]
        same = rp.data(f, c)[a:b] == rp.data(f, 'C')[o['decl'][0]:o['decl'][1]]
        moved = (o['decl'][0], o['decl'][1]) != (a, b)
        right = '%s#%d [%d,%d) %s %s' % (o['path'], o['ordinal'], o['decl'][0], o['decl'][1], o['decl_sha256'][:16],
                                         ('= (offset moved)' if moved else '=') if same else 'CHANGED')
        return src, left, right, 'IDENTICAL' if same else 'CHANGED'
    if dep[0] == 'file':
        _, f, c = dep
        p = full_path(f)
        bc, bC = K.blob_id(rp.repo, COMMITS[c], p), K.blob_id(rp.repo, K.C, p)
        sc = K.sha256(K.blob(rp.repo, COMMITS[c], p))
        left = '%s blob %s sha256 %s' % (p, bc[:12], sc[:16])
        right = '%s blob %s %s' % (p, bC[:12], '=' if bc == bC else 'CHANGED')
        return '%s@%s (whole file)' % (p, c), left, right, 'IDENTICAL' if bc == bC else 'CHANGED'
    if dep[0] == 'region':
        sr2 = rp.data('M', 'SOURCE-R2')
        same = rp.data('M', 'C')[:len(sr2)] == sr2
        h = K.sha256(sr2)[:16]
        return ('M model region [0,%d) (model_tests.rs at SOURCE-R2)' % len(sr2),
                'bytes [0,%d) %s' % (len(sr2), h), 'bytes [0,%d) %s' % (len(sr2), '=' if same else 'CHANGED'),
                'IDENTICAL' if same else 'CHANGED')
    if dep[0] == 'whole':
        c = dep[1]
        old, new = rp.data('M', c), rp.data('M', 'C')
        bo, bn = K.blob_id(rp.repo, COMMITS[c], K.MODEL), K.blob_id(rp.repo, K.C, K.MODEL)
        left = 'model_tests.rs blob %s (%d bytes)' % (bo[:12], len(old))
        if old == new:
            return 'model_tests.rs@%s (whole file)' % c, left, 'blob %s =' % bn[:12], 'IDENTICAL'
        ok = new.startswith(old) and ctx['layers_isolated'] and ctx['witness_non_interfering'] and ctx['no_hook_install']
        right = ('blob %s: byte prefix [0,%d) kept; later layers appended, each isolated; witnesses NON_INTERFERING'
                 % (bn[:12], len(old))) if ok else 'blob %s: NOT a byte prefix or a layer not isolated' % bn[:12]
        return 'model_tests.rs@%s (whole file)' % c, left, right, 'APPENDED_NONINTERFERING' if ok else 'CHANGED'
    raise ValueError(dep)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('repo')
    ap.add_argument('--source-closure', required=True)
    ap.add_argument('--witness-check', required=True)
    ap.add_argument('--proof-docs', required=True)
    ap.add_argument('--tsv', required=True)
    ap.add_argument('--out', required=True)
    args = ap.parse_args()
    rp = Repo(args.repo)
    sc = json.load(open(args.source_closure))
    wc = json.load(open(args.witness_check))
    pd = json.load(open(args.proof_docs))
    failures = []
    for name, d in (('source_closure', sc), ('witness_check', wc), ('proof_docs', pd)):
        if d.get('result') != 'PASS':
            failures.append('%s receipt is not PASS' % name)
    # scans on model_tests.rs at C
    c_data = rp.data('M', 'C')
    _, c_flat, c_toks, c_offs = rp.its('M', 'C')
    regions = sc['regions_at_C']
    code = [t for t in c_toks if t.kind not in ('ws', 'line_comment', 'block_comment')]
    hook_calls = {}
    for i, t in enumerate(code[:-3]):
        if t.text == 'hook' and code[i + 1].text == ':' and code[i + 2].text == ':':
            hook_calls[code[i + 3].text] = hook_calls.get(code[i + 3].text, 0) + 1
    no_hook_install = not hook_calls.get('install') and not hook_calls.get('remove')

    def region_of(off):
        for name, (lo, hi) in regions.items():
            if lo <= off < hi:
                return name
        return 'outside'

    adm_scan = {}
    for i, t in enumerate(code):
        hit = None
        if t.kind == 'ident' and t.text in ('adm', 'adm_kept'):
            hit = t.text
        elif t.kind == 'ident' and t.text == 'admissions' and i >= 2 and code[i - 1].text == '.' and code[i - 2].text == 'config':
            hit = 'config.admissions'
        if hit:
            off = c_offs[t.start]
            line = c_data.count(b'\n', 0, off) + 1
            adm_scan.setdefault(region_of(off), {}).setdefault(hit, []).append(line)
    sr2_lines = {}
    sr2 = rp.data('M', 'SOURCE-R2')
    s_toks = R.lex(sr2.decode())
    s_code = [t for t in s_toks if t.kind not in ('ws', 'line_comment', 'block_comment')]
    for i, t in enumerate(s_code):
        hit = None
        if t.kind == 'ident' and t.text in ('adm', 'adm_kept'):
            hit = t.text
        elif t.kind == 'ident' and t.text == 'admissions' and s_code[i - 1].text == '.' and s_code[i - 2].text == 'config':
            hit = 'config.admissions'
        if hit:
            sr2_lines.setdefault(hit, []).append(sr2.decode()[:t.start].count('\n') + 1)
    model_same_as_sr2 = adm_scan.get('MODEL', {}) == sr2_lines
    if not model_same_as_sr2:
        failures.append('the adm occurrences of the model region at C differ from SOURCE-R2')
    witness_adm = {k: v for k, v in adm_scan.items() if k.startswith('WITNESS')}
    ctx = {
        'layers_isolated': all(e['isolated'] for e in sc['layer_isolation']),
        'witness_non_interfering': all(v == 'NON_INTERFERING' for v in wc['verdicts'].values()),
        'no_hook_install': no_hook_install,
    }
    adopt_ok = K.blob_id(args.repo, K.C, ADOPT) == ADOPT_BLOB
    docs = {d['key']: d for d in pd['documents']}

    by_id = {r['id']: r for r in MR.ROWS}

    def deps_of(r, seen=None):
        seen = seen or set()
        out = []
        for c in r['cites']:
            out.extend(resolve(rp, c))
        for v in r['via']:
            if v in seen:
                continue
            seen.add(v)
            out.extend(deps_of(by_id[v], seen))
        return out

    rows_out = []
    for r in MR.ROWS:
        for v in r['via']:
            if v not in by_id:
                failures.append('%s relies on unknown row %s' % (r['id'], v))
        deps = deps_of(r)
        seen, items = set(), []
        for d in deps:
            key = (d[0], d[1], d[2], d[3]['path'], d[3]['ordinal']) if d[0] == 'item' else d
            if key in seen:
                continue
            seen.add(key)
            items.append(d)
        order = {'SOURCE-R2': 0, 'ADM': 1, 'GC': 2, 'EXT': 3, 'W1': 4, 'C': 5}

        def sort_key(d):
            if d[0] == 'item':
                return (0, d[1], order[d[2]], d[3]['decl'][0])
            return (1, str(d))

        items.sort(key=sort_key)
        cmp = [compare(rp, d, ctx) for d in items]
        statuses = {x[3] for x in cmp}
        kind = r['kind']
        if 'UNRESOLVED' in statuses:
            status = 'NOT_ESTABLISHED'
            failures.append('%s has an unresolved citation' % r['id'])
        elif 'CHANGED' in statuses:
            status = 'CHANGED_REVIEW_REQUIRED'
        elif kind == 'STATIC':
            status = 'IDENTICAL'
        elif kind == 'WHOLEFILE':
            status = 'NON_INTERFERING_CHANGE' if 'APPENDED_NONINTERFERING' in statuses else 'IDENTICAL'
        elif kind == 'EXECUTION':
            status = 'EXECUTION_PENDING'
        elif kind == 'OWNER':
            status = 'OWNER_ALREADY_SATISFIED' if adopt_ok else 'NOT_ESTABLISHED'
        elif kind in ('REVIEW', 'SCOPE'):
            status = 'NOT_ESTABLISHED'
        else:
            raise ValueError(kind)
        if kind == 'STATIC' and 'APPENDED_NONINTERFERING' in statuses:
            failures.append('%s is STATIC but cites the whole file' % r['id'])
        doc = docs.get(r['proof'])
        if doc:
            citation = '%s %s (sha256 %s; %s)' % (r['proof'], r['section'], doc['sha256'][:16],
                                                   doc['transported']['path'].split(':', 1)[0][:8])
        else:
            citation = '%s %s (C:%s)' % (r['proof'], r['section'], 'docs/evidence/p3-candidate11/ad1r4-consolidation-01/ad1r4/')
        note = r['note']
        if r['id'] == 'ADM.F2':
            outside = {k: {h: len(v) for h, v in d.items()} for k, d in sorted(adm_scan.items()) if k != 'MODEL'}
            note += ('; model region occurrences at C equal SOURCE-R2: %s; occurrences in later layers (not in any step): %s'
                     % (model_same_as_sr2, json.dumps(outside, sort_keys=True)))
        if r['id'] in ('ADM.AQ3', 'GC.AS2'):
            note += '; hook paths called in model_tests.rs at C: %s' % json.dumps(hook_calls, sort_keys=True)
        if kind == 'OWNER':
            note += '; adoption record blob at C %s' % ('= ' + ADOPT_BLOB[:8] if adopt_ok else 'MISMATCH')
        evidence = {'Admission': RC['source'], 'Symmetry': RC['source'], 'Extension': RC['source'],
                    'Composition AQ then SQ': RC['source'] + '; ' + RC['config']}.get(r['group'].split(':')[0],
                                                                                         RC['source'])
        if r['id'].startswith('N.'):
            evidence = {'N.4b': RC['witness'], 'N.5a': RC['build'], 'N.6': RC['build'], 'N.9': RC['config'],
                        'N.10': RC['config'], 'N.15b': RC['proof']}.get(r['id'], RC['source'])
        if kind == 'WHOLEFILE':
            evidence = RC['source'] + '; ' + RC['witness']
        evidence += '; ' + RC['matrix']
        rows_out.append({
            'id': r['id'], 'group': r['group'], 'kind': kind, 'statement': r['statement'],
            'proof_citation': citation, 'depends_on': ','.join(r['via']),
            'source_items': '; '.join(x[0] for x in cmp) or '(none: not a source statement)',
            'identity_at_proof_commit': '; '.join(x[1] for x in cmp) or '-',
            'identity_at_C': '; '.join(x[2] for x in cmp) or '-',
            'static_status': status, 'evidence': evidence, 'note': note})
    for o in rows_out:
        for k in COLUMNS:
            if '\t' in o[k] or '\n' in o[k]:
                failures.append('%s %s contains a tab or newline' % (o['id'], k))
    with open(args.tsv, 'w') as f:
        f.write('\t'.join(COLUMNS) + '\n')
        for o in rows_out:
            f.write('\t'.join(o[k] for k in COLUMNS) + '\n')
    counts = {}
    for o in rows_out:
        counts[o['static_status']] = counts.get(o['static_status'], 0) + 1
    group_counts = {}
    for o in rows_out:
        g = group_counts.setdefault(o['group'], {})
        g[o['static_status']] = g.get(o['static_status'], 0) + 1
    forbidden = [o['id'] for o in rows_out if any(w in (o['note'] + o['statement']).upper()
                                                  for w in ('PROOF ACCEPTED', 'THEOREM PROVED', 'ACCEPTED PROOF'))]
    if forbidden:
        failures.append('rows claim acceptance: %s' % forbidden)
    out = {'tool': 'matrix.py', 'rows': len(rows_out), 'status_counts': counts, 'group_status_counts': group_counts,
           'context': ctx, 'adm_scan_at_C': adm_scan, 'adm_scan_SOURCE_R2_model': sr2_lines,
           'witness_regions_adm_occurrences': witness_adm, 'hook_calls_at_C': hook_calls,
           'changed_rows': [o['id'] for o in rows_out if o['static_status'] == 'CHANGED_REVIEW_REQUIRED'],
           'not_established_rows': [o['id'] for o in rows_out if o['static_status'] == 'NOT_ESTABLISHED'],
           'tsv_sha256': K.sha256(open(args.tsv, 'rb').read()),
           'expectation_failures': failures, 'result': 'PASS' if not failures else 'FAIL'}
    open(args.out, 'w').write(json.dumps(out, indent=1, sort_keys=True) + '\n')
    print('matrix: %s rows %d %s' % (out['result'], len(rows_out), json.dumps(counts, sort_keys=True)), file=sys.stderr)
    for f in failures:
        print('  FAILED: ' + f, file=sys.stderr)
    sys.exit(0 if not failures else 1)


if __name__ == '__main__':
    main()
