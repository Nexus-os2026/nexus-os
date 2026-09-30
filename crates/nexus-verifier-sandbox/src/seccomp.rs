//! Compiling and installing the v1 seccomp allow-list (Linux x86_64).
//!
//! The program is built from [`crate::seccomp_policy::ALLOWED`] by
//! seccompiler (architecture pinned; foreign architectures killed), with an
//! explicit prefix that kills any x32-ABI syscall number, and with `ENOSYS`
//! for everything not allowed.

use std::collections::BTreeMap;
use std::io;

use seccompiler::{
    sock_filter, BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition,
    SeccompFilter, SeccompRule, TargetArch,
};
use sha2::{Digest, Sha256};

use crate::seccomp_policy::{Allowed, Condition, ALLOWED, DENIED_ERRNO, X32_SYSCALL_BIT};

/// `BPF_LD | BPF_W | BPF_ABS` (0x00 | 0x00 | 0x20).
const BPF_LD_W_ABS: u16 = 0x20;
/// `BPF_JMP | BPF_JSET | BPF_K` (0x05 | 0x40 | 0x00).
const BPF_JMP_JSET_K: u16 = 0x45;
/// `BPF_RET | BPF_K` (0x06 | 0x00).
const BPF_RET_K: u16 = 0x06;
/// `offsetof(struct seccomp_data, nr)`.
const SECCOMP_DATA_NR: u32 = 0;

fn rules_for(allowed: &Allowed) -> Result<Vec<SeccompRule>, seccompiler::Error> {
    let condition = |arg, len, op, value| SeccompCondition::new(arg, len, op, value);
    Ok(match allowed.condition {
        Condition::Always => Vec::new(),
        Condition::ArgIn { arg, values } => values
            .iter()
            .map(|value| {
                SeccompRule::new(vec![condition(
                    arg,
                    SeccompCmpArgLen::Qword,
                    SeccompCmpOp::Eq,
                    *value,
                )?])
            })
            .collect::<Result<_, _>>()?,
        Condition::ArgLowIn { arg, values } => values
            .iter()
            .map(|value| {
                SeccompRule::new(vec![condition(
                    arg,
                    SeccompCmpArgLen::Dword,
                    SeccompCmpOp::Eq,
                    *value,
                )?])
            })
            .collect::<Result<_, _>>()?,
        Condition::ArgMaskZero { arg, mask } => vec![SeccompRule::new(vec![condition(
            arg,
            SeccompCmpArgLen::Qword,
            SeccompCmpOp::MaskedEq(mask),
            0,
        )?])?],
    })
}

/// The v1 program, ready to install.
pub fn program() -> Result<BpfProgram, seccompiler::Error> {
    let mut rules = BTreeMap::new();
    for allowed in ALLOWED {
        rules.insert(allowed.nr, rules_for(allowed)?);
    }
    let filter = SeccompFilter::new(
        rules,
        SeccompAction::Errno(DENIED_ERRNO),
        SeccompAction::Allow,
        TargetArch::x86_64,
    )?;
    let body: BpfProgram = filter.try_into()?;
    let mut program = vec![
        sock_filter {
            code: BPF_LD_W_ABS,
            jt: 0,
            jf: 0,
            k: SECCOMP_DATA_NR,
        },
        sock_filter {
            code: BPF_JMP_JSET_K,
            jt: 0,
            jf: 1,
            k: X32_SYSCALL_BIT as u32,
        },
        sock_filter {
            code: BPF_RET_K,
            jt: 0,
            jf: 0,
            k: libc::SECCOMP_RET_KILL_PROCESS,
        },
    ];
    program.extend(body);
    Ok(program)
}

/// SHA-256 of the exact program instructions.
pub fn program_digest(program: &[sock_filter]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update((program.len() as u64).to_be_bytes());
    for instruction in program {
        hasher.update(instruction.code.to_be_bytes());
        hasher.update([instruction.jt, instruction.jf]);
        hasher.update(instruction.k.to_be_bytes());
    }
    hasher.finalize().into()
}

/// Install `program` on the calling thread (it sets `no_new_privs` first).
pub fn install(program: &[sock_filter]) -> io::Result<()> {
    seccompiler::apply_filter(program).map_err(|error| match error {
        seccompiler::Error::Seccomp(os) | seccompiler::Error::Prctl(os) => os,
        _ => io::Error::from_raw_os_error(libc::EINVAL),
    })
}
