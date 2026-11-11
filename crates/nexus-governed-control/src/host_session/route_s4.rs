//! SB-3E S4 (design 4.2, 4.5; AM-2 B.2), seam builds only: a route cell
//! poisons exactly one broker lock. Moved out of the `host_session` facade
//! (SB3E-R32.7), whose lines the Mother C1-S1 guard restricts.
use super::HostSession;

/// SB-3E S4 (design 4.2, 4.5): the broker lock a route cell poisons.
#[cfg(mother_route_seams)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteLock {
    ModelState,
    EvidenceInner,
    RunTable,
    Pending,
    Producers,
    Actions,
}

/// SB-3E S4: exactly the named lock is poisoned by a caught panic while it
/// is held (a prior panic under it); nothing else changes.
#[cfg(mother_route_seams)]
impl HostSession {
    pub fn route_poison(&self, lock: RouteLock) {
        const M: &str = "nexus-mother-route-seam-S4-session";
        std::hint::black_box(M.as_bytes());
        fn poison<T>(mutex: &std::sync::Mutex<T>) {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _held = mutex.lock();
                panic!("route seam S4: session lock poisoned");
            }));
        }
        match lock {
            RouteLock::ModelState => poison(&self.core.model.state),
            RouteLock::EvidenceInner => self.evidence.route_poison_inner(),
            RouteLock::RunTable => self.control.authority().runs().route_poison_runs(),
            RouteLock::Pending => poison(&self.core.pending),
            RouteLock::Producers => poison(&self.core.producers),
            RouteLock::Actions => poison(&self.core.actions),
        }
    }
}
