//! Mother's desktop (M2b-A): one native GTK4 window that shows the broker's
//! status, read-only, and a Stop button. No webview, no dialogs, no desk:
//! the desktop's confirmer answers no to everything, so Mother cannot act
//! from the desktop, and the window says so.
//!
//! Start order (design §6.4): refuse the GTK inspector; open Mother's home
//! (the broker's front door claims the process's Stop controller); then own
//! the GTK main context, `gtk::init`, and build the window.

#![forbid(unsafe_code)]

pub mod app;
pub mod confirm;
pub mod inspector;
pub mod status;
pub mod stop;
pub mod window;
