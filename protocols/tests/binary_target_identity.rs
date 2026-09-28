//! P0-FG1-R1 — the protocols server binary has its own output name.
//!
//! `crates/nexus-server` (dossier item J1, withdrawn) owns the binary name
//! `nexus-server`. The protocols server used to share it, and one Cargo
//! invocation that built both wrote them to the same output paths (on Windows
//! also `deps\nexus_server.exe`). This test pins this package's binary
//! targets through the executables Cargo gives its integration tests. It
//! checks target identity only: it never runs either binary.

use std::env::consts::EXE_SUFFIX;
use std::path::Path;

#[test]
fn p0_fg1_protocols_binary_target_identity() {
    // J1 alone owns the `nexus-server` output name, in either spelling.
    assert!(
        option_env!("CARGO_BIN_EXE_nexus-server").is_none(),
        "nexus-protocols must not build a binary named nexus-server"
    );
    assert!(
        option_env!("CARGO_BIN_EXE_nexus_server").is_none(),
        "nexus-protocols must not build a binary named nexus_server"
    );

    let server = Path::new(env!("CARGO_BIN_EXE_nexus-protocols-server"));
    let os = Path::new(env!("CARGO_BIN_EXE_nexus-os"));
    for (path, name) in [(server, "nexus-protocols-server"), (os, "nexus-os")] {
        assert!(
            path.is_absolute(),
            "{name}: Cargo must give an absolute path, got {}",
            path.display()
        );
        assert!(path.is_file(), "{name}: {} was not built", path.display());
        assert_eq!(
            path.file_name().and_then(|file| file.to_str()),
            Some(format!("{name}{EXE_SUFFIX}").as_str()),
            "{name}: unexpected executable name"
        );
    }
    assert_ne!(
        server, os,
        "the two protocols binaries must not share a path"
    );
}
