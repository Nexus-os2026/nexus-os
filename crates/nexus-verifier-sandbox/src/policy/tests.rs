use super::fs_access::*;
use super::*;

#[test]
fn p2b_sandbox_policy_requires_all_six_namespaces_and_landlock_abi_6() {
    let policy = SandboxPolicy::V1;
    let mut namespaces: Vec<_> = policy.namespaces.to_vec();
    namespaces.sort();
    assert_eq!(
        namespaces,
        vec![
            Namespace::User,
            Namespace::Pid,
            Namespace::Net,
            Namespace::Ipc,
            Namespace::Uts,
            Namespace::Cgroup
        ]
    );
    assert!(policy.landlock_min_abi >= 6);
    assert!(policy.no_new_privs);
    assert!(!policy.loopback_up, "no network: loopback stays down");
}

#[test]
fn p2b_every_filesystem_and_tcp_right_is_handled_and_both_scopes_apply() {
    let policy = SandboxPolicy::V1;
    assert_eq!(policy.handled_fs, ALL_ABI_5);
    assert_eq!(policy.handled_fs.count_ones(), 16);
    assert_eq!(
        policy.handled_net,
        net_access::BIND_TCP | net_access::CONNECT_TCP
    );
    assert_eq!(policy.scopes, scope::ABSTRACT_UNIX_SOCKET | scope::SIGNAL);
    let mut roles = policy.roles.to_vec();
    roles.sort();
    roles.dedup();
    assert_eq!(roles.len(), Role::ALL.len(), "every role once");
}

#[test]
fn p2b_candidate_input_is_read_only_and_never_executable() {
    assert_eq!(Role::CandidateInput.rights(), READ_FILE | READ_DIR);
}

#[test]
fn p2b_toolchain_and_runtime_are_read_and_execute_only() {
    assert_eq!(Role::ToolchainRoot.rights(), READ_FILE | READ_DIR | EXECUTE);
    assert_eq!(Role::RuntimeLoader.rights(), READ_FILE | EXECUTE);
    assert_eq!(Role::RuntimeLibrary.rights(), READ_FILE);
    for role in [
        Role::ToolchainRoot,
        Role::RuntimeLoader,
        Role::RuntimeLibrary,
    ] {
        let writes = WRITE_FILE | REMOVE_DIR | REMOVE_FILE | MAKE_DIR | MAKE_REG | REFER | TRUNCATE;
        assert_eq!(role.rights() & writes, 0, "{role:?} is never writable");
    }
}

#[test]
fn p2b_only_target_combines_write_and_execute() {
    for role in Role::ALL {
        let rights = role.rights();
        let executable = rights & EXECUTE != 0;
        let writable = rights & (WRITE_FILE | MAKE_REG) != 0;
        assert!(
            !(executable && writable) || role == Role::Target,
            "{role:?} must not be both writable and executable"
        );
    }
    assert_eq!(Role::Scratch.rights() & EXECUTE, 0);
}

#[test]
fn p2b_no_role_creates_sockets_fifos_devices_or_symlinks_or_uses_device_ioctls() {
    let never = MAKE_SOCK | MAKE_FIFO | MAKE_CHAR | MAKE_BLOCK | MAKE_SYM | IOCTL_DEV;
    for role in Role::ALL {
        assert_eq!(role.rights() & never, 0, "{role:?}");
        assert_eq!(
            role.rights() & !ALL_ABI_5,
            0,
            "{role:?} uses only known rights"
        );
    }
}

#[test]
fn p2b_device_files_are_single_files_with_minimal_rights() {
    assert_eq!(Role::DevNull.rights(), READ_FILE | WRITE_FILE);
    assert_eq!(Role::DevUrandom.rights(), READ_FILE);
    for role in [
        Role::DevNull,
        Role::DevUrandom,
        Role::RuntimeLoader,
        Role::RuntimeLibrary,
    ] {
        assert!(!role.is_directory(), "{role:?} is a single file");
        assert_eq!(role.rights() & READ_DIR, 0);
    }
}

#[test]
fn p2b_resource_policy_is_the_documented_constants() {
    let limits = ResourcePolicy::RUST_OFFLINE_V1;
    assert_eq!(limits.memory_max_bytes, 4 * 1024 * 1024 * 1024);
    assert_eq!(limits.swap_max_bytes, 0);
    assert_eq!(limits.pids_max, 256);
    assert_eq!(limits.cpu_quota_us_per_sec, 4_000_000);
    assert_eq!(limits.cpus(), 4);
    assert_eq!(limits.wall_timeout_secs, 600);
    assert!(limits.runtime_backstop_secs > limits.wall_timeout_secs);
    assert_eq!(limits.output_ceiling_bytes, 8 * 1024 * 1024);
    assert!(limits.excerpt_bytes > 0 && limits.excerpt_bytes <= limits.output_ceiling_bytes);
}

#[test]
fn p2b_policy_hashes_are_deterministic_and_cover_every_field() {
    let base = SandboxPolicy::V1;
    assert_eq!(base.hash(), SandboxPolicy::V1.hash());
    let variants = [
        SandboxPolicy { version: 2, ..base },
        SandboxPolicy {
            namespaces: &[Namespace::User, Namespace::Pid],
            ..base
        },
        SandboxPolicy {
            landlock_min_abi: 5,
            ..base
        },
        SandboxPolicy {
            handled_fs: ALL_ABI_5 & !IOCTL_DEV,
            ..base
        },
        SandboxPolicy {
            handled_net: net_access::BIND_TCP,
            ..base
        },
        SandboxPolicy {
            scopes: scope::SIGNAL,
            ..base
        },
        SandboxPolicy {
            roles: &[Role::ToolchainRoot],
            ..base
        },
        SandboxPolicy {
            no_new_privs: false,
            ..base
        },
        SandboxPolicy {
            loopback_up: true,
            ..base
        },
        SandboxPolicy {
            inherited_fds: "all",
            ..base
        },
        SandboxPolicy {
            environment: "inherited",
            ..base
        },
        SandboxPolicy {
            stdio: "inherited",
            ..base
        },
    ];
    for variant in variants {
        assert_ne!(variant.hash(), base.hash(), "{variant:?}");
    }

    let limits = ResourcePolicy::RUST_OFFLINE_V1;
    assert_eq!(limits.hash(), ResourcePolicy::RUST_OFFLINE_V1.hash());
    let changed = [
        ResourcePolicy {
            memory_max_bytes: 1,
            ..limits
        },
        ResourcePolicy {
            swap_max_bytes: 1,
            ..limits
        },
        ResourcePolicy {
            pids_max: 1,
            ..limits
        },
        ResourcePolicy {
            cpu_quota_us_per_sec: 1,
            ..limits
        },
        ResourcePolicy {
            wall_timeout_secs: 1,
            ..limits
        },
        ResourcePolicy {
            runtime_backstop_secs: 1,
            ..limits
        },
        ResourcePolicy {
            output_ceiling_bytes: 1,
            ..limits
        },
        ResourcePolicy {
            excerpt_bytes: 1,
            ..limits
        },
    ];
    for variant in changed {
        assert_ne!(variant.hash(), limits.hash(), "{variant:?}");
    }
}

#[test]
fn p2b_policy_identities_are_domain_separated() {
    assert_ne!(
        SandboxPolicy::V1.hash().bytes(),
        ResourcePolicy::RUST_OFFLINE_V1.hash().bytes()
    );
    assert_eq!(SandboxPolicy::V1.hash().to_hex().len(), 64);
}
