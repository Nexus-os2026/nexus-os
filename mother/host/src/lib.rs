//! Mother's host (Mother Nexus M2c, N2): the one production place that
//! builds the governed-control broker and its evidence store, and issues
//! Mother's agent door. Mother itself (`nexus-mother`) builds no broker and
//! holds only the door it is given.
//!
//! [`Home`] is the reference for the desktop host when it adopts Mother:
//! the broker built as the desktop builds it, Mother's journal file, and
//! [`Home::door`], which asks the Owner, through the confirmer the host
//! supplies, for every door. The Owner's side of the broker
//! ([`Home::owner`]) stays here; nothing given to Mother leads to it.
//!
//! P0: Mother's private store ([`Home::private`]) is in her home beside the
//! journal; the coordinator is opened with both.

#![deny(unsafe_code)]

use nexus_governed_control::authority::approval::ControlConfirmer;
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::evidence::MemoryEvidence;
use nexus_governed_control::authority::AuthorityError;
use nexus_governed_control::broker::Vault;
use nexus_governed_control::evidence_store::PrimaryEvidence;
use nexus_governed_control::governed::{AgentDoor, GovernedControl};
use nexus_mother::adapter::{create_private, mother_agent, FileJournal, FilePrivate};
use nexus_mother::journal::JournalError;
use nexus_mother::private::PrivateError;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The confirmer the host binds into every door it issues: the Owner's
/// native dialogs.
pub type Confirmer = Arc<dyn ControlConfirmer + Send + Sync>;

/// Mother's home: the broker built as the desktop builds it, the journal
/// file, and the Owner's confirmer. One per process: the broker claims the
/// process's emergency-stop controller.
pub struct Home {
    dir: PathBuf,
    control: Arc<GovernedControl>,
    confirmer: Confirmer,
}

/// Why a home cannot be opened.
#[derive(Debug)]
pub struct HomeError(pub String);

impl Home {
    /// Open (or create) the home at `dir`, an absolute path the host
    /// chose; `confirmer` is the Owner's native dialogs.
    pub fn open(dir: &Path, confirmer: Confirmer) -> Result<Self, HomeError> {
        if !dir.is_absolute() {
            return Err(HomeError("the home must be an absolute path".into()));
        }
        create_private(dir).map_err(|e| HomeError(e.to_string()))?;
        let broker = dir.join("broker");
        let evidence = PrimaryEvidence::open(
            &broker.join("p3-evidence").join("evidence.sqlite3"),
            vec![Arc::new(MemoryEvidence::new(256))],
        )
        .map_err(|e| HomeError(format!("evidence store: {}", e.as_str())))?;
        let control = GovernedControl::new(
            &broker,
            Vault::Disabled,
            evidence,
            Arc::new(SystemClock::default()),
        )
        .map_err(|e| HomeError(format!("broker: {e:?}")))?;
        Ok(Self {
            dir: dir.to_path_buf(),
            control: Arc::new(control),
            confirmer,
        })
    }

    /// A new home under the system's temporary directory (examples):
    /// refused if it already exists.
    pub fn fresh_temp(label: &str, confirmer: Confirmer) -> Result<Self, HomeError> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let dir = std::env::temp_dir().join(format!("nexus-mother-{label}-{nanos}"));
        if dir.exists() {
            return Err(HomeError("the home already exists".into()));
        }
        Self::open(&dir, confirmer)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn journal_path(&self) -> PathBuf {
        self.dir.join("mother-journal.jsonl")
    }

    pub fn journal(&self) -> Result<FileJournal, JournalError> {
        FileJournal::open(&self.journal_path())
    }

    /// P0: where Mother keeps the contracts and paused rests her journal
    /// names (`<home>/private`, 0700).
    pub fn private_path(&self) -> PathBuf {
        self.dir.join("private")
    }

    /// P0: Mother's private store, for `Coordinator::open`.
    ///
    /// P0-A (oracles first; registered as `host/src/p0_tests.rs`, moved
    /// here unchanged): the host gives Mother her private store in her
    /// home, beside the journal.
    ///
    /// ```
    /// use nexus_governed_control::authority::approval::{
    ///     AcknowledgementConfirmation, ActionConfirmation, AgentDoorConfirmation, ControlConfirmer,
    ///     GrantConfirmation, MissionConfirmation, MissionResumeConfirmation, ResumeConfirmation,
    /// };
    /// use nexus_mother::private::{ContentKind, PrivateStore};
    /// use nexus_mother_host::Home;
    /// use std::sync::Arc;
    ///
    /// /// Declines every dialog (no door is issued in this test).
    /// struct NoOne;
    ///
    /// impl ControlConfirmer for NoOne {
    ///     fn confirm_action(&self, _: &ActionConfirmation) -> bool {
    ///         false
    ///     }
    ///     fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
    ///         false
    ///     }
    ///     fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
    ///         false
    ///     }
    ///     fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
    ///         false
    ///     }
    ///     fn confirm_mission(&self, _: &MissionConfirmation) -> bool {
    ///         false
    ///     }
    ///     fn confirm_agent_door(&self, _: &AgentDoorConfirmation) -> bool {
    ///         false
    ///     }
    ///     fn confirm_held_mission(&self, _: &MissionResumeConfirmation) -> bool {
    ///         false
    ///     }
    /// }
    ///
    /// let home = Home::fresh_temp("p0-private", Arc::new(NoOne)).unwrap();
    /// let private = home.private().unwrap();
    /// let id = private.put(ContentKind::Contract, b"{}").unwrap();
    /// let path = home
    ///     .dir()
    ///     .join("private")
    ///     .join("contract")
    ///     .join(id.to_hex());
    /// assert!(path.is_file(), "{}", path.display());
    /// let again = home.private().unwrap();
    /// assert_eq!(again.get(ContentKind::Contract, &id).unwrap(), b"{}");
    /// #[cfg(unix)]
    /// {
    ///     use std::os::unix::fs::PermissionsExt;
    ///     let mode = std::fs::metadata(home.dir().join("private"))
    ///         .unwrap()
    ///         .permissions()
    ///         .mode();
    ///     assert_eq!(mode & 0o777, 0o700);
    /// }
    /// let _ = std::fs::remove_dir_all(home.dir());
    /// ```
    pub fn private(&self) -> Result<FilePrivate, PrivateError> {
        FilePrivate::open(&self.private_path())
    }

    /// A new door for Mother. The Owner is asked every time, through the
    /// host's confirmer, which the door keeps for the mission and
    /// held-mission dialogs it raises; a no gives no door.
    pub fn door(&self) -> Result<AgentDoor, AuthorityError> {
        self.control
            .issue_agent_door(mother_agent(), self.confirmer.clone())
    }

    /// The Owner's side of the broker (grants, approvals, the emergency
    /// stop). Never hand it to Mother.
    pub fn owner(&self) -> &Arc<GovernedControl> {
        &self.control
    }
}
