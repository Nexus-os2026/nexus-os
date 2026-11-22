//! SB-3E route-cells harness (design 1.3). Compiled only by the route lane,
//! whose wrapper adds `--cfg mother_route_seams`; every standard gate compiles
//! this target to 0 tests. Each S1 test runs in its own child process (one
//! session per child, design 3.3) and reaches the broker's points only
//! through its public API; the harness can arm and drain, never write.
#![cfg(all(target_os = "linux", mother_route_seams))]
#![deny(unsafe_code)]
// U19b-2: the manifest's 157 entries nest `cells!` past the default 128.
#![recursion_limit = "256"]
use nexus_governed_control::authority::approval::*;
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::ids::RunId;
use nexus_governed_control::broker::route_seams::{arm, drain_trace, Action, Entry, Held};
use nexus_governed_control::evidence_store::PrimaryEvidence;
use nexus_governed_control::host_session::{CommandKind, HostSession, SessionSeed, SessionState};
use nexus_governed_control::host_session::{SessionDoor, SessionIngress};
use nexus_governed_control::inference::{CallDeadline, CallMeter, Charged, InvocationOwner};
use nexus_governed_control::inference::{MeterRefusal, ModelIdentity, ModelPinSpec, ModelRequest};
use nexus_governed_control::inference::{ResponseSchema, RuntimeConfig, Think};
use std::io::{Read, Write};
use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

mod grant;
mod locks;
mod reader;
mod report;
mod s5;
mod s5_call_b;
mod s5_close;
mod s5_r34;
mod s5_r6;
mod s5_reg;
mod s5_report;
mod store;
/// W0b B5 (SB3E-R94.2): one credited cell's test, built only from its arm's
/// own literals: exactly one `#[test] fn`, one driver call through the cell
/// runner and one exact witness compare against its frozen row.
macro_rules! cell {
    ($slug:ident, $route:ident $sink:ident $dim:ident $value:ident, $stem:ident::$driver:ident) => {
        #[test]
        fn $slug() {
            let (name, id) = (concat!("cells::", stringify!($slug)), stringify!($slug));
            let ran = super::s5::cell(name, id, super::$stem::$driver, super::witness::render);
            if let Some(ran) = ran {
                super::witness::credited(
                    &ran,
                    stringify!($sink),
                    stringify!($dim),
                    stringify!($value),
                );
            }
        }
    };
}
/// W0b B5, C1: one control's test, as `cell!`, its witness against its
/// `(sink, dim)` row of `CONTROLS`.
macro_rules! ctl {
    ($slug:ident, $sink:ident $dim:ident, $stem:ident::$driver:ident) => {
        #[test]
        fn $slug() {
            let (name, id) = (concat!("cells::", stringify!($slug)), stringify!($slug));
            let ran = super::s5::cell(name, id, super::$stem::$driver, super::witness::render);
            if let Some(ran) = ran {
                super::witness::control(&ran, stringify!($sink), stringify!($dim));
            }
        }
    };
}
/// SB3E-R86.4 (mission Behaviour 1), W0b B5 (R94.2): the terminal arm lists
/// the controls; then one compact arm per credited cell, naming literally its
/// route, sink, dim, value and driver (`macro_rules!` cannot build
/// `rc_<slug>`, so the arm writes it). An entry with no arm fails to compile.
macro_rules! cells {
    () => { ctl!(ctl_call_chat, call_chat ingress, s5::ctl_call_chat); ctl!(ctl_close_obligations, close_obligations lifecycle, s5_close::ctl_close_obligations); ctl!(ctl_delivery, delivery lifecycle, s5_close::ctl_delivery); ctl!(ctl_mother_end, mother_end lifecycle, s5_close::ctl_mother_end); ctl!(ctl_primary_end, primary_end lifecycle, s5_close::ctl_primary_end); ctl!(ctl_producer_join, producer_join lifecycle, s5_close::ctl_producer_join); ctl!(ctl_seal, seal lifecycle, s5_close::ctl_seal); ctl!(ctl_registration_record, registration_record ingress, s5_reg::ctl_registration_record); ctl!(ctl_close_restrict, close_restrict lifecycle, s5_r6::ctl_close_restrict); ctl!(ctl_emergency_restrict, emergency_restrict lifecycle, s5_r6::ctl_emergency_restrict); ctl!(ctl_registration_reserve, registration_reserve ingress, s5_reg::ctl_registration_reserve); ctl!(ctl_registration_readiness, registration_readiness ingress, s5_reg::ctl_registration_readiness); ctl!(ctl_close_report, close_report lifecycle, s5_report::ctl_close_report); ctl!(ctl_registration_install, registration_install ingress, s5_reg::ctl_registration_install); ctl!(ctl_registration_version, registration_version ingress, s5_reg::ctl_registration_version); ctl!(ctl_registration_status, registration_status ingress, s5_reg::ctl_registration_status); ctl!(ctl_registration_tags, registration_tags ingress, s5_reg::ctl_registration_tags); ctl!(ctl_registration_show, registration_show ingress, s5_reg::ctl_registration_show); ctl!(ctl_call_version, call_version ingress, s5_call_b::ctl_call_version); ctl!(ctl_call_status, call_status ingress, s5_call_b::ctl_call_status); ctl!(ctl_call_tags, call_tags ingress, s5_call_b::ctl_call_tags); };
    (r5 call_chat ingress original_cell_unscheduled; $($rest:tt)*) => { cell!(rc_r5_call_chat_ingress_original_cell_unscheduled, r5 call_chat ingress original_cell_unscheduled, s5::original_cell_unscheduled); cells! { $($rest)* } };
    (r5 call_chat ingress repeated; $($rest:tt)*) => { cell!(rc_r5_call_chat_ingress_repeated, r5 call_chat ingress repeated, s5::repeated); cells! { $($rest)* } };
    (r5 call_chat ingress mission_stop; $($rest:tt)*) => { cell!(rc_r5_call_chat_ingress_mission_stop, r5 call_chat ingress mission_stop, s5::mission_stop); cells! { $($rest)* } };
    (r5 call_chat ingress stop_all; $($rest:tt)*) => { cell!(rc_r5_call_chat_ingress_stop_all, r5 call_chat ingress stop_all, s5::stop_all); cells! { $($rest)* } };
    (r5 call_chat ingress fatal; $($rest:tt)*) => { cell!(rc_r5_call_chat_ingress_fatal, r5 call_chat ingress fatal, s5::fatal); cells! { $($rest)* } };
    (r5 call_chat ingress quit; $($rest:tt)*) => { cell!(rc_r5_call_chat_ingress_quit, r5 call_chat ingress quit, s5::quit); cells! { $($rest)* } };
    (r9 close_obligations lifecycle Normal; $($rest:tt)*) => { cell!(rc_r9_close_obligations_lifecycle_normal, r9 close_obligations lifecycle Normal, s5_close::obligations_normal); cells! { $($rest)* } };
    (r9 close_obligations lifecycle Bootstrap; $($rest:tt)*) => { cell!(rc_r9_close_obligations_lifecycle_bootstrap, r9 close_obligations lifecycle Bootstrap, s5_close::obligations_bootstrap); cells! { $($rest)* } };
    (r9 close_obligations lifecycle Emergency; $($rest:tt)*) => { cell!(rc_r9_close_obligations_lifecycle_emergency, r9 close_obligations lifecycle Emergency, s5_close::obligations_emergency); cells! { $($rest)* } };
    (r9 close_obligations lifecycle Failed; $($rest:tt)*) => { cell!(rc_r9_close_obligations_lifecycle_failed, r9 close_obligations lifecycle Failed, s5_close::obligations_failed); cells! { $($rest)* } };
    (r9 close_obligations lifecycle ClosingRestricted; $($rest:tt)*) => { cell!(rc_r9_close_obligations_lifecycle_closingrestricted, r9 close_obligations lifecycle ClosingRestricted, s5_close::obligations_closingrestricted); cells! { $($rest)* } };
    (r9 delivery lifecycle SealedFinalizing; $($rest:tt)*) => { cell!(rc_r9_delivery_lifecycle_sealedfinalizing, r9 delivery lifecycle SealedFinalizing, s5_close::delivery_sealedfinalizing); cells! { $($rest)* } };
    (r9 mother_end lifecycle SealedFinalizing; $($rest:tt)*) => { cell!(rc_r9_mother_end_lifecycle_sealedfinalizing, r9 mother_end lifecycle SealedFinalizing, s5_close::mother_end_sealedfinalizing); cells! { $($rest)* } };
    (r9 producer_join lifecycle ClosingRestricted; $($rest:tt)*) => { cell!(rc_r9_producer_join_lifecycle_closingrestricted, r9 producer_join lifecycle ClosingRestricted, s5_close::join_closingrestricted); cells! { $($rest)* } };
    (r9 producer_join lifecycle ClosingNormal; $($rest:tt)*) => { cell!(rc_r9_producer_join_lifecycle_closingnormal, r9 producer_join lifecycle ClosingNormal, s5_close::join_closingnormal); cells! { $($rest)* } };
    (r9 seal lifecycle ClosingNormal; $($rest:tt)*) => { cell!(rc_r9_seal_lifecycle_closingnormal, r9 seal lifecycle ClosingNormal, s5_close::seal_closingnormal); cells! { $($rest)* } };
    (r9 seal lifecycle ClosingRestricted; $($rest:tt)*) => { cell!(rc_r9_seal_lifecycle_closingrestricted, r9 seal lifecycle ClosingRestricted, s5_close::seal_closingrestricted); cells! { $($rest)* } };
    (r0 registration_record ingress repeated; $($rest:tt)*) => { cell!(rc_r0_registration_record_ingress_repeated, r0 registration_record ingress repeated, s5_reg::record_repeated); cells! { $($rest)* } };
    (r0 registration_record ingress fatal; $($rest:tt)*) => { cell!(rc_r0_registration_record_ingress_fatal, r0 registration_record ingress fatal, s5_reg::record_fatal); cells! { $($rest)* } };
    (r0 registration_record ingress quit; $($rest:tt)*) => { cell!(rc_r0_registration_record_ingress_quit, r0 registration_record ingress quit, s5_reg::record_quit); cells! { $($rest)* } };
    (r6 close_restrict lifecycle Emergency; $($rest:tt)*) => { cell!(rc_r6_close_restrict_lifecycle_emergency, r6 close_restrict lifecycle Emergency, s5_r6::quit_emergency); cells! { $($rest)* } };
    (r6 close_restrict lifecycle Failed; $($rest:tt)*) => { cell!(rc_r6_close_restrict_lifecycle_failed, r6 close_restrict lifecycle Failed, s5_r6::quit_failed); cells! { $($rest)* } };
    (r6 emergency_restrict lifecycle Bootstrap; $($rest:tt)*) => { cell!(rc_r6_emergency_restrict_lifecycle_bootstrap, r6 emergency_restrict lifecycle Bootstrap, s5_r6::emergency_bootstrap); cells! { $($rest)* } };
    (r6 emergency_restrict lifecycle Emergency; $($rest:tt)*) => { cell!(rc_r6_emergency_restrict_lifecycle_emergency, r6 emergency_restrict lifecycle Emergency, s5_r6::emergency_emergency); cells! { $($rest)* } };
    (r6 emergency_restrict lifecycle Failed; $($rest:tt)*) => { cell!(rc_r6_emergency_restrict_lifecycle_failed, r6 emergency_restrict lifecycle Failed, s5_r6::emergency_failed); cells! { $($rest)* } };
    (r0 registration_reserve ingress repeated; $($rest:tt)*) => { cell!(rc_r0_registration_reserve_ingress_repeated, r0 registration_reserve ingress repeated, s5_reg::reserve_repeated); cells! { $($rest)* } };
    (r0 registration_reserve ingress fatal; $($rest:tt)*) => { cell!(rc_r0_registration_reserve_ingress_fatal, r0 registration_reserve ingress fatal, s5_reg::reserve_fatal); cells! { $($rest)* } };
    (r0 registration_reserve ingress quit; $($rest:tt)*) => { cell!(rc_r0_registration_reserve_ingress_quit, r0 registration_reserve ingress quit, s5_reg::reserve_quit); cells! { $($rest)* } };
    (r0 registration_readiness ingress repeated; $($rest:tt)*) => { cell!(rc_r0_registration_readiness_ingress_repeated, r0 registration_readiness ingress repeated, s5_reg::readiness_repeated); cells! { $($rest)* } };
    (r0 registration_readiness ingress fatal; $($rest:tt)*) => { cell!(rc_r0_registration_readiness_ingress_fatal, r0 registration_readiness ingress fatal, s5_reg::readiness_fatal); cells! { $($rest)* } };
    (r0 registration_readiness ingress quit; $($rest:tt)*) => { cell!(rc_r0_registration_readiness_ingress_quit, r0 registration_readiness ingress quit, s5_reg::readiness_quit); cells! { $($rest)* } };
    (r9 primary_end lifecycle SealedFinalizing; $($rest:tt)*) => { cell!(rc_r9_primary_end_lifecycle_sealedfinalizing, r9 primary_end lifecycle SealedFinalizing, s5_close::primary_end_sealedfinalizing); cells! { $($rest)* } };
    (r0 registration_reserve lifecycle Emergency; $($rest:tt)*) => { cell!(rc_r0_registration_reserve_lifecycle_emergency, r0 registration_reserve lifecycle Emergency, s5_reg::reserve_emergency); cells! { $($rest)* } };
    (r0 registration_reserve lifecycle Failed; $($rest:tt)*) => { cell!(rc_r0_registration_reserve_lifecycle_failed, r0 registration_reserve lifecycle Failed, s5_reg::reserve_failed); cells! { $($rest)* } };
    (r0 registration_reserve lifecycle ClosingNormal; $($rest:tt)*) => { cell!(rc_r0_registration_reserve_lifecycle_closingnormal, r0 registration_reserve lifecycle ClosingNormal, s5_reg::reserve_closingnormal); cells! { $($rest)* } };
    (r0 registration_reserve lifecycle ClosingRestricted; $($rest:tt)*) => { cell!(rc_r0_registration_reserve_lifecycle_closingrestricted, r0 registration_reserve lifecycle ClosingRestricted, s5_reg::reserve_closingrestricted); cells! { $($rest)* } };
    (r0 registration_record lifecycle Emergency; $($rest:tt)*) => { cell!(rc_r0_registration_record_lifecycle_emergency, r0 registration_record lifecycle Emergency, s5_reg::record_emergency); cells! { $($rest)* } };
    (r0 registration_record lifecycle Failed; $($rest:tt)*) => { cell!(rc_r0_registration_record_lifecycle_failed, r0 registration_record lifecycle Failed, s5_reg::record_failed); cells! { $($rest)* } };
    (r0 registration_record lifecycle ClosingNormal; $($rest:tt)*) => { cell!(rc_r0_registration_record_lifecycle_closingnormal, r0 registration_record lifecycle ClosingNormal, s5_reg::record_closingnormal); cells! { $($rest)* } };
    (r0 registration_record lifecycle ClosingRestricted; $($rest:tt)*) => { cell!(rc_r0_registration_record_lifecycle_closingrestricted, r0 registration_record lifecycle ClosingRestricted, s5_reg::record_closingrestricted); cells! { $($rest)* } };
    (r0 registration_readiness lifecycle Emergency; $($rest:tt)*) => { cell!(rc_r0_registration_readiness_lifecycle_emergency, r0 registration_readiness lifecycle Emergency, s5_reg::readiness_emergency); cells! { $($rest)* } };
    (r0 registration_readiness lifecycle Failed; $($rest:tt)*) => { cell!(rc_r0_registration_readiness_lifecycle_failed, r0 registration_readiness lifecycle Failed, s5_reg::readiness_failed); cells! { $($rest)* } };
    (r0 registration_readiness lifecycle ClosingNormal; $($rest:tt)*) => { cell!(rc_r0_registration_readiness_lifecycle_closingnormal, r0 registration_readiness lifecycle ClosingNormal, s5_reg::readiness_closingnormal); cells! { $($rest)* } };
    (r0 registration_readiness lifecycle ClosingRestricted; $($rest:tt)*) => { cell!(rc_r0_registration_readiness_lifecycle_closingrestricted, r0 registration_readiness lifecycle ClosingRestricted, s5_reg::readiness_closingrestricted); cells! { $($rest)* } };
    (r5 call_chat lifecycle Emergency; $($rest:tt)*) => { cell!(rc_r5_call_chat_lifecycle_emergency, r5 call_chat lifecycle Emergency, s5::chat_emergency); cells! { $($rest)* } };
    (r5 call_chat lifecycle Failed; $($rest:tt)*) => { cell!(rc_r5_call_chat_lifecycle_failed, r5 call_chat lifecycle Failed, s5::chat_failed); cells! { $($rest)* } };
    (r5 call_chat lifecycle ClosingNormal; $($rest:tt)*) => { cell!(rc_r5_call_chat_lifecycle_closingnormal, r5 call_chat lifecycle ClosingNormal, s5::chat_closingnormal); cells! { $($rest)* } };
    (r5 call_chat lifecycle ClosingRestricted; $($rest:tt)*) => { cell!(rc_r5_call_chat_lifecycle_closingrestricted, r5 call_chat lifecycle ClosingRestricted, s5::chat_closingrestricted); cells! { $($rest)* } };
    (r5 call_chat ingress CR; $($rest:tt)*) => { cell!(rc_r5_call_chat_ingress_cr, r5 call_chat ingress CR, s5::chat_cr); cells! { $($rest)* } };
    (r5 call_chat ingress LF; $($rest:tt)*) => { cell!(rc_r5_call_chat_ingress_lf, r5 call_chat ingress LF, s5::chat_lf); cells! { $($rest)* } };
    (r5 call_chat ingress CRLF; $($rest:tt)*) => { cell!(rc_r5_call_chat_ingress_crlf, r5 call_chat ingress CRLF, s5::chat_crlf); cells! { $($rest)* } };
    (r5 call_chat ingress EOF; $($rest:tt)*) => { cell!(rc_r5_call_chat_ingress_eof, r5 call_chat ingress EOF, s5::chat_eof); cells! { $($rest)* } };
    (r5 call_chat ingress partial_Ctrl_D; $($rest:tt)*) => { cell!(rc_r5_call_chat_ingress_partial_ctrl_d, r5 call_chat ingress partial_Ctrl_D, s5::chat_partial_ctrl_d); cells! { $($rest)* } };
    (r5 call_chat ingress master_close; $($rest:tt)*) => { cell!(rc_r5_call_chat_ingress_master_close, r5 call_chat ingress master_close, s5::chat_master_close); cells! { $($rest)* } };
    (r0 registration_record ingress CR; $($rest:tt)*) => { cell!(rc_r0_registration_record_ingress_cr, r0 registration_record ingress CR, s5_reg::record_cr); cells! { $($rest)* } };
    (r0 registration_record ingress LF; $($rest:tt)*) => { cell!(rc_r0_registration_record_ingress_lf, r0 registration_record ingress LF, s5_reg::record_lf); cells! { $($rest)* } };
    (r0 registration_record ingress CRLF; $($rest:tt)*) => { cell!(rc_r0_registration_record_ingress_crlf, r0 registration_record ingress CRLF, s5_reg::record_crlf); cells! { $($rest)* } };
    (r0 registration_record ingress EOF; $($rest:tt)*) => { cell!(rc_r0_registration_record_ingress_eof, r0 registration_record ingress EOF, s5_reg::record_eof); cells! { $($rest)* } };
    (r0 registration_record ingress partial_Ctrl_D; $($rest:tt)*) => { cell!(rc_r0_registration_record_ingress_partial_ctrl_d, r0 registration_record ingress partial_Ctrl_D, s5_reg::record_partial_ctrl_d); cells! { $($rest)* } };
    (r0 registration_record ingress master_close; $($rest:tt)*) => { cell!(rc_r0_registration_record_ingress_master_close, r0 registration_record ingress master_close, s5_reg::record_master_close); cells! { $($rest)* } };
    (r0 registration_reserve ingress CR; $($rest:tt)*) => { cell!(rc_r0_registration_reserve_ingress_cr, r0 registration_reserve ingress CR, s5_reg::reserve_cr); cells! { $($rest)* } };
    (r0 registration_reserve ingress LF; $($rest:tt)*) => { cell!(rc_r0_registration_reserve_ingress_lf, r0 registration_reserve ingress LF, s5_reg::reserve_lf); cells! { $($rest)* } };
    (r0 registration_reserve ingress CRLF; $($rest:tt)*) => { cell!(rc_r0_registration_reserve_ingress_crlf, r0 registration_reserve ingress CRLF, s5_reg::reserve_crlf); cells! { $($rest)* } };
    (r0 registration_reserve ingress EOF; $($rest:tt)*) => { cell!(rc_r0_registration_reserve_ingress_eof, r0 registration_reserve ingress EOF, s5_reg::reserve_eof); cells! { $($rest)* } };
    (r0 registration_reserve ingress partial_Ctrl_D; $($rest:tt)*) => { cell!(rc_r0_registration_reserve_ingress_partial_ctrl_d, r0 registration_reserve ingress partial_Ctrl_D, s5_reg::reserve_partial_ctrl_d); cells! { $($rest)* } };
    (r0 registration_reserve ingress master_close; $($rest:tt)*) => { cell!(rc_r0_registration_reserve_ingress_master_close, r0 registration_reserve ingress master_close, s5_reg::reserve_master_close); cells! { $($rest)* } };
    (r0 registration_readiness ingress CR; $($rest:tt)*) => { cell!(rc_r0_registration_readiness_ingress_cr, r0 registration_readiness ingress CR, s5_reg::readiness_cr); cells! { $($rest)* } };
    (r0 registration_readiness ingress LF; $($rest:tt)*) => { cell!(rc_r0_registration_readiness_ingress_lf, r0 registration_readiness ingress LF, s5_reg::readiness_lf); cells! { $($rest)* } };
    (r0 registration_readiness ingress CRLF; $($rest:tt)*) => { cell!(rc_r0_registration_readiness_ingress_crlf, r0 registration_readiness ingress CRLF, s5_reg::readiness_crlf); cells! { $($rest)* } };
    (r0 registration_readiness ingress EOF; $($rest:tt)*) => { cell!(rc_r0_registration_readiness_ingress_eof, r0 registration_readiness ingress EOF, s5_reg::readiness_eof); cells! { $($rest)* } };
    (r0 registration_readiness ingress partial_Ctrl_D; $($rest:tt)*) => { cell!(rc_r0_registration_readiness_ingress_partial_ctrl_d, r0 registration_readiness ingress partial_Ctrl_D, s5_reg::readiness_partial_ctrl_d); cells! { $($rest)* } };
    (r0 registration_readiness ingress master_close; $($rest:tt)*) => { cell!(rc_r0_registration_readiness_ingress_master_close, r0 registration_readiness ingress master_close, s5_reg::readiness_master_close); cells! { $($rest)* } };
    (r9 close_report lifecycle SealedFinalizing; $($rest:tt)*) => { cell!(rc_r9_close_report_lifecycle_sealedfinalizing, r9 close_report lifecycle SealedFinalizing, s5_report::sealedfinalizing); cells! { $($rest)* } };
    (r9 close_report lifecycle ClosingRestricted; $($rest:tt)*) => { cell!(rc_r9_close_report_lifecycle_closingrestricted, r9 close_report lifecycle ClosingRestricted, s5_report::closingrestricted); cells! { $($rest)* } };
    (r9 close_report fault none; $($rest:tt)*) => { cell!(rc_r9_close_report_fault_none, r9 close_report fault none, s5_report::fault_none); cells! { $($rest)* } };
    (r9 close_report schedule before; $($rest:tt)*) => { cell!(rc_r9_close_report_schedule_before, r9 close_report schedule before, s5_report::schedule_before); cells! { $($rest)* } };
    (r9 close_report schedule after; $($rest:tt)*) => { cell!(rc_r9_close_report_schedule_after, r9 close_report schedule after, s5_report::schedule_after); cells! { $($rest)* } };
    (r0 registration_install ingress CR; $($rest:tt)*) => { cell!(rc_r0_registration_install_ingress_cr, r0 registration_install ingress CR, s5_reg::install_cr); cells! { $($rest)* } };
    (r0 registration_install ingress LF; $($rest:tt)*) => { cell!(rc_r0_registration_install_ingress_lf, r0 registration_install ingress LF, s5_reg::install_lf); cells! { $($rest)* } };
    (r0 registration_install ingress CRLF; $($rest:tt)*) => { cell!(rc_r0_registration_install_ingress_crlf, r0 registration_install ingress CRLF, s5_reg::install_crlf); cells! { $($rest)* } };
    (r0 registration_install ingress EOF; $($rest:tt)*) => { cell!(rc_r0_registration_install_ingress_eof, r0 registration_install ingress EOF, s5_reg::install_eof); cells! { $($rest)* } };
    (r0 registration_install ingress partial_Ctrl_D; $($rest:tt)*) => { cell!(rc_r0_registration_install_ingress_partial_ctrl_d, r0 registration_install ingress partial_Ctrl_D, s5_reg::install_partial_ctrl_d); cells! { $($rest)* } };
    (r0 registration_install ingress master_close; $($rest:tt)*) => { cell!(rc_r0_registration_install_ingress_master_close, r0 registration_install ingress master_close, s5_reg::install_master_close); cells! { $($rest)* } };
    (r0 registration_install ingress repeated; $($rest:tt)*) => { cell!(rc_r0_registration_install_ingress_repeated, r0 registration_install ingress repeated, s5_reg::install_repeated); cells! { $($rest)* } };
    (r0 registration_install ingress quit; $($rest:tt)*) => { cell!(rc_r0_registration_install_ingress_quit, r0 registration_install ingress quit, s5_reg::install_quit); cells! { $($rest)* } };
    (r0 registration_install lifecycle Emergency; $($rest:tt)*) => { cell!(rc_r0_registration_install_lifecycle_emergency, r0 registration_install lifecycle Emergency, s5_reg::install_emergency); cells! { $($rest)* } };
    (r0 registration_install lifecycle Failed; $($rest:tt)*) => { cell!(rc_r0_registration_install_lifecycle_failed, r0 registration_install lifecycle Failed, s5_reg::install_failed); cells! { $($rest)* } };
    (r0 registration_install lifecycle ClosingNormal; $($rest:tt)*) => { cell!(rc_r0_registration_install_lifecycle_closingnormal, r0 registration_install lifecycle ClosingNormal, s5_reg::install_closingnormal); cells! { $($rest)* } };
    (r0 registration_install lifecycle ClosingRestricted; $($rest:tt)*) => { cell!(rc_r0_registration_install_lifecycle_closingrestricted, r0 registration_install lifecycle ClosingRestricted, s5_reg::install_closingrestricted); cells! { $($rest)* } };
    (r5 registration_version ingress CR; $($rest:tt)*) => { cell!(rc_r5_registration_version_ingress_cr, r5 registration_version ingress CR, s5_reg::version_cr); cells! { $($rest)* } };
    (r5 registration_version ingress LF; $($rest:tt)*) => { cell!(rc_r5_registration_version_ingress_lf, r5 registration_version ingress LF, s5_reg::version_lf); cells! { $($rest)* } };
    (r5 registration_version ingress CRLF; $($rest:tt)*) => { cell!(rc_r5_registration_version_ingress_crlf, r5 registration_version ingress CRLF, s5_reg::version_crlf); cells! { $($rest)* } };
    (r5 registration_version ingress EOF; $($rest:tt)*) => { cell!(rc_r5_registration_version_ingress_eof, r5 registration_version ingress EOF, s5_reg::version_eof); cells! { $($rest)* } };
    (r5 registration_version ingress partial_Ctrl_D; $($rest:tt)*) => { cell!(rc_r5_registration_version_ingress_partial_ctrl_d, r5 registration_version ingress partial_Ctrl_D, s5_reg::version_partial_ctrl_d); cells! { $($rest)* } };
    (r5 registration_version ingress master_close; $($rest:tt)*) => { cell!(rc_r5_registration_version_ingress_master_close, r5 registration_version ingress master_close, s5_reg::version_master_close); cells! { $($rest)* } };
    (r5 registration_version ingress repeated; $($rest:tt)*) => { cell!(rc_r5_registration_version_ingress_repeated, r5 registration_version ingress repeated, s5_reg::version_repeated); cells! { $($rest)* } };
    (r5 registration_version ingress quit; $($rest:tt)*) => { cell!(rc_r5_registration_version_ingress_quit, r5 registration_version ingress quit, s5_reg::version_quit); cells! { $($rest)* } };
    (r5 registration_status ingress CR; $($rest:tt)*) => { cell!(rc_r5_registration_status_ingress_cr, r5 registration_status ingress CR, s5_reg::status_cr); cells! { $($rest)* } };
    (r5 registration_status ingress LF; $($rest:tt)*) => { cell!(rc_r5_registration_status_ingress_lf, r5 registration_status ingress LF, s5_reg::status_lf); cells! { $($rest)* } };
    (r5 registration_status ingress CRLF; $($rest:tt)*) => { cell!(rc_r5_registration_status_ingress_crlf, r5 registration_status ingress CRLF, s5_reg::status_crlf); cells! { $($rest)* } };
    (r5 registration_status ingress EOF; $($rest:tt)*) => { cell!(rc_r5_registration_status_ingress_eof, r5 registration_status ingress EOF, s5_reg::status_eof); cells! { $($rest)* } };
    (r5 registration_status ingress partial_Ctrl_D; $($rest:tt)*) => { cell!(rc_r5_registration_status_ingress_partial_ctrl_d, r5 registration_status ingress partial_Ctrl_D, s5_reg::status_partial_ctrl_d); cells! { $($rest)* } };
    (r5 registration_status ingress master_close; $($rest:tt)*) => { cell!(rc_r5_registration_status_ingress_master_close, r5 registration_status ingress master_close, s5_reg::status_master_close); cells! { $($rest)* } };
    (r5 registration_status ingress repeated; $($rest:tt)*) => { cell!(rc_r5_registration_status_ingress_repeated, r5 registration_status ingress repeated, s5_reg::status_repeated); cells! { $($rest)* } };
    (r5 registration_status ingress quit; $($rest:tt)*) => { cell!(rc_r5_registration_status_ingress_quit, r5 registration_status ingress quit, s5_reg::status_quit); cells! { $($rest)* } };
    (r5 registration_tags ingress CR; $($rest:tt)*) => { cell!(rc_r5_registration_tags_ingress_cr, r5 registration_tags ingress CR, s5_reg::tags_cr); cells! { $($rest)* } };
    (r5 registration_tags ingress LF; $($rest:tt)*) => { cell!(rc_r5_registration_tags_ingress_lf, r5 registration_tags ingress LF, s5_reg::tags_lf); cells! { $($rest)* } };
    (r5 registration_tags ingress CRLF; $($rest:tt)*) => { cell!(rc_r5_registration_tags_ingress_crlf, r5 registration_tags ingress CRLF, s5_reg::tags_crlf); cells! { $($rest)* } };
    (r5 registration_tags ingress EOF; $($rest:tt)*) => { cell!(rc_r5_registration_tags_ingress_eof, r5 registration_tags ingress EOF, s5_reg::tags_eof); cells! { $($rest)* } };
    (r5 registration_tags ingress partial_Ctrl_D; $($rest:tt)*) => { cell!(rc_r5_registration_tags_ingress_partial_ctrl_d, r5 registration_tags ingress partial_Ctrl_D, s5_reg::tags_partial_ctrl_d); cells! { $($rest)* } };
    (r5 registration_tags ingress master_close; $($rest:tt)*) => { cell!(rc_r5_registration_tags_ingress_master_close, r5 registration_tags ingress master_close, s5_reg::tags_master_close); cells! { $($rest)* } };
    (r5 registration_tags ingress repeated; $($rest:tt)*) => { cell!(rc_r5_registration_tags_ingress_repeated, r5 registration_tags ingress repeated, s5_reg::tags_repeated); cells! { $($rest)* } };
    (r5 registration_tags ingress quit; $($rest:tt)*) => { cell!(rc_r5_registration_tags_ingress_quit, r5 registration_tags ingress quit, s5_reg::tags_quit); cells! { $($rest)* } };
    (r5 registration_show ingress CR; $($rest:tt)*) => { cell!(rc_r5_registration_show_ingress_cr, r5 registration_show ingress CR, s5_reg::show_cr); cells! { $($rest)* } };
    (r5 registration_show ingress LF; $($rest:tt)*) => { cell!(rc_r5_registration_show_ingress_lf, r5 registration_show ingress LF, s5_reg::show_lf); cells! { $($rest)* } };
    (r5 registration_show ingress CRLF; $($rest:tt)*) => { cell!(rc_r5_registration_show_ingress_crlf, r5 registration_show ingress CRLF, s5_reg::show_crlf); cells! { $($rest)* } };
    (r5 registration_show ingress EOF; $($rest:tt)*) => { cell!(rc_r5_registration_show_ingress_eof, r5 registration_show ingress EOF, s5_reg::show_eof); cells! { $($rest)* } };
    (r5 registration_show ingress partial_Ctrl_D; $($rest:tt)*) => { cell!(rc_r5_registration_show_ingress_partial_ctrl_d, r5 registration_show ingress partial_Ctrl_D, s5_reg::show_partial_ctrl_d); cells! { $($rest)* } };
    (r5 registration_show ingress master_close; $($rest:tt)*) => { cell!(rc_r5_registration_show_ingress_master_close, r5 registration_show ingress master_close, s5_reg::show_master_close); cells! { $($rest)* } };
    (r5 registration_show ingress repeated; $($rest:tt)*) => { cell!(rc_r5_registration_show_ingress_repeated, r5 registration_show ingress repeated, s5_reg::show_repeated); cells! { $($rest)* } };
    (r5 registration_show ingress quit; $($rest:tt)*) => { cell!(rc_r5_registration_show_ingress_quit, r5 registration_show ingress quit, s5_reg::show_quit); cells! { $($rest)* } };
    (r5 call_version ingress CR; $($rest:tt)*) => { cell!(rc_r5_call_version_ingress_cr, r5 call_version ingress CR, s5_call_b::version_cr); cells! { $($rest)* } };
    (r5 call_version ingress LF; $($rest:tt)*) => { cell!(rc_r5_call_version_ingress_lf, r5 call_version ingress LF, s5_call_b::version_lf); cells! { $($rest)* } };
    (r5 call_version ingress CRLF; $($rest:tt)*) => { cell!(rc_r5_call_version_ingress_crlf, r5 call_version ingress CRLF, s5_call_b::version_crlf); cells! { $($rest)* } };
    (r5 call_version ingress EOF; $($rest:tt)*) => { cell!(rc_r5_call_version_ingress_eof, r5 call_version ingress EOF, s5_call_b::version_eof); cells! { $($rest)* } };
    (r5 call_version ingress partial_Ctrl_D; $($rest:tt)*) => { cell!(rc_r5_call_version_ingress_partial_ctrl_d, r5 call_version ingress partial_Ctrl_D, s5_call_b::version_partial_ctrl_d); cells! { $($rest)* } };
    (r5 call_version ingress master_close; $($rest:tt)*) => { cell!(rc_r5_call_version_ingress_master_close, r5 call_version ingress master_close, s5_call_b::version_master_close); cells! { $($rest)* } };
    (r5 call_version ingress repeated; $($rest:tt)*) => { cell!(rc_r5_call_version_ingress_repeated, r5 call_version ingress repeated, s5_call_b::version_repeated); cells! { $($rest)* } };
    (r5 call_version ingress quit; $($rest:tt)*) => { cell!(rc_r5_call_version_ingress_quit, r5 call_version ingress quit, s5_call_b::version_quit); cells! { $($rest)* } };
    (r5 call_version lifecycle Emergency; $($rest:tt)*) => { cell!(rc_r5_call_version_lifecycle_emergency, r5 call_version lifecycle Emergency, s5_call_b::version_emergency); cells! { $($rest)* } };
    (r5 call_version lifecycle Failed; $($rest:tt)*) => { cell!(rc_r5_call_version_lifecycle_failed, r5 call_version lifecycle Failed, s5_call_b::version_failed); cells! { $($rest)* } };
    (r5 call_version lifecycle ClosingNormal; $($rest:tt)*) => { cell!(rc_r5_call_version_lifecycle_closingnormal, r5 call_version lifecycle ClosingNormal, s5_call_b::version_closingnormal); cells! { $($rest)* } };
    (r5 call_version lifecycle ClosingRestricted; $($rest:tt)*) => { cell!(rc_r5_call_version_lifecycle_closingrestricted, r5 call_version lifecycle ClosingRestricted, s5_call_b::version_closingrestricted); cells! { $($rest)* } };
    (r5 call_status ingress CR; $($rest:tt)*) => { cell!(rc_r5_call_status_ingress_cr, r5 call_status ingress CR, s5_call_b::status_cr); cells! { $($rest)* } };
    (r5 call_status ingress LF; $($rest:tt)*) => { cell!(rc_r5_call_status_ingress_lf, r5 call_status ingress LF, s5_call_b::status_lf); cells! { $($rest)* } };
    (r5 call_status ingress CRLF; $($rest:tt)*) => { cell!(rc_r5_call_status_ingress_crlf, r5 call_status ingress CRLF, s5_call_b::status_crlf); cells! { $($rest)* } };
    (r5 call_status ingress EOF; $($rest:tt)*) => { cell!(rc_r5_call_status_ingress_eof, r5 call_status ingress EOF, s5_call_b::status_eof); cells! { $($rest)* } };
    (r5 call_status ingress partial_Ctrl_D; $($rest:tt)*) => { cell!(rc_r5_call_status_ingress_partial_ctrl_d, r5 call_status ingress partial_Ctrl_D, s5_call_b::status_partial_ctrl_d); cells! { $($rest)* } };
    (r5 call_status ingress master_close; $($rest:tt)*) => { cell!(rc_r5_call_status_ingress_master_close, r5 call_status ingress master_close, s5_call_b::status_master_close); cells! { $($rest)* } };
    (r5 call_status ingress repeated; $($rest:tt)*) => { cell!(rc_r5_call_status_ingress_repeated, r5 call_status ingress repeated, s5_call_b::status_repeated); cells! { $($rest)* } };
    (r5 call_status ingress quit; $($rest:tt)*) => { cell!(rc_r5_call_status_ingress_quit, r5 call_status ingress quit, s5_call_b::status_quit); cells! { $($rest)* } };
    (r5 call_status lifecycle Emergency; $($rest:tt)*) => { cell!(rc_r5_call_status_lifecycle_emergency, r5 call_status lifecycle Emergency, s5_call_b::status_emergency); cells! { $($rest)* } };
    (r5 call_status lifecycle Failed; $($rest:tt)*) => { cell!(rc_r5_call_status_lifecycle_failed, r5 call_status lifecycle Failed, s5_call_b::status_failed); cells! { $($rest)* } };
    (r5 call_status lifecycle ClosingNormal; $($rest:tt)*) => { cell!(rc_r5_call_status_lifecycle_closingnormal, r5 call_status lifecycle ClosingNormal, s5_call_b::status_closingnormal); cells! { $($rest)* } };
    (r5 call_status lifecycle ClosingRestricted; $($rest:tt)*) => { cell!(rc_r5_call_status_lifecycle_closingrestricted, r5 call_status lifecycle ClosingRestricted, s5_call_b::status_closingrestricted); cells! { $($rest)* } };
    (r5 call_tags ingress CR; $($rest:tt)*) => { cell!(rc_r5_call_tags_ingress_cr, r5 call_tags ingress CR, s5_call_b::tags_cr); cells! { $($rest)* } };
    (r5 call_tags ingress LF; $($rest:tt)*) => { cell!(rc_r5_call_tags_ingress_lf, r5 call_tags ingress LF, s5_call_b::tags_lf); cells! { $($rest)* } };
    (r5 call_tags ingress CRLF; $($rest:tt)*) => { cell!(rc_r5_call_tags_ingress_crlf, r5 call_tags ingress CRLF, s5_call_b::tags_crlf); cells! { $($rest)* } };
    (r5 call_tags ingress EOF; $($rest:tt)*) => { cell!(rc_r5_call_tags_ingress_eof, r5 call_tags ingress EOF, s5_call_b::tags_eof); cells! { $($rest)* } };
    (r5 call_tags ingress partial_Ctrl_D; $($rest:tt)*) => { cell!(rc_r5_call_tags_ingress_partial_ctrl_d, r5 call_tags ingress partial_Ctrl_D, s5_call_b::tags_partial_ctrl_d); cells! { $($rest)* } };
    (r5 call_tags ingress master_close; $($rest:tt)*) => { cell!(rc_r5_call_tags_ingress_master_close, r5 call_tags ingress master_close, s5_call_b::tags_master_close); cells! { $($rest)* } };
    (r5 call_tags ingress repeated; $($rest:tt)*) => { cell!(rc_r5_call_tags_ingress_repeated, r5 call_tags ingress repeated, s5_call_b::tags_repeated); cells! { $($rest)* } };
    (r5 call_tags ingress quit; $($rest:tt)*) => { cell!(rc_r5_call_tags_ingress_quit, r5 call_tags ingress quit, s5_call_b::tags_quit); cells! { $($rest)* } };
    (r5 call_tags lifecycle Emergency; $($rest:tt)*) => { cell!(rc_r5_call_tags_lifecycle_emergency, r5 call_tags lifecycle Emergency, s5_call_b::tags_emergency); cells! { $($rest)* } };
    (r5 call_tags lifecycle Failed; $($rest:tt)*) => { cell!(rc_r5_call_tags_lifecycle_failed, r5 call_tags lifecycle Failed, s5_call_b::tags_failed); cells! { $($rest)* } };
    (r5 call_tags lifecycle ClosingNormal; $($rest:tt)*) => { cell!(rc_r5_call_tags_lifecycle_closingnormal, r5 call_tags lifecycle ClosingNormal, s5_call_b::tags_closingnormal); cells! { $($rest)* } };
    (r5 call_tags lifecycle ClosingRestricted; $($rest:tt)*) => { cell!(rc_r5_call_tags_lifecycle_closingrestricted, r5 call_tags lifecycle ClosingRestricted, s5_call_b::tags_closingrestricted); cells! { $($rest)* } };
}
// The manifest's text form is fixed by its two parsers (session_model_bridge.rs,
// check-citations.py): rustfmt does not reformat it.
#[rustfmt::skip]
mod cells;
mod witness;

const PATIENCE: Duration = Duration::from_secs(10);
const CHILD: &str = "SB3E_ROUTE_CHILD";
const NAME: &str = "qwen3.6:35b-a3b";
const DIGEST: &str = "a7eb95c53bcf96b4bdd008d0fab4a5dac88047d9c1a7a9ab88ed453423fbd87c";

/// Control: the lane's cfg reached this unit. Its presence in the test list
/// is what a standard gate (0 tests) and the lane (this test) differ by.
#[test]
fn route_lane_cfg_reached_the_harness() {
    assert!(cfg!(mother_route_seams));
}

/// Run `body` in a child of this test binary, under a 30 s deadline; the
/// child's pause failures arrive as `TIMEOUT` lines in its result file.
fn child(name: &str, body: fn(&Fixture)) {
    child_with(name, |_| Vec::new(), body);
}
/// As `child`, with the extra environment `env` makes in the child's owned
/// root (U4: the external shim's preload and its control and log paths).
fn child_with(name: &str, env: fn(&std::path::Path) -> Vec<(&str, PathBuf)>, body: fn(&Fixture)) {
    if let Some(root) = std::env::var_os(CHILD) {
        if std::env::var_os(store::SHIM_SHA256).is_some() {
            store::shim_witness();
        }
        body(&Fixture(PathBuf::from(root)));
        return println!("ROUTE CHILD PASSED {name}");
    }
    // A path LD_PRELOAD can name: no `:` (its list separator).
    let leaf = format!(
        "sb3e-route-{}-{}",
        std::process::id(),
        name.replace("::", "-")
    );
    let root = std::env::temp_dir().join(leaf);
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .unwrap();
    let fixture = Fixture(root);
    let extra = env(&fixture.0);
    let mut process = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(CHILD, &fixture.0)
        .envs(extra.iter().cloned())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while process.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            process.kill().unwrap();
        }
        std::thread::sleep(Duration::from_millis(5)); // Polls an exit only.
    }
    let out = process.wait_with_output().unwrap();
    let result = std::fs::read_to_string(fixture.0.join("result")).unwrap_or_default();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success() && result.is_empty(),
        "{name}: {result}{stdout}{stderr}"
    );
    assert!(stdout.contains(&format!("ROUTE CHILD PASSED {name}\n")));
    for line in stdout.lines().filter_map(|l| l.split_once("WITNESS ")) {
        println!("{name}: {}", line.1); // U4a-2: measured evidence (--nocapture).
    }
    if let Some((_, digest)) = extra.iter().find(|(k, _)| *k == store::SHIM_SHA256) {
        store::shim_witnessed(&digest.to_string_lossy(), &stdout);
    }
}

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        if std::env::var_os(CHILD).is_none() {
            let _ = std::fs::remove_dir_all(&self.0); // Only this owned root.
        }
    }
}
struct Opened {
    /// U19a: shared, so the reader's Stop route binds to it (terminal.rs:1115-1122).
    session: Arc<HostSession>,
    ingress: SessionIngress,
    evidence: Arc<PrimaryEvidence>,
}
impl Fixture {
    fn open(&self) -> Opened {
        assert!(nexus_governed_control::files::make_non_dumpable());
        let home = self.0.join("home");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&home)
            .unwrap();
        let seed = SessionSeed::new().unwrap();
        let ingress = seed.ingress();
        let store = home.join("broker/p3-evidence/evidence.sqlite3");
        let evidence = PrimaryEvidence::open(&store, vec![]).unwrap();
        let clock = Arc::new(SystemClock::default());
        let session = HostSession::open_with_history(
            seed,
            &home.join("broker"),
            &home,
            evidence.clone(),
            clock,
            Arc::new(Owner),
        )
        .unwrap();
        assert!(session.startup_report().ready());
        Opened {
            session: Arc::new(session),
            ingress,
            evidence,
        }
    }
    fn pause(&self, point: &'static str) -> Held {
        let result = Arc::new(std::fs::File::create(self.0.join("result")).unwrap());
        arm(
            point,
            Action::Pause {
                cell: "u3b",
                result,
            },
        )
    }
}
impl Opened {
    /// The public bootstrap, then one original door with its File run.
    fn door(&self, root: &std::path::Path) -> (SessionDoor, RunId, u64) {
        let workspace = root.join("workspace");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&workspace)
            .unwrap();
        self.session.bootstrap_workspace(&workspace, &[]).unwrap();
        self.session.finish_bootstrap().unwrap();
        let request = self.ingress.accept(CommandKind::Run, 1).unwrap();
        let door = self.session.open_file_door(&request).unwrap();
        let attachment = door.file_port().unwrap().attach().unwrap();
        door.claim_file_session(&attachment).unwrap();
        let run = door.open_file_run(&attachment).unwrap();
        (door, run, request.sequence())
    }
}
struct Meter(AtomicU32);
impl CallMeter for Meter {
    fn charge(&self) -> Result<Charged, MeterRefusal> {
        let call = self.0.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(Charged { call })
    }
    fn settle(&self, _: u32) {}
}
struct Never;
impl CallDeadline for Never {
    fn expired(&self) -> bool {
        false
    }
}
struct Owner;
impl ControlConfirmer for Owner {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        false
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        true
    }
    fn confirm_agent_door(&self, _: &AgentDoorConfirmation) -> bool {
        true
    }
    fn confirm_mission(&self, _: &MissionConfirmation) -> bool {
        true
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        false
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
}

/// A loopback runtime answering as the pinned healthy model, one request
/// per connection; it logs each request path in the order it was served.
fn fake_ollama() -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let served: Arc<Mutex<Vec<String>>> = Arc::default();
    let peer = served.clone();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().map_while(Result::ok) {
            let (mut head, mut byte) = (Vec::new(), [0u8; 1]);
            while !head.ends_with(b"\r\n\r\n") && stream.read(&mut byte).is_ok_and(|n| n == 1) {
                head.push(byte[0]);
            }
            let head = String::from_utf8_lossy(&head).to_ascii_lowercase();
            let length = head.split("content-length:").nth(1);
            let length = length.and_then(|l| l.lines().next()?.trim().parse().ok());
            let _ = stream.read_exact(&mut vec![0u8; length.unwrap_or(0)]);
            let path = head.split_whitespace().nth(1).unwrap_or_default();
            let body = match path {
                "/api/version" => r#"{"version":"0.40.13"}"#.to_string(),
                "/api/status" => r#"{"cloud":{"disabled":true,"source":"env"}}"#.to_string(),
                "/api/tags" => format!(
                    r#"{{"models":[{{"name":"{NAME}","model":"{NAME}","digest":"{DIGEST}"}}]}}"#
                ),
                "/api/chat" => format!(
                    r#"{{"model":"{NAME}","message":{{"role":"assistant","content":"{{\"ok\":true}}"}},"done":true,"done_reason":"stop","prompt_eval_count":587,"eval_count":378}}"#
                ),
                _ => r#"{"capabilities":["completion","thinking"]}"#.to_string(),
            };
            peer.lock().unwrap().push(path.to_string());
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\n\
                 content-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    (endpoint, served)
}
fn spec() -> ModelPinSpec {
    ModelPinSpec {
        name: NAME.into(),
        digest: DIGEST.into(),
        runtime_version: "0.40.13".into(),
        config: RuntimeConfig {
            num_ctx: 8192,
            temperature_milli: 0,
            seed: 7,
            think: Think::Off,
            truncate: false,
        },
    }
}

/// Unarmed, a real route (registration with its store appends, bootstrap,
/// door, run, cancel, Stop, close observation) records nothing and returns.
#[test]
fn route_unarmed_point_records_nothing_and_does_not_block() {
    child(
        "route_unarmed_point_records_nothing_and_does_not_block",
        |f| {
            let o = f.open();
            o.session
                .register_local_model(&fake_ollama().0, spec())
                .unwrap();
            let (door, run, _) = o.door(&f.0);
            let _batch = door.cancel_run(run).unwrap();
            o.ingress.emergency().unwrap();
            o.session.close_obligations().unwrap();
            assert_eq!(drain_trace(), [], "no point records while unarmed");
            let _control = arm("close.obligations", Action::Trace);
            o.session.close_obligations().unwrap();
            let fired: Vec<&str> = drain_trace().iter().map(|e| e.point).collect();
            assert_eq!(
                fired,
                ["close.obligations"],
                "armed, the same route records"
            );
        },
    );
}

/// An armed pause acknowledges with its entry, holds the route before its
/// effect, and the route completes once the hold is released.
#[test]
fn route_armed_pause_acknowledges_blocks_and_releases() {
    child("route_armed_pause_acknowledges_blocks_and_releases", |f| {
        let o = f.open();
        let held = f.pause("ingress.emergency");
        let ingress = o.session.ingress();
        let (done, stopped) = std::sync::mpsc::channel();
        std::thread::spawn(move || done.send(ingress.emergency().is_ok()));
        let entry = held.acknowledged.recv_timeout(PATIENCE).unwrap();
        assert_eq!(entry.point, "ingress.emergency");
        assert_eq!(entry.session, Some(o.session.identity()));
        // A bounded receive that must not complete while the hold stands.
        let blocked = stopped.recv_timeout(Duration::from_millis(200));
        assert_eq!(
            blocked,
            Err(RecvTimeoutError::Timeout),
            "the route is blocked"
        );
        assert_ne!(
            o.session.state(),
            SessionState::Emergency,
            "held before its CAS"
        );
        drop(held);
        assert_eq!(
            stopped.recv_timeout(PATIENCE),
            Ok(true),
            "released, it completes"
        );
        assert_eq!(o.session.state(), SessionState::Emergency);
    });
}

/// The entries carry the driver's own identity: session, request, run, the
/// attempt, the request kind and its per-attempt ordinal checked against the
/// peer's ordered requests, and the store's append kind.
#[test]
fn route_correlated_identity_is_recorded() {
    child("route_correlated_identity_is_recorded", |f| {
        let o = f.open();
        let _trace = arm("reg.reserve", Action::Trace);
        let (endpoint, peer) = fake_ollama();
        o.session.register_local_model(&endpoint, spec()).unwrap();
        let trace = drain_trace();
        let me = Some(o.session.identity());
        let gates: Vec<&Entry> = trace
            .iter()
            .filter(|e| e.point == "model.gate.before")
            .collect();
        let ordinals: Vec<_> = gates.iter().map(|e| e.request_ordinal).collect();
        assert_eq!(ordinals, [Some(1), Some(2), Some(3), Some(4)], "{trace:?}");
        assert!(gates
            .iter()
            .all(|e| e.kind == Some("metadata") && e.session == me));
        assert!(
            gates.iter().all(|e| e.request == Some(0)),
            "the Bootstrap request"
        );
        let seen: Vec<&str> = trace.iter().map(|e| e.point).collect();
        for point in ["reg.reserve", "model.reserved", "model.record.before"] {
            assert!(seen.contains(&point), "{point}: {seen:?}");
        }
        let commits: Vec<&Entry> = trace.iter().filter(|e| e.point == "store.commit").collect();
        assert!(!commits.is_empty() && commits.iter().all(|e| e.kind == Some("ordinary")));
        let (door, run, request) = o.door(&f.0);
        // Two attempts of one bound run, through the public door.
        let ask = MissionConfirmation {
            contract_hash: "ab".repeat(32),
            request: "msn-000001/1/1".into(),
            summary: vec!["Route cell mission.".into()],
            page_facts: None,
        };
        door.confirm_mission(&ask).unwrap();
        door.admit_files(run, &ask.contract_hash, &ask.request)
            .unwrap();
        let identity = ModelIdentity::from_spec(&spec()).unwrap().digest();
        let (hash, label) = (&ask.contract_hash, &ask.request);
        door.admit_inference(run, hash, label, identity).unwrap();
        let schema = ResponseSchema::new(r#"{"type":"object"}"#).unwrap();
        let call = ModelRequest::new("system".into(), "prompt".into(), schema).unwrap();
        let (owner, meter) = (InvocationOwner::new(), Meter(AtomicU32::new(0)));
        // Each call's own trace and peer requests: the attempt this call must
        // carry (independent of the trace) and its ordinals 1..n.
        for expected in [1, 2] {
            drain_trace();
            let served = peer.lock().unwrap().len();
            let outcome = door.model_call(run, &call, &meter, &Never, &owner.observer());
            let result = outcome.unwrap().into_result();
            assert!(result.is_ok(), "attempt {expected}: {:?}", result.err());
            let trace = drain_trace();
            let paths = peer.lock().unwrap()[served..].to_vec();
            let gates: Vec<&Entry> = trace.iter().filter(|e| e.point == "call.gate").collect();
            assert_eq!(gates.len(), paths.len(), "one gate per request served");
            for (n, (gate, path)) in (1..).zip(gates.iter().zip(&paths)) {
                let kind = if path == "/api/chat" {
                    "chat"
                } else {
                    "metadata"
                };
                let ids = (gate.session, gate.request, gate.run, gate.attempt);
                assert_eq!(
                    ids,
                    (me, Some(request), Some(run), Some(expected)),
                    "{path}"
                );
                assert_eq!(
                    (gate.kind, gate.request_ordinal),
                    (Some(kind), Some(n)),
                    "{path}"
                );
            }
            assert_eq!(paths.last().map(String::as_str), Some("/api/chat"));
        }
        let _batch = door.cancel_run(run).unwrap();
        let trace = drain_trace();
        let cancel = trace.iter().find(|e| e.point == "door.cancel_run").unwrap();
        let identity = (cancel.session, cancel.request, cancel.run);
        assert_eq!(identity, (me, Some(request), Some(run)));
        assert_eq!(
            cancel.invocation, None,
            "the broker sees no invocation identity"
        );
    });
}

/// An armed panic acknowledges, then begins an unwind out of the route.
#[test]
fn route_armed_panic_begins_an_unwind() {
    child("route_armed_panic_begins_an_unwind", |f| {
        let o = f.open();
        let held = arm("close.obligations", Action::Panic);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            o.session.close_obligations()
        }));
        let payload = caught.expect_err("the route unwound");
        let message = payload
            .downcast_ref::<String>()
            .cloned()
            .unwrap_or_default();
        assert!(
            message.contains("armed panic at close.obligations"),
            "{message}"
        );
        assert_eq!(
            held.acknowledged.try_recv().unwrap().point,
            "close.obligations"
        );
        assert!(
            o.session.close_obligations().is_ok(),
            "one-shot; nothing poisoned"
        );
    });
}

/// `at` returns `()` and is called only as a statement (also guarded in the
/// standard gate by the site inventory); a released pause returns the
/// route's own result, unchanged from the unarmed route's.
#[test]
fn route_at_cannot_return_into_the_route() {
    let seams = include_str!("../../../../crates/nexus-governed-control/src/broker/route_seams.rs");
    let exact = "\npub(crate) fn at(point: &'static str, ctx: Ctx<'_>) {\n";
    assert_eq!(
        seams.matches(exact).count(),
        1,
        "exact signature, unit return"
    );
    assert_eq!(seams.matches("fn at(").count(), 1, "one writer named at");
    child("route_at_cannot_return_into_the_route", |f| {
        let o = f.open();
        let before = format!("{:?}", o.session.close_obligations().map(|_| ()));
        let held = f.pause("close.obligations");
        let session = &o.session;
        let after = std::thread::scope(|s| {
            let observed = s.spawn(|| format!("{:?}", session.close_obligations().map(|_| ())));
            held.acknowledged.recv_timeout(PATIENCE).unwrap();
            drop(held);
            observed.join().unwrap()
        });
        assert_eq!(before, after);
    });
}

/// The `store.commit` hold fires on an ordinary append (the registration
/// record), inside the evidence mutex, and releases into a recorded result.
/// No public path creates an owed record before S2 (U4), so the owed-flush
/// half of U3b item 4 is not demonstrated here.
#[test]
fn route_store_commit_hold_fires_on_an_ordinary_append() {
    child("route_store_commit_hold_fires_on_an_ordinary_append", |f| {
        let o = f.open();
        let (endpoint, _) = fake_ollama();
        let held = f.pause("store.commit");
        let session = &o.session;
        std::thread::scope(|s| {
            let registering = s.spawn(|| session.register_local_model(&endpoint, spec()));
            let entry = held.acknowledged.recv_timeout(PATIENCE).unwrap();
            assert_eq!(entry.kind, Some("ordinary"));
            assert!(
                o.evidence.try_health().is_none(),
                "paused inside the store mutex"
            );
            drop(held);
            assert!(registering.join().unwrap().is_ok());
        });
        let trace = drain_trace();
        let results = trace.iter().filter(|e| e.point == "store.commit.result");
        assert!(
            results.count() >= 1,
            "COMMIT returned into the route's own result"
        );
    });
}
