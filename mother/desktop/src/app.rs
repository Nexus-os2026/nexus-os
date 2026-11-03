//! The desktop's start and Quit (design §6.4, §6.6, §11).

use crate::stop::{QuitOutcome, StopRoute};
use crate::window::{MainWindow, Probe};
use gtk::glib;
use gtk4 as gtk;
use nexus_governed_control::governed::GovernedControl;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// How long Quit waits for the Stop workers and the broker's shutdown.
pub const QUIT_WITHIN: Duration = Duration::from_secs(5);

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
    unimplemented!("M2b-A PR-2 code")
}

/// Start the production desktop at `home_dir`: `Home::open` with the
/// desktop's confirmer, which answers no.
pub fn start(home_dir: &Path, gtk_debug: Option<&OsStr>) -> Result<Started, StartError> {
    let _ = (home_dir, gtk_debug);
    unimplemented!("M2b-A PR-2 code")
}

/// The start order, with the broker opened by `open`: the inspector check,
/// then the broker (which claims the Stop controller), then the GTK main
/// context, `gtk::init` and the window.
pub fn launch(
    home_dir: &Path,
    gtk_debug: Option<&OsStr>,
    open: impl FnOnce(&Path) -> Result<Arc<GovernedControl>, String>,
) -> Result<Started, StartError> {
    let _ = (home_dir, gtk_debug, open);
    unimplemented!("M2b-A PR-2 code")
}

/// After the main loop ended: close the window and Quit within `within`.
pub fn finish(started: Started, within: Duration) -> QuitOutcome {
    let _ = (started, within);
    unimplemented!("M2b-A PR-2 code")
}

/// The binary: start, run the main loop, Quit; the exit code.
pub fn main_entry() -> i32 {
    unimplemented!("M2b-A PR-2 code")
}
