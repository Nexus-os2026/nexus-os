#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-PT-FB-EXPERIMENT-01: reads one gc_symmetry_* receipt and its cgroup samples (read-only) and prints
the measured figures plus the derivations the mission allows. Executor-written; it reads only, writes nothing.
  extract-run.py <receipt.txt> <samples.txt> <admissions N>"""
import re, sys
rec, smp, N = sys.argv[1], sys.argv[2], int(sys.argv[3])
t = open(rec).read()
def g(p, s=t, f=0):
    m = re.search(p, s, re.M)
    return m.group(1) if m else None
out = []
P = out.append
P(f"receipt {rec}")
P(f"  outcome line            {g(r'^  outcome: (\S+)')}")
P(f"  OUTCOME (launcher)      {g(r'^OUTCOME: (.*)$')}")
P(f"  exit status             {g(r'^exit status: (\d+)')}")
P(f"  test result             {g(r'^(test result: .*)$')}")
P(f"  bounds                  {g(r'^  bounds: (.*)$')}")
P(f"  actors                  {g(r'^  actors: (.*)$')}")
m = re.search(r'^  states (\d+) \(expanded (\d+)\), transitions (\d+) \(with a fault (\d+)\), states with no enabled step (\d+)', t, re.M)
K, X, T, F, D = (int(v) for v in m.groups()) if m else (None,)*5
ev = re.search(r'\((\d+) evaluations, (\d+) admitted\)', t)
EV, AD = (int(ev.group(1)), int(ev.group(2))) if ev else (None, None)
kf = g(r'held to the exchange; (\d+) expanded keys are their own mirror')
Kf = int(kf) if kf else None
l1 = g(r'^  L1: (.*)$')
nf = re.search(r'HOLDS: (\d+) non-fault edges, no cycle; (\d+) end states', l1 or '')
P(f"  keys discovered K       {K}")
P(f"  keys expanded           {X}")
P(f"  keys unexpanded         {None if K is None else K - X}")
P(f"  transitions (non-ADM)   {T}   with a fault {F}   without {None if T is None else T - F}")
P(f"  admission evaluations   {EV}   admitted {AD}")
P(f"  self-mirrored keys Kf   {Kf}")
P(f"  no enabled step         {D}")
P(f"  L1 line                 {l1}")
if nf and T is not None:
    P(f"  L1 non-fault edges = transitions - fault edges: {int(nf.group(1))} = {T - F} -> {'EQUAL' if int(nf.group(1)) == T - F else 'DIFFERENT'}; end states {nf.group(2)} = no-step {D} -> {'EQUAL' if int(nf.group(2)) == D else 'DIFFERENT'}")
P(f"  elapsed/tables          {g(r'^  (elapsed .*)$')}")
props = re.findall(r'^  ([A-Z]+[0-9]*|KEY): (.*)$', t, re.M)
props = [(p, v) for p, v in props if p not in ('FAIL', 'INCOMPLETE')]
P(f"  property lines          {len(props)}")
for p, v in props:
    P(f"    {p}: {v}")
taken = re.findall(r'^    \[taken    \] (.*)$', t, re.M)
nt = re.findall(r'^    \[NOT TAKEN\] (.*)$', t, re.M)
outside = [x for x in taken if x.endswith('(not in the listed set)')]
P(f"  branches                listed taken {len(taken) - len(outside)}, listed NOT TAKEN {len(nt)}, taken outside the list {len(outside)}")
P(f"  'symmetry inapplicable' {t.count('symmetry inapplicable')}; 'quotient inapplicable' {t.count('quotient inapplicable')}; panicked {t.count('panicked')}; FAIL lines {len(re.findall(r'^  FAIL: ', t, re.M))}; INCOMPLETE lines {len(re.findall(r'^  INCOMPLETE: ', t, re.M))}")
for k, pat in (('Maximum resident set size (KiB)', r'Maximum resident set size \(kbytes\)'), ('Elapsed wall clock', r'Elapsed \(wall clock\) time \(h:mm:ss or m:ss\)'),
               ('User time (s)', r'User time \(seconds\)'), ('System time (s)', r'System time \(seconds\)'), ('Percent of CPU', r'Percent of CPU this job got'),
               ('Swaps', r'Swaps'), ('Exit status', r'Exit status')):
    P(f"  /usr/bin/time {k}: {g(r'^\s*' + pat + r': (.*)$')}")
P(f"  scope after             {g(r'^scope after the run: (.*)$')}")
P(f"  exe sha256 after        {g(r'^executable sha256 after the run: (.*)$')}")
rows = [l.split() for l in open(smp) if re.match(r'^\d\d:\d\d:\d\d \d', l)]
if rows:
    P(f"  samples                 {len(rows)} (2 s cadence) {rows[0][0]}..{rows[-1][0]}")
    P(f"  memory.peak (last)      {rows[-1][2]} B; max memory.current {max(int(r[1]) for r in rows)} B")
    P(f"  max memory.swap.current {max(int(r[3]) for r in rows)} B; oom {rows[-1][4]} oom_kill {rows[-1][5]}")
    P(f"  min host MemAvailable   {min(int(r[6]) for r in rows)} KiB")
    P(f"  cpu.usage_usec (last)   {rows[-1][7]}")
for l in open(smp):
    if l.startswith(('memory.max', 'memory.swap.max', 'RuntimeMaxUSec', 'scope cgroup')):
        P(f"  {l.rstrip()}")
if K is not None and Kf is not None and K == X:
    RQ = 2 * K - Kf
    P("  DERIVED (valid only under S1-S15 [symmetry, GC-SYMMETRY-01 + CLOSURE-PREP-01 E-PT/E-B] and Theorem 1 [ADM-QUOTIENT-01]):")
    P(f"    admission-quotient states |R_Q| = 2K - Kf = 2 x {K} - {Kf} = {RQ}")
    P(f"    original states |R_G| = (N+1)|R_Q| = {N+1} x {RQ} = {(N+1) * RQ}")
    if EV == K:
        P(f"    evaluations = K (one per key), so ADM-Q evaluations = 2 x {EV} - {Kf} = {2 * EV - Kf} = |R_Q|; original ADM edges = N x |R_Q| = {N * RQ}")
    P("  DERIVED bounds only (the fixed-orbit shares of edges, no-step keys and admitted evaluations are not printed):")
    P(f"    ADM-Q non-ADM transitions T_Q in [{T}, {2 * T}]; fault edges F_Q in [{F}, {2 * F}]; no-step in [{D}, {2 * D}]; admitted in [{AD}, {2 * AD}]")
    P(f"    original non-ADM+ADM T_G = (N+1)T_Q + N|R_Q| in [{(N+1) * T + N * RQ}, {(N+1) * 2 * T + N * RQ}]; F_G = (N+1)F_Q in [{(N+1) * F}, {(N+1) * 2 * F}]")
print("\n".join(out))
