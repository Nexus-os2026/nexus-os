use super::*;
use std::collections::BTreeSet;

fn allowed(name: &str) -> Option<&'static Allowed> {
    ALLOWED.iter().find(|rule| rule.name == name)
}

#[test]
fn p2c_allowed_syscalls_are_unique_and_disjoint_from_the_never_list() {
    let names: BTreeSet<&str> = ALLOWED.iter().map(|rule| rule.name).collect();
    let numbers: BTreeSet<i64> = ALLOWED.iter().map(|rule| rule.nr).collect();
    assert_eq!(names.len(), ALLOWED.len(), "unique names");
    assert_eq!(numbers.len(), ALLOWED.len(), "unique numbers");
    for (name, nr) in NEVER {
        assert!(allowed(name).is_none(), "{name} must never be allowed");
        assert!(!numbers.contains(nr), "{name} ({nr}) must never be allowed");
    }
    for rule in ALLOWED {
        assert_eq!(rule.nr as u64 & X32_SYSCALL_BIT, 0, "{}", rule.name);
    }
}

#[test]
fn p2c_every_mission_required_denial_is_never_allowed() {
    let never: BTreeSet<&str> = NEVER.iter().map(|(name, _)| *name).collect();
    for required in [
        "socket",
        "unshare",
        "setns",
        "clone3",
        "mount",
        "umount2",
        "open_tree",
        "move_mount",
        "fsopen",
        "fsconfig",
        "fsmount",
        "fspick",
        "mount_setattr",
        "chroot",
        "pivot_root",
        "ptrace",
        "process_vm_readv",
        "process_vm_writev",
        "process_madvise",
        "process_mrelease",
        "pidfd_getfd",
        "pidfd_open",
        "pidfd_send_signal",
        "bpf",
        "perf_event_open",
        "add_key",
        "request_key",
        "keyctl",
        "userfaultfd",
        "io_uring_setup",
        "io_uring_enter",
        "io_uring_register",
        "init_module",
        "finit_module",
        "delete_module",
        "kexec_load",
        "kexec_file_load",
        "reboot",
        "swapon",
        "swapoff",
        "open_by_handle_at",
        "name_to_handle_at",
        "chmod",
        "fchmodat",
        "fchmodat2",
        "chown",
        "lchown",
        "fchownat",
        "setxattr",
        "lsetxattr",
        "removexattr",
        "lremovexattr",
        "utime",
        "utimes",
        "futimesat",
        "seccomp",
    ] {
        assert!(never.contains(required), "{required} is on the never list");
        assert!(allowed(required).is_none(), "{required} is not allowed");
    }
}

#[test]
fn p2c_conditional_syscalls_exclude_the_dangerous_arguments() {
    let clone = allowed("clone").expect("clone");
    assert_eq!(
        clone.condition,
        Condition::ArgMaskZero {
            arg: 0,
            mask: CLONE_NAMESPACE_FLAGS
        }
    );
    for flag in [
        0x0002_0000u64,
        0x0200_0000,
        0x0400_0000,
        0x0800_0000,
        0x1000_0000,
        0x2000_0000,
        0x4000_0000,
    ] {
        assert_ne!(CLONE_NAMESPACE_FLAGS & flag, 0, "namespace flag {flag:#x}");
    }
    assert_eq!(
        allowed("socketpair").unwrap().condition,
        Condition::ArgIn {
            arg: 0,
            values: &[1]
        },
        "AF_UNIX pairs only"
    );
    assert_eq!(
        allowed("prlimit64").unwrap().condition,
        Condition::ArgIn {
            arg: 0,
            values: &[0]
        },
        "own limits only"
    );
    assert_eq!(
        allowed("utimensat").unwrap().condition,
        Condition::ArgIn {
            arg: 1,
            values: &[0]
        },
        "descriptor-based only"
    );
    let values = |name: &str| match allowed(name).unwrap().condition {
        Condition::ArgIn { values, .. } | Condition::ArgLowIn { values, .. } => values,
        other => panic!("{name}: {other:?}"),
    };
    for forbidden in [22u64, 38, 47, 35, 0x5961_6d61, 36, 24] {
        // PR_SET_SECCOMP, PR_SET_NO_NEW_PRIVS, PR_CAP_AMBIENT, PR_SET_MM,
        // PR_SET_PTRACER, PR_SET_CHILD_SUBREAPER, PR_CAPBSET_DROP
        assert!(
            !values("prctl").contains(&forbidden),
            "prctl {forbidden:#x}"
        );
    }
    for forbidden in [8u64, 10, 15, 1024, 1026] {
        // F_SETOWN, F_SETSIG, F_SETOWN_EX, F_SETLEASE, F_NOTIFY
        assert!(!values("fcntl").contains(&forbidden), "fcntl {forbidden}");
    }
    for forbidden in [0x5412u64, 0x541C, 0x540E] {
        // TIOCSTI, TIOCLINUX, TIOCSCTTY
        assert!(
            !values("ioctl").contains(&forbidden),
            "ioctl {forbidden:#x}"
        );
    }
}

#[test]
fn p2c_the_policy_encoding_is_deterministic() {
    let encode = |rules: &[Allowed]| {
        let mut bytes = Vec::new();
        put_policy(rules, &mut |part: &[u8]| bytes.extend_from_slice(part));
        bytes
    };
    assert_eq!(encode(ALLOWED), encode(ALLOWED));
    assert_ne!(encode(ALLOWED), encode(&ALLOWED[1..]));
    let mut widened: Vec<Allowed> = ALLOWED.to_vec();
    let clone = widened
        .iter_mut()
        .find(|rule| rule.name == "clone")
        .unwrap();
    clone.condition = Condition::Always;
    assert_ne!(encode(ALLOWED), encode(&widened), "conditions are covered");
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn p2c_syscall_numbers_match_the_kernel_abi() {
    macro_rules! abi {
        ($($name:ident),* $(,)?) => {
            [$((stringify!($name), libc::$name as i64)),*]
        };
    }
    let known = abi![
        SYS_read,
        SYS_write,
        SYS_open,
        SYS_close,
        SYS_stat,
        SYS_fstat,
        SYS_lstat,
        SYS_poll,
        SYS_lseek,
        SYS_mmap,
        SYS_mprotect,
        SYS_munmap,
        SYS_brk,
        SYS_rt_sigaction,
        SYS_rt_sigprocmask,
        SYS_rt_sigreturn,
        SYS_ioctl,
        SYS_pread64,
        SYS_pwrite64,
        SYS_readv,
        SYS_writev,
        SYS_access,
        SYS_pipe,
        SYS_select,
        SYS_sched_yield,
        SYS_mremap,
        SYS_msync,
        SYS_mincore,
        SYS_madvise,
        SYS_dup,
        SYS_dup2,
        SYS_nanosleep,
        SYS_getitimer,
        SYS_alarm,
        SYS_setitimer,
        SYS_getpid,
        SYS_sendfile,
        SYS_sendto,
        SYS_recvfrom,
        SYS_sendmsg,
        SYS_recvmsg,
        SYS_shutdown,
        SYS_socketpair,
        SYS_clone,
        SYS_execve,
        SYS_exit,
        SYS_wait4,
        SYS_kill,
        SYS_uname,
        SYS_fcntl,
        SYS_flock,
        SYS_fsync,
        SYS_fdatasync,
        SYS_truncate,
        SYS_ftruncate,
        SYS_getcwd,
        SYS_chdir,
        SYS_fchdir,
        SYS_rename,
        SYS_mkdir,
        SYS_rmdir,
        SYS_link,
        SYS_unlink,
        SYS_readlink,
        SYS_umask,
        SYS_gettimeofday,
        SYS_getrlimit,
        SYS_getrusage,
        SYS_sysinfo,
        SYS_times,
        SYS_getuid,
        SYS_getgid,
        SYS_geteuid,
        SYS_getegid,
        SYS_setpgid,
        SYS_getppid,
        SYS_getpgrp,
        SYS_setsid,
        SYS_getgroups,
        SYS_getresuid,
        SYS_getresgid,
        SYS_getpgid,
        SYS_getsid,
        SYS_rt_sigpending,
        SYS_rt_sigtimedwait,
        SYS_rt_sigsuspend,
        SYS_sigaltstack,
        SYS_statfs,
        SYS_fstatfs,
        SYS_prctl,
        SYS_arch_prctl,
        SYS_gettid,
        SYS_tkill,
        SYS_time,
        SYS_futex,
        SYS_sched_getaffinity,
        SYS_getdents64,
        SYS_set_tid_address,
        SYS_restart_syscall,
        SYS_fadvise64,
        SYS_clock_gettime,
        SYS_clock_getres,
        SYS_clock_nanosleep,
        SYS_exit_group,
        SYS_tgkill,
        SYS_waitid,
        SYS_openat,
        SYS_mkdirat,
        SYS_newfstatat,
        SYS_unlinkat,
        SYS_renameat,
        SYS_linkat,
        SYS_readlinkat,
        SYS_faccessat,
        SYS_pselect6,
        SYS_ppoll,
        SYS_set_robust_list,
        SYS_get_robust_list,
        SYS_splice,
        SYS_utimensat,
        SYS_dup3,
        SYS_pipe2,
        SYS_preadv,
        SYS_pwritev,
        SYS_prlimit64,
        SYS_getcpu,
        SYS_renameat2,
        SYS_getrandom,
        SYS_execveat,
        SYS_copy_file_range,
        SYS_preadv2,
        SYS_pwritev2,
        SYS_statx,
        SYS_rseq,
        SYS_close_range,
        SYS_faccessat2,
        SYS_socket,
        SYS_connect,
        SYS_accept,
        SYS_bind,
        SYS_listen,
        SYS_fork,
        SYS_vfork,
        SYS_chmod,
        SYS_fchmod,
        SYS_chown,
        SYS_fchown,
        SYS_lchown,
        SYS_ptrace,
        SYS_syslog,
        SYS_setuid,
        SYS_setgid,
        SYS_setreuid,
        SYS_setregid,
        SYS_setgroups,
        SYS_setresuid,
        SYS_setresgid,
        SYS_setfsuid,
        SYS_setfsgid,
        SYS_capset,
        SYS_utime,
        SYS_mknod,
        SYS_personality,
        SYS_modify_ldt,
        SYS_pivot_root,
        SYS_chroot,
        SYS_acct,
        SYS_settimeofday,
        SYS_mount,
        SYS_umount2,
        SYS_swapon,
        SYS_swapoff,
        SYS_reboot,
        SYS_sethostname,
        SYS_setdomainname,
        SYS_iopl,
        SYS_ioperm,
        SYS_init_module,
        SYS_delete_module,
        SYS_quotactl,
        SYS_setxattr,
        SYS_lsetxattr,
        SYS_fsetxattr,
        SYS_removexattr,
        SYS_lremovexattr,
        SYS_fremovexattr,
        SYS_io_setup,
        SYS_io_submit,
        SYS_lookup_dcookie,
        SYS_clock_settime,
        SYS_utimes,
        SYS_mbind,
        SYS_kexec_load,
        SYS_add_key,
        SYS_request_key,
        SYS_keyctl,
        SYS_inotify_add_watch,
        SYS_mknodat,
        SYS_fchownat,
        SYS_futimesat,
        SYS_fchmodat,
        SYS_unshare,
        SYS_move_pages,
        SYS_perf_event_open,
        SYS_fanotify_init,
        SYS_fanotify_mark,
        SYS_name_to_handle_at,
        SYS_open_by_handle_at,
        SYS_clock_adjtime,
        SYS_setns,
        SYS_process_vm_readv,
        SYS_process_vm_writev,
        SYS_kcmp,
        SYS_finit_module,
        SYS_seccomp,
        SYS_memfd_create,
        SYS_kexec_file_load,
        SYS_bpf,
        SYS_userfaultfd,
        SYS_pidfd_send_signal,
        SYS_io_uring_setup,
        SYS_io_uring_enter,
        SYS_io_uring_register,
        SYS_open_tree,
        SYS_move_mount,
        SYS_fsopen,
        SYS_fsconfig,
        SYS_fsmount,
        SYS_fspick,
        SYS_pidfd_open,
        SYS_clone3,
        SYS_openat2,
        SYS_pidfd_getfd,
        SYS_process_madvise,
        SYS_mount_setattr,
        SYS_quotactl_fd,
        SYS_landlock_create_ruleset,
        SYS_landlock_add_rule,
        SYS_landlock_restrict_self,
        SYS_memfd_secret,
        SYS_process_mrelease,
        SYS_fchmodat2,
    ];
    let abi: std::collections::BTreeMap<&str, i64> = known
        .iter()
        .map(|(name, nr)| (name.strip_prefix("SYS_").unwrap(), *nr))
        .collect();
    for rule in ALLOWED {
        assert_eq!(abi.get(rule.name), Some(&rule.nr), "{}", rule.name);
    }
    for (name, nr) in NEVER {
        if let Some(expected) = abi.get(name) {
            assert_eq!(expected, nr, "{name}");
        } else {
            // Syscalls newer than this libc: checked against the kernel's
            // x86_64 table by number.
            assert!(
                matches!(*name, "setxattrat" | "removexattrat" | "open_tree_attr"),
                "{name} is unchecked"
            );
        }
    }
}
