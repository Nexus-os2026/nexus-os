//! Executable identity on the real filesystem.
use super::*;
use crate::harness_tests::canonical_tool;

#[test]
fn system_executables_are_pinned_by_content_and_location() {
    let sha256sum = Path::new(canonical_tool("sha256sum"));
    let env = Path::new(canonical_tool("env"));
    let a = inspect(sha256sum, Trust::System).unwrap();
    let b = inspect(sha256sum, Trust::System).unwrap();
    assert_eq!(a, b, "stable");
    let other = inspect(env, Trust::System).unwrap();
    assert_ne!(a.digest, other.digest);
    let relative = env.strip_prefix("/").unwrap();
    assert!(inspect(relative, Trust::System).is_err(), "relative");
    // The same file, named through a `..` detour.
    let dir = env.parent().unwrap();
    let detour = dir
        .join("..")
        .join(dir.file_name().unwrap())
        .join(env.file_name().unwrap());
    assert!(inspect(&detour, Trust::System).is_err(), "not canonical");
    assert!(inspect(
        Path::new("/usr/bin/definitely-not-installed"),
        Trust::System
    )
    .is_err());
    assert!(
        inspect(Path::new("/usr/bin"), Trust::System).is_err(),
        "a directory"
    );
}

#[test]
fn a_user_writable_location_is_not_trusted() {
    let dir = std::env::temp_dir().join(format!("nexus-p3-exe-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let copy = dir.join("true-copy");
    std::fs::copy(canonical_tool("true"), &copy).unwrap();
    let copy = std::fs::canonicalize(&copy).unwrap();
    assert_eq!(
        inspect(&copy, Trust::System).unwrap_err(),
        AuthorityError::Closed("the executable is not in a trusted location")
    );
    // As a fixture it can be pinned, and any change is a new identity.
    let pinned = inspect(&copy, Trust::Fixture).unwrap();
    std::fs::write(&copy, b"#!/bin/false\n").unwrap();
    let changed = inspect(&copy, Trust::Fixture);
    assert!(changed.map(|c| c.digest != pinned.digest).unwrap_or(true));
    std::fs::remove_dir_all(&dir).unwrap();
}

/// An installation directory anything in which another user owns or may
/// write is refused (a system browser's whole directory must be root-only).
#[test]
fn an_installation_others_could_change_is_refused() {
    let dir = std::env::temp_dir().join(format!("nexus-p3-installation-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("locales")).unwrap();
    std::fs::write(dir.join("locales").join("en.pak"), b"pak").unwrap();
    let refused = super::inspect_tree(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(
        refused,
        Err(AuthorityError::Closed(
            "the executable's installation is not in a trusted location"
        ))
    );
}

/// M0-FIX2 F13 fixture: a private installation with an executable copy of
/// `true` at `real/tool` and a symbolic link `bin/tool -> ../real/tool`.
struct Installation {
    dir: PathBuf,
}

impl Installation {
    fn new(label: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("nexus-m0-fix2-f13-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("real")).unwrap();
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        let dir = std::fs::canonicalize(&dir).unwrap();
        std::fs::copy(canonical_tool("true"), dir.join("real").join("tool")).unwrap();
        std::os::unix::fs::symlink("../real/tool", dir.join("bin").join("tool")).unwrap();
        Self { dir }
    }

    fn real(&self) -> String {
        self.dir.join("real").join("tool").display().to_string()
    }

    fn link(&self) -> String {
        self.dir.join("bin").join("tool").display().to_string()
    }
}

impl Drop for Installation {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn names(paths: &[String]) -> Vec<&str> {
    paths.iter().map(String::as_str).collect()
}

/// M0-FIX2 F13 oracle: a symbolic link among a tool's trusted locations is
/// followed to another of its trusted locations, and what is pinned is
/// that canonical target (exactly what `inspect` pins for it).
#[test]
fn m0_fix2_f13_a_link_to_a_listed_location_pins_its_canonical_target() {
    let install = Installation::new("listed");
    let candidates = [install.link(), install.real()];
    let identity = resolve(&names(&candidates), Trust::Fixture).unwrap();
    assert_eq!(identity.path, Path::new(&install.real()));
    assert_eq!(
        identity,
        inspect(Path::new(&install.real()), Trust::Fixture).unwrap()
    );
    // `inspect` itself still refuses the link.
    assert_eq!(
        inspect(Path::new(&install.link()), Trust::Fixture).unwrap_err(),
        AuthorityError::Closed("an executable is named by its canonical path")
    );
}

/// M0-FIX2 F13 oracle: a link to a location the tool does not list is
/// refused, even though that target is a valid executable on its own.
#[test]
fn m0_fix2_f13_a_link_to_an_unlisted_location_is_refused() {
    let install = Installation::new("unlisted");
    assert!(inspect(Path::new(&install.real()), Trust::Fixture).is_ok());
    let candidates = [install.link()];
    assert!(
        resolve(&names(&candidates), Trust::Fixture).is_err(),
        "a link was followed to an unlisted location"
    );
}

/// M0-FIX2 F13 oracle: with no listed location installed, nothing is
/// resolved (no `PATH` lookup, no other fallback).
#[test]
fn m0_fix2_f13_no_installed_location_is_refused() {
    let install = Installation::new("absent");
    let candidates = [
        install.dir.join("none").join("tool").display().to_string(),
        install.dir.join("bin").join("other").display().to_string(),
    ];
    assert!(resolve(&names(&candidates), Trust::Fixture).is_err());
    // A bare name is never looked up in `PATH`.
    assert!(resolve(&["sha256sum"], Trust::System).is_err());
    assert!(resolve(&[], Trust::System).is_err());
}

/// M0-FIX2 F13 oracle: a listed, canonical location is still accepted only
/// if `inspect` accepts it.
#[test]
fn m0_fix2_f13_a_listed_location_must_still_pass_inspection() {
    use std::os::unix::fs::PermissionsExt;
    let install = Installation::new("writable");
    std::fs::set_permissions(install.real(), std::fs::Permissions::from_mode(0o777)).unwrap();
    let candidates = [install.link(), install.real()];
    assert!(resolve(&names(&candidates), Trust::Fixture).is_err());
}

/// M0-FIX2 F13 review (MINOR-1) oracle: the first listed location that is
/// present decides. If it resolves outside the list it is refused, and a
/// valid later location does not hide that.
#[test]
fn m0_fix2_f13_a_present_first_location_outside_the_list_is_not_passed_over() {
    let install = Installation::new("first-unlisted");
    let other = Installation::new("first-unlisted-other");
    // First: a link to an unlisted (valid) executable. Later: a valid,
    // listed one.
    let candidates = [install.link(), other.real()];
    assert!(inspect(Path::new(&install.real()), Trust::Fixture).is_ok());
    assert_eq!(
        resolve(&names(&candidates), Trust::Fixture).unwrap_err(),
        AuthorityError::Closed("the executable resolves outside its trusted locations")
    );
}

/// M0-FIX2 F13 review (MINOR-1) oracle: a present first location that fails
/// `inspect` is refused with `inspect`'s error, even though a valid later
/// location exists.
#[test]
fn m0_fix2_f13_a_present_first_location_failing_inspection_is_not_passed_over() {
    use std::os::unix::fs::PermissionsExt;
    let install = Installation::new("first-writable");
    let other = Installation::new("first-writable-other");
    std::fs::set_permissions(install.real(), std::fs::Permissions::from_mode(0o777)).unwrap();
    let candidates = [install.real(), other.real()];
    assert_eq!(
        resolve(&names(&candidates), Trust::Fixture).unwrap_err(),
        AuthorityError::Closed("the executable is not in a trusted location")
    );
}

/// M0-FIX2 F13 review (MINOR-1): only a location that is not there at all
/// is passed over (a host without the first location uses the next).
#[test]
fn m0_fix2_f13_an_absent_location_is_passed_over() {
    let install = Installation::new("first-absent");
    let candidates = [
        install.dir.join("none").join("tool").display().to_string(),
        install.real(),
    ];
    let identity = resolve(&names(&candidates), Trust::Fixture).unwrap();
    assert_eq!(identity.path, Path::new(&install.real()));
}
