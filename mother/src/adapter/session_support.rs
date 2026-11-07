//! Real mandatory session fixture. No raw backend authority is exported.
use crate::adapter::{create_private, FileJournal, FilePrivate, SessionPort};
use crate::journal::JournalStore;
use crate::oracles::session_support::Owner;
use nexus_governed_control::authority::clock::ManualClock;
use nexus_governed_control::host_session::{CommandKind, HostSession, SessionSeed};
use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static NEXT_HOME: AtomicU64 = AtomicU64::new(0);

pub(crate) struct Fixture {
    pub dir: PathBuf,
    pub workspace: PathBuf,
    pub session: HostSession,
    pub port: Arc<SessionPort>,
    pub owner: Arc<Owner>,
    pub clock: Arc<ManualClock>,
    next: AtomicU64,
}
impl Fixture {
    pub fn new(label: &str) -> Self {
        Self::with_inspector(label, false)
    }
    pub fn canonical(label: &str) -> Self {
        Self::with_inspector(label, true)
    }
    fn with_inspector(label: &str, canonical: bool) -> Self {
        // A PID alone is reused after wrap; a leaked or concurrent home is
        // never reopened, and this fixture removes only the home it created.
        let parent = std::env::temp_dir();
        create_private(&parent).unwrap();
        let dir = parent.join(format!(
            "m2dd-{label}-{}-{}-{}",
            std::process::id(),
            NEXT_HOME.fetch_add(1, Ordering::SeqCst),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        match std::fs::DirBuilder::new().mode(0o700).create(&dir) {
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                panic!("fixture home must be fresh: {}", dir.display())
            }
            created => created.unwrap(),
        }
        let broker = dir.join("broker");
        let evidence = nexus_governed_control::evidence_store::PrimaryEvidence::open(
            &broker.join("p3-evidence/evidence.sqlite3"),
            vec![],
        )
        .unwrap();
        let clock = Arc::new(ManualClock::default());
        let owner = Arc::new(Owner::default());
        let mut session = HostSession::open_with_history(
            SessionSeed::new().unwrap(),
            &broker,
            &dir,
            evidence,
            clock.clone(),
            owner.clone(),
        )
        .unwrap();
        if canonical {
            session
                .bind_history_inspector(Arc::new(crate::adapter::SessionHistoryInspector))
                .unwrap();
        }
        let workspace = dir.join("workspace");
        create_private(&workspace).unwrap();
        let info = session.bootstrap_workspace(&workspace, &[]).unwrap();
        session.finish_bootstrap().unwrap();
        let request = session.ingress().accept(CommandKind::Run, 1).unwrap();
        let port = Arc::new(SessionPort::new(
            session.open_file_door(&request).unwrap(),
            info.name,
        ));
        Self {
            dir,
            workspace,
            session,
            port,
            owner,
            clock,
            next: AtomicU64::new(2),
        }
    }
    pub fn journal(&self) -> FileJournal {
        FileJournal::open(&self.dir.join("mother-journal.jsonl")).unwrap()
    }
    pub fn lines(&self) -> Vec<String> {
        self.journal().load().unwrap()
    }
    pub fn private(&self) -> FilePrivate {
        FilePrivate::open_session(&self.dir.join("private"), self.port.context()).unwrap()
    }
    pub fn approve(
        &self,
    ) -> Result<
        nexus_governed_control::control::EffectOutput,
        nexus_governed_control::host_session::SessionFailure,
    > {
        let sequence = self.next.fetch_add(1, Ordering::SeqCst);
        let request = self
            .session
            .ingress()
            .accept(CommandKind::Approve, sequence)
            .unwrap();
        self.session.approve_file(&request)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Only the uniquely named home this fixture created.
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

// TH-R1: the PID-reuse collision, made deterministic. One child leaks its
// home (exit without Drop, its session unclosed, like a crashed owner). The
// parent moves that home to the name a second, waiting child would derive
// from its own PID, then lets it build its fixture. Each fixture must still
// get a fresh, startup-ready home; the leaked one is never reopened.
#[test]
fn session_fixture_never_reuses_a_leaked_home() {
    use std::io::{BufRead, Write};
    use std::process::{Command, Stdio};
    const MODE: &str = "M2DD_TH_R1_CHILD";
    const TEST: &str = "adapter::session_support::session_fixture_never_reuses_a_leaked_home";
    match std::env::var(MODE).as_deref() {
        Ok("leak") => {
            let home = Fixture::new("stale-home");
            println!("TH-HOME {}", home.dir.display());
            std::process::exit(0);
        }
        Ok("fresh") => {
            let mut go = String::new();
            std::io::stdin().read_line(&mut go).unwrap();
            let home = Fixture::new("stale-home");
            assert!(home.session.startup_report().ready());
            println!("TH-HOME {}", home.dir.display());
            return;
        }
        _ => {}
    }
    let home_of = |text: &str| {
        let line = text
            .lines()
            .find_map(|l| l.strip_prefix("TH-HOME "))
            .unwrap();
        PathBuf::from(line)
    };
    let leak = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", TEST, "--nocapture"])
        .env(MODE, "leak")
        .output()
        .unwrap();
    assert!(leak.status.success());
    let leaked = home_of(&String::from_utf8_lossy(&leak.stdout));
    let mut fresh = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", TEST, "--nocapture"])
        .env(MODE, "fresh")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let name = leaked.file_name().unwrap().to_str().unwrap().replacen(
        &format!("-{}", leak_pid(&leaked)),
        &format!("-{}", fresh.id()),
        1,
    );
    let planted = leaked.with_file_name(name);
    std::fs::rename(&leaked, &planted).unwrap();
    let journal = std::fs::read(planted.join("mother-journal.jsonl")).ok();
    fresh.stdin.take().unwrap().write_all(b"go\n").unwrap();
    let stdout = std::io::BufReader::new(fresh.stdout.take().unwrap());
    let reader = std::thread::spawn(move || stdout.lines().map(Result::unwrap).collect::<Vec<_>>());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let status = loop {
        if let Some(status) = fresh.try_wait().unwrap() {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            fresh.kill().unwrap();
            break fresh.wait().unwrap();
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    let out = reader.join().unwrap().join("\n");
    let mut err = String::new();
    std::io::Read::read_to_string(&mut fresh.stderr.take().unwrap(), &mut err).unwrap();
    let reused = std::fs::read(planted.join("mother-journal.jsonl")).ok() != journal;
    let _ = std::fs::remove_dir_all(&planted);
    assert!(
        status.success(),
        "a leaked home at this PID's name must not be reopened: {out}{err}"
    );
    assert_ne!(home_of(&out), planted, "fresh home, never the leaked one");
    assert!(!reused, "the leaked home is left untouched");
}
fn leak_pid(home: &std::path::Path) -> u32 {
    // m2dd-stale-home-{pid}[-...]: the first numeric component after the label.
    let name = home.file_name().unwrap().to_str().unwrap();
    let rest = name.strip_prefix("m2dd-stale-home-").unwrap();
    rest.split('-').next().unwrap().parse().unwrap()
}
