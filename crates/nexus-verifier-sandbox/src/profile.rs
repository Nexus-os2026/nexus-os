//! Backend-owned verifier profiles (Phase Two v1).
//!
//! A profile fixes everything about what runs: the toolchain it needs, the
//! executable inside that toolchain, the exact arguments, the working
//! directory, the exact environment and the resource limits. Nothing about a
//! launch comes from a caller or a model: a profile name only selects one of
//! the compiled-in profiles, and paths in the environment are filled in from
//! backend-owned objects (the verified toolchain and the verification
//! workspace), never from text.
//!
//! Exactly one production profile exists: [`VerifierProfileId::RustCargoTestOfflineV1`].

use sha2::Digest as _;

use crate::hash::{domain, put_bytes, put_u64};
use crate::policy::ResourcePolicy;
use crate::ProfileHash;

const PROFILE_DOMAIN: &[u8] = b"nexus.verifier.profile.v1";

/// Name of a compiled-in verifier profile. It selects; it grants nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VerifierProfileId {
    RustCargoTestOfflineV1,
}

impl VerifierProfileId {
    /// Every production profile, in advertisement order.
    pub const PRODUCTION: &'static [Self] = &[Self::RustCargoTestOfflineV1];

    pub fn name(self) -> &'static str {
        self.profile().name
    }

    /// The profile with exactly this name, if one is compiled in. There is
    /// no normalization: a differently spelled name selects nothing.
    pub fn lookup(name: &str) -> Option<Self> {
        Self::PRODUCTION
            .iter()
            .copied()
            .find(|id| id.profile().name == name)
    }

    pub fn profile(self) -> &'static VerifierProfile {
        match self {
            Self::RustCargoTestOfflineV1 => &RUST_CARGO_TEST_OFFLINE_V1,
        }
    }
}

/// A directory of the verification workspace, or the toolchain root, that a
/// profile may name. Its host path is known only at launch, from the
/// backend's retained object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Location {
    ToolchainRoot,
    Input,
    Target,
    Home,
    Tmp,
    CargoHome,
}

impl Location {
    fn code(self) -> u64 {
        self as u64
    }
}

/// An environment value: fixed text, or a relative path beneath a
/// backend-owned location.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    Literal(&'static str),
    Path(Location, &'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnvVar {
    pub key: &'static str,
    pub value: Value,
}

/// The toolchain a profile requires, matched against the packaged
/// toolchain's verified manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolchainRequirement {
    pub name: &'static str,
    pub version: &'static str,
    pub host: &'static str,
    pub target: &'static str,
}

/// One compiled-in verifier profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifierProfile {
    pub id: VerifierProfileId,
    pub name: &'static str,
    pub version: u64,
    /// Bounded, human-readable description for the native prompt.
    pub display_name: &'static str,
    pub toolchain: ToolchainRequirement,
    /// Toolchain-relative path of the executable that runs.
    pub executable: &'static str,
    pub argv0: &'static str,
    pub args: &'static [&'static str],
    pub cwd: Location,
    pub env: &'static [EnvVar],
    pub resources: ResourcePolicy,
    /// The only exit status that means the verification passed.
    pub passing_exit_code: i32,
}

const MUSL_TARGET: &str = "x86_64-unknown-linux-musl";

/// `rust.cargo-test.offline.v1`: the unit and integration tests of a single
/// dependency-free Rust library package, built for the static musl target
/// with the packaged toolchain's own linker, offline.
pub const RUST_CARGO_TEST_OFFLINE_V1: VerifierProfile = VerifierProfile {
    id: VerifierProfileId::RustCargoTestOfflineV1,
    name: "rust.cargo-test.offline.v1",
    version: 1,
    display_name: "Rust library tests (offline)",
    toolchain: ToolchainRequirement {
        name: "rust",
        version: "1.94.0",
        host: "x86_64-unknown-linux-gnu",
        target: MUSL_TARGET,
    },
    executable: "bin/cargo",
    argv0: "cargo",
    args: &[
        "test",
        "--offline",
        "--locked",
        "--no-fail-fast",
        "--lib",
        "--tests",
    ],
    cwd: Location::Input,
    env: &[
        EnvVar {
            key: "HOME",
            value: Value::Path(Location::Home, ""),
        },
        EnvVar {
            key: "TMPDIR",
            value: Value::Path(Location::Tmp, ""),
        },
        EnvVar {
            key: "CARGO_HOME",
            value: Value::Path(Location::CargoHome, ""),
        },
        EnvVar {
            key: "CARGO_TARGET_DIR",
            value: Value::Path(Location::Target, ""),
        },
        EnvVar {
            key: "RUSTC",
            value: Value::Path(Location::ToolchainRoot, "bin/rustc"),
        },
        EnvVar {
            key: "CARGO_BUILD_TARGET",
            value: Value::Literal(MUSL_TARGET),
        },
        EnvVar {
            key: "CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER",
            value: Value::Path(
                Location::ToolchainRoot,
                "lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld",
            ),
        },
        EnvVar {
            key: "CARGO_ENCODED_RUSTFLAGS",
            value: Value::Literal("-Clink-self-contained=yes\u{1f}-Clinker-flavor=ld.lld"),
        },
        EnvVar {
            key: "CARGO_NET_OFFLINE",
            value: Value::Literal("true"),
        },
        EnvVar {
            key: "CARGO_TERM_COLOR",
            value: Value::Literal("never"),
        },
        EnvVar {
            key: "CARGO_INCREMENTAL",
            value: Value::Literal("0"),
        },
        EnvVar {
            key: "CARGO_BUILD_JOBS",
            value: Value::Literal("4"),
        },
        EnvVar {
            key: "RUST_TEST_THREADS",
            value: Value::Literal("4"),
        },
        EnvVar {
            key: "LC_ALL",
            value: Value::Literal("C"),
        },
        EnvVar {
            key: "TZ",
            value: Value::Literal("UTC"),
        },
    ],
    resources: ResourcePolicy::RUST_OFFLINE_V1,
    passing_exit_code: 0,
};

impl VerifierProfile {
    /// Canonical identity of everything the profile would run and how.
    pub fn hash(&self) -> ProfileHash {
        let mut hasher = domain(PROFILE_DOMAIN);
        put_bytes(&mut hasher, self.name.as_bytes());
        put_u64(&mut hasher, self.version);
        put_bytes(&mut hasher, self.display_name.as_bytes());
        for text in [
            self.toolchain.name,
            self.toolchain.version,
            self.toolchain.host,
            self.toolchain.target,
            self.executable,
            self.argv0,
        ] {
            put_bytes(&mut hasher, text.as_bytes());
        }
        put_u64(&mut hasher, self.args.len() as u64);
        for arg in self.args {
            put_bytes(&mut hasher, arg.as_bytes());
        }
        put_u64(&mut hasher, self.cwd.code());
        put_u64(&mut hasher, self.env.len() as u64);
        for var in self.env {
            put_bytes(&mut hasher, var.key.as_bytes());
            match var.value {
                Value::Literal(text) => {
                    put_u64(&mut hasher, 0);
                    put_bytes(&mut hasher, text.as_bytes());
                }
                Value::Path(location, relative) => {
                    put_u64(&mut hasher, 1);
                    put_u64(&mut hasher, location.code());
                    put_bytes(&mut hasher, relative.as_bytes());
                }
            }
        }
        hasher.update(self.resources.hash().bytes());
        hasher.update(self.passing_exit_code.to_be_bytes());
        ProfileHash::finish(hasher)
    }
}

#[cfg(test)]
mod tests;
