// SB-3E literal cell manifest (design 2.5, 3.1). U2 carries no cells.
// The `cells!` macro and this directory's test target arrive with the
// route_cells lane (U3). Until then this file is parsed as text only, by
// session_model_bridge.rs and check-citations.py; no target compiles it.
// One entry per credited cell: `<route> <sink> <dim> <value>;`, for example
// `r5 call_chat ingress quit;` (slug `rc_r5_call_chat_ingress_quit`).
cells! {
}
