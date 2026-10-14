"""The proof-precondition matrix rows (mission section 8): authored data.

Mission P3-C11-DR01-D2A1-FINAL-SOURCE-PROOF-READINESS-01. Each row is one named
statement of a sealed proof document or of AD-1-R4 N. Its citations are transcribed
from the document, with the document's own line numbers at the document's own
source commit; matrix.py resolves each to items by byte range and compares them with
Candidate C. The executor authors the rows and their kinds; matrix.py computes every
identity and every status. Nothing here accepts a proof.

Cite forms:
  ('L', f, commit, a, b)   lines a..b of file f at commit (f: M model_tests.rs, P protocol.rs,
                           U unit_tests.rs), resolved to the innermost items covering them
  ('N', f, commit, path)   the item `path` (e.g. 'fn hold') of file f at commit
  ('F', path, commit)      a whole file, by blob, at commit
  ('R', 'MODEL')           the model region: every byte of model_tests.rs at SOURCE-R2
  ('W', commit)            the whole of model_tests.rs, from its blob at commit to C
Kinds:
  STATIC      the statement rests on source; status IDENTICAL, or CHANGED_REVIEW_REQUIRED
  WHOLEFILE   as STATIC, and also on the whole model_tests.rs blob (a ('W', ..) cite);
              NON_INTERFERING_CHANGE when C only appends isolated, non-interfering layers
  EXECUTION   needs a run at the exact final commit: EXECUTION_PENDING
  OWNER       an Owner decision: OWNER_ALREADY_SATISFIED (adoption record at C)
  REVIEW      the Architect's independent review: NOT_ESTABLISHED until it is made
  SCOPE       a stated limit of the claim, which no static check establishes: NOT_ESTABLISHED
"""
SR2 = 'SOURCE-R2'
ADM = 'ADM'
GC = 'GC'
EXT = 'EXT'
W1 = 'W1'

AP = 'ADM-PROOF'   # ADM-QUOTIENT-01 receipts/stage2/01-formal-argument.txt (lines at SOURCE-R2)
GP = 'GC-PROOF'    # GC-SYMMETRY-01 receipts/stage2/00-FORMAL-SYMMETRY-ANALYSIS.txt (lines at ADM)
EP = 'EXT-PROOF'   # CLOSURE-PREP-01 receipts/stage2/00-FPT-FB-SYMMETRY-PROOF.txt (lines at GC)
AD = 'AD-1-R4'     # the adopted addendum at C


def L(f, c, a, b=None):
    return ('L', f, c, a, a if b is None else b)


def N(c, path, f='M'):
    return ('N', f, c, path)


DIAG51 = (
    ['quotient_x%d_matches_the_original' % i for i in range(1, 6)]
    + ['quotient_negative_e2_admission_views_twice', 'quotient_negative_e9_admission_ignores_the_latch',
       'quotient_negative_p12_admission_ignores_recovery', 'quotient_negative_p1_admission_ignores_unapplied',
       'quotient_observer_reports_the_admission_properties', 'quotient_refuses_an_admission_that_keeps_bits',
       'quotient_refuses_an_admission_that_keeps_its_counter', 'quotient_refuses_an_admission_that_writes_the_table',
       'an_admission_that_keeps_its_counter_fails_l1_in_the_original']
    + ['gc_symmetry_x2_matches_the_original', 'gc_symmetry_sym1_matches_the_original',
       'gc_symmetry_sym_pt_matches_the_original', 'gc_symmetry_sym_b_matches_the_original',
       'gc_symmetry_sym_bi_matches_the_original']
    + ['gc_symmetry_sym_pt_takes_the_a4_steps', 'gc_symmetry_sym_b_takes_the_batch_steps',
       'gc_symmetry_sym_bi_takes_the_batch_inserts']
    + ['gc_symmetry_negative_p1_on_sym_pt', 'gc_symmetry_negative_p12_on_sym_b', 'gc_symmetry_negative_p1_on_sym_bi',
       'gc_symmetry_negative_e2_admission_views_twice', 'gc_symmetry_negative_e9_admission_ignores_the_latch',
       'gc_symmetry_negative_p1_admission_ignores_unapplied', 'gc_symmetry_allow_list_is_exact',
       'gc_symmetry_refuses_a_batch_under_f3s_name', 'gc_symmetry_refuses_an_asymmetric_step_on_sym_pt',
       'gc_symmetry_refuses_an_asymmetric_step_on_sym_b', 'gc_symmetry_refuses_an_asymmetric_step_on_sym_bi',
       'gc_symmetry_refuses_f_pt_with_a_lower_fault_budget', 'gc_symmetry_refuses_f_pt_with_a_raised_cap',
       'gc_symmetry_refuses_f_b_with_another_actor_change', 'gc_symmetry_refuses_f_b_with_a_raised_time_cap',
       'gc_symmetry_refuses_acknowledgements', 'gc_symmetry_refuses_a_configuration_not_shown_symmetric',
       'gc_symmetry_refuses_an_acknowledgement_row', 'gc_symmetry_refuses_an_asymmetric_step',
       'gc_symmetry_refuses_another_shape_under_a_known_name', 'gc_symmetry_refuses_a_raised_cap',
       'gc_symmetry_refuses_f_g', 'gc_symmetry_refuses_kept_bits', 'gc_symmetry_refuses_other_callers']
    + ['gc_symmetry_finds_the_original_l1_cycle', 'gc_symmetry_initial_states', 'gc_symmetry_l1_on_finite_graphs',
       'gc_symmetry_properties_follow_the_exchange', 'gc_symmetry_renames_consistently'])
assert len(DIAG51) == 51 and len(set(DIAG51)) == 51

R = []


def row(id_, group, statement, proof, section, kind, cites=(), via=(), note=''):
    R.append({'id': id_, 'group': group, 'statement': statement, 'proof': proof, 'section': section,
              'kind': kind, 'cites': list(cites), 'via': list(via), 'note': note})


# ---------------- admission observer quotient (ADM-QUOTIENT-01; lines at SOURCE-R2) ----------------
G = 'Admission: source facts'
row('ADM.F1', G, 'World has adm and adm_kept; initial() sets adm = admissions, adm_kept = None', AP, '0 F1', 'STATIC',
    [L('M', SR2, 1046, 1064), L('M', SR2, 1146, 1147)])
row('ADM.F2', G, 'every occurrence of adm/adm_kept/config.admissions; no non-ADM step, helper, property or protocol.rs path reads them',
    AP, '0 F2', 'STATIC',
    [L('M', SR2, 1056), L('M', SR2, 1058), L('M', SR2, 1146, 1147), L('M', SR2, 1691, 1697), L('M', SR2, 2123, 2124),
     L('M', SR2, 2160, 2161), L('M', SR2, 3251), L('M', SR2, 3257), L('M', SR2, 3556), L('M', SR2, 4033), ('R', 'MODEL'),
     ('F', 'P', SR2)],
    note='a whole-model claim: the model region is cited whole; matrix.py re-scans every occurrence at C by region')
row('ADM.F3', G, 'encode writes adm as one byte; decode reads it; key = PRE || [a] || POST', AP, '0 F3', 'STATIC',
    [L('M', SR2, 1521, 1721), L('M', SR2, 1976, 2168)])
row('ADM.F4', G, 'step_adm: enabled iff adm != 0; one view (one load); only write adm -= 1; no oracle', AP, '0 F4', 'STATIC',
    [L('M', SR2, 3250, 3272), L('P', SR2, 225, 229), L('P', SR2, 189, 195), L('M', SR2, 3275, 3321), L('M', SR2, 3367),
     L('M', SR2, 3817, 3843)])
row('ADM.F5', G, 'transition_properties does not read adm; an ADM edge carries only admission_properties violations', AP,
    '0 F5', 'STATIC', [L('M', SR2, 3514, 3545)])
row('ADM.F6', G, 'state_properties and KEY do not depend on adm', AP, '0 F6', 'STATIC',
    [L('M', SR2, 3403, 3511), L('M', SR2, 3813, 3815)])
row('ADM.F7', G, 'terminal is the only other consumer of adm (adm == 0)', AP, '0 F7', 'STATIC', [L('M', SR2, 3548, 3569)])
row('ADM.F8', G, 'thread-local hook counts reset and read around each step; no callback installed in exploration', AP,
    '0 F8', 'STATIC',
    [L('P', SR2, 2018, 2082), L('M', SR2, 3823), L('M', SR2, 3825), L('M', SR2, 1466), L('U', SR2, 2187, 2299)])
row('ADM.F9', G, 'explore: BFS from k0 = encode(initial); first violation reported; L1 after a complete search', AP,
    '0 F9', 'STATIC', [L('M', SR2, 3882, 4110)])
row('ADM.F10', G, 'a step is a function of (key, choice prefix); no HashMap, clock or global state enters a step', AP,
    '0 F10', 'STATIC', [L('M', SR2, 3849, 3879), ('R', 'MODEL')])
G = 'Admission: lemmas and theorem'
row('ADM.L-A', G, 'L-A: other actors independent of adm', AP, '2 L-A', 'STATIC', via=['ADM.F2', 'ADM.F3', 'ADM.F5', 'ADM.F6', 'ADM.F10'])
row('ADM.L-B', G, 'L-B: ADM is a pure observer (one successor set_adm(k, a-1); OBS(pi))', AP, '2 L-B', 'STATIC',
    via=['ADM.F4', 'ADM.F5', 'ADM.F8'])
row('ADM.Theorem1', G, 'Theorem 1: nodes(G) = lifts of R_Q over 0..N; R_Q = nodes with adm = N', AP, '2 THEOREM 1', 'STATIC',
    via=['ADM.L-A', 'ADM.L-B', 'ADM.F1'])
row('ADM.Corollary1', G, 'Corollary 1: edges of G are lifts of Q edges or ADM edges with OBS(pi)', AP, '2 COROLLARY 1',
    'STATIC', via=['ADM.Theorem1'])
G = 'Admission: obligations Q1-Q15'
row('ADM.Q1', G, 'ADM changes no protocol, storage, table, fault-budget, token or worker state', AP, '3 Q1', 'STATIC',
    [N(ADM, 'fn observe_admission')], ['ADM.F4'])
row('ADM.Q2', G, 'ADM changes only its counter, plus test observations', AP, '3 Q2', 'STATIC',
    [N(ADM, 'fn observe_admission')], ['ADM.F4', 'ADM.F8'])
row('ADM.Q3', G, 'ADM adds no fault, waits on no actor, changes no other actor', AP, '3 Q3', 'STATIC',
    via=['ADM.F4', 'ADM.F2', 'ADM.L-A'])
row('ADM.Q4', G, 'every reachable protocol state is reachable with the full admission budget', AP, '3 Q4', 'STATIC',
    via=['ADM.Theorem1'])
row('ADM.Q5', G, 'every successful admission of G is an observer evaluation in Q (complete run)', AP, '3 Q5', 'STATIC',
    [N(ADM, 'fn expand'), N(ADM, 'fn observe_admission')], ['ADM.L-B', 'ADM.Theorem1'],
    note='its completeness condition is AQ5 (execution)')
row('ADM.Q6', G, 'P1, P12, P15, E6, E9 evaluated under the same admission conditions', AP, '3 Q6', 'STATIC',
    via=['ADM.F2', 'ADM.F4', 'ADM.F5'])
row('ADM.Q7', G, 'the one-view rule stays checkable; no added publication; no contaminated counts', AP, '3 Q7', 'STATIC',
    [N(ADM, 'fn observe_admission'), N(ADM, 'fn expand'), L('M', SR2, 1466)], ['ADM.F8', 'ADM.F5'])
row('ADM.Q8', G, 'all other state and transition properties keep their semantics', AP, '3 Q8', 'STATIC',
    via=['ADM.F6', 'ADM.L-A', 'ADM.F5', 'ADM.Corollary1'])
row('ADM.Q9', G, 'D1 preserved, judged with the remaining admissions spent', AP, '3 Q9', 'STATIC',
    [N(ADM, 'fn explore_experimental')], ['ADM.F7', 'ADM.L-A', 'ADM.L-B'])
row('ADM.Q10', G, 'L1 preserved in both directions (ADM edges are observations, never edges)', AP, '3 Q10', 'STATIC',
    [N(ADM, 'fn explore_experimental')], ['ADM.Corollary1'])
row('ADM.Q11', G, 'every violation trace of Q maps to an original trace', AP, '3 Q11', 'STATIC',
    [L('M', SR2, 3849, 3879), N(ADM, 'fn trace_with')])
row('ADM.Q12', G, 'the reduction removes no counterexample (sets of violated properties equal; first-named may differ)',
    AP, '3 Q12', 'STATIC', via=['ADM.Q11', 'ADM.F6', 'ADM.L-A', 'ADM.L-B', 'ADM.Q5', 'ADM.Q9', 'ADM.Q10'])
row('ADM.Q13', G, 'the visited key keeps every relevant field except the constant admission byte', AP, '3 Q13', 'STATIC',
    [N(SR2, 'const KEY_FIELDS')], ['ADM.F3'])
row('ADM.Q14', G, 'the quotient never applies to A2 (fails closed on kept bits)', AP, '3 Q14', 'STATIC',
    [N(ADM, 'fn observe_admission'), N(ADM, 'fn expand')], ['ADM.F1', 'ADM.F2'])
row('ADM.Q15', G, 'mutants under M1-M3 keep admission timing; every 7.5 control keeps the original explorer', AP, '3 Q15',
    'STATIC', via=['ADM.F2', 'ADM.F4'])
G = 'Admission: assumptions AQ1-AQ6'
row('ADM.AQ1', G, 'the model is the one at 244c5b23 (blobs beb48fe7, a2df2841, 55dc6640)', AP, '4 AQ1', 'WHOLEFILE',
    [('W', SR2), ('F', 'P', SR2), ('F', 'U', SR2), ('R', 'MODEL')])
row('ADM.AQ2', G, 'adm_kept is None in every reachable state (checked at run time)', AP, '4 AQ2', 'EXECUTION',
    [N(ADM, 'fn observe_admission'), N(ADM, 'fn expand')], ['ADM.F1', 'ADM.F2'])
row('ADM.AQ3', G, 'no hook callback installed in the exploring thread', AP, '4 AQ3', 'WHOLEFILE',
    [('W', SR2)], ['ADM.F8'], note='matrix.py scans model_tests.rs at C: no hook::install, no hook::remove')
row('ADM.AQ4', G, 'determinism of a step given (key, prefix)', AP, '4 AQ4', 'STATIC', via=['ADM.F10'])
row('ADM.AQ5', G, 'every verdict claim is for a COMPLETE run', AP, '4 AQ5', 'EXECUTION',
    [N(ADM, 'fn explore_experimental'), N(EXT, 'fn explore_symmetric_kept')])
row('ADM.AQ6', G, 'equivalence of the explored model only; A-1, A-2, 6.2 not discharged', AP, '4 AQ6', 'SCOPE',
    note='scope limit kept by AD-1-R4 J; no static check establishes A-1/A-2')

# ---------------- gating-caller symmetry (GC-SYMMETRY-01; lines at ADM) ----------------
G = 'Symmetry: identity uses I1-I13'
row('GC.I1', G, 'gating_key(g), gating_member(g)', GP, '1 I1', 'STATIC', [L('M', ADM, 279), L('M', ADM, 300)])
row('GC.I2', G, 'SimRow::{Gating, Correction, Other, Explanation} identities', GP, '1 I2', 'STATIC', [L('M', ADM, 336, 386)])
row('GC.I3', G, 'original_of, gating_of', GP, '1 I3', 'STATIC', [L('M', ADM, 308), L('M', ADM, 314)])
row('GC.I4', G, 'Core occupants and the refusal table (identity-bearing causes)', GP, '1 I4', 'STATIC',
    [L('P', ADM, 772, 785), L('P', ADM, 420, 441)])
row('GC.I5', G, 'World: rows, gc, ghost.issued/told bits', GP, '1 I5', 'STATIC', [L('M', ADM, 1046), L('M', ADM, 1039, 1041)])
row('GC.I6', G, 'step_gc(w, g)', GP, '1 I6', 'STATIC', [L('M', ADM, 3019, 3105)])
row('GC.I7', G, 'state_properties P7/P8 loop over callers', GP, '1 I7', 'STATIC', [L('M', ADM, 3472, 3493)])
row('GC.I8', G, 'admission_properties P12 mask', GP, '1 I8', 'STATIC', [L('M', ADM, 3285, 3303)])
row('GC.I9', G, 'explanations() mask; survey and recheck (AckTxn only)', GP, '1 I9', 'STATIC',
    [L('M', ADM, 434, 447), L('M', ADM, 449), L('M', ADM, 822)])
row('GC.I10', G, 'malform "set" removes the lowest mask bit', GP, '1 I10', 'STATIC', [L('M', ADM, 3117, 3122)])
row('GC.I11', G, 'initial_rows seeds SimRow::Other(1) under gating_conflict', GP, '1 I11', 'STATIC', [L('M', ADM, 1096, 1098)])
row('GC.I12', G, 'E11 loop and encode_core iterate refusals in BTreeMap order', GP, '1 I12', 'STATIC',
    [L('M', ADM, 3533), L('M', ADM, 1513)])
row('GC.I13', G, 'actor order Gc(0) before Gc(1); first violation kept', GP, '1 I13', 'STATIC',
    [L('M', ADM, 2337), L('M', ADM, 3917), L('M', ADM, 3935)])
G = 'Symmetry: Lemma E and S1-S15'
row('GC.LemmaE', G, 'Lemma E: step(pi w, sigma a, ch) is the pi-image of step(w, a, ch)', GP, '3 LEMMA E', 'STATIC',
    [L('M', ADM, 411), L('M', ADM, 466), L('M', ADM, 499), L('M', ADM, 509), L('M', ADM, 792), L('M', ADM, 856),
     L('P', ADM, 845, 876), L('P', ADM, 1161, 1226), L('P', ADM, 1413), L('P', ADM, 1492), L('P', ADM, 1548),
     L('P', ADM, 1765), L('P', ADM, 1859), L('P', ADM, 1934), L('M', ADM, 3019, 3105), L('M', ADM, 3403),
     L('M', ADM, 3514), L('M', ADM, 2475, 2489), L('M', ADM, 3043, 3055), L('M', ADM, 2668, 2689),
     L('M', ADM, 2870, 2935), L('M', ADM, 3258, 3261), ('R', 'MODEL'), ('F', 'P', ADM)],
    ['GC.I1', 'GC.I2', 'GC.I3', 'GC.I4', 'GC.I5', 'GC.I6', 'GC.I7', 'GC.I8', 'GC.I9', 'GC.I10', 'GC.I11', 'GC.I12', 'GC.I13'],
    note='argued over the transitive step closure: the model region and protocol.rs are cited whole')
row('GC.S1', G, 'pi(init) = init for F-3 and X-2 (runtime-asserted)', GP, '4 S1', 'STATIC',
    [N(ADM, 'fn initial_rows'), N(ADM, 'fn initial'), N(GC, 'fn assert_symmetric')])
row('GC.S2', G, 'both GC actors run step_gc with g the only parameter', GP, '4 S2', 'STATIC', [L('M', ADM, 3019, 3105)],
    ['GC.LemmaE'])
row('GC.S3', G, 'the transformation renames every caller-naming value', GP, '4 S3', 'STATIC',
    [N(GC, 'fn mirror_x32'), N(GC, 'fn mirror_key'), N(GC, 'fn mirror_part'), N(GC, 'fn mirror_member'),
     N(GC, 'fn mirror_cause'), N(GC, 'fn mirror_occupant'), N(GC, 'fn mirror_row'), N(GC, 'fn mirror_world')])
row('GC.S4', G, 'rows renamed in place; sequences keep their meaning', GP, '4 S4', 'STATIC',
    [L('M', ADM, 718), N(ADM, 'fn durable_frontier'), N(ADM, 'fn survey')])
row('GC.S5', G, 'gc and ghost bits swapped; other ghost fields name no caller', GP, '4 S5', 'STATIC',
    [L('M', ADM, 1039, 1041), N(GC, 'fn mirror_world'), N(GC, 'fn mirror_bits')])
row('GC.S6', G, 'pi is an involution (runtime-asserted at every expanded key)', GP, '4 S6', 'STATIC',
    [N(GC, 'fn mirror'), N(GC, 'fn mirror_world'), N(GC, 'fn mirror_x32'), N(GC, 'fn explore_symmetric_kept')])
row('GC.S7', G, 'every transition has its mirror transition (same fault, labels, names, multiplicity)', GP, '4 S7',
    'STATIC', [N(GC, 'fn hold'), N(GC, 'fn explore_symmetric_kept')], ['GC.LemmaE', 'GC.S1'])
row('GC.S8', G, 'violated-property name sets invariant; first-reported name may differ', GP, '4 S8', 'STATIC',
    via=['GC.I7', 'GC.I12', 'GC.I13'])
row('GC.S9', G, 'D1 invariant: enabledness and terminal() fixed by pi', GP, '4 S9', 'STATIC', [L('M', ADM, 3548, 3569)],
    ['GC.LemmaE'])
row('GC.S10', G, 'L1 preserved in both directions on the finite quotient', GP, '4 S10', 'STATIC',
    [L('M', ADM, 3957, 4001), N(GC, 'fn kahn')], ['GC.S7'])
row('GC.S11', G, 'quotient paths unfold to original traces, replayed in the original semantics', GP, '4 S11', 'STATIC',
    [N(ADM, 'fn step_with'), N(ADM, 'fn last_step_violations'), N(GC, 'fn unfold'), N(GC, 'fn replay')])
row('GC.S12', G, 'pi is an automorphism of the admission quotient (composition AQ then SQ)', GP, '4 S12', 'STATIC',
    [L('M', ADM, 3250, 3272), L('M', ADM, 3275, 3321), N(ADM, 'fn with_admissions'), N(ADM, 'fn observe_admission')],
    ['ADM.Theorem1', 'GC.LemmaE'])
row('GC.S13', G, 'canonical(k) = min(k, pi_K k); distinct orbits never share a key', GP, '4 S13', 'STATIC',
    [N(GC, 'fn canonical'), N(ADM, 'const KEY_FIELDS')], ['GC.S6'])
row('GC.S14', G, 'restriction: the prototype accepts only the listed symmetric configurations (at GC: X-2, SYM-1, F-3; F-PT and F-B refused)',
    GP, '4 S14', 'STATIC', [N(GC, 'const SYMMETRIC'), N(GC, 'fn assert_symmetric'), N(GC, 'fn shape')],
    note='SYMMETRIC was changed by EXT (F-PT, F-B admitted); C equals EXT; the extension is the subject of EXT.E-PT/E-B')
row('GC.S15', G, 'explore, explore_experimental, successors, expand unchanged; the symmetry path is new', GP, '4 S15',
    'STATIC', [N(SR2, 'fn explore'), N(SR2, 'fn successors'), N(ADM, 'fn explore_experimental'), N(ADM, 'fn expand'),
               N(GC, 'fn explore_symmetric'), N(GC, 'fn explore_symmetric_kept')])
G = 'Symmetry: asymmetries ASYM-1..6'
row('GC.ASYM-1', G, 'SetFacts digest mask (I9); unreachable with acks = 0; refused', GP, '5 ASYM-1', 'STATIC',
    [N(GC, 'fn assert_symmetric')], ['GC.I9'])
row('GC.ASYM-2', G, 'malform removes the lowest mask bit (I10); refused', GP, '5 ASYM-2', 'STATIC',
    [N(GC, 'fn assert_symmetric')], ['GC.I10'])
row('GC.ASYM-3', G, "F-G seeds G2's identity (I11); pi(init) != init; refused", GP, '5 ASYM-3', 'STATIC',
    [N(GC, 'fn assert_symmetric')], ['GC.I11'])
row('GC.ASYM-4', G, 'first-reported property among simultaneous caller-attributed violations', GP, '5 ASYM-4', 'STATIC',
    via=['GC.I7', 'GC.I12', 'GC.I13'])
row('GC.ASYM-5', G, 'trace notes and details name G1/G2 (presentation only)', GP, '5 ASYM-5', 'STATIC',
    [L('M', ADM, 3043, 3055)], ['GC.I7'])
row('GC.ASYM-6', G, 'actor order and BTreeMap order of gating keys (enumeration only)', GP, '5 ASYM-6', 'STATIC',
    via=['GC.I12', 'GC.I13'])
G = 'Symmetry: assumptions AS1-AS4'
row('GC.AS1', G, 'KEY holds on every expanded key (checked at run time, key and mirror)', GP, '6 AS1', 'EXECUTION',
    [L('M', ADM, 3813, 3815), N(GC, 'fn explore_symmetric_kept')])
row('GC.AS2', G, 'no hook callback in the explorer thread (as AQ3)', GP, '6 AS2', 'WHOLEFILE', [('W', ADM)], ['ADM.F8'],
    note='matrix.py scans model_tests.rs at C: no hook::install, no hook::remove')
row('GC.AS3', G, 'the step function is deterministic (as AQ4)', GP, '6 AS3', 'STATIC', via=['ADM.F10'])
row('GC.AS4', G, "Lemma E's inspection is complete for the source at 83816e66 only; a later model change voids it", GP,
    '6 AS4', 'WHOLEFILE', [('W', ADM), ('R', 'MODEL'), ('F', 'P', ADM)])

# ---------------- F-PT / F-B extension (CLOSURE-PREP-01; lines at GC) ----------------
BU = [L('M', GC, n) for n in (67, 98, 219, 2377, 2779, 4043, 5245, 6808)]
BA = [L('M', GC, n) for n in (46, 86, 150, 224, 229, 1109, 2040, 2054, 2333, 2394, 3281)] + [L('M', GC, 4390, 4440)]
G = 'Extension: lemmas'
row('EXT.E-PT', G, "E-PT: A4's paths read and write only caller-free data", EP, '1 E-PT', 'STATIC',
    [L('M', GC, 2375, 2385), L('M', GC, 2389, 2399), L('M', GC, 2510, 2638), L('M', GC, 2777, 2788),
     L('M', GC, 6266, 6344), N(GC, 'const FPT')] + BU)
row('EXT.E-B', G, "E-B: the batch paths compare only fact identities; caller-free", EP, '1 E-B', 'STATIC',
    [L('P', GC, 815, 833), L('P', GC, 867), L('P', GC, 1161, 1226), L('P', GC, 1330, 1407), L('M', GC, 718),
     L('M', GC, 671, 694), L('M', GC, 6363, 6373), L('M', GC, 1109, 1114), L('M', GC, 2040, 2063),
     L('M', GC, 2332, 2347), L('M', GC, 2406, 2501), L('M', GC, 2243, 2316), L('M', GC, 2361, 2371),
     L('M', GC, 3461), N(GC, 'const FB')] + BA)
row('EXT.E-R', G, 'E-R: protocol.rs never iterates the refusal table in key order', EP, '1 E-R', 'STATIC',
    [L('P', GC, 1156), L('P', GC, 1162), L('P', GC, 1575), L('P', GC, 1221), L('P', GC, 1533), L('P', GC, 1560),
     L('P', GC, 1943), L('M', GC, 1513), L('M', GC, 3533)])
G = 'Extension: F-PT matrix 1-17'
FPT_ITEMS = [
    (1, 'initial world invariant', [N(GC, 'fn initial'), N(GC, 'fn initial_rows'), N(EXT, 'fn assert_symmetric')], []),
    (2, 'callers exchangeable', [L('M', GC, 3019, 3105)], ['GC.LemmaE']),
    (3, 'rows/identities transform', [N(GC, 'fn mirror_row')], ['GC.S3']),
    (4, 'sequences unchanged', [L('M', GC, 718), N(GC, 'fn durable_frontier')], []),
    (5, 'occupants/causes/refusals', [L('P', GC, 1533)], []),
    (6, 'worker/retained tokens', [N(GC, 'fn step_wk'), N(GC, 'fn worker_op'), N(GC, 'fn step_replay')], []),
    (7, 'GC states and history bits', [], ['EXT.E-PT']),
    (8, 'fault choices and budgets', [N(GC, 'impl Oracle')], ['EXT.E-PT']),
    (9, 'every property invariant (names-set; ASYM-4 kept)', [N(GC, 'fn state_properties'), N(GC, 'fn transition_properties')],
     ['GC.I7', 'GC.I12', 'GC.ASYM-4']),
    (10, 'D1 invariant', [L('M', GC, 3548, 3569)], ['GC.LemmaE']),
    (11, 'L1 cycle existence', [], ['GC.S10']),
    (12, 'admission-observer composition', [L('M', GC, n) for n in (1146, 3251, 3257, 3556, 4033)], ['GC.S12']),
    (13, 'counterexample unfolding', [N(GC, 'fn unfold'), N(GC, 'fn replay'), N(GC, 'fn step_with')], ['GC.S11']),
    (14, 'no new asymmetry from A4', [], ['EXT.E-PT']),
    (15, 'FB batch writer: not applicable (batch false)', [N(GC, 'const FPT')], []),
    (16, 'every original transition has a mapped transition', [L('M', GC, 6408), N(GC, 'fn hold')], ['GC.S7', 'EXT.E-PT']),
    (17, 'original explorer required for control attribution', [], ['GC.S15', 'ADM.Q12', 'GC.ASYM-4']),
]
for n, text, cites, via in FPT_ITEMS:
    row('EXT.FPT.%d' % n, G, 'F-PT %d: %s' % (n, text), EP, '2 item %d' % n, 'STATIC', cites, via)
G = 'Extension: F-B matrix 1-17'
FB_ITEMS = [
    (1, 'initial world invariant', [L('M', GC, 1109, 1114), N(GC, 'fn initial'), N(EXT, 'fn assert_symmetric')], []),
    (2, 'callers exchangeable (sigma by position: G1 3 <-> G2 4)', [L('M', GC, 6363, 6373), N(GC, 'fn pack'),
                                                                  N(GC, 'fn unpack')], ['GC.LemmaE']),
    (3, 'rows/identities transform', [], ['EXT.E-B']),
    (4, 'sequences unchanged', [L('M', GC, 671, 694), L('M', GC, 718)], []),
    (5, 'occupants/causes/refusals', [L('P', GC, 1221, 1224), L('M', GC, 499)], []),
    (6, 'worker/retained tokens', [N(GC, 'fn step_wk'), N(GC, 'fn worker_op'), N(GC, 'fn step_replay')], []),
    (7, 'GC states and history bits', [], ['EXT.E-B']),
    (8, 'fault choices and budgets', [L('M', GC, 671, 694)], ['EXT.E-B']),
    (9, 'every property invariant (names-set; ASYM-4 kept)', [L('M', GC, 2394), L('M', GC, 3281), L('M', GC, 3461),
                                                             L('M', GC, 2243, 2316)], ['GC.ASYM-4']),
    (10, 'D1 invariant', [L('M', GC, 3548, 3569)], []),
    (11, 'L1 cycle existence', [], ['GC.S10']),
    (12, 'admission-observer composition', [], ['EXT.E-B', 'GC.S12']),
    (13, 'counterexample unfolding', [N(GC, 'fn unfold'), N(GC, 'fn replay'), N(GC, 'fn actors')], ['GC.S11']),
    (14, 'F-PT blanket Uncertain: not applicable (blanket_uncertain false)', [N(GC, 'const FB')], []),
    (15, 'no new asymmetry from the batch', [], ['EXT.E-B']),
    (16, 'every original transition has a mapped transition', [N(GC, 'fn hold')], ['GC.S7', 'EXT.E-B']),
    (17, 'original explorer required for control attribution', [], ['GC.S15', 'ADM.Q12', 'GC.ASYM-4']),
]
for n, text, cites, via in FB_ITEMS:
    row('EXT.FB.%d' % n, G, 'F-B %d: %s' % (n, text), EP, '3 item %d' % n, 'STATIC', cites, via)
G = 'Extension: conditions C-1..C-6'
row('EXT.C-1', G, 'inherits AS1-AS4 and AQ1-AQ6; inspection valid for the source at 77a146ca only', EP, '4 C-1',
    'WHOLEFILE', [('W', EXT)], ['ADM.AQ1', 'GC.AS4'],
    note='the whole-file comparison is from EXT, the implementation of this proof (GC -> EXT is the extension itself)')
row('EXT.C-2', G, 'the runtime guard (hold, D1 alike, involution, assert_symmetric) remains the backstop', EP, '4 C-2',
    'EXECUTION', [N(EXT, 'fn explore_symmetric_kept'), N(EXT, 'fn hold'), N(EXT, 'fn assert_symmetric')])
row('EXT.C-3', G, 'no full F-PT or F-B run by that mission; sizes and verdicts not established', EP, '4 C-3', 'EXECUTION',
    [N(EXT, 'fn gc_symmetry_fpt'), N(EXT, 'fn gc_symmetry_fb')])
row('EXT.C-4', G, 'allow-list membership is a precondition, not acceptance under 8.3', EP, '4 C-4', 'REVIEW',
    [N(EXT, 'const SYMMETRIC')])
row('EXT.C-5', G, 'exhaustive small differentials hold the proof to fixtures (observations, not proof)', EP, '4 C-5',
    'EXECUTION', [N(EXT, 'const SYM_PT'), N(EXT, 'const SYM_B'), N(EXT, 'const SYM_BI')])
row('EXT.C-6', G, 'ASYM-1..6 unchanged; none reachable in F-PT or F-B', EP, '4 C-6', 'STATIC',
    via=['GC.ASYM-1', 'GC.ASYM-2', 'GC.ASYM-3', 'GC.ASYM-4', 'GC.ASYM-5', 'GC.ASYM-6'])

# ---------------- composition AQ then SQ (AD-1-R4 B, D, E.1) ----------------
G = 'Composition AQ then SQ'
row('COMP.F-3', G, 'F-3 by the composed quotient explore_symmetric(&F3, step_adm, Mode::Quotient)', AD, 'B(1), E.1',
    'STATIC', [N(SR2, 'const F3'), N(SR2, 'fn step_adm'), N(ADM, 'fn expand'), N(ADM, 'fn observe_admission'),
               N(EXT, 'fn explore_symmetric'), N(EXT, 'fn explore_symmetric_kept'), N(EXT, 'fn gc_symmetry_f3')],
    ['ADM.Theorem1', 'GC.LemmaE', 'GC.S12'], note='the run itself is N.11-N.14 (execution)')
row('COMP.F-PT', G, 'F-PT by the composed quotient (gc_symmetry_fpt)', AD, 'B(1), E.1', 'STATIC',
    [N(SR2, 'const FPT'), N(EXT, 'fn symmetric_full_exploration'), N(EXT, 'fn gc_symmetry_fpt')],
    ['COMP.F-3', 'EXT.E-PT'], note='the run itself is N.11-N.14 (execution)')
row('COMP.F-B', G, 'F-B by the composed quotient (gc_symmetry_fb)', AD, 'B(1), E.1', 'STATIC',
    [N(SR2, 'const FB'), N(EXT, 'fn symmetric_full_exploration'), N(EXT, 'fn gc_symmetry_fb')],
    ['COMP.F-3', 'EXT.E-B'], note='the run itself is N.11-N.14 (execution)')

# ---------------- AD-1-R4 N.1-N.15 (identities stated at W1 = 42dd1bd1) ----------------
G = 'AD-1-R4 N'
B_NAMED = ['fn initial', 'fn actors', 'fn successors', 'fn step', 'struct Io', 'struct Oracle', 'impl Oracle', 'fn encode',
           'fn decode', 'fn state_properties', 'fn transition_properties', 'fn admission_properties', 'fn terminal',
           'const PROPERTIES', 'const BRANCHES', 'fn explore', 'struct Config', 'const BASE', 'fn default_run',
           'const FULL_CAP', 'const FULL_TIME', 'const DEFAULT_CAP', 'const DEFAULT_TIME']
row('N.1', G, 'protocol.rs including its test-only hook (blob a2df2841)', AD, 'N.1', 'STATIC',
    [('F', 'P', W1), N(W1, 'mod hook', 'P')])
row('N.2', G, 'the model items (initial, actors, every step_*, Io, Oracle, encode, decode, properties, terminal, PROPERTIES, BRANCHES, constants)',
    AD, 'N.2', 'STATIC', [N(SR2, p) for p in B_NAMED] + [('STEPS', SR2), ('CONFIGS', SR2), ('IMPLS_IO', SR2)])
row('N.3', G, 'the quotient code (reviewed text at 2d83b74b)', AD, 'N.3', 'STATIC',
    [N(EXT, p) for p in ('fn expand', 'fn observe_admission', 'fn explore_experimental', 'fn mirror', 'fn explore_symmetric',
                         'fn explore_symmetric_kept', 'fn assert_symmetric', 'fn shape', 'const SYMMETRIC')])
row('N.4a', G, 'unit_tests.rs (55dc6640), ledger.rs (d872b908), lib.rs (b806ed57)', AD, 'N.4', 'STATIC',
    [('F', 'U', W1), ('F', 'crates/nexus-governed-control/src/ledger.rs', W1),
     ('F', 'crates/nexus-governed-control/src/lib.rs', W1)])
row('N.4b', G, 'the rest of model_tests.rs (blob 5ad26ddc at 42dd1bd1)', AD, 'N.4', 'WHOLEFILE', [('W', W1)],
    note='C adds the C4 witness (A, 04e42a7d): see WITNESS-NONINTERFERENCE.md; the Architect rules on non-interference (N rule)')
row('N.5a', G, 'rust-toolchain.toml (726005b2: channel 1.94.0)', AD, 'N.5', 'STATIC', [('F', 'rust-toolchain.toml', W1)])
row('N.5b', G, 'the compiler that actually built the executable (rustc --version)', AD, 'N.5', 'EXECUTION')
row('N.6', G, 'Cargo.lock, crate and workspace Cargo.toml, .cargo/config.toml; panic = unwind (default)', AD, 'N.6',
    'STATIC', [('F', 'Cargo.lock', W1), ('F', 'crates/nexus-governed-control/Cargo.toml', W1), ('F', 'Cargo.toml', W1),
               ('F', '.cargo/config.toml', W1)])
row('N.7', G, 'the build command and environment; effective profile, features and cfg as built', AD, 'N.7', 'EXECUTION')
row('N.8', G, 'the 51 diagnostic tests of AD-1-R4 D pass on the pinned executable', AD, 'N.8', 'EXECUTION',
    [N(EXT, 'fn ' + t) for t in DIAG51])
row('N.9', G, 'F3, FPT, FB constants field by field; cap 30,000,000; time 45 min', AD, 'N.9', 'STATIC',
    [N(SR2, 'const F3'), N(SR2, 'const FPT'), N(SR2, 'const FB'), N(SR2, 'const BASE'), N(SR2, 'const FULL_CAP'),
     N(SR2, 'const FULL_TIME'), N(SR2, 'struct Config')], note='field values: CONFIGURATION-IDENTITY.md')
row('N.10', G, 'the printed bounds and actors equal N.9 and the N.10 actor lists', AD, 'N.10', 'EXECUTION',
    [N(SR2, 'fn actors')], note='static derivation from fn actors matches (CONFIGURATION-IDENTITY.md); printed output pending')
row('N.11', G, 'exactly one test selected with --exact: gc_symmetry_f3, gc_symmetry_fpt or gc_symmetry_fb', AD, 'N.11',
    'EXECUTION', [N(EXT, 'fn gc_symmetry_f3'), N(EXT, 'fn gc_symmetry_fpt'), N(EXT, 'fn gc_symmetry_fb')])
row('N.12', G, 'the run completes with 0 unexpanded keys, PASS, no runtime-guard refusal', AD, 'N.12', 'EXECUTION')
row('N.13', G, '45-minute and 16 GiB limits, no swap, one exploration at a time', AD, 'N.13', 'EXECUTION')
row('N.14', G, 'the pinned build of N.5-N.7; sha256 matches before and after', AD, 'N.14', 'EXECUTION')
row('N.15a', G, 'B(1) review passed against N.1-N.8 (independent Architect review)', AD, 'N.15', 'REVIEW',
    note='the review this packet prepares; not made by the executor')
row('N.15b', G, 'the Owner has approved the addendum', AD, 'N.15', 'OWNER',
    note='OWNER-ADOPTION-2026-10-14.md at C (audit record; a string is not authority)')
ROWS = R
