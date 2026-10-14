#!/usr/bin/env python3
"""Static non-interference review of the C1/C25 and C4 witness additions (mission section 6).

Mission P3-C11-DR01-D2A1-FINAL-SOURCE-PROOF-READINESS-01. Read only: git objects in,
one JSON document out. No build, no test, no witness invocation.

For each addition (C1/C25: EXT -> W1; C4: W1 -> W4) it checks, by bytes and by tokens:
  N01 the addition is a pure byte append to the earlier file, lexically isolated;
  N02 no earlier item changes (bytes and offsets);
  N03 no `impl` and no `trait` anywhere in the addition, at any depth;
  N04 no name it defines (top level) is mentioned anywhere in the earlier text, none
      collides with an earlier item, with a protocol.rs item or with a prelude name;
  N05 no `use` declaration;
  N06 no macro definition (`macro_rules`, `macro_export`, proc-macro attributes);
  N07 its attributes are outer `#[test]` only: no inner attribute, no cfg/cfg_attr;
  N08 no global, static or thread-local state, no process-global side effect;
  N09 the test hook: protocol.rs (which holds it) unchanged; the addition calls only
      `hook::take_counts`;
  N10 configuration constants unchanged; the addition defines no `Config` constant;
  N11 PROPERTIES and BRANCHES unchanged and not mentioned by the addition;
  N12 every earlier test unchanged; the `#[ignore]` inventory unchanged;
  N13 no item of the addition is in the name-dependency closure of the explorers and
      every test of the earlier text, computed at C;
  N14 crate attributes, features and every compiled file other than model_tests.rs
      unchanged between the two commits; no include!/env!/path/link-level construct.
Verdict per addition: NON_INTERFERING if every check passes, else PROOF_REVIEW_BLOCKER.
"""
import argparse
import json
import sys

import common as K
import rust_items as R
import source_closure as S

PRELUDE = {
    'Copy', 'Send', 'Sized', 'Sync', 'Unpin', 'Drop', 'Fn', 'FnMut', 'FnOnce', 'drop', 'Box', 'ToOwned',
    'Clone', 'PartialEq', 'PartialOrd', 'Eq', 'Ord', 'AsRef', 'AsMut', 'Into', 'From', 'Default', 'Iterator',
    'Extend', 'IntoIterator', 'DoubleEndedIterator', 'ExactSizeIterator', 'Option', 'Some', 'None', 'Result',
    'Ok', 'Err', 'String', 'ToString', 'Vec', 'TryFrom', 'TryInto', 'FromIterator', 'bool', 'char', 'str',
    'u8', 'u16', 'u32', 'u64', 'u128', 'usize', 'i8', 'i16', 'i32', 'i64', 'i128', 'isize', 'f32', 'f64',
    'Self', 'self', 'super', 'crate', 'std', 'core', 'alloc', 'test',
    # names the file imports (use lines 22-26 at every version)
    'BTreeMap', 'BTreeSet', 'Write', 'catch_unwind', 'resume_unwind', 'AssertUnwindSafe', 'Mutex',
    'Duration', 'Instant',
}
FORBIDDEN_IDENTS = [
    'static', 'thread_local', 'lazy_static', 'OnceLock', 'OnceCell', 'LazyLock', 'LazyCell', 'Lazy',
    'Mutex', 'RwLock', 'Condvar', 'SERIAL', 'set_var', 'remove_var', 'set_hook', 'take_hook', 'unsafe',
    'spawn', 'File', 'fs', 'process', 'exit', 'abort', 'Command', 'env', 'include', 'include_str',
    'include_bytes', 'option_env', 'asm', 'global_asm', 'no_mangle', 'export_name', 'link',
    'global_allocator', 'panic_handler', 'path', 'cfg', 'cfg_attr', 'macro_export', 'proc_macro',
    'proc_macro_derive', 'proc_macro_attribute', 'extern', 'crate', 'super', 'mod', 'use', 'impl',
    'trait', 'macro_rules', 'union', 'Atomic', 'AtomicBool', 'AtomicU8', 'AtomicU32', 'AtomicU64',
    'AtomicUsize', 'AtomicI64', 'UnsafeCell', 'Cell', 'RefCell', 'install', 'remove', 'report', 'Rc', 'Arc',
]
ADDITIONS = [('C1/C25', K.EXT, K.W1), ('C4', K.W1, K.W4)]
TEST_ROOTS_NOTE = 'explore, explore_experimental, explore_symmetric(_kept), every model root, and every test of the earlier text'


def attributes(toks, offs, lo, hi):
    """Every attribute (#[..] or #![..]) whose '#' lies in [lo, hi), as text."""
    code = [t for t in toks if t.kind not in ('ws', 'line_comment', 'block_comment')]
    out = []
    for i, t in enumerate(code):
        if t.text == '#' and offs[t.start] >= lo and offs[t.start] < hi:
            j = i + 1
            inner = code[j].text == '!'
            if inner:
                j += 1
            if code[j].text != '[':
                continue
            depth, k = 0, j
            while True:
                if code[k].text in '([{':
                    depth += 1
                elif code[k].text in ')]}':
                    depth -= 1
                k += 1
                if depth == 0:
                    break
            text = ''.join(x.text if x.kind != 'str' else x.text for x in code[i:k])
            out.append({'offset': offs[t.start], 'inner': inner, 'text': text})
    return out


def hook_calls(toks, offs, lo, hi):
    code = [t for t in toks if t.kind not in ('ws', 'line_comment', 'block_comment')]
    out = []
    for i, t in enumerate(code):
        if (t.kind == 'ident' and t.text == 'hook' and offs[t.start] >= lo and offs[t.start] < hi
                and code[i + 1].text == ':' and code[i + 2].text == ':'):
            out.append(code[i + 3].text)
    return out


def keyword_sites(toks, offs, lo, hi, region_items, src_bytes):
    """Every item keyword token in [lo, hi); those that head a top-level item of the
    region are marked; the others are listed with context."""
    heads = set()
    for it in region_items:
        a, b = it['decl']
        for t in toks:
            if offs[t.start] >= a and offs[t.end] <= b and t.kind == 'ident' and t.text == it['kind']:
                heads.add(offs[t.start])
                break
    sites = []
    for t in toks:
        if t.kind != 'ident' or not (lo <= offs[t.start] < hi):
            continue
        if t.text in ('fn', 'struct', 'enum', 'const', 'type', 'static', 'impl', 'trait', 'mod', 'use',
                      'macro_rules', 'extern', 'union', 'let'):
            if t.text == 'let':
                continue
            o = offs[t.start]
            ctx = src_bytes[max(lo, o - 30):o + 50].decode('utf-8', 'replace').replace('\n', ' ')
            sites.append({'keyword': t.text, 'offset': o, 'heads_region_item': o in heads, 'context': ctx})
    return sites


def ignored_tests(data):
    its, toks, offs = R.extract(data)
    out = []
    for it in its:
        if it['kind'] != 'fn':
            continue
        a, b = it['span']
        head = data[a:it['decl'][1]].split(b'{', 1)[0]
        if b'#[ignore' in head:
            out.append(it['name'])
    return out


def test_fns(data):
    its, _, _ = R.extract(data)
    out = []
    for it in its:
        if it['kind'] == 'fn':
            head = data[it['span'][0]:it['decl'][1]].split(b'{', 1)[0]
            if b'#[test]' in head:
                out.append(it)
    return out


def check_addition(label, base, new, c_data, c_its, proto_names, full_closure, proto_same, proto_blob,
                   other_files, outside, names=('base', 'new')):
    """The fourteen checks for one addition: `new` is `base` plus the addition, `c_data` the file at C.
    Git-derived facts (protocol.rs unchanged, the other compiled files, the paths that differ outside
    docs/evidence/) are passed in, so that negative_controls.py can run the checks on synthetic input."""
    lo, hi = len(base), len(new)
    checks = []

    def check(cid, ok, evidence):
        checks.append({'check': cid, 'result': 'PASS' if ok else 'FAIL', 'evidence': evidence})

    # the region at C must be the same bytes as at the witness commit
    same_at_c = c_data[lo:hi] == new[lo:hi] and c_data.startswith(new[:hi])
    n_its, n_toks, n_offs = R.extract(new)
    b_its, b_toks, b_offs = R.extract(base)
    region_items = [it for it in n_its if it['span'][0] >= lo and it['span'][1] <= hi]
    # N01
    prefix = new.startswith(base)
    iso = S.lexically_isolated(base, new)
    first_item_starts_region = bool(region_items) and region_items[0]['span'][0] >= lo
    check('N01 pure byte append, lexically isolated, at the end of the file', prefix and iso and same_at_c
          and first_item_starts_region and base.endswith(b'}\n'),
          {'base': names[0], 'new': names[1], 'base_bytes': lo, 'new_bytes': hi,
           'appended_sha256': K.sha256(new[lo:]), 'byte_prefix': prefix, 'lexically_isolated': iso,
           'region_bytes_identical_at_C': same_at_c, 'base_ends_with': base[-2:].decode()})
    # N02
    b_idx = S.index(b_its)
    n_idx = S.index(n_its)
    changed = [k for k, it in b_idx.items()
               if k not in n_idx or n_idx[k]['decl'] != it['decl'] or n_idx[k]['span'] != it['span']
               or n_idx[k]['decl_sha256'] != it['decl_sha256'] or n_idx[k]['span_sha256'] != it['span_sha256']]
    check('N02 no earlier item changed', not changed, {'earlier_items': len(b_idx), 'changed': changed})
    # N03-N08 token scans
    r_ids = R.identifiers(n_toks, n_offs, lo, hi)
    sites = keyword_sites(n_toks, n_offs, lo, hi, region_items, new)
    impl_trait = [s for s in sites if s['keyword'] in ('impl', 'trait')]
    check('N03 no impl or trait at any depth', not impl_trait, {'sites': impl_trait})
    defined = sorted({it['name'] for it in region_items})
    pre_ids = R.identifiers(n_toks, n_offs, 0, lo)
    earlier_names = {it['name'] for it in R.flatten(b_its)}
    coll_text = sorted(n for n in defined if n in pre_ids)
    coll_items = sorted(n for n in defined if n in earlier_names)
    coll_proto = sorted(n for n in defined if n in proto_names)
    coll_prelude = sorted(n for n in defined if n in PRELUDE)
    check('N04 no defined name is mentioned earlier or collides', not (coll_text or coll_items or coll_proto or coll_prelude),
          {'defined_top_level': [it['path'] for it in region_items], 'mentioned_in_earlier_text': coll_text,
           'collide_with_earlier_items': coll_items, 'collide_with_protocol_rs_items': coll_proto,
           'collide_with_prelude_or_file_imports': coll_prelude})
    uses = [s for s in sites if s['keyword'] == 'use']
    check('N05 no use declaration', not uses, {'sites': uses})
    macros = [s for s in sites if s['keyword'] == 'macro_rules']
    attrs = attributes(n_toks, n_offs, lo, hi)
    macro_attrs = [a for a in attrs if any(x in a['text'] for x in ('macro_export', 'proc_macro'))]
    check('N06 no macro definition', not macros and not macro_attrs, {'macro_rules': macros, 'macro_attributes': macro_attrs})
    bad_attrs = [a for a in attrs if a['inner'] or a['text'] != '#[test]']
    check('N07 attributes: outer #[test] only', not bad_attrs,
          {'attributes': sorted({a['text'] for a in attrs}), 'count': len(attrs), 'other': bad_attrs})
    hits = {k: len(r_ids[k]) for k in FORBIDDEN_IDENTS if k in r_ids}
    other_kw = [s for s in sites if s['keyword'] in ('static', 'mod', 'extern', 'union')]
    check('N08 no global, static, thread-local or process-global state', not hits and not other_kw,
          {'forbidden_identifier_hits': hits, 'scanned_identifiers': FORBIDDEN_IDENTS, 'keyword_sites': other_kw,
           'statics_in_earlier_model_text': sorted(it['name'] for it in b_its if it['kind'] == 'static'),
           'note': "'static is a lifetime token and is not counted"})
    calls = hook_calls(n_toks, n_offs, lo, hi)
    check('N09 hook unchanged; only hook::take_counts called', proto_same and set(calls) <= {'take_counts'},
          {'protocol_rs_blob': proto_blob, 'protocol_rs_unchanged': proto_same,
           'hook_paths_called': calls})
    cfg_items = [it['path'] for it in R.flatten(b_its)
                 if it['name'] in ('Config', 'default_run', 'BASE', 'DEFAULT_CAP', 'DEFAULT_TIME', 'FULL_CAP',
                                   'FULL_TIME', 'TICKET_LIMIT', 'REFUSAL_LIMIT', 'SYMMETRIC')
                 or (it['kind'] == 'const' and base[it['decl'][0]:it['decl'][1]].split(b'=')[0].strip().endswith(b': Config'))]
    cfg_changed = [p for p in cfg_items if any(k[0] == p for k in changed)]
    region_consts = [(it['name'], new[it['decl'][0]:it['decl'][1]].split(b'=')[0].decode().split(':', 1)[1].strip())
                     for it in region_items if it['kind'] == 'const']
    cfg_new = [n for n, ty in region_consts if ty == 'Config']
    check('N10 configuration constants unchanged; no Config constant defined', not cfg_changed and not cfg_new,
          {'configuration_items_checked': cfg_items, 'changed': cfg_changed, 'region_consts_and_types': region_consts})
    pb = {k: len(r_ids.get(k, [])) for k in ('PROPERTIES', 'BRANCHES')}
    pb_same = all(any(k[0] == 'const ' + n for k in b_idx) and not any(k[0] == 'const ' + n for k in changed)
                  for n in ('PROPERTIES', 'BRANCHES'))
    check('N11 PROPERTIES and BRANCHES unchanged and not mentioned', pb_same and not any(pb.values()),
          {'mentions_in_addition': pb, 'unchanged': pb_same})
    b_tests = [it['name'] for it in test_fns(base)]
    n_tests = [it['name'] for it in test_fns(new)]
    new_tests = [t for t in n_tests if t not in b_tests]
    ign_b, ign_n = ignored_tests(base), ignored_tests(new)
    check('N12 earlier tests unchanged; #[ignore] inventory unchanged',
          all(t in n_tests for t in b_tests) and n_tests[:len(b_tests)] == b_tests and ign_b == ign_n
          and not any(k[0].startswith('fn ') and k[0][3:] in b_tests for k in changed),
          {'earlier_tests': len(b_tests), 'tests_after': len(n_tests), 'added_tests': new_tests,
           'ignored_before': len(ign_b), 'ignored_after': len(ign_n)})
    reg_c = {(it['path'], it['ordinal']) for it in c_its if it['span'][0] >= lo and it['span'][1] <= hi}
    reached = [it['path'] for it in full_closure if (it['path'], it['ordinal']) in reg_c]
    check('N13 not in the closure of the explorers and earlier tests (at C)', not reached,
          {'closure_items': len(full_closure), 'addition_items_reached': reached})
    linkish = {k: len(r_ids[k]) for k in ('include', 'include_str', 'include_bytes', 'env', 'option_env', 'path',
                                          'link', 'no_mangle', 'export_name', 'cfg', 'cfg_attr', 'extern', 'crate')
               if k in r_ids}
    check('N14 crate attributes, features and other compiled files unchanged', all(other_files.values())
          and outside == [K.MODEL] and not linkish,
          {'other_files_unchanged': other_files, 'paths_outside_docs_evidence_changed': outside,
           'include_env_path_link_cfg_hits': linkish})
    # informational: existing items the addition uses (read-only uses)
    existing = sorted(n for n in r_ids if n in earlier_names or n in proto_names)
    verdict = 'NON_INTERFERING' if all(c['result'] == 'PASS' for c in checks) else 'PROOF_REVIEW_BLOCKER'
    return {'addition': label, 'region_at_C_bytes': [lo, hi],
            'region_lines_at_C': [base.count(b'\n') + 1, new.count(b'\n')],
            'checks': checks, 'existing_names_used': existing, 'item_keyword_sites': sites, 'verdict': verdict}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('repo')
    ap.add_argument('--out')
    args = ap.parse_args()
    repo = args.repo
    doc = {'tool': 'witness_check.py', 'mission': 'P3-C11-DR01-D2A1-FINAL-SOURCE-PROOF-READINESS-01',
           'additions': []}
    c_data = K.blob(repo, K.C, K.MODEL)
    w4_data = K.blob(repo, K.W4, K.MODEL)
    doc['W4_to_C_model_identical'] = c_data == w4_data
    proto = K.blob(repo, K.C, K.PROTO)
    p_its, _, _ = R.extract(proto)
    proto_names = {it['name'] for it in R.flatten(p_its)}
    c_its, c_toks, c_offs = R.extract(c_data)
    # closure of everything the earlier (audited) text runs, computed at C
    ext_data = K.blob(repo, K.EXT, K.MODEL)
    roots = (S.B_ROOT_NAMES + S.C_ROOT_NAMES + S.D_ROOT_NAMES
             + [it['name'] for it in c_its if it['decl'][1] <= len(ext_data) and it['kind'] == 'fn'
                and it['name'].startswith('step_')]
             + [it['name'] for it in test_fns(ext_data)])
    full_closure = S.closure(c_data, roots)
    doc['explorer_closure_at_C'] = {'roots': len(roots), 'items': len(full_closure), 'note': TEST_ROOTS_NOTE}
    overall = []
    for label, base_c, new_c in ADDITIONS:
        base = K.blob(repo, base_c, K.MODEL)
        new = K.blob(repo, new_c, K.MODEL)
        other_files = {p: K.blob_id(repo, base_c, p) == K.blob_id(repo, new_c, p) for p in K.FILES if p != K.MODEL}
        tb, tn = K.tree_map(repo, base_c), K.tree_map(repo, new_c)
        outside = sorted(p for p in set(tb) | set(tn)
                         if not p.startswith(K.EVIDENCE_PREFIX) and tb.get(p) != tn.get(p))
        proto_same = K.blob(repo, base_c, K.PROTO) == K.blob(repo, new_c, K.PROTO) == proto
        ent = check_addition(label, base, new, c_data, c_its, proto_names, full_closure, proto_same,
                             K.blob_id(repo, new_c, K.PROTO), other_files, outside, (K.SHORT[base_c], K.SHORT[new_c]))
        ent.update({'base_commit': base_c, 'witness_commit': new_c, 'base_model_blob': K.blob_id(repo, base_c, K.MODEL),
                    'witness_model_blob': K.blob_id(repo, new_c, K.MODEL)})
        overall.append(ent['verdict'])
        doc['additions'].append(ent)
    doc['verdicts'] = {a['addition']: a['verdict'] for a in doc['additions']}
    ok = doc['W4_to_C_model_identical'] and all(v == 'NON_INTERFERING' for v in overall)
    doc['result'] = 'PASS' if ok else 'FAIL'
    text = json.dumps(doc, indent=1, sort_keys=True)
    if args.out:
        open(args.out, 'w').write(text + '\n')
    else:
        print(text)
    print('witness_check: %s %s' % (doc['result'], doc['verdicts']), file=sys.stderr)
    for a in doc['additions']:
        for c in a['checks']:
            if c['result'] != 'PASS':
                print('  FAILED %s %s' % (a['addition'], c['check']), file=sys.stderr)
    sys.exit(0 if ok else 1)


if __name__ == '__main__':
    main()
