//! P0-A (design §5.2, oracles first): the host gives Mother her private
//! store in her home, beside the journal. Written in commit 1 against the
//! designed API (`Home::private`), which does not exist on main; declared
//! only in the implementation commit, which does not change it.

use super::Home;
use nexus_governed_control::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, AgentDoorConfirmation, ControlConfirmer,
    GrantConfirmation, MissionConfirmation, MissionResumeConfirmation, ResumeConfirmation,
};
use nexus_mother::private::{ContentKind, PrivateStore};
use std::sync::Arc;

/// Declines every dialog (no door is issued in this test).
struct NoOne;

impl ControlConfirmer for NoOne {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        false
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        false
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        false
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
    fn confirm_mission(&self, _: &MissionConfirmation) -> bool {
        false
    }
    fn confirm_agent_door(&self, _: &AgentDoorConfirmation) -> bool {
        false
    }
    fn confirm_held_mission(&self, _: &MissionResumeConfirmation) -> bool {
        false
    }
}

#[test]
fn p0_home_private_is_the_private_store_in_the_home() {
    let home = Home::fresh_temp("p0-private", Arc::new(NoOne)).unwrap();
    let private = home.private().unwrap();
    let id = private.put(ContentKind::Contract, b"{}").unwrap();
    let path = home
        .dir()
        .join("private")
        .join("contract")
        .join(id.to_hex());
    assert!(path.is_file(), "{}", path.display());
    let again = home.private().unwrap();
    assert_eq!(again.get(ContentKind::Contract, &id).unwrap(), b"{}");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(home.dir().join("private"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700);
    }
    let _ = std::fs::remove_dir_all(home.dir());
}
