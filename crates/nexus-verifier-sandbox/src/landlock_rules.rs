//! Applying the v1 Landlock policy (Linux).
//!
//! Strict: every handled right, the TCP rights and both scopes of ABI 6 are
//! required (`HardRequirement`); anything the kernel cannot enforce is an
//! error, never a partial sandbox. A role's rights come from
//! [`crate::policy::Role::rights`]; the caller supplies only the objects.

use std::os::fd::BorrowedFd;

use landlock::{
    Access, AccessFs, AccessNet, BitFlags, CompatLevel, Compatible, LandlockStatus, PathBeneath,
    Ruleset, RulesetAttr, RulesetCreatedAttr, RulesetStatus, Scope, ABI,
};

use crate::policy::{Role, SandboxPolicy};

/// The ABI every rule set is built for.
pub const REQUIRED_ABI: ABI = ABI::V6;

#[derive(Debug)]
pub enum LandlockFailure {
    /// The policy's handled rights are not exactly the ABI 6 set.
    PolicyMismatch,
    /// A role's rights are not a valid access set.
    InvalidRights(Role),
    /// The kernel refused or could not fully enforce the rule set.
    Ruleset(landlock::RulesetError),
    /// The rule set was not fully enforced, or `no_new_privs` is not set, or
    /// the effective ABI is below the requirement.
    NotEnforced,
}

impl LandlockFailure {
    /// An errno-like code for setup reporting.
    pub fn code(&self) -> i32 {
        match self {
            Self::PolicyMismatch => -10,
            Self::InvalidRights(_) => -11,
            Self::Ruleset(_) => -12,
            Self::NotEnforced => -13,
        }
    }
}

fn abi_at_least(status: &LandlockStatus) -> bool {
    match status {
        LandlockStatus::Available { effective_abi, .. } => {
            *effective_abi as i32 >= REQUIRED_ABI as i32
        }
        _ => false,
    }
}

/// Restrict the calling process to exactly `rules`.
pub fn restrict_self(rules: &[(Role, BorrowedFd<'_>)]) -> Result<(), LandlockFailure> {
    let policy = SandboxPolicy::V1;
    let handled_fs = AccessFs::from_all(REQUIRED_ABI);
    let handled_net = AccessNet::from_all(REQUIRED_ABI);
    let scopes = Scope::from_all(REQUIRED_ABI);
    if handled_fs.bits() != policy.handled_fs
        || handled_net.bits() != policy.handled_net
        || scopes.bits() != policy.scopes
    {
        return Err(LandlockFailure::PolicyMismatch);
    }
    let mut ruleset = Ruleset::default()
        .set_compatibility(CompatLevel::HardRequirement)
        .handle_access(handled_fs)
        .map_err(LandlockFailure::Ruleset)?
        .handle_access(handled_net)
        .map_err(LandlockFailure::Ruleset)?
        .scope(scopes)
        .map_err(LandlockFailure::Ruleset)?
        .create()
        .map_err(LandlockFailure::Ruleset)?;
    for (role, fd) in rules {
        let access: BitFlags<AccessFs> = BitFlags::from_bits(role.rights())
            .map_err(|_| LandlockFailure::InvalidRights(*role))?;
        ruleset = ruleset
            .add_rule(PathBeneath::new(*fd, access))
            .map_err(LandlockFailure::Ruleset)?;
    }
    let status = ruleset.restrict_self().map_err(LandlockFailure::Ruleset)?;
    if status.ruleset != RulesetStatus::FullyEnforced
        || !status.no_new_privs
        || !abi_at_least(&status.landlock)
    {
        return Err(LandlockFailure::NotEnforced);
    }
    Ok(())
}

/// The running kernel's Landlock ABI (0 if Landlock is unavailable).
pub fn kernel_abi() -> i32 {
    // SAFETY: LANDLOCK_CREATE_RULESET_VERSION with a null attribute only
    // queries the ABI version.
    let version = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            std::ptr::null::<libc::c_void>(),
            0usize,
            1u32,
        )
    };
    if version < 0 {
        0
    } else {
        version as i32
    }
}

/// Whether a kernel ABI satisfies the policy's minimum.
pub fn abi_is_supported(abi: i32) -> bool {
    abi >= SandboxPolicy::V1.landlock_min_abi as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p2i_nc_33_a_landlock_below_abi_6_is_never_accepted() {
        let available = |effective_abi| LandlockStatus::Available {
            effective_abi,
            kernel_abi: None,
        };
        for abi in [ABI::V1, ABI::V2, ABI::V3, ABI::V4, ABI::V5] {
            assert!(!abi_at_least(&available(abi)), "{abi:?}");
        }
        assert!(abi_at_least(&available(ABI::V6)));
        // A kernel newer than this crate knows is enforced at ABI 6.
        assert!(abi_at_least(&LandlockStatus::Available {
            effective_abi: ABI::V6,
            kernel_abi: Some(7),
        }));
        assert!(!abi_at_least(&LandlockStatus::NotEnabled));
        assert!(!abi_at_least(&LandlockStatus::NotImplemented));
        assert!(!abi_is_supported(0) && !abi_is_supported(5));
        assert!(abi_is_supported(6) && abi_is_supported(7));
    }
}
