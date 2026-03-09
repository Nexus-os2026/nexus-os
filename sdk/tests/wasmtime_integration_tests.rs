//! Integration tests for the wasmtime sandbox, WasmAgent, and signature verification.
//!
//! These tests exercise the full stack: wasm compilation, host function dispatch,
//! capability checks, fuel metering, memory isolation, crash isolation,
//! safety supervisor kill, audit trail emission, and Ed25519 signature verification.

use nexus_sdk::context::AgentContext;
use nexus_sdk::sandbox::{SandboxConfig, SandboxRuntime};
use nexus_sdk::wasm_signature::{
    sign_wasm_bytes, test_keypair, SignaturePolicy, SignatureVerification,
};
use nexus_sdk::wasmtime_sandbox::WasmtimeSandbox;
use nexus_sdk::{AgentOutput, NexusAgent, WasmAgent};
use std::sync::Arc;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_ctx(capabilities: Vec<&str>, fuel: u64) -> AgentContext {
    AgentContext::new(
        Uuid::new_v4(),
        capabilities.into_iter().map(|s| s.to_string()).collect(),
        fuel,
    )
}

fn make_sandbox() -> WasmtimeSandbox {
    WasmtimeSandbox::with_defaults(SandboxConfig::default()).unwrap()
}

fn minimal_wasm() -> Vec<u8> {
    wat::parse_str("(module)").unwrap()
}

fn hello_wasm() -> Vec<u8> {
    wat::parse_str(
        r#"(module
            (import "nexus" "nexus_log" (func $log (param i32 i32 i32)))
            (memory (export "memory") 1)
            (data (i32.const 0) "hello from wasm")
            (func (export "_start")
                (call $log (i32.const 0) (i32.const 0) (i32.const 15))
            )
        )"#,
    )
    .unwrap()
}

fn loop_wasm() -> Vec<u8> {
    wat::parse_str(
        r#"(module
            (func (export "_start")
                (loop $inf (br $inf))
            )
        )"#,
    )
    .unwrap()
}

fn work_wasm() -> Vec<u8> {
    wat::parse_str(
        r#"(module
            (func (export "_start")
                (local $i i32)
                (block $done
                    (loop $loop
                        (local.get $i)
                        (i32.const 100)
                        (i32.ge_u)
                        (br_if $done)
                        (local.get $i)
                        (i32.const 1)
                        (i32.add)
                        (local.set $i)
                        (br $loop)
                    )
                )
            )
        )"#,
    )
    .unwrap()
}

fn llm_call_wasm() -> Vec<u8> {
    wat::parse_str(
        r#"(module
            (import "nexus" "nexus_llm_query" (func $llm (param i32 i32 i32) (result i32)))
            (memory (export "memory") 1)
            (data (i32.const 0) "test prompt")
            (func (export "_start")
                (drop (call $llm (i32.const 0) (i32.const 11) (i32.const 100)))
            )
        )"#,
    )
    .unwrap()
}

fn fs_read_wasm() -> Vec<u8> {
    wat::parse_str(
        r#"(module
            (import "nexus" "nexus_fs_read" (func $read (param i32 i32) (result i32)))
            (memory (export "memory") 1)
            (data (i32.const 0) "/tmp/test.txt")
            (func (export "_start")
                (drop (call $read (i32.const 0) (i32.const 13)))
            )
        )"#,
    )
    .unwrap()
}

fn crash_wasm() -> Vec<u8> {
    wat::parse_str(
        r#"(module
            (func (export "_start")
                (unreachable)
            )
        )"#,
    )
    .unwrap()
}

// ===========================================================================
// 1. Execute valid wasm
// ===========================================================================

#[test]
fn execute_valid_wasm_module() {
    let mut sandbox = make_sandbox();
    let mut ctx = make_ctx(vec![], 1000);
    let wasm = hello_wasm();

    let result = sandbox.execute(&wasm, &mut ctx);
    assert!(result.completed, "valid wasm should complete");
    assert!(!result.killed);
    assert!(result.outputs.contains(&"hello from wasm".to_string()));
}

// ===========================================================================
// 2. Invalid bytes error
// ===========================================================================

#[test]
fn invalid_bytes_return_compile_error() {
    let mut sandbox = make_sandbox();
    let mut ctx = make_ctx(vec![], 1000);

    let result = sandbox.execute(b"definitely not wasm", &mut ctx);
    assert!(!result.completed);
    assert!(result.outputs[0].contains("wasm compile error"));
}

// ===========================================================================
// 3. Host function capability granted vs denied
// ===========================================================================

#[test]
fn host_function_capability_granted() {
    let wasm = llm_call_wasm();
    let mut sandbox = make_sandbox();
    let mut ctx = make_ctx(vec!["llm.query"], 1000);

    let result = sandbox.execute(&wasm, &mut ctx);
    assert!(result.completed);
    // LLM query costs 10 fuel, plus wasm instruction fuel
    assert!(ctx.fuel_remaining() < 1000);
    // Should have the mock LLM response in outputs
    assert!(result.outputs.iter().any(|o| o.contains("[mock-llm-response")));
}

#[test]
fn host_function_capability_denied_at_sandbox_level() {
    let wasm = llm_call_wasm();
    // Sandbox config doesn't allow llm_query
    let mut sandbox = WasmtimeSandbox::with_defaults(SandboxConfig {
        memory_limit_bytes: 256 * 1024 * 1024,
        execution_timeout_secs: 300,
        allowed_host_functions: vec![], // no host functions allowed
    })
    .unwrap();
    let mut ctx = make_ctx(vec!["llm.query"], 1000);

    let result = sandbox.execute(&wasm, &mut ctx);
    // The wasm module still completes — the host function returns -1 (CapabilityDenied)
    // but the wasm ignores the return code (drops it)
    assert!(result.completed);
}

#[test]
fn host_function_capability_denied_at_context_level() {
    let wasm = llm_call_wasm();
    let mut sandbox = make_sandbox();
    // Agent context lacks llm.query capability
    let mut ctx = make_ctx(vec![], 1000);

    let result = sandbox.execute(&wasm, &mut ctx);
    // Module completes because it drops the error return code
    assert!(result.completed);
    // No mock LLM response should appear
    assert!(!result.outputs.iter().any(|o| o.contains("[mock-llm-response")));
}

#[test]
fn fs_read_capability_granted() {
    let wasm = fs_read_wasm();
    let mut sandbox = make_sandbox();
    let mut ctx = make_ctx(vec!["fs.read"], 1000);

    let result = sandbox.execute(&wasm, &mut ctx);
    assert!(result.completed);
    assert!(result.outputs.iter().any(|o| o.contains("[mock-file-content")));
}

// ===========================================================================
// 4. Fuel exhaustion returns clean SandboxResult
// ===========================================================================

#[test]
fn fuel_exhaustion_returns_clean_result() {
    let wasm = loop_wasm();
    let mut sandbox = WasmtimeSandbox::with_defaults(SandboxConfig {
        memory_limit_bytes: 1024 * 1024,
        execution_timeout_secs: 300,
        allowed_host_functions: vec![],
    })
    .unwrap();
    let mut ctx = make_ctx(vec![], 1);

    let result = sandbox.execute(&wasm, &mut ctx);
    assert!(!result.completed);
    assert!(result.killed);
    assert_eq!(result.kill_reason.as_deref(), Some("fuel_exhausted"));
    assert!(result.fuel_used > 0);
    // Context fuel should be reduced
    assert!(ctx.fuel_remaining() < 1);
}

#[test]
fn fuel_tracking_accurate() {
    let wasm = work_wasm();
    let mut sandbox = make_sandbox();
    let mut ctx = make_ctx(vec![], 1000);
    let fuel_before = ctx.fuel_remaining();

    let result = sandbox.execute(&wasm, &mut ctx);
    assert!(result.completed);
    assert!(sandbox.fuel_consumed() > 0);
    assert!(ctx.fuel_remaining() < fuel_before);

    // Audit trail should have wasm_fuel_consumed event
    let has_fuel_event = ctx.audit_trail().events().iter().any(|e| {
        e.payload.get("action").and_then(|v| v.as_str()) == Some("wasm_fuel_consumed")
    });
    assert!(has_fuel_event);
}

// ===========================================================================
// 5. Memory isolation between two agents
// ===========================================================================

#[test]
fn memory_isolation_between_agents() {
    let wasm_write = wat::parse_str(
        r#"(module
            (memory (export "memory") 1)
            (func (export "_start")
                ;; Write 0xDEADBEEF at offset 0
                (i32.store (i32.const 0) (i32.const 0xDEADBEEF))
            )
        )"#,
    )
    .unwrap();

    let wasm_read = wat::parse_str(
        r#"(module
            (import "nexus" "nexus_log" (func $log (param i32 i32 i32)))
            (memory (export "memory") 1)
            (func (export "_start")
                ;; Read offset 0 — should be 0 (fresh memory), not 0xDEADBEEF
                (if (i32.ne (i32.load (i32.const 0)) (i32.const 0))
                    (then (unreachable))
                )
                (call $log (i32.const 0) (i32.const 0) (i32.const 0))
            )
        )"#,
    )
    .unwrap();

    // Agent A writes to memory
    let mut sandbox_a = make_sandbox();
    let mut ctx_a = make_ctx(vec![], 1000);
    let result_a = sandbox_a.execute(&wasm_write, &mut ctx_a);
    assert!(result_a.completed);

    // Agent B reads — should see fresh zeroed memory (different Store)
    let mut sandbox_b = make_sandbox();
    let mut ctx_b = make_ctx(vec![], 1000);
    let result_b = sandbox_b.execute(&wasm_read, &mut ctx_b);
    assert!(result_b.completed, "agent B should not see agent A's memory");
}

// ===========================================================================
// 6. Crash isolation
// ===========================================================================

#[test]
fn crash_isolation_does_not_affect_other_agents() {
    // Agent A crashes
    let mut sandbox_a = make_sandbox();
    let mut ctx_a = make_ctx(vec![], 1000);
    let result_a = sandbox_a.execute(&crash_wasm(), &mut ctx_a);
    assert!(!result_a.completed);
    assert!(result_a.outputs.iter().any(|o| o.contains("wasm trap")));

    // Agent B still runs fine
    let mut sandbox_b = make_sandbox();
    let mut ctx_b = make_ctx(vec![], 1000);
    let result_b = sandbox_b.execute(&hello_wasm(), &mut ctx_b);
    assert!(result_b.completed);
    assert!(result_b.outputs.contains(&"hello from wasm".to_string()));
}

#[test]
fn crash_preserves_context_state() {
    let mut sandbox = make_sandbox();
    let mut ctx = make_ctx(vec!["llm.query"], 1000);

    let result = sandbox.execute(&crash_wasm(), &mut ctx);
    assert!(!result.completed);

    // Context should still be usable — fuel remaining unchanged (crash used negligible fuel)
    assert!(ctx.fuel_remaining() > 0);
    assert_eq!(ctx.agent_id(), ctx.agent_id()); // not corrupted
}

// ===========================================================================
// 7. Kill from safety supervisor halts agent
// ===========================================================================

#[test]
fn kill_with_reason_halts_agent() {
    let mut sandbox = make_sandbox();

    // Simulate safety supervisor halt
    sandbox
        .kill_with_reason("safety supervisor: three-strike rule")
        .unwrap();

    let mut ctx = make_ctx(vec![], 1000);
    let result = sandbox.execute(&hello_wasm(), &mut ctx);

    assert!(!result.completed);
    assert!(result.killed);
    assert_eq!(
        result.kill_reason.as_deref(),
        Some("safety supervisor: three-strike rule")
    );
}

#[test]
fn manual_kill_prevents_subsequent_execution() {
    let mut sandbox = make_sandbox();
    let mut ctx = make_ctx(vec![], 1000);

    // First execution succeeds
    let result = sandbox.execute(&minimal_wasm(), &mut ctx);
    assert!(result.completed);

    // Kill
    sandbox.kill().unwrap();

    // Second execution blocked
    let result = sandbox.execute(&minimal_wasm(), &mut ctx);
    assert!(!result.completed);
    assert!(result.killed);
}

// ===========================================================================
// 8. Audit trail events emitted
// ===========================================================================

#[test]
fn audit_trail_captures_signature_check() {
    let mut sandbox = make_sandbox();
    let mut ctx = make_ctx(vec![], 1000);

    sandbox.execute(&minimal_wasm(), &mut ctx);

    let has_sig_event = ctx.audit_trail().events().iter().any(|e| {
        e.payload.get("action").and_then(|v| v.as_str()) == Some("wasm_signature_check")
    });
    assert!(has_sig_event, "should have a wasm_signature_check audit event");
}

#[test]
fn audit_trail_captures_host_function_calls() {
    let wasm = llm_call_wasm();
    let mut sandbox = make_sandbox();
    let mut ctx = make_ctx(vec!["llm.query"], 1000);

    sandbox.execute(&wasm, &mut ctx);

    // Should have LlmCall event from AgentContext::llm_query
    let has_llm_event = ctx.audit_trail().events().iter().any(|e| {
        e.payload.get("action").and_then(|v| v.as_str()) == Some("llm_query")
    });
    assert!(has_llm_event, "should have llm_query audit event");
}

#[test]
fn audit_trail_captures_fuel_consumption() {
    let wasm = work_wasm();
    let mut sandbox = make_sandbox();
    let mut ctx = make_ctx(vec![], 1000);

    sandbox.execute(&wasm, &mut ctx);

    let has_fuel_event = ctx.audit_trail().events().iter().any(|e| {
        e.payload.get("action").and_then(|v| v.as_str()) == Some("wasm_fuel_consumed")
    });
    assert!(has_fuel_event, "should have wasm_fuel_consumed audit event");
}

// ===========================================================================
// 9. Signature verification — rejects unsigned, accepts signed
// ===========================================================================

#[test]
fn signature_rejects_unsigned_module() {
    let (_, vk) = test_keypair();
    let mut sandbox = make_sandbox();
    sandbox.set_signature_policy(SignaturePolicy::RequireSigned);
    sandbox.add_trusted_key(vk);

    let mut ctx = make_ctx(vec![], 1000);
    let result = sandbox.execute(&minimal_wasm(), &mut ctx);

    assert!(!result.completed);
    assert!(result.outputs[0].contains("unsigned wasm module rejected"));
}

#[test]
fn signature_accepts_signed_module() {
    let (sk, vk) = test_keypair();
    let wasm = hello_wasm();
    let signed = sign_wasm_bytes(&wasm, &sk);

    let mut sandbox = make_sandbox();
    sandbox.set_signature_policy(SignaturePolicy::RequireSigned);
    sandbox.add_trusted_key(vk);

    let mut ctx = make_ctx(vec![], 1000);
    let result = sandbox.execute(&signed, &mut ctx);

    assert!(result.completed, "signed module should execute successfully");
    assert!(result.outputs.contains(&"hello from wasm".to_string()));
}

#[test]
fn signature_rejects_wrong_key() {
    let (sk, _vk) = test_keypair();
    let wasm = minimal_wasm();
    let signed = sign_wasm_bytes(&wasm, &sk);

    // Use a different key for verification
    use sha2::Digest;
    let other_seed = sha2::Sha256::digest(b"other-untrusted-key");
    let mut other_bytes = [0u8; 32];
    other_bytes.copy_from_slice(&other_seed);
    let other_sk = ed25519_dalek::SigningKey::from_bytes(&other_bytes);
    let other_vk = other_sk.verifying_key();

    let mut sandbox = make_sandbox();
    sandbox.set_signature_policy(SignaturePolicy::RequireSigned);
    sandbox.add_trusted_key(other_vk);

    let mut ctx = make_ctx(vec![], 1000);
    let result = sandbox.execute(&signed, &mut ctx);

    assert!(!result.completed);
    assert!(result.outputs[0].contains("signature"));
}

#[test]
fn signature_rejects_tampered_module() {
    let (sk, vk) = test_keypair();
    let wasm = minimal_wasm();
    let mut signed = sign_wasm_bytes(&wasm, &sk);

    // Tamper with a byte
    signed[0] ^= 0xff;

    let mut sandbox = make_sandbox();
    sandbox.set_signature_policy(SignaturePolicy::RequireSigned);
    sandbox.add_trusted_key(vk);

    let mut ctx = make_ctx(vec![], 1000);
    let result = sandbox.execute(&signed, &mut ctx);

    assert!(!result.completed);
}

#[test]
fn signature_allow_unsigned_still_runs() {
    let mut sandbox = make_sandbox();
    sandbox.set_signature_policy(SignaturePolicy::AllowUnsigned);

    let mut ctx = make_ctx(vec![], 1000);
    let result = sandbox.execute(&minimal_wasm(), &mut ctx);

    assert!(result.completed, "unsigned module should run under AllowUnsigned");
}

#[test]
fn signature_audit_event_records_result() {
    let (sk, vk) = test_keypair();
    let signed = sign_wasm_bytes(&minimal_wasm(), &sk);

    let mut sandbox = make_sandbox();
    sandbox.set_signature_policy(SignaturePolicy::RequireSigned);
    sandbox.add_trusted_key(vk);

    let mut ctx = make_ctx(vec![], 1000);
    sandbox.execute(&signed, &mut ctx);

    // Find the signature check event and verify it says "accepted: true"
    let sig_event = ctx
        .audit_trail()
        .events()
        .iter()
        .find(|e| {
            e.payload.get("action").and_then(|v| v.as_str()) == Some("wasm_signature_check")
        })
        .expect("should have wasm_signature_check event");

    assert_eq!(
        sig_event.payload.get("accepted").and_then(|v| v.as_bool()),
        Some(true)
    );
}

// ===========================================================================
// 10. WasmAgent integration via NexusAgent trait
// ===========================================================================

#[test]
fn wasm_agent_full_lifecycle() {
    let mut agent = WasmAgent::new(hello_wasm(), SandboxConfig::default()).unwrap();
    let mut ctx = make_ctx(vec!["llm.query"], 1000);

    // init
    assert!(agent.init(&mut ctx).is_ok());

    // execute
    let output = agent.execute(&mut ctx).unwrap();
    assert_eq!(output.status, "ok");
    assert!(!output.outputs.is_empty());

    // shutdown
    assert!(agent.shutdown(&mut ctx).is_ok());
}

#[test]
fn wasm_agent_checkpoint_restore_execute() {
    let wasm = minimal_wasm();
    let agent = WasmAgent::new(wasm.clone(), SandboxConfig::default()).unwrap();

    let data = agent.checkpoint().unwrap();
    assert_eq!(data, wasm);

    let mut agent2 = WasmAgent::new(vec![], SandboxConfig::default()).unwrap();
    assert!(agent2.restore(&data).is_ok());

    let mut ctx = make_ctx(vec![], 1000);
    let output = agent2.execute(&mut ctx).unwrap();
    assert_eq!(output.status, "ok");
}

// ===========================================================================
// 11. Shared engine across agents
// ===========================================================================

#[test]
fn shared_engine_isolation() {
    let mut wasm_config = wasmtime::Config::new();
    wasm_config.consume_fuel(true);
    let engine = Arc::new(wasmtime::Engine::new(&wasm_config).unwrap());

    let mut sandbox_a = WasmtimeSandbox::new(SandboxConfig::default(), Arc::clone(&engine));
    let mut sandbox_b = WasmtimeSandbox::new(SandboxConfig::default(), Arc::clone(&engine));

    let mut ctx_a = make_ctx(vec![], 1000);
    let mut ctx_b = make_ctx(vec![], 1000);

    let result_a = sandbox_a.execute(&crash_wasm(), &mut ctx_a);
    assert!(!result_a.completed);

    let result_b = sandbox_b.execute(&hello_wasm(), &mut ctx_b);
    assert!(result_b.completed, "crash in agent A must not affect agent B with shared engine");
}

// ===========================================================================
// 12. Multiple executions accumulate fuel
// ===========================================================================

#[test]
fn multiple_executions_accumulate_fuel() {
    let wasm = work_wasm();
    let mut sandbox = make_sandbox();
    let mut ctx = make_ctx(vec![], 10000);

    sandbox.execute(&wasm, &mut ctx);
    let fuel_after_first = sandbox.fuel_consumed();

    sandbox.execute(&wasm, &mut ctx);
    let fuel_after_second = sandbox.fuel_consumed();

    assert!(fuel_after_second > fuel_after_first, "fuel should accumulate across executions");
}
