#!/usr/bin/env python3
"""Exact source closure from the proof-audited sources to Candidate C (mission section 5).

Mission P3-C11-DR01-D2A1-FINAL-SOURCE-PROOF-READINESS-01. Read only: it reads git
objects and writes one JSON document to stdout (or --out). No build, no test.

It establishes, by git object, byte range and item extraction (rust_items.py):
  1. the single-parent lineage SOURCE-R2 -> ... -> C -> D and the expected trees;
  2. blob, sha256 and size of every authenticated file at every lineage commit;
  3. every path outside docs/evidence/ that differs between each lineage commit and C;
  4. how model_tests.rs changed between consecutive versions (byte-prefix append or
     exact changed byte ranges), with lexical isolation of every append;
  5. every item of every audited version mapped to C by (path, ordinal) and compared
     by bytes and by byte offset;
  6. the relevant items of mission section 5 (A to E), each with its baseline and C
     byte ranges and sha256, and a status: IDENTICAL, APPENDED-AFTER-NO-REFERENCE,
     CHANGED or NOT ESTABLISHED;
  7. the name-dependency closure of each section's roots, computed in the audited
     version itself and mapped to C.
Exit status 0 only if every expectation it states holds.
"""
import argparse
import difflib
import json
import sys

import common as K
import rust_items as R

MODEL_VERSIONS = [K.SOURCE_R2, K.ADM, K.GC, K.EXT, K.W1, K.W4, K.C]

# Section B roots: the original model and explorer (mission 5.B; AD-1-R4 N.2).
B_ROOT_NAMES = ['initial', 'actors', 'successors', 'step', 'Io', 'Oracle', 'encode', 'decode',
                'state_properties', 'transition_properties', 'admission_properties', 'terminal',
                'PROPERTIES', 'BRANCHES', 'explore', 'Config', 'default_run', 'BASE', 'DEFAULT_CAP',
                'DEFAULT_TIME', 'FULL_CAP', 'FULL_TIME', 'TICKET_LIMIT', 'REFUSAL_LIMIT']
# Section C roots: the admission observer quotient (mission 5.C; AD-1-R4 N.3).
C_ROOT_NAMES = ['expand', 'observe_admission', 'explore_experimental']
# Section D roots: the gating-caller symmetry quotient (mission 5.D; AD-1-R4 N.3).
D_ROOT_NAMES = ['mirror', 'shape', 'assert_symmetric', 'explore_symmetric', 'explore_symmetric_kept',
                'SYMMETRIC']
# Section B also covers every step_* and every Config constant (found by pattern below).

FAILURES = []


def expect(cond, what):
    if not cond:
        FAILURES.append(what)
    return bool(cond)


def model_items(data):
    its, toks, offs = R.extract(data)
    return its, toks, offs


def flat(its):
    return list(R.flatten(its))


def index(its):
    return {(it['path'], it['ordinal']): it for it in flat(its)}


def impl_subjects(it, data):
    """Identifiers in an impl header (the trait and the type)."""
    if it['kind'] != 'impl':
        return set()
    head = it['name']
    out, cur = set(), ''
    for ch in head + ' ':
        if ch.isalnum() or ch == '_':
            cur += ch
        else:
            if cur and not cur[0].isdigit() and cur not in ('for', 'impl', 'dyn', 'mut'):
                out.add(cur)
            cur = ''
    return out


def item_refs(it, toks, offs):
    a, b = it['decl']
    return {t.text for t in toks if t.kind == 'ident' and offs[t.start] >= a and offs[t.end] <= b}


def closure(data, root_names, region=None):
    """Top-level items of `data` reachable from the root names by identifier mention.

    An item is reached when its name (or, for an impl, a name in its header) is an
    identifier mentioned by a reached item. Over-approximate by design: a mention by
    name, in any namespace, counts. `region` (lo, hi) restricts the result to items
    whose span lies inside it; traversal itself is not restricted."""
    its, toks, offs = R.extract(data)
    by_name = {}
    for it in its:
        names = {it['name']} | impl_subjects(it, data)
        for n in names:
            by_name.setdefault(n, []).append(it)
    seen, order = set(), []
    work = list(root_names)
    done_names = set()
    while work:
        n = work.pop()
        if n in done_names:
            continue
        done_names.add(n)
        for it in by_name.get(n, []):
            key = (it['path'], it['ordinal'])
            if key in seen:
                continue
            seen.add(key)
            order.append(it)
            work.extend(sorted(item_refs(it, toks, offs)))
    out = [it for it in order if region is None or (it['span'][0] >= region[0] and it['span'][1] <= region[1])]
    out.sort(key=lambda it: it['decl'][0])
    return out


def compare(base_it, c_idx, base_data, c_data):
    key = (base_it['path'], base_it['ordinal'])
    c_it = c_idx.get(key)
    if c_it is None:
        return None, 'NOT ESTABLISHED (absent at C)'
    a0, a1 = base_it['decl']
    b0, b1 = c_it['decl']
    same_decl = base_data[a0:a1] == c_data[b0:b1]
    s0, s1 = base_it['span']
    t0, t1 = c_it['span']
    same_span = base_data[s0:s1] == c_data[t0:t1]
    if same_decl and same_span and (a0, a1) == (b0, b1):
        return c_it, 'IDENTICAL'
    if same_decl and same_span:
        return c_it, 'IDENTICAL (bytes; offset moved)'
    if same_decl:
        return c_it, 'CHANGED (leading comments differ; declaration bytes equal)'
    return c_it, 'CHANGED'


def row(section, base_commit, base_it, c_it, status):
    return {
        'section': section,
        'item': base_it['path'] if base_it else c_it['path'],
        'ordinal': (base_it or c_it)['ordinal'],
        'baseline': K.SHORT[base_commit] if base_commit else None,
        'baseline_decl': base_it['decl'] if base_it else None,
        'baseline_lines': base_it['lines'] if base_it else None,
        'baseline_decl_sha256': base_it['decl_sha256'] if base_it else None,
        'c_decl': c_it['decl'] if c_it else None,
        'c_lines': c_it['lines'] if c_it else None,
        'c_decl_sha256': c_it['decl_sha256'] if c_it else None,
        'c_span_sha256': c_it['span_sha256'] if c_it else None,
        'status': status,
    }


def hunks(old, new):
    """Changed byte ranges between two versions, by line diff mapped to bytes."""
    ol = old.splitlines(keepends=True)
    nl = new.splitlines(keepends=True)
    ooff = [0]
    for l in ol:
        ooff.append(ooff[-1] + len(l))
    noff = [0]
    for l in nl:
        noff.append(noff[-1] + len(l))
    out = []
    for tag, i1, i2, j1, j2 in difflib.SequenceMatcher(None, ol, nl, autojunk=False).get_opcodes():
        if tag == 'equal':
            continue
        out.append({'op': tag, 'old_bytes': [ooff[i1], ooff[i2]], 'old_lines': [i1 + 1, i2],
                    'new_bytes': [noff[j1], noff[j2]], 'new_lines': [j1 + 1, j2],
                    'old_text': b''.join(ol[i1:i2]).decode(), 'new_text_sha256': K.sha256(b''.join(nl[j1:j2])),
                    'new_text': b''.join(nl[j1:j2]).decode() if j2 - j1 <= 12 else '(%d lines)' % (j2 - j1)})
    return out


def lexically_isolated(old, new):
    """True if lexing `new` yields exactly old's tokens over [0, len(old)): the appended
    text cannot reopen or close a comment, string or item of the earlier text."""
    to = R.lex(old.decode())
    tn = R.lex(new.decode())
    n_old = len(old.decode())
    # with a pure byte-prefix append, str offsets of the prefix coincide
    pre = [(t.kind, t.text, t.start, t.end) for t in tn if t.end <= n_old]
    full = [(t.kind, t.text, t.start, t.end) for t in to]
    # the last old token may be whitespace that the new text extends; compare all but it
    if full and full[-1][0] == 'ws':
        return pre[:len(full) - 1] == full[:-1]
    return pre == full


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('repo')
    ap.add_argument('--out')
    args = ap.parse_args()
    repo = args.repo
    doc = {'tool': 'source_closure.py', 'mission': 'P3-C11-DR01-D2A1-FINAL-SOURCE-PROOF-READINESS-01'}

    # 1. lineage
    lineage = []
    for i, c in enumerate(K.LINEAGE):
        info = K.commit_info(repo, c)
        info['role'] = K.ROLE[c]
        info['short'] = K.SHORT[c]
        if i:
            expect(info['parents'] == [K.LINEAGE[i - 1]], 'parent of %s is %s' % (K.SHORT[c], K.SHORT[K.LINEAGE[i - 1]]))
        if c in K.EXPECTED_TREE:
            expect(info['tree'] == K.EXPECTED_TREE[c], 'tree of %s is %s' % (K.SHORT[c], K.EXPECTED_TREE[c]))
        lineage.append(info)
    expect(lineage[K.LINEAGE.index(K.C)]['parents'] == [K.C_PARENT], 'C has the single parent R4')
    doc['lineage'] = lineage

    # 2. file identities
    files = {}
    cache = {}
    for c in K.LINEAGE:
        files[K.SHORT[c]] = {}
        for p in K.FILES:
            data = K.blob(repo, c, p)
            cache[(c, p)] = data
            bid = K.blob_id(repo, c, p)
            expect(K.git_blob_hash(data) == bid, 'object hash of %s:%s' % (K.SHORT[c], p))
            files[K.SHORT[c]][p] = {'blob': bid, 'sha256': K.sha256(data), 'bytes': len(data)}
    doc['files'] = files
    for p in K.FILES:
        if p == K.MODEL:
            continue
        ids = {files[K.SHORT[c]][p]['blob'] for c in K.LINEAGE}
        expect(len(ids) == 1, '%s has one blob across the lineage' % p)
    model_ids = [files[K.SHORT[c]][K.MODEL]['blob'] for c in MODEL_VERSIONS]
    doc['model_blobs'] = dict(zip([K.SHORT[c] for c in MODEL_VERSIONS], model_ids))

    # 3. paths outside docs/evidence/ that differ from C
    ctree = K.tree_map(repo, K.C)
    delta = {}
    for c in K.LINEAGE:
        if c == K.C:
            continue
        t = K.tree_map(repo, c)
        keys = set(t) | set(ctree)
        diff = sorted(p for p in keys if not p.startswith(K.EVIDENCE_PREFIX) and t.get(p) != ctree.get(p))
        delta[K.SHORT[c]] = [{'path': p, 'at': t.get(p), 'at_C': ctree.get(p)} for p in diff]
    doc['outside_evidence_delta_vs_C'] = delta
    for c in [K.SOURCE_R2, K.ADM, K.GC, K.EXT, K.W1]:
        expect([d['path'] for d in delta[K.SHORT[c]]] == [K.MODEL],
               'only model_tests.rs differs outside docs/evidence between %s and C' % K.SHORT[c])
    for c in [K.W4, K.B, K.C_PARENT, K.D]:
        expect(delta[K.SHORT[c]] == [], 'nothing differs outside docs/evidence between %s and C' % K.SHORT[c])

    # 4. model_tests.rs relation chain
    data = {c: cache[(c, K.MODEL)] for c in MODEL_VERSIONS}
    chain = []
    for a, b in zip(MODEL_VERSIONS, MODEL_VERSIONS[1:]):
        old, new = data[a], data[b]
        ent = {'from': K.SHORT[a], 'to': K.SHORT[b], 'from_bytes': len(old), 'to_bytes': len(new)}
        if old == new:
            ent['relation'] = 'IDENTICAL'
        elif new.startswith(old):
            ent['relation'] = 'BYTE-PREFIX APPEND'
            ent['appended_bytes'] = [len(old), len(new)]
            ent['appended_sha256'] = K.sha256(new[len(old):])
            ent['old_ends_with_newline'] = old.endswith(b'\n')
            ent['lexically_isolated'] = lexically_isolated(old, new)
            expect(ent['lexically_isolated'], 'append %s->%s is lexically isolated' % (K.SHORT[a], K.SHORT[b]))
        else:
            ent['relation'] = 'CHANGED'
            ent['hunks'] = hunks(old, new)
        chain.append(ent)
    doc['model_chain'] = chain
    rel = {(e['from'], e['to']): e['relation'] for e in chain}
    expect(rel[('SOURCE-R2', 'ADM')] == 'BYTE-PREFIX APPEND', 'ADM appends to SOURCE-R2')
    expect(rel[('ADM', 'GC')] == 'BYTE-PREFIX APPEND', 'GC appends to ADM')
    expect(rel[('GC', 'EXT')] == 'CHANGED', 'EXT changes GC (the extension)')
    expect(rel[('EXT', 'W1')] == 'BYTE-PREFIX APPEND', 'W1 appends to EXT')
    expect(rel[('W1', 'W4')] == 'BYTE-PREFIX APPEND', 'W4 appends to W1')
    expect(rel[('W4', 'C')] == 'IDENTICAL', 'C model_tests.rs equals W4')
    ext = [e for e in chain if e['from'] == 'GC'][0]
    # the extension's changes: the SYMMETRIC doc comment and line, then an append at the end
    ok_ext = (len(ext['hunks']) == 3 and ext['hunks'][2]['op'] == 'insert'
              and ext['hunks'][2]['old_bytes'][0] == len(data[K.GC]))
    expect(ok_ext, 'GC->EXT is two in-place hunks plus one append at the end of the file')
    prefix_of_c = {K.SHORT[c]: data[K.C].startswith(data[c]) for c in MODEL_VERSIONS}
    doc['byte_prefix_of_C'] = prefix_of_c
    for c in [K.SOURCE_R2, K.ADM, K.EXT, K.W1, K.W4]:
        expect(prefix_of_c[K.SHORT[c]], '%s model_tests.rs is a byte prefix of C' % K.SHORT[c])
    expect(not prefix_of_c['GC'], 'GC model_tests.rs is not a byte prefix of C (the extension changed it)')
    regions = {
        'MODEL': [0, len(data[K.SOURCE_R2])],
        'AQ': [len(data[K.SOURCE_R2]), len(data[K.ADM])],
        'SQ+EXT': [len(data[K.ADM]), len(data[K.EXT])],
        'WITNESS-C1-C25': [len(data[K.EXT]), len(data[K.W1])],
        'WITNESS-C4': [len(data[K.W1]), len(data[K.W4])],
    }
    doc['regions_at_C'] = regions

    # 5. every audited item mapped to C
    c_data = data[K.C]
    c_its, c_toks, c_offs = model_items(c_data)
    c_idx = index(c_its)
    mapping = {}
    for b in [K.SOURCE_R2, K.ADM, K.GC, K.EXT, K.W1, K.W4]:
        b_its, _, _ = model_items(data[b])
        counts = {}
        changed = []
        for it in flat(b_its):
            c_it, status = compare(it, c_idx, data[b], c_data)
            counts[status] = counts.get(status, 0) + 1
            if status != 'IDENTICAL':
                changed.append(row('map', b, it, c_it, status))
        mapping[K.SHORT[b]] = {'items': len(flat(b_its)), 'status_counts': counts, 'not_identical': changed}
    doc['item_mapping'] = mapping
    for b in ['SOURCE-R2', 'ADM', 'EXT', 'W1', 'W4']:
        expect(mapping[b]['status_counts'] == {'IDENTICAL': mapping[b]['items']},
               'every item of %s is IDENTICAL at C (bytes and offsets)' % b)
    gc_not = mapping['GC']['not_identical']
    gc_changed = [r for r in gc_not if r['status'].startswith('CHANGED') or r['status'].startswith('NOT')]
    expect([r['item'] for r in gc_changed] == ['const SYMMETRIC'],
           'the only item of GC changed at C is const SYMMETRIC')
    expect(all(r['status'] == 'IDENTICAL (bytes; offset moved)' for r in gc_not if r['item'] != 'const SYMMETRIC'),
           'every other GC item is byte-identical at C, at an offset moved by the extension hunk')

    # 6/7. sections A-E
    sections = []
    # A. protocol.rs
    p_c = cache[(K.C, K.PROTO)]
    for b in [K.SOURCE_R2, K.ADM, K.GC, K.EXT, K.W1, K.W4]:
        same = cache[(b, K.PROTO)] == p_c
        sections.append({'section': 'A', 'item': 'protocol.rs (whole file, incl. test-only hook)',
                         'baseline': K.SHORT[b], 'baseline_blob': files[K.SHORT[b]][K.PROTO]['blob'],
                         'c_blob': files['C'][K.PROTO]['blob'], 'sha256': K.sha256(p_c),
                         'status': 'IDENTICAL' if same else 'CHANGED'})
        expect(same, 'protocol.rs at C is byte-identical to %s' % K.SHORT[b])
    p_its, _, _ = R.extract(p_c)
    hook = [it for it in p_its if it['path'] == 'mod hook']
    expect(len(hook) == 1, 'protocol.rs has one mod hook')
    doc['protocol_items'] = len(flat(p_its))
    doc['protocol_hook'] = {'decl': hook[0]['decl'], 'lines': hook[0]['lines'], 'decl_sha256': hook[0]['decl_sha256']}

    # B. model and explorer, baseline SOURCE-R2
    sr2 = data[K.SOURCE_R2]
    sr2_its, _, _ = model_items(sr2)
    named_b = []
    for it in sr2_its:
        n = it['name']
        subj = impl_subjects(it, sr2)
        cfg_const = it['kind'] == 'const' and sr2[it['decl'][0]:it['decl'][1]].split(b'=')[0].strip().endswith(b': Config')
        if (n in B_ROOT_NAMES or (it['kind'] == 'fn' and n.startswith('step_')) or subj & {'Io', 'Oracle', 'Enc', 'Dec'}
                or n in ('Enc', 'Dec') or cfg_const):
            named_b.append(it)
    b_closure = closure(sr2, [it['name'] for it in named_b] + sorted(set().union(*[impl_subjects(i, sr2) for i in named_b])))
    for it in named_b:
        c_it, status = compare(it, c_idx, sr2, c_data)
        sections.append(row('B', K.SOURCE_R2, it, c_it, status))
    doc['B_named'] = len(named_b)
    doc['B_closure'] = [{'item': it['path'], 'decl': it['decl']} for it in b_closure]
    b_closure_status = {}
    for it in b_closure:
        _, status = compare(it, c_idx, sr2, c_data)
        b_closure_status[status] = b_closure_status.get(status, 0) + 1
    doc['B_closure_status'] = b_closure_status
    expect(set(b_closure_status) == {'IDENTICAL'}, 'every item in the section B closure is IDENTICAL at C')

    # C. admission observer quotient, baseline ADM (its region)
    adm = data[K.ADM]
    aq_region = regions['AQ']
    c_cl = closure(adm, C_ROOT_NAMES, region=aq_region)
    for it in c_cl:
        c_it, status = compare(it, c_idx, adm, c_data)
        sections.append(row('C', K.ADM, it, c_it, status))
    doc['C_closure'] = [it['path'] for it in c_cl]

    # D. symmetry quotient, baseline GC and EXT (its region)
    gc = data[K.GC]
    gc_region = [len(adm), len(gc)]
    d_cl = closure(gc, D_ROOT_NAMES, region=gc_region)
    ext = data[K.EXT]
    ext_its, _, _ = model_items(ext)
    ext_idx = index(ext_its)
    for it in d_cl:
        c_it, status = compare(it, c_idx, gc, c_data)
        r = row('D', K.GC, it, c_it, status)
        e_it = ext_idx.get((it['path'], it['ordinal']))
        _, st_ext = compare(e_it, c_idx, ext, c_data) if e_it else (None, 'NOT ESTABLISHED (absent at EXT)')
        r['status_vs_EXT'] = st_ext
        sections.append(r)
    doc['D_closure'] = [it['path'] for it in d_cl]
    expect(all(r['status_vs_EXT'] == 'IDENTICAL' for r in sections if r['section'] == 'D'),
           'every section D item is IDENTICAL at C relative to EXT')

    # E. the F-PT/F-B extension: every EXT item that is new or changed relative to GC
    gc_its, _, _ = model_items(gc)
    gc_idx = index(gc_its)
    for it in flat(ext_its):
        g = gc_idx.get((it['path'], it['ordinal']))
        if g is not None and gc[g['span'][0]:g['span'][1]] == ext[it['span'][0]:it['span'][1]]:
            continue
        c_it, status = compare(it, c_idx, ext, c_data)
        r = row('E', K.EXT, it, c_it, status)
        r['vs_GC'] = 'CHANGED' if g is not None else 'ADDED'
        sections.append(r)
    expect(all(r['status'] == 'IDENTICAL' for r in sections if r['section'] == 'E'),
           'every section E item is IDENTICAL at C relative to EXT')

    # witness items: appended after the audited text, never mentioned before it
    pre_ids = R.identifiers(c_toks, c_offs, 0, regions['WITNESS-C1-C25'][0])
    for label in ('WITNESS-C1-C25', 'WITNESS-C4'):
        lo, hi = regions[label]
        for it in c_its:
            if it['span'][0] >= lo and it['span'][1] <= hi:
                mentioned = it['name'] in pre_ids
                status = 'APPENDED-AFTER-NO-REFERENCE' if not mentioned else 'NOT ESTABLISHED (name mentioned before the append)'
                r = row(label, None, None, it, status)
                sections.append(r)
                expect(not mentioned, '%s item %s is not mentioned in the audited text' % (label, it['path']))
    doc['sections'] = sections
    doc['section_status_counts'] = {}
    for r in sections:
        key = '%s %s' % (r['section'], r['status'])
        doc['section_status_counts'][key] = doc['section_status_counts'].get(key, 0) + 1

    # every closure computed in an audited version stays inside the audited text
    for label, cl in (('B', b_closure),):
        expect(all(it['span'][1] <= regions['MODEL'][1] for it in cl), 'closure %s lies in the model region' % label)

    # 8. layer isolation at C: the text before each appended layer cannot name, or be
    # re-resolved by, anything the layer adds. Layers in C's byte coordinates.
    ext_hunk = [e for e in chain if e['from'] == 'GC'][0]['hunks']
    ext_append_lo = ext_hunk[2]['new_bytes'][0]
    layers = [('AQ', regions['AQ'][0], regions['AQ'][1]),
              ('SQ (with the EXT in-place hunk)', regions['SQ+EXT'][0], ext_append_lo),
              ('EXT append', ext_append_lo, regions['SQ+EXT'][1]),
              ('WITNESS-C1-C25', regions['WITNESS-C1-C25'][0], regions['WITNESS-C1-C25'][1]),
              ('WITNESS-C4', regions['WITNESS-C4'][0], regions['WITNESS-C4'][1])]
    sym = [it for it in c_its if it['path'] == 'const SYMMETRIC'][0]
    iso = []
    for label, lo, hi in layers:
        reg = [it for it in c_its if it['span'][0] >= lo and it['span'][1] <= hi]
        defined = sorted({it['name'] for it in reg})
        pre = R.identifiers(c_toks, c_offs, 0, lo)
        mentioned = {n: pre[n] for n in defined if n in pre}
        # by design the extension's SYMMETRIC (in place, before the append) names its fixtures
        allowed = {}
        if label == 'EXT append':
            allowed = {n: offs for n, offs in mentioned.items()
                       if n in ('SYM_PT', 'SYM_B', 'SYM_BI') and all(sym['decl'][0] <= o < sym['decl'][1] for o in offs)}
        unexpected = sorted(n for n in mentioned if n not in allowed)
        item_kinds = sorted({it['kind'] for it in reg})
        bad_items = [it['path'] for it in reg if it['kind'] in ('impl', 'trait', 'use', 'static', 'macro_rules', 'mod',
                                                             'extern', 'union', 'macro')]
        code = [t for t in c_toks if t.kind not in ('ws', 'line_comment', 'block_comment')
                and lo <= c_offs[t.start] < hi]
        inner_attrs = sum(1 for i, t in enumerate(code[:-1]) if t.text == '#' and code[i + 1].text == '!')
        cfg_attrs = sum(1 for i, t in enumerate(code[:-2]) if t.text == '#' and code[i + 1].text == '['
                        and code[i + 2].text in ('cfg', 'cfg_attr'))
        ent = {'layer': label, 'bytes_at_C': [lo, hi], 'items': len(reg), 'item_kinds': item_kinds,
               'names_mentioned_earlier': sorted(mentioned), 'allowed_by_design': sorted(allowed),
               'unexpected_mentions': unexpected, 'impl_trait_use_static_macro_mod_items': bad_items,
               'inner_attributes': inner_attrs, 'cfg_attributes': cfg_attrs}
        ent['isolated'] = not unexpected and not bad_items and not inner_attrs and not cfg_attrs
        expect(ent['isolated'], 'layer %s is isolated from the text before it' % label)
        iso.append(ent)
    expect([e['allowed_by_design'] for e in iso if e['layer'] == 'EXT append'] == [['SYM_B', 'SYM_BI', 'SYM_PT']],
           'the EXT append is named before it only by SYMMETRIC, and only as SYM_PT, SYM_B and SYM_BI')
    doc['layer_isolation'] = iso

    doc['expectation_failures'] = FAILURES
    doc['result'] = 'PASS' if not FAILURES else 'FAIL'
    text = json.dumps(doc, indent=1, sort_keys=True)
    if args.out:
        open(args.out, 'w').write(text + '\n')
    else:
        print(text)
    print('source_closure: %s (%d expectation failures)' % (doc['result'], len(FAILURES)), file=sys.stderr)
    for f in FAILURES:
        print('  FAILED: ' + f, file=sys.stderr)
    sys.exit(0 if not FAILURES else 1)


if __name__ == '__main__':
    main()
