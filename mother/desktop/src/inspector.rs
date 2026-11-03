//! The GTK inspector stays off (design §3 T12, S9). It is in the same-user
//! exclusion by every route; this refusal is defence in depth only: the app
//! does not start when `GTK_DEBUG` asks for the interactive inspector.

use std::ffi::OsStr;

/// Whether `GTK_DEBUG` (its raw value, if set) asks for the inspector. A
/// value that is not valid text counts as asking (fail closed).
pub fn inspector_requested(gtk_debug: Option<&OsStr>) -> bool {
    let _ = gtk_debug;
    unimplemented!("M2b-A PR-2 code")
}
