"""Classify a Cargo.lock delta: removed, added, changed (by name), with who
depends on each affected package in the before and after lockfiles."""
import sys, tomllib, json
def load(p):
    d = tomllib.load(open(p, 'rb'))
    pk = {}
    for p_ in d['package']:
        pk[(p_['name'], p_['version'])] = p_
    return pk
b, a = load(sys.argv[1]), load(sys.argv[2])
def names(pk):
    out = {}
    for (n, v) in pk: out.setdefault(n, set()).add(v)
    return out
bn, an = names(b), names(a)
def dependents(pk, name, ver):
    res = []
    for (n, v), p in pk.items():
        for dep in p.get('dependencies', []):
            parts = dep.split(' ')
            if parts[0] == name and (len(parts) == 1 or parts[1] == ver):
                res.append(f'{n}@{v}')
    return sorted(res)
rows = []
for n in sorted(set(bn) | set(an)):
    bv, av = bn.get(n, set()), an.get(n, set())
    if bv == av: continue
    for v in sorted(bv - av):
        rows.append(dict(kind='removed', name=n, version=v, dependents_before=dependents(b, n, v)))
    for v in sorted(av - bv):
        rows.append(dict(kind='added', name=n, version=v, dependents_after=dependents(a, n, v)))
# checksum changes for same name+version
for k in set(b) & set(a):
    if b[k].get('checksum') != a[k].get('checksum') or b[k].get('source') != a[k].get('source'):
        rows.append(dict(kind='source-or-checksum-changed', name=k[0], version=k[1]))
json.dump(rows, open(sys.argv[3], 'w'), indent=1)
print(f"before {len(b)} packages, after {len(a)}; removed {sum(r['kind']=='removed' for r in rows)}, added {sum(r['kind']=='added' for r in rows)}, other {sum(r['kind'] not in ('removed','added') for r in rows)}")
for r in rows:
    d = r.get('dependents_before') or r.get('dependents_after') or []
    print(f"{r['kind']:8} {r['name']}@{r['version']}  <- {', '.join(d)[:230]}")
