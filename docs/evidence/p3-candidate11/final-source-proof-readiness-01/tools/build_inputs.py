#!/usr/bin/env python3
"""Build-input closure from the proof/experimental lineage to Candidate C (mission section 7).

Mission P3-C11-DR01-D2A1-FINAL-SOURCE-PROOF-READINESS-01; basis AD-1-R4 N.5-N.7. Read
only: git objects in, one JSON document out. Nothing is built.

It reports, at every lineage commit and at C:
  - Cargo.lock, rust-toolchain.toml, workspace Cargo.toml, the crate's Cargo.toml and
    .cargo/config.toml: blob, sha256; and a classification of any change;
  - every path outside docs/evidence/ that differs (the whole compiled input set lies
    there: the crate, its path dependencies, the workspace and the lock file);
  - parsed facts at C: toolchain channel; every [profile.*] key, and that no profile,
    config or [env] entry sets a panic strategy or rustflags; the crate's features
    (none declared), lints, build script (none) and the declared features of its
    dependencies; crate attributes (lib.rs inner attributes), the module path to the
    ledger with its cfg attributes, and every cfg attribute in protocol.rs and
    model_tests.rs;
  - the crate's path-dependency closure, its build scripts, and every include!/
    include_str!/include_bytes! in it, so that nothing under docs/ is compiled in.
Classification vocabulary for a changed input: SEMANTICALLY_RELEVANT,
SEMANTICALLY_IRRELEVANT_WITH_REASON, ARCHITECT_REVIEW_REQUIRED. The executor does not
approve a changed input; with no changed input, every row is IDENTICAL.
"""
import argparse
import json
import posixpath
import re
import sys
import tomllib

import common as K
import rust_items as R

CRATE_DIR = 'crates/nexus-governed-control'
EXPECT = {  # AD-1-R4 N.5/N.6 identities at 42dd1bd1 (informational there; checked here)
    K.TOOLCHAIN: '726005b2', K.LOCK: '1af53a60', K.CRATE_TOML: '2543c745', K.WS_TOML: '2c558a44',
    K.CARGO_CFG: '05a42b50', K.UNIT: '55dc6640', K.LEDGER: 'd872b908', K.LIB: 'b806ed57', K.PROTO: 'a2df2841'}
FAILURES = []


def expect(cond, what):
    if not cond:
        FAILURES.append(what)
    return bool(cond)


def cfg_attrs(data):
    toks = R.lex(data.decode())
    code = [t for t in toks if t.kind not in ('ws', 'line_comment', 'block_comment')]
    out = []
    for i, t in enumerate(code):
        if t.text == '#':
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
            text = ''.join(x.text for x in code[i:k])
            line = data[:t.start].count(b'\n') + 1 if data.isascii() else None
            out.append({'inner': inner, 'text': text, 'line': line})
    return out


def path_dep_closure(repo, commit, start_dir, ws):
    """Directories of the crate and its (non-dev) path dependencies, recursively."""
    ws_deps = ws.get('workspace', {}).get('dependencies', {})
    seen, order, work = set(), [], [start_dir]
    root = True
    while work:
        d = work.pop()
        if d in seen:
            continue
        seen.add(d)
        order.append(d)
        man = tomllib.loads(K.blob(repo, commit, d + '/Cargo.toml').decode())
        tables = [man.get('dependencies', {}), man.get('build-dependencies', {})]
        if root:
            tables.append(man.get('dev-dependencies', {}))
        for tgt in man.get('target', {}).values():
            tables.append(tgt.get('dependencies', {}))
            tables.append(tgt.get('build-dependencies', {}))
        for table in tables:
            for name, spec in table.items():
                if isinstance(spec, dict) and spec.get('workspace'):
                    spec = ws_deps.get(name, {})
                    if isinstance(spec, dict) and 'path' in spec:
                        work.append(posixpath.normpath(spec['path']))
                    continue
                if isinstance(spec, dict) and 'path' in spec:
                    work.append(posixpath.normpath(posixpath.join(d, spec['path'])))
        root = False
    return sorted(order)


INCLUDE = re.compile(rb'\b(include|include_str|include_bytes)!\s*\(\s*(r#*)?"([^"]*)"')


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('repo')
    ap.add_argument('--out')
    args = ap.parse_args()
    repo = args.repo
    doc = {'tool': 'build_inputs.py', 'mission': 'P3-C11-DR01-D2A1-FINAL-SOURCE-PROOF-READINESS-01'}

    # identities across the lineage
    commits = [K.SOURCE_R2, K.ADM, K.GC, K.EXT, K.W1, K.W4, K.B, K.C_PARENT, K.C]
    rows = []
    for p in K.BUILD_FILES + [K.LIB, K.LEDGER, K.PROTO, K.UNIT]:
        ids = {}
        for c in commits:
            data = K.blob(repo, c, p)
            ids[K.SHORT[c]] = {'blob': K.blob_id(repo, c, p), 'sha256': K.sha256(data), 'bytes': len(data)}
        distinct = {v['blob'] for v in ids.values()}
        status = 'IDENTICAL' if len(distinct) == 1 else 'ARCHITECT_REVIEW_REQUIRED'
        expect(len(distinct) == 1, '%s identical across the lineage' % p)
        expect(ids['C']['blob'].startswith(EXPECT[p]), '%s blob at C starts %s (AD-1-R4 N)' % (p, EXPECT[p]))
        rows.append({'input': p, 'identities': ids, 'distinct_blobs': len(distinct), 'classification': status})
    doc['inputs'] = rows

    # the whole compiled input set lies outside docs/evidence/
    ctree = K.tree_map(repo, K.C)
    outside = {}
    for c in commits[:-1]:
        t = K.tree_map(repo, c)
        outside[K.SHORT[c]] = sorted(p for p in set(t) | set(ctree)
                                     if not p.startswith(K.EVIDENCE_PREFIX) and t.get(p) != ctree.get(p))
    doc['paths_outside_docs_evidence_differing_from_C'] = outside
    for c in [K.SOURCE_R2, K.ADM, K.GC, K.EXT, K.W1]:
        expect(outside[K.SHORT[c]] == [K.MODEL], 'only model_tests.rs differs outside docs/evidence (%s..C)' % K.SHORT[c])
    for c in [K.W4, K.B, K.C_PARENT]:
        expect(outside[K.SHORT[c]] == [], 'nothing differs outside docs/evidence (%s..C)' % K.SHORT[c])

    # parsed facts at C
    tc = tomllib.loads(K.blob(repo, K.C, K.TOOLCHAIN).decode())
    ws = tomllib.loads(K.blob(repo, K.C, K.WS_TOML).decode())
    crate = tomllib.loads(K.blob(repo, K.C, K.CRATE_TOML).decode())
    cfg = tomllib.loads(K.blob(repo, K.C, K.CARGO_CFG).decode())
    profiles = ws.get('profile', {})
    doc['toolchain'] = tc
    doc['workspace_profiles'] = profiles
    doc['cargo_config'] = cfg
    expect(tc.get('toolchain', {}).get('channel') == '1.94.0', 'toolchain channel 1.94.0')
    rel = profiles.get('release', {})
    expect(rel.get('opt-level') == 3 and rel.get('lto') == 'thin' and rel.get('codegen-units') == 16
           and rel.get('strip') == 'symbols', 'release profile opt-level 3, lto thin, codegen-units 16, strip symbols')
    panic_keys = [(name, prof['panic']) for name, prof in profiles.items() if 'panic' in prof]
    expect(not panic_keys, 'no workspace profile sets panic')
    expect('profile' not in crate, 'the crate manifest has no profile table')
    expect(set(cfg) == {'build', 'env'} and cfg['build'] == {'jobs': 8}
           and cfg['env'] == {'CARGO_PROFILE_TEST_DEBUG': '1'},
           '.cargo/config.toml is exactly [build] jobs = 8 and [env] CARGO_PROFILE_TEST_DEBUG = "1"')
    env_panic = [k for k in cfg.get('env', {}) if 'PANIC' in k or 'RUSTFLAGS' in k]
    expect(not env_panic, 'no [env] entry sets a panic strategy or rustflags')
    expect('features' not in crate, 'the crate declares no features')
    expect('build' not in crate.get('package', {}), 'the crate declares no build script')
    expect('lints' not in crate, 'the crate has no [lints] table (workspace lints not inherited)')
    deps = {}
    for tname, table in [('dependencies', crate.get('dependencies', {})),
                         ('dev-dependencies', crate.get('dev-dependencies', {}))] + \
            [('target %s dependencies' % k, v.get('dependencies', {})) for k, v in crate.get('target', {}).items()]:
        for n, spec in table.items():
            deps['%s %s' % (tname, n)] = spec
    doc['crate_dependency_declarations'] = deps
    doc['panic_strategy'] = {
        'profile_panic_keys': panic_keys,
        'config_or_env_panic_or_rustflags': env_panic,
        'note': ('No manifest, profile, config or [env] entry sets a panic strategy, so it is the default, unwind '
                 '(AD-1-R4 N.6), on which the explorers\' catch_unwind depends. Cargo documents that test '
                 'targets ignore a profile panic setting; that documentation is cited, not verified here. '
                 'RUSTFLAGS or CARGO_* in the build environment are execution-time inputs (N.7), pending.')}

    # crate attributes and cfgs on the path to the model
    lib = K.blob(repo, K.C, K.LIB)
    ledger = K.blob(repo, K.C, K.LEDGER)
    proto = K.blob(repo, K.C, K.PROTO)
    model = K.blob(repo, K.C, K.MODEL)
    lib_attrs = cfg_attrs(lib)
    doc['lib_rs_inner_attributes'] = [a['text'] for a in lib_attrs if a['inner']]
    doc['lib_rs_ledger_declaration'] = [l.decode() for l in lib.splitlines() if b'mod ledger' in l]
    doc['ledger_rs_attributes'] = [a['text'] for a in cfg_attrs(ledger)]
    doc['ledger_rs_protocol_declaration'] = [l.decode() for l in ledger.splitlines() if b'mod protocol' in l]
    doc['protocol_rs_cfg_attributes'] = [a for a in cfg_attrs(proto) if 'cfg' in a['text']]
    m_attrs = cfg_attrs(model)
    doc['model_tests_rs_cfg_attributes'] = [a for a in m_attrs if 'cfg' in a['text']]
    doc['model_tests_rs_inner_attributes'] = [a for a in m_attrs if a['inner']]
    expect(doc['lib_rs_inner_attributes'] == ['#![deny(unsafe_code)]'], 'lib.rs inner attributes: #![deny(unsafe_code)] only')
    expect(doc['lib_rs_ledger_declaration'] == ['pub(crate) mod ledger;'], 'lib.rs declares pub(crate) mod ledger;')
    expect(doc['ledger_rs_attributes'] == ['#[cfg_attr(not(test),allow(dead_code))]'],
           'ledger.rs: one attribute, cfg_attr(not(test), allow(dead_code)) on mod protocol')
    expect(not doc['model_tests_rs_cfg_attributes'] and not doc['model_tests_rs_inner_attributes'],
           'model_tests.rs has no cfg/cfg_attr and no inner attribute')
    p_cfg = [a['text'] for a in doc['protocol_rs_cfg_attributes']]
    lint_only = ('#![cfg_attr(not(test),deny(clippy::indexing_slicing,clippy::arithmetic_side_effects,'
                 'clippy::unwrap_used,clippy::expect_used,clippy::panic))]')
    # Six #[cfg(test)] (the two test modules and the hook's module and three report
    # calls) and one inner, lint-only cfg_attr that is inactive in a test build.
    expect(sorted(p_cfg) == sorted(['#[cfg(test)]'] * 6 + [lint_only]),
           'protocol.rs cfg attributes: six #[cfg(test)] and one lint-only #![cfg_attr(not(test), deny(clippy::...))]')
    doc['protocol_rs_cfg_test_count'] = p_cfg.count('#[cfg(test)]')
    doc['protocol_rs_lint_only_cfg_attr'] = {
        'text': lint_only, 'active_in_test_build': False,
        'note': 'a lint level (deny of five Clippy lints) in a non-test build only; lint levels change no generated code'}

    # path-dependency closure, its build scripts and include macros
    closure = path_dep_closure(repo, K.C, CRATE_DIR, ws)
    doc['path_dependency_closure'] = closure
    files = [p for p in ctree if any(p.startswith(d + '/') for d in closure)]
    builds = sorted(p for p in files if p.endswith('/build.rs') and p.count('/') == next(
        d for d in closure if p.startswith(d + '/')).count('/') + 1)
    doc['build_scripts'] = builds
    includes = []
    for p in sorted(files):
        if not p.endswith('.rs'):
            continue
        data = K.blob(repo, K.C, p)
        for m in INCLUDE.finditer(data):
            target = posixpath.normpath(posixpath.join(posixpath.dirname(p), m.group(3).decode()))
            includes.append({'file': p, 'macro': m.group(1).decode(), 'argument': m.group(3).decode(),
                             'resolves_to': target})
    doc['include_macros_in_closure'] = includes
    expect(all(not i['resolves_to'].startswith('docs/') for i in includes), 'no include macro in the closure reaches docs/')
    expect(all(any(i['resolves_to'].startswith(d + '/') for d in closure) for i in includes),
           'every include macro in the closure resolves inside the closure')
    for b in builds:
        text = K.blob(repo, K.C, b)
        doc.setdefault('build_script_reads', {})[b] = {
            'mentions_docs': b'docs' in text, 'sha256': K.sha256(text),
            'rerun_if_changed': [l.decode().strip() for l in text.splitlines() if b'rerun-if-changed' in l]}
        expect(b'docs' not in text, '%s does not mention docs' % b)
    doc['changed_inputs'] = [r['input'] for r in rows if r['classification'] != 'IDENTICAL']
    doc['execution_pending'] = ['the compiler that actually builds the executable (rustc --version) (N.5)',
                                'the build command, environment, RUSTFLAGS/CARGO_*/RUSTC_WRAPPER, effective profile '
                                'and enabled cfg/features as the build reports them (N.7)',
                                'the pinned executable sha256 before and after (N.14)']
    doc['expectation_failures'] = FAILURES
    doc['result'] = 'PASS' if not FAILURES else 'FAIL'
    text = json.dumps(doc, indent=1, sort_keys=True)
    if args.out:
        open(args.out, 'w').write(text + '\n')
    else:
        print(text)
    print('build_inputs: %s (%d expectation failures)' % (doc['result'], len(FAILURES)), file=sys.stderr)
    for f in FAILURES:
        print('  FAILED: ' + f, file=sys.stderr)
    sys.exit(0 if not FAILURES else 1)


if __name__ == '__main__':
    main()
