//! Sandbox and resource policy identities (Phase Two v1).
//!
//! The sandbox policy is the complete set of isolation commitments a launch
//! must establish and verify. It has no optional part: a launch that cannot
//! establish every commitment does not run. The resource policy is the
//! profile-owned set of limits the backend applies to the execution's cgroup.
//!
//! Landlock rights are given by their kernel UAPI bit values, so this module
//! states the policy without depending on the sandbox implementation.

use sha2::Sha256;

use crate::hash::{domain, put_bytes, put_u64};
use crate::{ResourcePolicyHash, SandboxPolicyHash};

const SANDBOX_DOMAIN: &[u8] = b"nexus.verifier.sandbox_policy.v1";
const RESOURCE_DOMAIN: &[u8] = b"nexus.verifier.resource_policy.v1";

/// Landlock filesystem access rights (`LANDLOCK_ACCESS_FS_*`).
pub mod fs_access {
    pub const EXECUTE: u64 = 1 << 0;
    pub const WRITE_FILE: u64 = 1 << 1;
    pub const READ_FILE: u64 = 1 << 2;
    pub const READ_DIR: u64 = 1 << 3;
    pub const REMOVE_DIR: u64 = 1 << 4;
    pub const REMOVE_FILE: u64 = 1 << 5;
    pub const MAKE_CHAR: u64 = 1 << 6;
    pub const MAKE_DIR: u64 = 1 << 7;
    pub const MAKE_REG: u64 = 1 << 8;
    pub const MAKE_SOCK: u64 = 1 << 9;
    pub const MAKE_FIFO: u64 = 1 << 10;
    pub const MAKE_BLOCK: u64 = 1 << 11;
    pub const MAKE_SYM: u64 = 1 << 12;
    pub const REFER: u64 = 1 << 13;
    pub const TRUNCATE: u64 = 1 << 14;
    pub const IOCTL_DEV: u64 = 1 << 15;
    /// Every filesystem right Landlock ABI 1 to 5 defines (ABI 6 adds none).
    pub const ALL_ABI_5: u64 = (1 << 16) - 1;
}

/// Landlock network access rights (`LANDLOCK_ACCESS_NET_*`, ABI 4).
pub mod net_access {
    pub const BIND_TCP: u64 = 1 << 0;
    pub const CONNECT_TCP: u64 = 1 << 1;
}

/// Landlock scopes (`LANDLOCK_SCOPE_*`, ABI 6).
pub mod scope {
    pub const ABSTRACT_UNIX_SOCKET: u64 = 1 << 0;
    pub const SIGNAL: u64 = 1 << 1;
}

/// A namespace every launch creates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Namespace {
    User,
    Pid,
    Net,
    Ipc,
    Uts,
    Cgroup,
}

impl Namespace {
    pub fn name(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Pid => "pid",
            Self::Net => "net",
            Self::Ipc => "ipc",
            Self::Uts => "uts",
            Self::Cgroup => "cgroup",
        }
    }
}

/// What a granted filesystem object is to the verifier. The rights of each
/// role are fixed here; a launch supplies objects for roles, never rights.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    /// The verified toolchain tree.
    ToolchainRoot,
    /// The dynamic loader the toolchain's host executables need.
    RuntimeLoader,
    /// One exact root-owned shared library the toolchain needs.
    RuntimeLibrary,
    /// The materialized candidate: read only.
    CandidateInput,
    /// Build output; generated test executables run from here.
    Target,
    /// Private home, temporary and tool-state directories.
    Scratch,
    DevNull,
    DevUrandom,
}

impl Role {
    pub const ALL: [Role; 8] = [
        Role::ToolchainRoot,
        Role::RuntimeLoader,
        Role::RuntimeLibrary,
        Role::CandidateInput,
        Role::Target,
        Role::Scratch,
        Role::DevNull,
        Role::DevUrandom,
    ];

    /// The Landlock rights granted beneath an object in this role.
    pub fn rights(self) -> u64 {
        use fs_access::*;
        const WORKSPACE_WRITE: u64 = READ_FILE
            | READ_DIR
            | WRITE_FILE
            | REMOVE_DIR
            | REMOVE_FILE
            | MAKE_DIR
            | MAKE_REG
            | REFER
            | TRUNCATE;
        match self {
            Self::ToolchainRoot => READ_FILE | READ_DIR | EXECUTE,
            Self::RuntimeLoader => READ_FILE | EXECUTE,
            Self::RuntimeLibrary => READ_FILE,
            Self::CandidateInput => READ_FILE | READ_DIR,
            Self::Target => WORKSPACE_WRITE | EXECUTE,
            Self::Scratch => WORKSPACE_WRITE,
            Self::DevNull => READ_FILE | WRITE_FILE,
            Self::DevUrandom => READ_FILE,
        }
    }

    /// Whether objects in this role are directories (rules beneath them) or
    /// single files.
    pub fn is_directory(self) -> bool {
        matches!(
            self,
            Self::ToolchainRoot | Self::CandidateInput | Self::Target | Self::Scratch
        )
    }

    fn code(self) -> u64 {
        self as u64
    }
}

/// The complete isolation policy of a launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SandboxPolicy {
    pub version: u64,
    /// Every namespace is created, in one `unshare`, before untrusted code.
    pub namespaces: &'static [Namespace],
    /// Landlock is required, in strict mode, at this ABI or later.
    pub landlock_min_abi: u64,
    /// Handled filesystem rights: anything not granted to a role is denied.
    pub handled_fs: u64,
    /// Handled network rights: no rule grants any, so all are denied.
    pub handled_net: u64,
    pub scopes: u64,
    pub roles: &'static [Role],
    pub no_new_privs: bool,
    /// Loopback is never brought up in the private network namespace.
    pub loopback_up: bool,
    /// Only descriptors 0 to 2 and the verified executable survive exec.
    pub inherited_fds: &'static str,
    /// The verifier's environment is exactly the profile's variables.
    pub environment: &'static str,
    /// stdin is `/dev/null`; stdout and stderr are bounded pipes.
    pub stdio: &'static str,
    /// The seccomp allow-list installed last, before exec.
    pub seccomp: &'static [crate::seccomp_policy::Allowed],
}

impl SandboxPolicy {
    pub const V1: Self = Self {
        version: 1,
        namespaces: &[
            Namespace::User,
            Namespace::Pid,
            Namespace::Net,
            Namespace::Ipc,
            Namespace::Uts,
            Namespace::Cgroup,
        ],
        landlock_min_abi: 6,
        handled_fs: fs_access::ALL_ABI_5,
        handled_net: net_access::BIND_TCP | net_access::CONNECT_TCP,
        scopes: scope::ABSTRACT_UNIX_SOCKET | scope::SIGNAL,
        roles: &Role::ALL,
        no_new_privs: true,
        loopback_up: false,
        inherited_fds: "stdio+verified-executable",
        environment: "exact-profile-variables",
        stdio: "stdin-dev-null;stdout-stderr-bounded-pipes",
        seccomp: crate::seccomp_policy::ALLOWED,
    };

    pub fn hash(&self) -> SandboxPolicyHash {
        let mut hasher = domain(SANDBOX_DOMAIN);
        put_u64(&mut hasher, self.version);
        put_u64(&mut hasher, self.namespaces.len() as u64);
        for namespace in self.namespaces {
            put_bytes(&mut hasher, namespace.name().as_bytes());
        }
        put_u64(&mut hasher, self.landlock_min_abi);
        put_u64(&mut hasher, self.handled_fs);
        put_u64(&mut hasher, self.handled_net);
        put_u64(&mut hasher, self.scopes);
        put_u64(&mut hasher, self.roles.len() as u64);
        for role in self.roles {
            put_u64(&mut hasher, role.code());
            put_u64(&mut hasher, role.rights());
            put_u64(&mut hasher, u64::from(role.is_directory()));
        }
        put_u64(&mut hasher, u64::from(self.no_new_privs));
        put_u64(&mut hasher, u64::from(self.loopback_up));
        put_bytes(&mut hasher, self.inherited_fds.as_bytes());
        put_bytes(&mut hasher, self.environment.as_bytes());
        put_bytes(&mut hasher, self.stdio.as_bytes());
        crate::seccomp_policy::put_policy(self.seccomp, &mut |part| put_bytes(&mut hasher, part));
        SandboxPolicyHash::finish(hasher)
    }
}

/// Profile-owned resource limits, applied to the execution's cgroup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourcePolicy {
    /// `memory.max`. Writes to the tmpfs workspace are charged here too.
    pub memory_max_bytes: u64,
    /// `memory.swap.max`.
    pub swap_max_bytes: u64,
    /// `pids.max`.
    pub pids_max: u64,
    /// CPU time per wall-clock second, in microseconds (`cpu.max`).
    pub cpu_quota_us_per_sec: u64,
    /// Wall-clock deadline measured by the backend.
    pub wall_timeout_secs: u64,
    /// The scope's own runtime limit, a crash backstop beyond the deadline.
    pub runtime_backstop_secs: u64,
    /// Hard ceiling for each of stdout and stderr; exceeding it ends the run.
    pub output_ceiling_bytes: u64,
    /// Bytes of each stream kept for a bounded display excerpt.
    pub excerpt_bytes: u64,
}

impl ResourcePolicy {
    pub const RUST_OFFLINE_V1: Self = Self {
        memory_max_bytes: 4 * 1024 * 1024 * 1024,
        swap_max_bytes: 0,
        pids_max: 256,
        cpu_quota_us_per_sec: 4_000_000,
        wall_timeout_secs: 600,
        runtime_backstop_secs: 660,
        output_ceiling_bytes: 8 * 1024 * 1024,
        excerpt_bytes: 16 * 1024,
    };

    pub fn hash(&self) -> ResourcePolicyHash {
        let mut hasher: Sha256 = domain(RESOURCE_DOMAIN);
        for value in [
            self.memory_max_bytes,
            self.swap_max_bytes,
            self.pids_max,
            self.cpu_quota_us_per_sec,
            self.wall_timeout_secs,
            self.runtime_backstop_secs,
            self.output_ceiling_bytes,
            self.excerpt_bytes,
        ] {
            put_u64(&mut hasher, value);
        }
        ResourcePolicyHash::finish(hasher)
    }

    /// Whole CPUs the quota amounts to (for display).
    pub fn cpus(&self) -> u64 {
        self.cpu_quota_us_per_sec / 1_000_000
    }
}

#[cfg(test)]
mod tests;
