//! Shell executor integration tests — verify commands actually execute on this machine.
//! These tests run REAL commands via the GovernedShell actuator to confirm:
//! - PATH resolution works (sh -c finds /usr/bin/free, etc.)
//! - Output is captured correctly
//! - Allowlist permits monitoring commands
//! - Blocklist blocks dangerous commands

use nexus_kernel::actuators::{ActuatorContext, ActuatorRegistry};
use nexus_kernel::audit::AuditTrail;
use nexus_kernel::autonomy::AutonomyLevel;
use nexus_kernel::cognitive::PlannedAction;
use std::collections::HashSet;

fn make_exec_context() -> ActuatorContext {
    let mut caps = HashSet::new();
    caps.insert("process.exec".to_string());
    caps.insert("fs.read".to_string());
    ActuatorContext {
        agent_id: "test-sysmon".into(),
        agent_name: "Test Sysmon".into(),
        working_dir: std::env::temp_dir().join("nexus-test-agent"),
        autonomy_level: AutonomyLevel::L3,
        capabilities: caps,
        fuel_remaining: 1000.0,
        egress_allowlist: vec![],
        action_review_engine: None,
        hitl_approved: false,
    }
}

#[test]
fn test_sh_free_m_produces_real_output() {
    let registry = ActuatorRegistry::with_defaults();
    let ctx = make_exec_context();
    let mut audit = AuditTrail::new();

    let action = PlannedAction::ShellCommand {
        command: "free".into(),
        args: vec!["-m".into()],
    };

    let result = registry.execute_action(&action, &ctx, &mut audit);
    match result {
        Ok(r) => {
            assert!(r.success, "free -m should succeed: {}", r.output);
            assert!(
                r.output.contains("Mem:"),
                "output should contain 'Mem:': {}",
                r.output
            );
            eprintln!("free -m output:\n{}", r.output);
        }
        Err(e) => panic!("free -m should not fail: {e}"),
    }
}

#[test]
fn test_sh_df_h_produces_real_output() {
    let registry = ActuatorRegistry::with_defaults();
    let ctx = make_exec_context();
    let mut audit = AuditTrail::new();

    let action = PlannedAction::ShellCommand {
        command: "df".into(),
        args: vec!["-h".into(), "/".into()],
    };

    let result = registry.execute_action(&action, &ctx, &mut audit);
    match result {
        Ok(r) => {
            assert!(r.success, "df -h / should succeed: {}", r.output);
            assert!(
                r.output.contains("Filesystem") || r.output.contains("/dev/"),
                "output should contain filesystem info: {}",
                r.output
            );
        }
        Err(e) => panic!("df -h should not fail: {e}"),
    }
}

#[test]
fn test_sh_cat_proc_loadavg() {
    let registry = ActuatorRegistry::with_defaults();
    let ctx = make_exec_context();
    let mut audit = AuditTrail::new();

    let action = PlannedAction::ShellCommand {
        command: "cat".into(),
        args: vec!["/proc/loadavg".into()],
    };

    let result = registry.execute_action(&action, &ctx, &mut audit);
    match result {
        Ok(r) => {
            assert!(r.success, "cat /proc/loadavg should succeed: {}", r.output);
            // /proc/loadavg format: "0.50 0.30 0.20 1/234 5678"
            assert!(
                r.output.contains('/'),
                "output should contain load average format: {}",
                r.output
            );
        }
        Err(e) => panic!("cat /proc/loadavg should not fail: {e}"),
    }
}

#[test]
fn test_sh_uptime() {
    let registry = ActuatorRegistry::with_defaults();
    let ctx = make_exec_context();
    let mut audit = AuditTrail::new();

    let action = PlannedAction::ShellCommand {
        command: "uptime".into(),
        args: vec![],
    };

    let result = registry.execute_action(&action, &ctx, &mut audit);
    match result {
        Ok(r) => {
            assert!(r.success, "uptime should succeed: {}", r.output);
            assert!(
                r.output.contains("load average") || r.output.contains("up"),
                "output should contain uptime info: {}",
                r.output
            );
        }
        Err(e) => panic!("uptime should not fail: {e}"),
    }
}

#[test]
fn test_sh_echo_hello() {
    let registry = ActuatorRegistry::with_defaults();
    let ctx = make_exec_context();
    let mut audit = AuditTrail::new();

    let action = PlannedAction::ShellCommand {
        command: "echo".into(),
        args: vec!["hello world".into()],
    };

    let result = registry.execute_action(&action, &ctx, &mut audit);
    match result {
        Ok(r) => {
            assert!(r.success, "echo should succeed");
            assert!(
                r.output.trim() == "hello world",
                "output should be 'hello world': '{}'",
                r.output.trim()
            );
        }
        Err(e) => panic!("echo should not fail: {e}"),
    }
}

#[test]
fn test_sh_uname() {
    let registry = ActuatorRegistry::with_defaults();
    let ctx = make_exec_context();
    let mut audit = AuditTrail::new();

    let action = PlannedAction::ShellCommand {
        command: "uname".into(),
        args: vec!["-a".into()],
    };

    let result = registry.execute_action(&action, &ctx, &mut audit);
    match result {
        Ok(r) => {
            assert!(r.success, "uname -a should succeed");
            assert!(
                r.output.contains("Linux"),
                "output should contain 'Linux': {}",
                r.output
            );
        }
        Err(e) => panic!("uname should not fail: {e}"),
    }
}

#[test]
fn test_blocked_rm_rejected() {
    let registry = ActuatorRegistry::with_defaults();
    let ctx = make_exec_context();
    let mut audit = AuditTrail::new();

    let action = PlannedAction::ShellCommand {
        command: "rm".into(),
        args: vec!["-rf".into(), "/tmp/test".into()],
    };

    let result = registry.execute_action(&action, &ctx, &mut audit);
    assert!(result.is_err(), "rm should be blocked by the blocklist");
}

#[test]
fn test_blocked_sudo_rejected() {
    let registry = ActuatorRegistry::with_defaults();
    let ctx = make_exec_context();
    let mut audit = AuditTrail::new();

    let action = PlannedAction::ShellCommand {
        command: "sudo".into(),
        args: vec!["ls".into()],
    };

    let result = registry.execute_action(&action, &ctx, &mut audit);
    assert!(result.is_err(), "sudo should be blocked");
}

#[test]
fn test_unknown_command_rejected() {
    let registry = ActuatorRegistry::with_defaults();
    let ctx = make_exec_context();
    let mut audit = AuditTrail::new();

    let action = PlannedAction::ShellCommand {
        command: "totally_nonexistent_command_xyz".into(),
        args: vec![],
    };

    let result = registry.execute_action(&action, &ctx, &mut audit);
    assert!(
        result.is_err(),
        "unknown command should be blocked by allowlist"
    );
}

#[test]
fn test_no_capability_rejected() {
    let registry = ActuatorRegistry::with_defaults();
    // Context with NO capabilities
    let ctx = ActuatorContext {
        agent_id: "no-caps".into(),
        agent_name: "No Caps".into(),
        working_dir: std::env::temp_dir(),
        autonomy_level: AutonomyLevel::L1,
        capabilities: HashSet::new(),
        fuel_remaining: 1000.0,
        egress_allowlist: vec![],
        action_review_engine: None,
        hitl_approved: false,
    };
    let mut audit = AuditTrail::new();

    let action = PlannedAction::ShellCommand {
        command: "echo".into(),
        args: vec!["test".into()],
    };

    let result = registry.execute_action(&action, &ctx, &mut audit);
    assert!(
        result.is_err(),
        "should fail without process.exec capability"
    );
}
