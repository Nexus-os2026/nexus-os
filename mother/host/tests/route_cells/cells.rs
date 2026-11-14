// SB-3E literal cell manifest (design 2.5, 3.1): the six credited R5
// `call_chat` ingress cells (SB3E-R54 U5-1, R86.4: U5b; `door_fence` is held,
// `budget` is not credited). The route_cells lane compiles it through
// `macro_rules! cells` in main.rs, whose terminal arm is the control
// `ctl_call_chat`; session_model_bridge.rs and check-citations.py parse it
// as text. One entry per credited cell: `<route> <sink> <dim> <value>;`
// (slug `rc_<route>_<sink>_<dim>_<value>`).
cells! {
    r5 call_chat ingress original_cell_unscheduled;
    r5 call_chat ingress repeated;
    r5 call_chat ingress mission_stop;
    r5 call_chat ingress stop_all;
    r5 call_chat ingress fatal;
    r5 call_chat ingress quit;
}
