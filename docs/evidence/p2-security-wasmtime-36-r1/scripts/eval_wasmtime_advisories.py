"""Evaluate every RustSec advisory for crate `wasmtime` against a version,
from the advisory TOML front matter ([versions] patched / unaffected)."""
import re, sys, tomllib, pathlib, json
db, version = pathlib.Path(sys.argv[1]), sys.argv[2]
def parse(v):
    v = v.split('-')[0].split('+')[0]; p = [int(x) for x in v.split('.')]
    return tuple(p + [0] * (3 - len(p)))
def match_one(req, ver):
    req = req.strip()
    m = re.match(r'(>=|<=|>|<|=|\^|~)?\s*(.+)', req); op, r = m.group(1) or '^', m.group(2).strip()
    parts = r.split('.'); rv = parse(r)
    if op == '>=': return ver >= rv
    if op == '>': return ver > rv
    if op == '<=': return ver <= rv
    if op == '<': return ver < rv
    if op == '=': return ver == rv
    if op == '~':
        upper = (rv[0], rv[1] + 1, 0) if len(parts) > 1 else (rv[0] + 1, 0, 0)
        return rv <= ver < upper
    # caret
    if rv[0] > 0: upper = (rv[0] + 1, 0, 0)
    elif rv[1] > 0: upper = (0, rv[1] + 1, 0)
    else: upper = (0, 0, rv[2] + 1)
    return rv <= ver < upper
def match(reqs, ver):
    return any(all(match_one(c, ver) for c in r.split(',')) for r in reqs)
ver = parse(version); rows = []
for f in sorted((db / 'crates' / 'wasmtime').glob('*.md')):
    text = f.read_text(); front = text.split('```toml', 1)[1].split('```', 1)[0]
    a = tomllib.loads(front); adv = a['advisory']; vs = a.get('versions', {})
    patched, unaffected = vs.get('patched', []), vs.get('unaffected', [])
    withdrawn = adv.get('withdrawn')
    safe_p, safe_u = match(patched, ver), match(unaffected, ver)
    affected = not (safe_p or safe_u) and not withdrawn
    title = re.search(r'^# (.+)$', text, re.M)
    rows.append(dict(id=adv['id'], date=adv.get('date'), cvss=adv.get('cvss'), informational=adv.get('informational'),
                     withdrawn=withdrawn, patched=patched, unaffected=unaffected,
                     why='patched' if safe_p else 'unaffected' if safe_u else 'withdrawn' if withdrawn else '',
                     affected=affected, title=title.group(1) if title else ''))
print(json.dumps(dict(crate='wasmtime', version=version, advisories=len(rows),
      affected=[r['id'] for r in rows if r['affected']], rows=rows), indent=1))
