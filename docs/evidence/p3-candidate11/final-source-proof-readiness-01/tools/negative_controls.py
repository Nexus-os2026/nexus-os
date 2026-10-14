#!/usr/bin/env python3
"""Sensitivity of the static checkers: injected defects must be detected.

Mission P3-C11-DR01-D2A1-FINAL-SOURCE-PROOF-READINESS-01. Read only: it reads git objects
and runs the checkers on synthetic, in-memory variants. Nothing is written to any file
but stdout.

witness_check: each case appends one defect to the real C4 addition (W1 -> W4) and
requires the verdict PROOF_REVIEW_BLOCKER with the named check failing.
config_identity: each case edits C's model_tests.rs text in memory (a configuration
constant or SYMMETRIC) and requires the named expectation to fail.
A case counts as DETECTED only if the expected check fails; a crash counts as NOT DETECTED.
Usage: negative_controls.py <repo>
"""
import sys
import traceback

import common as K
import config_identity as CI
import rust_items as R
import source_closure as S
import witness_check as W

WITNESS_CASES = [
    ('inherent impl on a model type', 'impl World {\n    fn shadow(&self) {}\n}\n', 'N03'),
    ('trait impl inside a function body', 'fn x() {\n    impl Default for Phase {\n        fn default() -> Phase {\n            todo!()\n        }\n    }\n}\n', 'N03'),
    ('module-level name glob-imported and used earlier (Member)', 'struct Member;\n', 'N04'),
    ('name the earlier text spells (sequence)', 'fn sequence() {}\n', 'N13'),
    ('use declaration', 'use std::collections::HashMap;\n', 'N05'),
    ('macro definition', 'macro_rules! m {\n    () => {};\n}\n', 'N06'),
    ('inner attribute', '#![allow(unused)]\n', 'N07'),
    ('cfg attribute', '#[cfg(test)]\nfn y() {}\n', 'N07'),
    ('static', 'static X: u8 = 0;\n', 'N08'),
    ('thread-local', 'thread_local! {\n    static T: u8 = 0;\n}\n', 'N08'),
    ('environment write', 'fn z() {\n    std::env::set_var("A", "B");\n}\n', 'N08'),
    ('hook install', 'fn h() {\n    hook::install(|_| {});\n}\n', 'N09'),
    ('Config constant', 'const F9: Config = F3;\n', 'N10'),
    ('PROPERTIES mention', 'fn p() -> usize {\n    PROPERTIES.len()\n}\n', 'N11'),
    ('ignored test', '#[test]\n#[ignore]\nfn t() {}\n', 'N12'),
    ('include_str', 'const S: &str = include_str!("x");\n', 'N14'),
]


def witness_cases(repo):
    base = K.blob(repo, K.W1, K.MODEL)
    real = K.blob(repo, K.W4, K.MODEL)
    proto = K.blob(repo, K.C, K.PROTO)
    p_its, _, _ = R.extract(proto)
    proto_names = {it['name'] for it in R.flatten(p_its)}
    others = {p: True for p in K.FILES if p != K.MODEL}
    out = []

    def run(name, new, expect, proto_same=True, other_files=others, outside=(K.MODEL,), base_bytes=base):
        try:
            c_its, _, _ = R.extract(new)
            ext = K.blob(repo, K.EXT, K.MODEL)
            roots = (S.B_ROOT_NAMES + S.C_ROOT_NAMES + S.D_ROOT_NAMES
                     + [it['name'] for it in W.test_fns(ext)])
            closure = S.closure(new, roots)
            ent = W.check_addition('synthetic', base_bytes, new, new, c_its, proto_names, closure, proto_same,
                                   'synthetic', other_files, list(outside), ('W1', 'W4+defect'))
            failed = sorted(c['check'].split()[0] for c in ent['checks'] if c['result'] != 'PASS')
            ok = ent['verdict'] == 'PROOF_REVIEW_BLOCKER' and expect in failed
            out.append((name, expect, 'DETECTED' if ok else 'NOT-DETECTED', ent['verdict'], failed))
        except Exception:
            out.append((name, expect, 'NOT-DETECTED (crash)', '-', [traceback.format_exc().splitlines()[-1]]))

    # the unmodified addition must pass
    c_its, _, _ = R.extract(real)
    roots = S.B_ROOT_NAMES + S.C_ROOT_NAMES + S.D_ROOT_NAMES + [it['name'] for it in W.test_fns(K.blob(repo, K.EXT, K.MODEL))]
    ent = W.check_addition('control', base, real, real, c_its, proto_names, S.closure(real, roots), True, 'x',
                           others, [K.MODEL], ('W1', 'W4'))
    out.append(('unmodified C4 addition (must pass)', '-', 'PASS' if ent['verdict'] == 'NON_INTERFERING' else 'FAIL',
                ent['verdict'], []))
    for name, snippet, expect in WITNESS_CASES:
        run(name, real + b'\n' + snippet.encode(), expect)
    # an in-place edit of the earlier text, not an append
    edited = base.replace(b'const TICKET_LIMIT: u32 = 8;', b'const TICKET_LIMIT: u32 = 9;', 1)
    run('in-place edit of an earlier constant', edited + real[len(base):], 'N02', base_bytes=base)
    run('protocol.rs changed', real, 'N09', proto_same=False)
    run('another compiled file changed (lib.rs)', real, 'N14',
        other_files=dict(others, **{K.LIB: False}), outside=(K.LIB, K.MODEL))
    return out


CONFIG_CASES = [
    ('F-G added to SYMMETRIC', b'const SYMMETRIC: [Config; 8] = [X2, SYM1, F3, SYM_PT, SYM_B, SYM_BI, FPT, FB];',
     b'const SYMMETRIC: [Config; 8] = [X2, SYM1, F3, SYM_PT, SYM_B, SYM_BI, FPT, FG];',
     'SYMMETRIC is exactly'),
    ('F-1 admitted under its own shape', b'const SYMMETRIC: [Config; 8] = [X2, SYM1, F3, SYM_PT, SYM_B, SYM_BI, FPT, FB];',
     b'const SYMMETRIC: [Config; 8] = [X2, SYM1, F1, SYM_PT, SYM_B, SYM_BI, FPT, FB];',
     'F1 is refused'),
    ('F-3 with three faults', b'const F3: Config = Config {\n    name: "F-3",\n    conflict: [true, true],\n    env: true,\n    faults: 2,',
     b'const F3: Config = Config {\n    name: "F-3",\n    conflict: [true, true],\n    env: true,\n    faults: 3,',
     'F3 matches AD-1-R4 N.9'),
    ('F-PT with batch on', b'const FPT: Config = Config {\n    name: "F-PT",\n    blanket_uncertain: true,',
     b'const FPT: Config = Config {\n    name: "F-PT",\n    batch: true,\n    blanket_uncertain: true,',
     'FPT matches AD-1-R4 N.9'),
    ('the full cap raised', b'const FULL_CAP: u64 = 30_000_000;', b'const FULL_CAP: u64 = 60_000_000;',
     'FULL_CAP = 30,000,000'),
    ('the time bound raised', b'const FULL_TIME: Duration = Duration::from_secs(45 * 60);',
     b'const FULL_TIME: Duration = Duration::from_secs(46 * 60);', 'FULL_TIME = 45 minutes'),
    ('an actor added to actors()', b'    all.push(Actor::Adm);\n', b'    all.push(Actor::Adm);\n    all.push(Actor::Adm);\n',
     'fn actors at C is exactly'),
]


def config_cases(repo):
    data = K.blob(repo, K.C, K.MODEL)
    sr2 = K.blob(repo, K.SOURCE_R2, K.MODEL)
    ext = K.blob(repo, K.EXT, K.MODEL)
    out = []
    doc, fails = CI.analyze(data, sr2, ext)
    out.append(('unmodified C (must pass)', '-', 'PASS' if not fails else 'FAIL', fails[:2]))
    for name, old, new, expect in CONFIG_CASES:
        try:
            assert data.count(old) == 1, 'the case text must occur once'
            _, fails = CI.analyze(data.replace(old, new), sr2, ext)
            ok = any(f.startswith(expect) or expect in f for f in fails)
            out.append((name, expect, 'DETECTED' if ok else 'NOT-DETECTED', fails))
        except Exception:
            out.append((name, expect, 'NOT-DETECTED (crash)', [traceback.format_exc().splitlines()[-1]]))
    return out


def main():
    repo = sys.argv[1]
    bad = 0
    print('NEGATIVE CONTROLS — static checkers (P3-C11-DR01-D2A1-FINAL-SOURCE-PROOF-READINESS-01)')
    print('\nwitness_check.check_addition: one defect appended to the real C4 addition (W1 -> W4)')
    for name, expect, verdict, v, failed in witness_cases(repo):
        print('  %-58s expect %-4s -> %-21s verdict %-20s failed %s' % (name, expect, verdict, v, failed))
        bad += verdict not in ('DETECTED', 'PASS')
    print('\nconfig_identity.analyze: one edit to the configuration text of model_tests.rs at C (in memory)')
    for name, expect, verdict, fails in config_cases(repo):
        print('  %-40s expect %-36r -> %-21s %s' % (name, expect, verdict, fails[:3]))
        bad += verdict not in ('DETECTED', 'PASS')
    print('\nnegative_controls: %s (%d not detected or failing controls)' % ('PASS' if not bad else 'FAIL', bad))
    sys.exit(0 if not bad else 1)


if __name__ == '__main__':
    main()
