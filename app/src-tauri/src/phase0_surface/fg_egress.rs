//! P0-FINAL-GATE-CLOSURE guards for items B and F (destinations and Nexus
//! Link), C (credentials in process arguments) and I (helper programs).
//!
//! The guards live in `fg_egress/tests.rs`: the Phase Zero production-source
//! scanners in `phase0_surface/tests.rs` skip files named `tests.rs`, so the
//! needles and fixtures these guards spell out are never read as production
//! code.

mod tests;
