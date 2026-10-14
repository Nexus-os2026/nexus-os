#!/usr/bin/env python3
"""Configuration identity at Candidate C (mission section 9), statically. No execution.

Mission P3-C11-DR01-D2A1-FINAL-SOURCE-PROOF-READINESS-01; basis AD-1-R4 N.9, N.10, E.1, K.

It evaluates every top-level `const NAME: Config = ...;` of model_tests.rs at C from its
tokens (struct literals, struct update `..BASE`, `default_run(..)` evaluated from its own
body, the cap and time constants), then checks:
  - F3, FPT and FB field by field against AD-1-R4 N.9, every field of `Config` covered;
  - the 30,000,000-key cap and the 45-minute time bound of each;
  - the actor lists of AD-1-R4 N.10, from a transliteration of `actors` that is bound to
    the exact bytes of `fn actors` (a different text fails the check);
  - the SYMMETRIC allow-list's exact membership, and, by a static model of
    `assert_symmetric` bound to its exact bytes, which named configurations it admits:
    exactly the eight members, each by its own shape; every other configuration refused;
  - the three required runs' tests call `explore_symmetric` with F3, FPT and FB, the
    admission observer `step_adm` and `Mode::Quotient`, with no cap or time override;
  - the explorers' cap and time checks are present as AD-1-R4 K states them.
The runtime half of assert_symmetric (the initial state is its own mirror) is a runtime
guard, not evaluated here.
"""
import argparse
import json
import sys

import common as K
import rust_items as R

# sha256 of the exact declaration bytes the transliterations below were written against.
BOUND = {
    'fn actors': None,          # filled from SOURCE-R2 and required equal at C (see main)
    'fn shape': None,
    'fn assert_symmetric': None,
    'fn default_run': None,
}
N9 = {  # AD-1-R4 N.9, F3 field by field
    'facts': 2, 'conflict': [True, True], 'env': True, 'batch': False, 'faults': 2, 'admissions': 2, 'gating': 2,
    'gating_conflict': False, 'acks': 0, 'ack_attempts': 0, 'malformed': False, 'cell_constraint': False,
    'worker_unwinds': False, 'drop': False, 'replays': 0, 'blanket_uncertain': False, 'l1': True,
    'cap': 30_000_000, 'time': ('Duration', 45 * 60)}
N10 = {'F3': ['Fw(0)', 'Fw(1)', 'Se', 'Wk', 'Gc(0)', 'Gc(1)', 'Adm', 'Env'],
       'FPT': ['Fw(0)', 'Fw(1)', 'Se', 'Wk', 'Gc(0)', 'Gc(1)', 'Adm', 'Env'],
       'FB': ['Fw(0)', 'Se', 'Wk', 'Gc(0)', 'Gc(1)', 'Adm', 'Env']}
SYMMETRIC_EXPECTED = ['X2', 'SYM1', 'F3', 'SYM_PT', 'SYM_B', 'SYM_BI', 'FPT', 'FB']
SYMMETRIC_NAMES = ['X-2', 'SYM-1', 'F-3', 'SYM-PT', 'SYM-B', 'SYM-BI', 'F-PT', 'F-B']
FAILURES = []


def expect(cond, what):
    if not cond:
        FAILURES.append(what)
    return bool(cond)


class Parser:
    def __init__(self, toks, env):
        self.t = [t for t in toks if t.kind not in ('ws', 'line_comment', 'block_comment')]
        self.i = 0
        self.env = env

    def peek(self, k=0):
        return self.t[self.i + k].text if self.i + k < len(self.t) else None

    def take(self, text=None):
        tok = self.t[self.i]
        if text is not None and tok.text != text:
            raise ValueError('expected %r, found %r' % (text, tok.text))
        self.i += 1
        return tok

    def expr(self):
        left = self.term()
        while self.peek() in ('*', '+'):
            op = self.take().text
            right = self.term()
            if isinstance(left, tuple) and left[0] == 'Duration' and isinstance(right, tuple):
                left = ('Duration', left[1] + right[1] if op == '+' else None)
            else:
                left = left * right if op == '*' else left + right
        return left

    def term(self):
        tok = self.take()
        if tok.kind == 'num':
            return int(tok.text.replace('_', '').rstrip('u64').rstrip('u8') or '0')
        if tok.kind == 'str':
            return tok.text[1:-1]
        if tok.text in ('true', 'false'):
            return tok.text == 'true'
        if tok.text == '[':
            vals = []
            while self.peek() != ']':
                vals.append(self.expr())
                if self.peek() == ',':
                    self.take(',')
            self.take(']')
            return vals
        if tok.text == 'Duration' and self.peek() == ':':
            self.take(':'), self.take(':')
            fn = self.take().text
            if fn == 'ZERO':
                return ('Duration', 0)
            self.take('(')
            v = self.expr()
            self.take(')')
            assert fn == 'from_secs', fn
            return ('Duration', v)
        if tok.text == 'Config' and self.peek() == '{':
            return self.struct_lit()
        if tok.text == 'default_run' and self.peek() == '(':
            self.take('(')
            arg = self.expr()
            self.take(')')
            return self.env['default_run'](arg)
        if tok.kind == 'ident':
            if tok.text not in self.env:
                raise ValueError('unknown name %s' % tok.text)
            return self.env[tok.text]
        raise ValueError('unexpected %r' % tok.text)

    def struct_lit(self):
        self.take('{')
        fields, base = {}, None
        while self.peek() != '}':
            if self.peek() == '.' and self.peek(1) == '.':
                self.take('.'), self.take('.')
                base = self.expr()
            else:
                name = self.take().text
                self.take(':')
                fields[name] = self.expr()
            if self.peek() == ',':
                self.take(',')
        self.take('}')
        out = dict(base) if base is not None else {}
        out.update(fields)
        return out


def decl_text(data, it):
    return data[it['decl'][0]:it['decl'][1]].decode()


def analyze(data, sr2, ext):
    """The whole check on model_tests.rs bytes at C, SOURCE-R2 and EXT; returns (doc, failures)."""
    del FAILURES[:]
    doc = {'tool': 'config_identity.py', 'mission': 'P3-C11-DR01-D2A1-FINAL-SOURCE-PROOF-READINESS-01'}
    its, _, _ = R.extract(data)
    by = {it['path']: it for it in its}
    sr2_its, _, _ = R.extract(sr2)
    ext_its, _, _ = R.extract(ext)
    sr2_by = {it['path']: it for it in sr2_its}
    ext_by = {it['path']: it for it in ext_its}
    # bind each transliterated item to its audited bytes (SOURCE-R2 for the model, EXT for SQ)
    bound = {}
    for path, src in (('fn actors', sr2_by), ('fn default_run', sr2_by), ('fn shape', ext_by),
                      ('fn assert_symmetric', ext_by), ('struct Config', sr2_by)):
        bound[path] = {'audited_decl_sha256': src[path]['decl_sha256'], 'c_decl_sha256': by[path]['decl_sha256'],
                       'c_lines': by[path]['lines']}
        expect(src[path]['decl_sha256'] == by[path]['decl_sha256'], '%s at C has its audited bytes' % path)
    doc['bound_items'] = bound

    # struct Config fields
    cfg_text = decl_text(data, by['struct Config'])
    ctoks = [t for t in R.lex(cfg_text) if t.kind not in ('ws', 'line_comment', 'block_comment')]
    fields = []
    depth = 0
    for i, t in enumerate(ctoks):
        if t.text == '{':
            depth += 1
        elif t.text == '}':
            depth -= 1
        elif depth == 1 and t.kind == 'ident' and ctoks[i + 1].text == ':' and ctoks[i - 1].text in ('{', ','):
            fields.append(t.text)
    doc['config_fields'] = fields
    expect(set(fields) == set(N9) | {'name'} and len(fields) == len(N9) + 1,
           'Config has exactly the fields of AD-1-R4 N.9 plus name')

    # evaluate constants in file order
    env = {}

    def default_run(cfg):
        body = decl_text(data, by['fn default_run'])
        inner = body[body.index('{') + 1:body.rindex('}')]
        p = Parser(R.lex(inner), dict(env, config=cfg))
        return p.expr()

    env['default_run'] = default_run
    consts = {}
    order = []
    for it in its:
        if it['kind'] != 'const':
            continue
        text = decl_text(data, it)
        head, _, rhs = text.partition('=')
        ty = head.split(':', 1)[1].strip()
        if ty not in ('Config', 'u64', 'Duration') and not ty.startswith('[Config;'):
            continue
        rhs = rhs.strip().rstrip(';')
        if ty.startswith('[Config;'):
            names = [t.text for t in R.lex(rhs) if t.kind == 'ident']
            consts[it['name']] = ('array', names)
            continue
        val = Parser(R.lex(rhs), env).expr()
        env[it['name']] = val
        if ty == 'Config':
            consts[it['name']] = val
            order.append(it['name'])
        else:
            consts[it['name']] = val
    doc['constants'] = {k: v for k, v in consts.items() if not (isinstance(v, tuple) and v[0] == 'array')}
    doc['config_order'] = order
    expect(consts['FULL_CAP'] == 30_000_000, 'FULL_CAP = 30,000,000')
    expect(consts['FULL_TIME'] == ('Duration', 2700), 'FULL_TIME = 45 minutes')
    expect(consts['DEFAULT_CAP'] == 250_000 and consts['DEFAULT_TIME'] == ('Duration', 180), 'default caps 250,000 and 180 s')

    # N.9
    n9 = {}
    for name, extra in (('F3', {}), ('FPT', {'blanket_uncertain': True}), ('FB', {'batch': True})):
        want = dict(N9, **extra)
        got = consts[name]
        rows = []
        for f in fields:
            if f == 'name':
                continue
            rows.append({'field': f, 'expected': want[f], 'actual': got[f], 'match': got[f] == want[f]})
        n9[name] = {'name': got['name'], 'rows': rows, 'all_match': all(r['match'] for r in rows)}
        expect(n9[name]['all_match'], '%s matches AD-1-R4 N.9 field by field' % name)
    expect(n9['F3']['name'] == 'F-3' and n9['FPT']['name'] == 'F-PT' and n9['FB']['name'] == 'F-B', 'names F-3, F-PT, F-B')
    doc['n9'] = n9

    # N.10: actors(), transliterated from its bound text
    def actors(cfg):
        writers = 1 if cfg['batch'] else cfg['facts']
        out = ['Fw(%d)' % i for i in range(writers)] + ['Se', 'Wk']
        out += ['Gc(%d)' % g for g in range(cfg['gating'])]
        out += ['Ack(%d)' % a for a in range(cfg['acks'])]
        out.append('Adm')
        if cfg['env']:
            out.append('Env')
        if cfg['replays'] > 0:
            out.append('Replay')
        return out

    expected_actors_text = (
        'fn actors(config: &Config) -> Vec<Actor> {\n'
        '    let writers = if config.batch { 1 } else { config.facts };\n'
        '    let mut all: Vec<Actor> = (0..writers).map(Actor::Fw).collect();\n'
        '    all.push(Actor::Se);\n'
        '    all.push(Actor::Wk);\n'
        '    all.extend((0..config.gating).map(Actor::Gc));\n'
        '    all.extend((0..config.acks).map(Actor::Ack));\n'
        '    all.push(Actor::Adm);\n'
        '    if config.env {\n'
        '        all.push(Actor::Env);\n'
        '    }\n'
        '    if config.replays > 0 {\n'
        '        all.push(Actor::Replay);\n'
        '    }\n'
        '    all\n'
        '}')
    expect(decl_text(data, by['fn actors']) == expected_actors_text,
           'fn actors at C is exactly the text the transliteration was written against')
    n10 = {name: {'derived': actors(consts[name]), 'expected': N10[name]} for name in N10}
    for name in N10:
        n10[name]['match'] = n10[name]['derived'] == n10[name]['expected']
        expect(n10[name]['match'], '%s actors equal AD-1-R4 N.10' % name)
    doc['n10'] = n10

    # SYMMETRIC and a static model of assert_symmetric
    sym = consts['SYMMETRIC'][1]
    expect(sym == SYMMETRIC_EXPECTED, 'SYMMETRIC is exactly [X2, SYM1, F3, SYM_PT, SYM_B, SYM_BI, FPT, FB]')
    sym_head = decl_text(data, by['const SYMMETRIC']).split('=')[0]
    expect(sym_head.strip() == 'const SYMMETRIC: [Config; 8]', 'SYMMETRIC is declared [Config; 8]')
    members = sym
    doc['symmetric'] = {'members': members, 'names': [consts[m]['name'] for m in members]}
    expect(doc['symmetric']['names'] == SYMMETRIC_NAMES, 'SYMMETRIC names in order')
    expected_assert = decl_text(ext, ext_by['fn assert_symmetric'])
    for frag in ('config.gating == 2', '!config.gating_conflict',
                 'config.acks == 0 && config.ack_attempts == 0 && !config.malformed',
                 'SYMMETRIC.iter().find(|known| shape(known) == shape(config))',
                 'config.cap <= known.cap && config.time <= known.time',
                 'mirror(config, &init) == init'):
        expect(frag in expected_assert, 'assert_symmetric contains %r' % frag)
    shape_text = decl_text(data, by['fn shape'])
    expect('cap: 0' in shape_text and 'time: Duration::ZERO' in shape_text and '..*config' in shape_text
           and '{:?}' in shape_text, 'shape is Debug of the configuration with cap 0 and time zero')

    def shape(cfg):
        return tuple((f, json.dumps(cfg[f])) for f in fields if f not in ('cap', 'time'))

    def admitted(cfg):
        if cfg['gating'] != 2 or cfg['gating_conflict']:
            return False, 'refused: callers'
        if cfg['acks'] or cfg['ack_attempts'] or cfg['malformed']:
            return False, 'refused: acknowledgements'
        known = [m for m in members if shape(consts[m]) == shape(cfg)]
        if not known:
            return False, 'refused: shape not in SYMMETRIC'
        k = consts[known[0]]
        if not (cfg['cap'] <= k['cap'] and cfg['time'][1] <= k['time'][1]):
            return False, 'refused: raised cap'
        return True, 'admitted as %s (subject to the runtime initial-state mirror check)' % known[0]

    table = []
    for name in order:
        ok, why = admitted(consts[name])
        table.append({'const': name, 'name': consts[name]['name'], 'admitted': ok, 'why': why,
                      'gating': consts[name]['gating'], 'acks': consts[name]['acks'],
                      'gating_conflict': consts[name]['gating_conflict'], 'cap': consts[name]['cap'],
                      'time_s': consts[name]['time'][1]})
    doc['admission_table'] = table
    adm = sorted(r['const'] for r in table if r['admitted'])
    expect(adm == sorted(SYMMETRIC_EXPECTED), 'exactly the eight SYMMETRIC members are admitted')
    expect(all(r['why'] == 'admitted as %s (subject to the runtime initial-state mirror check)' % r['const']
               for r in table if r['admitted']), 'each admitted configuration matches its own shape')
    shapes = [shape(consts[m]) for m in members]
    expect(len(set(shapes)) == len(shapes), 'the eight SYMMETRIC shapes are distinct')
    for forbidden in ('F1', 'F2', 'F4', 'F5', 'F6', 'F7', 'F8', 'FB2', 'FG', 'FW', 'FD', 'FR', 'X1', 'X3', 'X4', 'X5'):
        expect(not [r for r in table if r['const'] == forbidden][0]['admitted'], '%s is refused' % forbidden)

    # the three required runs
    calls = {}
    for test, want in (('fn gc_symmetry_f3', 'explore_symmetric(&F3, step_adm, Mode::Quotient)'),
                       ('fn gc_symmetry_fpt', 'symmetric_full_exploration(&FPT);'),
                       ('fn gc_symmetry_fb', 'symmetric_full_exploration(&FB);')):
        text = decl_text(data, by[test])
        head = data[by[test]['span'][0]:by[test]['decl'][0]].decode() + text.split('{', 1)[0]
        calls[test] = {'contains': want, 'ok': want in text, 'lines': by[test]['lines'],
                       'ignored': '#[ignore' in data[by[test]['span'][0]:by[test]['decl'][1]].decode().split('fn ', 1)[0]}
        expect(want in text, '%s calls %s' % (test, want))
    sfe = decl_text(data, by['fn symmetric_full_exploration'])
    expect('explore_symmetric(config, step_adm, Mode::Quotient)' in sfe,
           'symmetric_full_exploration calls explore_symmetric(config, step_adm, Mode::Quotient)')
    calls['fn symmetric_full_exploration'] = {'contains': 'explore_symmetric(config, step_adm, Mode::Quotient)',
                                              'ok': True, 'lines': by['fn symmetric_full_exploration']['lines']}
    doc['required_runs'] = calls

    # the explorers' cap and time checks (AD-1-R4 K)
    caps = {}
    for fn in ('fn explore', 'fn explore_experimental', 'fn explore_symmetric_kept'):
        text = decl_text(data, by[fn])
        caps[fn] = {'cap_check': 'visited.len() as u64 > config.cap' in text,
                    'time_check': 'start.elapsed() > config.time' in text, 'lines': by[fn]['lines']}
        expect(caps[fn]['cap_check'] and caps[fn]['time_check'], '%s checks cap and time' % fn)
    doc['explorer_caps'] = caps
    doc['runtime_only'] = ['assert_symmetric: the initial state is its own mirror (evaluated by the run, not here)',
                           'the printed bounds and actor lines (N.10) are execution output; the static derivation above is not a substitute']
    doc['expectation_failures'] = list(FAILURES)
    doc['result'] = 'PASS' if not FAILURES else 'FAIL'
    return doc, list(FAILURES)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('repo')
    ap.add_argument('--out')
    args = ap.parse_args()
    repo = args.repo
    doc, failures = analyze(K.blob(repo, K.C, K.MODEL), K.blob(repo, K.SOURCE_R2, K.MODEL),
                            K.blob(repo, K.EXT, K.MODEL))
    text = json.dumps(doc, indent=1, sort_keys=True)
    if args.out:
        open(args.out, 'w').write(text + '\n')
    else:
        print(text)
    print('config_identity: %s (%d expectation failures)' % (doc['result'], len(failures)), file=sys.stderr)
    for f in failures:
        print('  FAILED: ' + f, file=sys.stderr)
    sys.exit(0 if not failures else 1)


if __name__ == '__main__':
    main()
