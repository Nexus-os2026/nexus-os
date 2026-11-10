//! SB-3E route-cells harness (design 1.3). Compiled only by the route lane,
//! whose wrapper adds `--cfg mother_route_seams`; every standard gate compiles
//! this target to 0 tests.
#![cfg(all(target_os = "linux", mother_route_seams))]

/// Control: the lane's cfg reached this unit. Its presence in the test list
/// is what a standard gate (0 tests) and the lane (this test) differ by.
#[test]
fn route_lane_cfg_reached_the_harness() {
    assert!(cfg!(mother_route_seams));
}
