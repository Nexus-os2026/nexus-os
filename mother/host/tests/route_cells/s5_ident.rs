//! SB-3E S5 section U19c: s5_ident.rs
// SB3E-R228: the identity_view rows bound by the mission's end (quit x:298,
// Failed x:607, ClosingNormal x:608) and the inference_binding rows bound by
// the CAS refusal (repeated x:307, quit x:312), harness composition until M-5b
// (F1), each value applied while the sink is held at its first arrival; the
// controls are held and released. Each is one call of s5's `identified`,
// which names the advance's last step and the mission's end (C1). Open for
// M-5b: the nine identity stop-class rows (S2) and the six bind byte rows,
// whose ledger varies with the two cancellers (F5).
use super::{
    s5::{identified, Facts},
    Fixture,
};
const IDV: (&str, &str) = ("identity_view", "ingress");
const IDV_L: (&str, &str) = ("identity_view", "lifecycle");
const BIND: (&str, &str) = ("inference_binding", "ingress");
/// C1: the advance's last step and the mission's end, measured (Step 0 §5).
const NOT_PINNED: (&str, &str) = ("Err(Stopped)", "Block { reason: ModelNotPinned }");
const STOPPED: (&str, &str) = ("Err(Stopped)", "Stop { reason: EmergencyStop }");
const REFUSED: (&str, &str) = ("Err(Stopped)", "Block { reason: ModelAdmissionRefused }");
const PASSED: (&str, &str) = ("Ok(())", "none");
pub(super) fn identity_quit(f: &Fixture) -> Facts {
    identified(f, IDV, "quit", NOT_PINNED)
}
pub(super) fn identity_failed(f: &Fixture) -> Facts {
    identified(f, IDV_L, "Failed", NOT_PINNED)
}
pub(super) fn identity_closingnormal(f: &Fixture) -> Facts {
    identified(f, IDV_L, "ClosingNormal", NOT_PINNED)
}
pub(super) fn ctl_identity_view(f: &Fixture) -> Facts {
    identified(f, IDV, "none", PASSED)
}
pub(super) fn bind_repeated(f: &Fixture) -> Facts {
    identified(f, BIND, "repeated", STOPPED)
}
pub(super) fn bind_quit(f: &Fixture) -> Facts {
    identified(f, BIND, "quit", REFUSED)
}
pub(super) fn ctl_inference_binding(f: &Fixture) -> Facts {
    identified(f, BIND, "none", PASSED)
}
