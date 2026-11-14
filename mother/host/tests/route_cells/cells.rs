// SB-3E literal cell manifest (design 2.5, 3.1). U5a carries no cells.
// The route_cells lane compiles it through `macro_rules! cells` in main.rs,
// whose terminal arm is the control `ctl_call_chat`; session_model_bridge.rs
// and check-citations.py parse it as text.
// One entry per credited cell: `<route> <sink> <dim> <value>;`, for example
// `r5 call_chat ingress quit;` (slug `rc_r5_call_chat_ingress_quit`).
cells! {
}
