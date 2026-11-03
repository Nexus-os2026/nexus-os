//! The GTK inspector stays off (design §3 T12, S9). It is in the same-user
//! exclusion by every route; this refusal is defence in depth only: the app
//! does not start when `GTK_DEBUG` asks for the interactive inspector.

use std::ffi::OsStr;

/// Whether `GTK_DEBUG` (its raw value, if set) asks for the inspector:
/// one of its flags is `interactive` or `all` (GTK splits the value at
/// commas, colons, semicolons and spaces). A value that is not valid text
/// counts as asking (fail closed).
pub fn inspector_requested(gtk_debug: Option<&OsStr>) -> bool {
    let Some(value) = gtk_debug else {
        return false;
    };
    let Some(text) = value.to_str() else {
        return true;
    };
    text.split([',', ':', ';', ' ', '\t'])
        .map(str::trim)
        .any(|flag| flag.eq_ignore_ascii_case("interactive") || flag.eq_ignore_ascii_case("all"))
}
