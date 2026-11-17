// SB-3E literal cell manifest (design 2.5, 3.1): the six credited R5
// `call_chat` ingress cells (SB3E-R54 U5-1, R86.4: U5b; `door_fence` is held,
// `budget` is not credited) and U9a's eleven R9 close-chain lifecycle cells
// (SB3E-R123.1). The route_cells lane compiles it through
// `macro_rules! cells` in main.rs, whose terminal arm is the control
// `ctl_call_chat`; session_model_bridge.rs and check-citations.py parse it
// as text. One entry per credited cell: `<route> <sink> <dim> <value>;`
// (slug `rc_<route>_<sink>_<dim>_<value>`, lowercase).
cells! {
    r5 call_chat ingress original_cell_unscheduled;
    r5 call_chat ingress repeated;
    r5 call_chat ingress mission_stop;
    r5 call_chat ingress stop_all;
    r5 call_chat ingress fatal;
    r5 call_chat ingress quit;
    r9 close_obligations lifecycle Normal;
    r9 close_obligations lifecycle Bootstrap;
    r9 close_obligations lifecycle Emergency;
    r9 close_obligations lifecycle Failed;
    r9 close_obligations lifecycle ClosingRestricted;
    r9 delivery lifecycle SealedFinalizing;
    r9 mother_end lifecycle SealedFinalizing;
    r9 producer_join lifecycle ClosingRestricted;
    r9 producer_join lifecycle ClosingNormal;
    r9 seal lifecycle ClosingNormal;
    r9 seal lifecycle ClosingRestricted;
}
