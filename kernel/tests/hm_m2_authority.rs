//! P3-C11-HM-M2: the dormant kernel authority core of P3-HM-CONTRACT-02
//! (sealed context, §4.2 resolution, labelled writes, route-first recall,
//! §12.4 admission, reply acceptance, events and back-fill).
//!
//! Synthetic databases only: files in a test-owned temporary directory.
//! Crash steps run in child processes of this test binary; readiness is a
//! line on a pipe, never a sleep. Test names carry their node (`n1_` …).

use std::path::{Path, PathBuf};

use nexus_kernel::hm::{
    unit_start_state, unit_taint_step, EvaluatedLabel, OriginLabel, RouteClass,
};

// ── shared helpers ──────────────────────────────────────────────────────────

const LABELS: [OriginLabel; 3] = [
    OriginLabel::UnrestrictedOrigin,
    OriginLabel::LegacyUnknown,
    OriginLabel::LocalOnlyOrigin,
];

const ROUTES: [RouteClass; 2] = [RouteClass::Local, RouteClass::NonLocal];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Every `.rs` file of the `nexus_kernel::hm` module.
fn hm_sources() -> Vec<(String, String)> {
    let kernel = workspace_root().join("kernel/src");
    let mut files = vec![kernel.join("hm.rs")];
    let mut sub: Vec<PathBuf> = std::fs::read_dir(kernel.join("hm"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "rs"))
        .collect();
    sub.sort();
    files.extend(sub);
    files
        .into_iter()
        .map(|p| {
            let rel = p
                .strip_prefix(workspace_root())
                .unwrap()
                .display()
                .to_string();
            (rel, std::fs::read_to_string(&p).unwrap())
        })
        .collect()
}

// ── N1: taint algebra (§2.3, §2.12, §6.3, §7.5) ─────────────────────────────

#[test]
fn hm_m2_n1_origin_order_and_join_exhaustive() {
    use OriginLabel::*;
    assert!(UnrestrictedOrigin < LegacyUnknown && LegacyUnknown < LocalOnlyOrigin);
    for a in LABELS {
        assert_eq!(a.join(a), a, "idempotent");
        assert_eq!(
            a.join(UnrestrictedOrigin),
            a,
            "UNRESTRICTED is the identity"
        );
        assert_eq!(
            a.join(LocalOnlyOrigin),
            LocalOnlyOrigin,
            "LOCAL_ONLY absorbs"
        );
        for b in LABELS {
            let j = a.join(b);
            assert_eq!(j, b.join(a), "commutative");
            assert!(j >= a && j >= b, "upper bound");
            // least upper bound: no smaller label bounds both
            for c in LABELS {
                if c >= a && c >= b {
                    assert!(j <= c, "least upper bound {a:?} {b:?} {c:?}");
                }
                assert_eq!(a.join(b).join(c), a.join(b.join(c)), "associative");
            }
        }
    }
    let table: Vec<(&str, &str, &str)> = LABELS
        .iter()
        .flat_map(|a| {
            LABELS
                .iter()
                .map(move |b| (a.class_name(), b.class_name(), a.join(*b).class_name()))
        })
        .collect();
    assert_eq!(
        table,
        vec![
            (
                "UNRESTRICTED_ORIGIN",
                "UNRESTRICTED_ORIGIN",
                "UNRESTRICTED_ORIGIN"
            ),
            ("UNRESTRICTED_ORIGIN", "LEGACY_UNKNOWN", "LEGACY_UNKNOWN"),
            (
                "UNRESTRICTED_ORIGIN",
                "LOCAL_ONLY_ORIGIN",
                "LOCAL_ONLY_ORIGIN"
            ),
            ("LEGACY_UNKNOWN", "UNRESTRICTED_ORIGIN", "LEGACY_UNKNOWN"),
            ("LEGACY_UNKNOWN", "LEGACY_UNKNOWN", "LEGACY_UNKNOWN"),
            ("LEGACY_UNKNOWN", "LOCAL_ONLY_ORIGIN", "LOCAL_ONLY_ORIGIN"),
            (
                "LOCAL_ONLY_ORIGIN",
                "UNRESTRICTED_ORIGIN",
                "LOCAL_ONLY_ORIGIN"
            ),
            ("LOCAL_ONLY_ORIGIN", "LEGACY_UNKNOWN", "LOCAL_ONLY_ORIGIN"),
            (
                "LOCAL_ONLY_ORIGIN",
                "LOCAL_ONLY_ORIGIN",
                "LOCAL_ONLY_ORIGIN"
            ),
        ]
    );
}

#[test]
fn hm_m2_n1_unbound_evaluates_local_only() {
    assert_eq!(
        EvaluatedLabel::Unbound.effective(),
        OriginLabel::LocalOnlyOrigin
    );
    for l in LABELS {
        assert_eq!(EvaluatedLabel::Bound(l).effective(), l);
        assert!(EvaluatedLabel::Unbound.effective() >= l);
    }
}

#[test]
fn hm_m2_n1_unit_start_state_all_combinations() {
    for local_only in [false, true] {
        for floor in [false, true] {
            let (taint, route) = unit_start_state(local_only, floor);
            if local_only || floor {
                assert_eq!(
                    (taint, route),
                    (OriginLabel::LocalOnlyOrigin, RouteClass::Local)
                );
            } else {
                assert_eq!(
                    (taint, route),
                    (OriginLabel::UnrestrictedOrigin, RouteClass::NonLocal)
                );
            }
        }
    }
}

#[test]
fn hm_m2_n1_taint_step_exhaustive_never_falls_and_route_tightens_only() {
    // Every (taint, route, label): the taint joins the label; the route
    // class becomes local once the taint is restricted and never loosens.
    for taint in LABELS {
        for route in ROUTES {
            for label in LABELS {
                let (t, r) = unit_taint_step(taint, route, label);
                assert_eq!(t, taint.join(label), "{taint:?} {route:?} {label:?}");
                assert!(t >= taint, "taint never falls");
                let restricted = t != OriginLabel::UnrestrictedOrigin;
                let expect = if route == RouteClass::Local || restricted {
                    RouteClass::Local
                } else {
                    RouteClass::NonLocal
                };
                assert_eq!(r, expect, "{taint:?} {route:?} {label:?}");
                // idempotent
                assert_eq!(unit_taint_step(t, r, label), (t, r));
            }
        }
    }
    // Every sequence of three steps from every start state: monotone, and
    // the result is the join of the start and the sequence.
    for local_only in [false, true] {
        for floor in [false, true] {
            let start = unit_start_state(local_only, floor);
            for a in LABELS {
                for b in LABELS {
                    for c in LABELS {
                        let mut state = start;
                        let mut prev = state;
                        for l in [a, b, c] {
                            state = unit_taint_step(state.0, state.1, l);
                            assert!(state.0 >= prev.0);
                            assert!(
                                !(prev.1 == RouteClass::Local && state.1 == RouteClass::NonLocal)
                            );
                            prev = state;
                        }
                        assert_eq!(state.0, start.0.join(a).join(b).join(c));
                    }
                }
            }
        }
    }
}

/// Public functions of `nexus_kernel::hm` whose parameters name a label or a
/// route class. Only the pure algebra may take one: a caller-named label is
/// never an authority input.
fn label_taking_pub_fns(src: &str) -> Vec<String> {
    let flat: String = src.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out = Vec::new();
    let mut rest = flat.as_str();
    while let Some(i) = rest.find("pub fn ") {
        rest = &rest[i + "pub fn ".len()..];
        let name: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        let Some(open) = rest.find('(') else { break };
        let mut depth = 0usize;
        let mut end = open;
        for (j, ch) in rest[open..].char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = open + j;
                        break;
                    }
                }
                _ => {}
            }
        }
        let params = &rest[open + 1..end];
        if ["OriginLabel", "EvaluatedLabel", "RouteClass"]
            .iter()
            .any(|t| params.contains(t))
        {
            out.push(name);
        }
    }
    out
}

#[test]
fn hm_m2_n1_no_public_fn_takes_a_label_as_input() {
    // Planted negative controls: the scanner must flag these.
    assert_eq!(
        label_taking_pub_fns("impl X { pub fn admit_as(&self,\n label: OriginLabel) {} }"),
        vec!["admit_as"]
    );
    assert_eq!(
        label_taking_pub_fns("pub fn f(e: EvaluatedLabel) -> u8 { 0 }"),
        vec!["f"]
    );
    assert_eq!(
        label_taking_pub_fns("pub fn g(r: RouteClass) {}"),
        vec!["g"]
    );
    assert!(label_taking_pub_fns("pub fn h(&self) -> OriginLabel { x }").is_empty());

    const PURE_ALGEBRA: [&str; 2] = ["join", "unit_taint_step"];
    let mut found = Vec::new();
    for (file, src) in hm_sources() {
        for name in label_taking_pub_fns(&src) {
            if !PURE_ALGEBRA.contains(&name.as_str()) {
                found.push(format!("{file}: {name}"));
            }
        }
    }
    assert!(found.is_empty(), "label-taking public fns: {found:?}");
}
