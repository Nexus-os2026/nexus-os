// SB-3E literal cell manifest (design 2.5, 3.1): the six credited R5
// `call_chat` ingress cells (SB3E-R54 U5-1, R86.4: U5b; `door_fence` is held,
// `budget` is not credited), U9a's eleven R9 close-chain lifecycle cells
// (SB3E-R123.1), U6a-1's three R0 registration_record ingress cells
// (SB3E-R120.1), U9b's five R6 held-sink lifecycle cells (SB3E-R135) and
// SEAM-1's three registration_reserve and three registration_readiness
// ingress cells and the R9 primary_end lifecycle cell (SB3E-R139). The
// route_cells lane compiles it through
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
    r0 registration_record ingress repeated;
    r0 registration_record ingress fatal;
    r0 registration_record ingress quit;
    r6 close_restrict lifecycle Emergency;
    r6 close_restrict lifecycle Failed;
    r6 emergency_restrict lifecycle Bootstrap;
    r6 emergency_restrict lifecycle Emergency;
    r6 emergency_restrict lifecycle Failed;
    r0 registration_reserve ingress repeated;
    r0 registration_reserve ingress fatal;
    r0 registration_reserve ingress quit;
    r0 registration_readiness ingress repeated;
    r0 registration_readiness ingress fatal;
    r0 registration_readiness ingress quit;
    r9 primary_end lifecycle SealedFinalizing;
    r0 registration_reserve lifecycle Emergency;
    r0 registration_reserve lifecycle Failed;
    r0 registration_reserve lifecycle ClosingNormal;
    r0 registration_reserve lifecycle ClosingRestricted;
    r0 registration_record lifecycle Emergency;
    r0 registration_record lifecycle Failed;
    r0 registration_record lifecycle ClosingNormal;
    r0 registration_record lifecycle ClosingRestricted;
    r0 registration_readiness lifecycle Emergency;
    r0 registration_readiness lifecycle Failed;
    r0 registration_readiness lifecycle ClosingNormal;
    r0 registration_readiness lifecycle ClosingRestricted;
    r5 call_chat lifecycle Emergency;
    r5 call_chat lifecycle Failed;
    r5 call_chat lifecycle ClosingNormal;
    r5 call_chat lifecycle ClosingRestricted;
    r5 call_chat ingress CR;
    r5 call_chat ingress LF;
    r5 call_chat ingress CRLF;
    r5 call_chat ingress EOF;
    r5 call_chat ingress partial_Ctrl_D;
    r5 call_chat ingress master_close;
    r0 registration_record ingress CR;
    r0 registration_record ingress LF;
    r0 registration_record ingress CRLF;
    r0 registration_record ingress EOF;
    r0 registration_record ingress partial_Ctrl_D;
    r0 registration_record ingress master_close;
}
