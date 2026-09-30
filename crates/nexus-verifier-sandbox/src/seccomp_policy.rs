//! The v1 verifier seccomp allow-list, as data (x86_64 only).
//!
//! It is compiled to BPF on Linux ([`crate::policy`] hashes this table, so
//! every platform computes the same policy identity). The compiled program:
//!
//! - kills a process that uses another architecture or the x32 ABI;
//! - allows exactly the syscalls below, some only for the listed argument
//!   values;
//! - fails every other syscall with `ENOSYS`. That includes `socket`,
//!   `unshare`, `setns`, namespace-bearing `clone`, `clone3` (so libc falls
//!   back to the inspectable `clone`), the mount API, `chroot`,
//!   `pivot_root`, `ptrace`, `process_vm_*`, `process_madvise`,
//!   `process_mrelease`, every pidfd call, `bpf`, `perf_event_open`, the
//!   keyring, `userfaultfd`, every `io_uring` call, module loading, `kexec`,
//!   `reboot`, swap control, `open_by_handle_at`, `name_to_handle_at`, and
//!   path-based metadata mutation (`chmod` and `chown` families, the
//!   `xattr` setters, `utime`, `utimes`, `futimesat`, and `utimensat` with a
//!   path), which Landlock does not mediate.
//!
//! The list is what the first profile's toolchain and ordinary test code
//! need (measured), not everything a program might want.

/// `CLONE_NEWNS | CLONE_NEWCGROUP | CLONE_NEWUTS | CLONE_NEWIPC |
/// CLONE_NEWUSER | CLONE_NEWPID | CLONE_NEWNET`.
pub const CLONE_NAMESPACE_FLAGS: u64 =
    0x0002_0000 | 0x0200_0000 | 0x0400_0000 | 0x0800_0000 | 0x1000_0000 | 0x2000_0000 | 0x4000_0000;

/// The x32 ABI bit in an x86_64 syscall number.
pub const X32_SYSCALL_BIT: u64 = 0x4000_0000;

/// `ENOSYS`: the result of every syscall the list does not allow.
pub const DENIED_ERRNO: u32 = 38;

/// When an allowed syscall is allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Condition {
    Always,
    /// Argument `arg` (a 64-bit value) equals one of `values`.
    ArgIn {
        arg: u8,
        values: &'static [u64],
    },
    /// The low 32 bits of argument `arg` equal one of `values`.
    ArgLowIn {
        arg: u8,
        values: &'static [u64],
    },
    /// Argument `arg` has none of the bits in `mask`.
    ArgMaskZero {
        arg: u8,
        mask: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Allowed {
    pub name: &'static str,
    pub nr: i64,
    pub condition: Condition,
}

const fn always(name: &'static str, nr: i64) -> Allowed {
    Allowed {
        name,
        nr,
        condition: Condition::Always,
    }
}

const AF_UNIX: u64 = 1;
const PR_SET_PDEATHSIG: u64 = 1;
const PR_GET_PDEATHSIG: u64 = 2;
const PR_GET_DUMPABLE: u64 = 3;
const PR_SET_DUMPABLE: u64 = 4;
const PR_SET_NAME: u64 = 15;
const PR_GET_NAME: u64 = 16;
const PR_GET_SECCOMP: u64 = 21;
const PR_GET_NO_NEW_PRIVS: u64 = 39;
const PR_SET_VMA: u64 = 0x5356_4d41;
const TCGETS: u64 = 0x5401;
const TIOCGWINSZ: u64 = 0x5413;
const FIONREAD: u64 = 0x541B;
const FIONBIO: u64 = 0x5421;
const FIONCLEX: u64 = 0x5450;
const FIOCLEX: u64 = 0x5451;

/// `fcntl` commands: duplication, descriptor and status flags, record and
/// open-file-description locks, and the pipe size query. Not `F_SETOWN`,
/// `F_SETSIG`, `F_NOTIFY` or leases.
const FCNTL_COMMANDS: &[u64] = &[0, 1, 2, 3, 4, 5, 6, 7, 36, 37, 38, 1030, 1032];

/// The v1 allow-list.
pub const ALLOWED: &[Allowed] = &[
    always("read", 0),
    always("write", 1),
    always("open", 2),
    always("close", 3),
    always("stat", 4),
    always("fstat", 5),
    always("lstat", 6),
    always("poll", 7),
    always("lseek", 8),
    always("mmap", 9),
    always("mprotect", 10),
    always("munmap", 11),
    always("brk", 12),
    always("rt_sigaction", 13),
    always("rt_sigprocmask", 14),
    always("rt_sigreturn", 15),
    Allowed {
        name: "ioctl",
        nr: 16,
        condition: Condition::ArgLowIn {
            arg: 1,
            values: &[TCGETS, TIOCGWINSZ, FIONREAD, FIONBIO, FIONCLEX, FIOCLEX],
        },
    },
    always("pread64", 17),
    always("pwrite64", 18),
    always("readv", 19),
    always("writev", 20),
    always("access", 21),
    always("pipe", 22),
    always("select", 23),
    always("sched_yield", 24),
    always("mremap", 25),
    always("msync", 26),
    always("mincore", 27),
    always("madvise", 28),
    always("dup", 32),
    always("dup2", 33),
    always("nanosleep", 35),
    always("getitimer", 36),
    always("alarm", 37),
    always("setitimer", 38),
    always("getpid", 39),
    always("sendfile", 40),
    always("sendto", 44),
    always("recvfrom", 45),
    always("sendmsg", 46),
    always("recvmsg", 47),
    always("shutdown", 48),
    Allowed {
        name: "socketpair",
        nr: 53,
        condition: Condition::ArgIn {
            arg: 0,
            values: &[AF_UNIX],
        },
    },
    Allowed {
        name: "clone",
        nr: 56,
        condition: Condition::ArgMaskZero {
            arg: 0,
            mask: CLONE_NAMESPACE_FLAGS,
        },
    },
    always("execve", 59),
    always("exit", 60),
    always("wait4", 61),
    always("kill", 62),
    always("uname", 63),
    Allowed {
        name: "fcntl",
        nr: 72,
        condition: Condition::ArgLowIn {
            arg: 1,
            values: FCNTL_COMMANDS,
        },
    },
    always("flock", 73),
    always("fsync", 74),
    always("fdatasync", 75),
    always("truncate", 76),
    always("ftruncate", 77),
    always("getcwd", 79),
    always("chdir", 80),
    always("fchdir", 81),
    always("rename", 82),
    always("mkdir", 83),
    always("rmdir", 84),
    always("link", 86),
    always("unlink", 87),
    always("readlink", 89),
    always("umask", 95),
    always("gettimeofday", 96),
    always("getrlimit", 97),
    always("getrusage", 98),
    always("sysinfo", 99),
    always("times", 100),
    always("getuid", 102),
    always("getgid", 104),
    always("geteuid", 107),
    always("getegid", 108),
    always("setpgid", 109),
    always("getppid", 110),
    always("getpgrp", 111),
    always("setsid", 112),
    always("getgroups", 115),
    always("getresuid", 118),
    always("getresgid", 120),
    always("getpgid", 121),
    always("getsid", 124),
    always("rt_sigpending", 127),
    always("rt_sigtimedwait", 128),
    always("rt_sigsuspend", 130),
    always("sigaltstack", 131),
    always("statfs", 137),
    always("fstatfs", 138),
    Allowed {
        name: "prctl",
        nr: 157,
        condition: Condition::ArgIn {
            arg: 0,
            values: &[
                PR_SET_PDEATHSIG,
                PR_GET_PDEATHSIG,
                PR_GET_DUMPABLE,
                PR_SET_DUMPABLE,
                PR_SET_NAME,
                PR_GET_NAME,
                PR_GET_SECCOMP,
                PR_GET_NO_NEW_PRIVS,
                PR_SET_VMA,
            ],
        },
    },
    always("arch_prctl", 158),
    always("gettid", 186),
    always("tkill", 200),
    always("time", 201),
    always("futex", 202),
    always("sched_getaffinity", 204),
    always("getdents64", 217),
    always("set_tid_address", 218),
    always("restart_syscall", 219),
    always("fadvise64", 221),
    always("clock_gettime", 228),
    always("clock_getres", 229),
    always("clock_nanosleep", 230),
    always("exit_group", 231),
    always("tgkill", 234),
    always("waitid", 247),
    always("openat", 257),
    always("mkdirat", 258),
    always("newfstatat", 262),
    always("unlinkat", 263),
    always("renameat", 264),
    always("linkat", 265),
    always("readlinkat", 267),
    always("faccessat", 269),
    always("pselect6", 270),
    always("ppoll", 271),
    always("set_robust_list", 273),
    always("get_robust_list", 274),
    always("splice", 275),
    Allowed {
        name: "utimensat",
        nr: 280,
        condition: Condition::ArgIn {
            arg: 1,
            values: &[0],
        },
    },
    always("dup3", 292),
    always("pipe2", 293),
    always("preadv", 295),
    always("pwritev", 296),
    Allowed {
        name: "prlimit64",
        nr: 302,
        condition: Condition::ArgIn {
            arg: 0,
            values: &[0],
        },
    },
    always("getcpu", 309),
    always("renameat2", 316),
    always("getrandom", 318),
    always("execveat", 322),
    always("copy_file_range", 326),
    always("preadv2", 327),
    always("pwritev2", 328),
    always("statx", 332),
    always("rseq", 334),
    always("close_range", 436),
    always("faccessat2", 439),
];

/// Syscalls the policy must never allow, by name and x86_64 number. The
/// allow-list is checked against it.
pub const NEVER: &[(&str, i64)] = &[
    ("socket", 41),
    ("connect", 42),
    ("accept", 43),
    ("bind", 49),
    ("listen", 50),
    ("fork", 57),
    ("vfork", 58),
    ("chmod", 90),
    ("fchmod", 91),
    ("chown", 92),
    ("fchown", 93),
    ("lchown", 94),
    ("ptrace", 101),
    ("syslog", 103),
    ("setuid", 105),
    ("setgid", 106),
    ("setreuid", 113),
    ("setregid", 114),
    ("setgroups", 116),
    ("setresuid", 117),
    ("setresgid", 119),
    ("setfsuid", 122),
    ("setfsgid", 123),
    ("capset", 126),
    ("utime", 132),
    ("mknod", 133),
    ("personality", 135),
    ("modify_ldt", 154),
    ("pivot_root", 155),
    ("chroot", 161),
    ("acct", 163),
    ("settimeofday", 164),
    ("mount", 165),
    ("umount2", 166),
    ("swapon", 167),
    ("swapoff", 168),
    ("reboot", 169),
    ("sethostname", 170),
    ("setdomainname", 171),
    ("iopl", 172),
    ("ioperm", 173),
    ("init_module", 175),
    ("delete_module", 176),
    ("quotactl", 179),
    ("setxattr", 188),
    ("lsetxattr", 189),
    ("fsetxattr", 190),
    ("removexattr", 197),
    ("lremovexattr", 198),
    ("fremovexattr", 199),
    ("io_setup", 206),
    ("io_submit", 209),
    ("lookup_dcookie", 212),
    ("clock_settime", 227),
    ("utimes", 235),
    ("mbind", 237),
    ("kexec_load", 246),
    ("add_key", 248),
    ("request_key", 249),
    ("keyctl", 250),
    ("inotify_add_watch", 254),
    ("mknodat", 259),
    ("fchownat", 260),
    ("futimesat", 261),
    ("fchmodat", 268),
    ("unshare", 272),
    ("move_pages", 279),
    ("perf_event_open", 298),
    ("fanotify_init", 300),
    ("fanotify_mark", 301),
    ("name_to_handle_at", 303),
    ("open_by_handle_at", 304),
    ("clock_adjtime", 305),
    ("setns", 308),
    ("process_vm_readv", 310),
    ("process_vm_writev", 311),
    ("kcmp", 312),
    ("finit_module", 313),
    ("seccomp", 317),
    ("memfd_create", 319),
    ("kexec_file_load", 320),
    ("bpf", 321),
    ("userfaultfd", 323),
    ("pidfd_send_signal", 424),
    ("io_uring_setup", 425),
    ("io_uring_enter", 426),
    ("io_uring_register", 427),
    ("open_tree", 428),
    ("move_mount", 429),
    ("fsopen", 430),
    ("fsconfig", 431),
    ("fsmount", 432),
    ("fspick", 433),
    ("pidfd_open", 434),
    ("clone3", 435),
    ("openat2", 437),
    ("pidfd_getfd", 438),
    ("process_madvise", 440),
    ("mount_setattr", 442),
    ("quotactl_fd", 443),
    ("landlock_create_ruleset", 444),
    ("landlock_add_rule", 445),
    ("landlock_restrict_self", 446),
    ("memfd_secret", 447),
    ("process_mrelease", 448),
    ("fchmodat2", 452),
    ("setxattrat", 463),
    ("removexattrat", 466),
    ("open_tree_attr", 467),
];

/// Canonical encoding of an allow-list and the actions around it, for the
/// sandbox policy identity.
pub fn put_policy(rules: &[Allowed], put: &mut dyn FnMut(&[u8])) {
    put(b"arch:x86_64;x32:kill;foreign-arch:kill;unlisted:enosys");
    put(&DENIED_ERRNO.to_be_bytes());
    put(&X32_SYSCALL_BIT.to_be_bytes());
    put(&(rules.len() as u64).to_be_bytes());
    for rule in rules {
        put(rule.name.as_bytes());
        put(&rule.nr.to_be_bytes());
        match rule.condition {
            Condition::Always => put(&[0]),
            Condition::ArgIn { arg, values } | Condition::ArgLowIn { arg, values } => {
                let kind = if matches!(rule.condition, Condition::ArgIn { .. }) {
                    1
                } else {
                    2
                };
                put(&[kind, arg]);
                put(&(values.len() as u64).to_be_bytes());
                for value in values {
                    put(&value.to_be_bytes());
                }
            }
            Condition::ArgMaskZero { arg, mask } => {
                put(&[3, arg]);
                put(&mask.to_be_bytes());
            }
        }
    }
}

#[cfg(test)]
mod tests;
