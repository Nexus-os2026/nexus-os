//! R2-F01: original mission clocks cross the fresh native Approve root.
use super::*;
use crate::authority::clock::Clock;
const TEST: &str =
    "governed::session_routes::budget_fix2::fix2_native_approval_inherits_original_budget";
const CASE: &str = "M2DD_FIX2_NATIVE_BUDGET";
const POINTS: [&str; 8] = [
    "before_accept",
    "before_approve",
    "action_confirm",
    "request.final_hold",
    "session.before.revalidation_review",
    "session.before.request_approval",
    "session.before.write_grant",
    "session.before.apply",
];
struct Answer {
    expire: std::sync::Mutex<Option<Box<dyn FnOnce() + Send>>>,
    calls: AtomicUsize,
}
impl ControlConfirmer for Answer {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(expire) = self.expire.lock().unwrap().take() {
            expire();
        }
        true
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        false
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        false
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
}
#[test]
fn fix2_native_approval_inherits_original_budget() {
    let Ok(case) = std::env::var(CASE) else {
        let mut failures = Vec::new();
        for point in POINTS {
            for clock in ["mono", "wall", "control"] {
                let case = format!("{point}/{clock}");
                let out = super::budget_fix1::child_case(TEST, CASE, &case);
                let text = format!(
                    "{}{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
                println!("{text}");
                if !out.status.success() || !text.contains(&format!("FIX2 NATIVE VERIFIED {case}"))
                {
                    failures.push(case);
                }
            }
        }
        assert!(failures.is_empty(), "failed schedules: {failures:?}");
        return;
    };
    let (point, which) = case.split_once('/').unwrap();
    assert!(POINTS.contains(&point));
    let f = Fixture::new();
    f.door
        .context()
        .narrow_mission_deadlines(
            f.clock.clone(),
            f.clock.monotonic_ms() + 120_000,
            f.clock.wall_ms() + 120_000,
        )
        .unwrap();
    let id = f.pending();
    let before_reviews = matching(&events(&f.ledger_path()), "review.computed", None);
    f.clock.advance(Duration::from_millis(119_999));
    let clock = f.clock.clone();
    let which_owned = which.to_owned();
    let reached = Arc::new(AtomicUsize::new(0));
    let observed = reached.clone();
    let expire = move || {
        observed.fetch_add(1, Ordering::SeqCst);
        match which_owned.as_str() {
            "mono" => clock.advance(Duration::from_millis(1)),
            "wall" => clock.suspend(Duration::from_millis(1)),
            "control" => (),
            _ => panic!("unknown clock"),
        }
    };
    let mut expire = Some(expire);
    if point == "before_accept" {
        expire.take().unwrap()();
    } else if point != "before_approve" && point != "action_confirm" {
        let point = *POINTS.iter().find(|p| **p == point).unwrap();
        if point == "request.final_hold" {
            f.gov
                .authority()
                .runs()
                .review2_hooks
                .arm(point, expire.take().unwrap());
        } else {
            f.gov.files().hooks().arm(point, expire.take().unwrap());
        }
    }
    let request = f
        ._session
        .ingress()
        .accept(CommandKind::Approve, 2)
        .unwrap();
    if point == "before_approve" {
        expire.take().unwrap()();
    }
    f.sequence.store(2, Ordering::SeqCst);
    assert_eq!(f._session.pending_for_test(), Some(id));
    let answer = Answer {
        expire: std::sync::Mutex::new(if point == "action_confirm" {
            Some(Box::new(expire.take().unwrap()) as Box<dyn FnOnce() + Send>)
        } else {
            None
        }),
        calls: AtomicUsize::new(0),
    };
    let result = f._session.approve_file_confirmed(&request, &answer);
    assert_eq!(
        reached.load(Ordering::SeqCst),
        1,
        "exact final boundary must execute"
    );
    let target = f._dirs.0.path().join("selected/workspace/session.txt");
    assert_eq!(
        target.exists(),
        which == "control",
        "original mission expiry must prevent native publication at {case}: {result:?}"
    );
    let facts = f._session.operation_observation(&request).unwrap();
    assert_eq!(facts.apply_handed_off, which == "control");
    if which != "control" {
        let refused_call = match point {
            "session.before.request_approval" => "request_approval",
            "session.before.write_grant" => "write_grant",
            "session.before.apply" => "apply",
            _ => "revalidation_review",
        };
        assert!(
            facts
                .calls
                .iter()
                .filter(|c| c.call == refused_call)
                .all(|c| c.entered == 0),
            "expiry precedes the actual native admission: {refused_call}"
        );
        if matches!(point, "before_accept" | "before_approve") {
            assert_eq!(
                answer.calls.load(Ordering::SeqCst),
                0,
                "expired approval must not ask a dialogue"
            );
        }
        if matches!(refused_call, "revalidation_review") {
            assert_eq!(
                matching(&events(&f.ledger_path()), "review.computed", None),
                before_reviews,
                "no native revalidation record after expiry"
            );
        }
    }
    if which == "control" {
        assert!(result.is_ok());
        assert_eq!(std::fs::read(&target).unwrap(), b"session oracle\n");
        // Independent restoration is still valid after the mission budget.
        f.clock.advance(Duration::from_millis(1));
        assert!(f.undo().is_ok());
        assert!(!target.exists());
    }
    println!(
        "FIX2 NATIVE VERIFIED {case} apply={} published={} conforms=true",
        facts.apply_handed_off,
        which == "control"
    );
}
