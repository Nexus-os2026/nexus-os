//! SB-3E S3 (design 4.2), seam builds only: a route cell revokes the
//! original door's own File grant by the production revocation.
use super::{DoorKind, SessionDoor};

/// SB-3E S3: `GovernedControl::revoke_grant` on this original door's File
/// grant: the revocation, its `GrantRevoked` record, then `settle()`. A
/// continuation door holds no grant and revokes nothing.
#[cfg(mother_route_seams)]
impl SessionDoor {
    pub fn route_revoke_original_grant(&self) -> bool {
        const M: &str = "nexus-mother-route-seam-S3-door";
        std::hint::black_box(M.as_bytes());
        let DoorKind::Original { file_grant } = self.kind else {
            return false;
        };
        let Some(control) = self.door.session_backend().upgrade() else {
            return false;
        };
        control.revoke_grant(file_grant).is_ok()
    }
}
