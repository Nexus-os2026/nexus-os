//! Executable identity on the real filesystem.
use super::*;

#[test]
fn system_executables_are_pinned_by_content_and_location() {
    let a = inspect(Path::new("/usr/bin/sha256sum"), Trust::System).unwrap();
    let b = inspect(Path::new("/usr/bin/sha256sum"), Trust::System).unwrap();
    assert_eq!(a, b, "stable");
    let other = inspect(Path::new("/usr/bin/env"), Trust::System).unwrap();
    assert_ne!(a.digest, other.digest);
    assert!(
        inspect(Path::new("usr/bin/env"), Trust::System).is_err(),
        "relative"
    );
    assert!(
        inspect(Path::new("/usr/bin/../bin/env"), Trust::System).is_err(),
        "not canonical"
    );
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
    std::fs::copy("/usr/bin/true", &copy).unwrap();
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
