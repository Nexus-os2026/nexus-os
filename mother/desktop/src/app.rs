//! The desktop's start and Quit (design §6.4, §6.6, §11).
//!
//! The start order:
//! 1. refuse to start if `GTK_DEBUG` asks for the inspector;
//! 2. open Mother's home: the broker's front door claims the process's
//!    Stop controller (F11); on failure, exit with no window;
//! 3. own the default GTK main context on this thread, then `gtk::init`
//!    and the window. With no display the app exits; the home was opened
//!    first and is dropped, and no door was issued.

use crate::confirm::NoDialogs;
use crate::inspector::inspector_requested;
use crate::status::spawn_feed;
use crate::stop::{quit, Desk, QuitOutcome, StopRoute};
use crate::window::{build, AtStart, MainWindow, Probe};
use gtk::glib;
use gtk::prelude::*;
use gtk4 as gtk;
use nexus_governed_control::governed::GovernedControl;
use nexus_mother_host::Home;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

/// How long Quit waits for the Stop workers and the broker's shutdown.
pub const QUIT_WITHIN: Duration = Duration::from_secs(5);

/// How often the backend reads the broker's status.
const STATUS_EVERY: Duration = Duration::from_millis(250);

/// Why the desktop did not start.
#[derive(Debug)]
pub enum StartError {
    /// `GTK_DEBUG` asks for the inspector (S9): nothing was opened.
    InspectorRequested,
    /// No valid identity home.
    NoHome,
    /// Mother's home or its broker could not be opened.
    Broker(String),
    /// The GTK main context could not be owned.
    Context(String),
    /// No display, or `gtk::init` failed. The home was opened first.
    Display(String),
}

impl std::fmt::Display for StartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StartError::InspectorRequested => write!(
                f,
                "GTK_DEBUG asks for the GTK inspector; Mother does not start with it"
            ),
            StartError::NoHome => write!(f, "no valid identity home (HOME)"),
            StartError::Broker(why) => write!(f, "Mother's home could not be opened: {why}"),
            StartError::Context(why) => write!(f, "the GTK main context: {why}"),
            StartError::Display(why) => write!(f, "no display: {why}"),
        }
    }
}

/// A started desktop: the broker, the Stop route and the window.
pub struct Started {
    pub owner: Arc<GovernedControl>,
    pub route: Arc<StopRoute>,
    pub window: MainWindow,
    pub probe: Arc<Probe>,
    pub main_loop: glib::MainLoop,
    pub home_dir: PathBuf,
}

/// Mother's home: `<identity home>/.nexus/mother`.
pub fn home_dir() -> Result<PathBuf, StartError> {
    nexus_kernel::identity_home::nexus_state_path("mother").map_err(|_| StartError::NoHome)
}

/// Start the production desktop at `home_dir`: `Home::open` with the
/// desktop's confirmer, which answers no.
pub fn start(home_dir: &Path, gtk_debug: Option<&OsStr>) -> Result<Started, StartError> {
    launch(home_dir, gtk_debug, |dir| {
        Home::open(dir, Arc::new(NoDialogs))
            .map(|home| home.owner().clone())
            .map_err(|e| e.0)
    })
}

/// The start order, with the broker opened by `open`: the inspector check,
/// then the broker (which claims the Stop controller), then the GTK main
/// context, `gtk::init` and the window.
pub fn launch(
    home_dir: &Path,
    gtk_debug: Option<&OsStr>,
    open: impl FnOnce(&Path) -> Result<Arc<GovernedControl>, String>,
) -> Result<Started, StartError> {
    // 1. The inspector: nothing is opened if it is asked for.
    if inspector_requested(gtk_debug) {
        return Err(StartError::InspectorRequested);
    }
    // 2. The broker, before GTK.
    let owner = open(home_dir).map_err(StartError::Broker)?;
    let at_start = if owner.status().emergency_stopped && owner.authority().recovery_required() {
        AtStart::StoppedForRecovery
    } else {
        AtStart::NotStopped
    };
    // 3. This thread owns the default main context for the process's life;
    //    backend threads never call GTK.
    let context: &'static glib::MainContext = Box::leak(Box::new(glib::MainContext::default()));
    let guard = context
        .acquire()
        .map_err(|e| StartError::Context(e.to_string()))?;
    if !context.is_owner() {
        return Err(StartError::Context("not the owner after acquiring".into()));
    }
    std::mem::forget(guard);
    gtk::init().map_err(|e| StartError::Display(e.to_string()))?;
    gtk::Settings::default()
        .ok_or_else(|| StartError::Display("no GTK settings".into()))?
        .set_gtk_enable_primary_paste(false);

    let route = StopRoute::new(Desk::new(), owner.clone());
    let probe = Arc::new(Probe::default());
    let main_loop = glib::MainLoop::new(Some(context), false);
    let ending = main_loop.clone();
    let window = build(
        route.clone(),
        spawn_feed(owner.clone(), STATUS_EVERY),
        home_dir,
        at_start,
        probe.clone(),
        Rc::new(move || ending.quit()),
    );
    Ok(Started {
        owner,
        route,
        window,
        probe,
        main_loop,
        home_dir: home_dir.to_path_buf(),
    })
}

/// After the main loop ended: close the window and Quit within `within`.
pub fn finish(started: Started, within: Duration) -> QuitOutcome {
    started.window.window.destroy();
    quit(&started.route, &started.owner, within)
}

/// The binary: start, run the main loop, Quit; the exit code.
pub fn main_entry() -> i32 {
    let gtk_debug = std::env::var_os("GTK_DEBUG");
    let home = match home_dir() {
        Ok(home) => home,
        Err(e) => {
            eprintln!("nexus-mother: {e}");
            return 2;
        }
    };
    let started = match start(&home, gtk_debug.as_deref()) {
        Ok(started) => started,
        Err(e) => {
            eprintln!("nexus-mother: {e}");
            return 2;
        }
    };
    started.main_loop.run();
    let outcome = finish(started, QUIT_WITHIN);
    eprintln!("nexus-mother: {}", outcome.report());
    outcome.exit_code()
}
