//! Tests for computer use tools: screen_capture, screen_interact, screen_analyze.

use nexus_code::governance::{Capability, CapabilityManager, CapabilityScope};
use nexus_code::tools::screen_interact::is_blocked_combo;
use nexus_code::tools::{NxTool, ToolContext, ToolRegistry};
use serde_json::json;

fn test_ctx() -> ToolContext {
    ToolContext {
        working_dir: std::env::temp_dir(),
        blocked_paths: vec![],
        max_file_scope: None,
        non_interactive: true,
    }
}

// ═══════════════════════════════════════════════════════
// 1-3: Tool registration tests
// ═══════════════════════════════════════════════════════

#[test]
fn test_screen_capture_tool_registered() {
    let mut registry = ToolRegistry::with_defaults();
    assert!(registry.get("screen_capture").is_none());
    registry.register_computer_use_tools();
    assert!(registry.get("screen_capture").is_some());
    assert_eq!(
        registry.get("screen_capture").unwrap().name(),
        "screen_capture"
    );
}

#[test]
fn test_screen_interact_tool_registered() {
    let mut registry = ToolRegistry::with_defaults();
    assert!(registry.get("screen_interact").is_none());
    registry.register_computer_use_tools();
    assert!(registry.get("screen_interact").is_some());
    assert_eq!(
        registry.get("screen_interact").unwrap().name(),
        "screen_interact"
    );
}

#[test]
fn test_screen_analyze_tool_registered() {
    let mut registry = ToolRegistry::with_defaults();
    assert!(registry.get("screen_analyze").is_none());
    registry.register_computer_use_tools();
    assert!(registry.get("screen_analyze").is_some());
    assert_eq!(
        registry.get("screen_analyze").unwrap().name(),
        "screen_analyze"
    );
}

// ═══════════════════════════════════════════════════════
// 4: Blocked combos
// ═══════════════════════════════════════════════════════

#[test]
fn test_screen_interact_blocked_combos() {
    assert!(is_blocked_combo("ctrl+alt+Delete"));
    assert!(is_blocked_combo("ctrl+alt+del"));
    assert!(is_blocked_combo("super+l"));
    assert!(is_blocked_combo("alt+F4"));
    assert!(is_blocked_combo("ctrl+alt+BackSpace"));
    // Safe combos should pass
    assert!(!is_blocked_combo("ctrl+s"));
    assert!(!is_blocked_combo("ctrl+c"));
    assert!(!is_blocked_combo("ctrl+shift+t"));
    assert!(!is_blocked_combo("Return"));
}

// ═══════════════════════════════════════════════════════
// 5: Governance denied without capability
// ═══════════════════════════════════════════════════════

#[test]
fn test_computer_use_capability_required() {
    // Default capabilities do NOT include ComputerUse
    let mut caps = CapabilityManager::with_defaults();
    let result = caps.check(Capability::ComputerUse, "screen: click");
    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        nexus_code::error::NxError::CapabilityDenied { capability, .. } => {
            assert_eq!(capability, "computer.use");
        }
        _ => panic!("Expected CapabilityDenied, got {:?}", err),
    }
}

// ═══════════════════════════════════════════════════════
// 6: Governance allowed with capability
// ═══════════════════════════════════════════════════════

#[test]
fn test_computer_use_capability_granted() {
    let mut caps = CapabilityManager::with_defaults();
    caps.grant(Capability::ComputerUse, CapabilityScope::Full);
    let result = caps.check(Capability::ComputerUse, "screen: click");
    assert!(result.is_ok());
}

// ═══════════════════════════════════════════════════════
// 7: --computer-use flag enables tools
// ═══════════════════════════════════════════════════════

#[test]
fn test_computer_use_flag_enables_tools() {
    let config = nexus_code::config::NxConfig::default();
    let mut app = nexus_code::app::App::new(config).unwrap();
    assert_eq!(app.tool_registry.list().len(), 11);
    assert!(!app.is_computer_use_active());

    app.enable_computer_use();

    assert_eq!(app.tool_registry.list().len(), 14);
    assert!(app.is_computer_use_active());
    assert!(app.tool_registry.get("screen_capture").is_some());
    assert!(app.tool_registry.get("screen_interact").is_some());
    assert!(app.tool_registry.get("screen_analyze").is_some());
}

// ═══════════════════════════════════════════════════════
// 8-10: Fuel cost tests
// ═══════════════════════════════════════════════════════

#[test]
fn test_fuel_cost_screen_capture() {
    let tool = nexus_code::tools::screen_capture::ScreenCaptureTool;
    assert_eq!(tool.estimated_fuel(&json!({})), 1);
}

#[test]
fn test_fuel_cost_screen_interact() {
    let tool = nexus_code::tools::screen_interact::ScreenInteractTool;
    assert_eq!(tool.estimated_fuel(&json!({"action": "click"})), 1);
}

#[test]
fn test_fuel_cost_screen_analyze() {
    let tool = nexus_code::tools::screen_analyze::ScreenAnalyzeTool;
    assert_eq!(tool.estimated_fuel(&json!({"question": "what?"})), 5);
}

// ═══════════════════════════════════════════════════════
// 11-12: Status bar tool count + screen indicator
// ═══════════════════════════════════════════════════════

#[test]
fn test_status_bar_shows_14_tools_when_enabled() {
    let config = nexus_code::config::NxConfig::default();
    let mut app = nexus_code::app::App::new(config).unwrap();
    app.enable_computer_use();
    assert_eq!(app.tool_registry.list().len(), 14);
}

#[test]
fn test_screen_off_indicator_default() {
    let config = nexus_code::config::NxConfig::default();
    let app = nexus_code::app::App::new(config).unwrap();
    assert!(!app.is_computer_use_active());
}

// ═══════════════════════════════════════════════════════
// 13: Screen on indicator when enabled
// ═══════════════════════════════════════════════════════

#[test]
fn test_screen_on_indicator_when_enabled() {
    let config = nexus_code::config::NxConfig::default();
    let mut app = nexus_code::app::App::new(config).unwrap();
    app.enable_computer_use();
    assert!(app.is_computer_use_active());
}

// ═══════════════════════════════════════════════════════
// 14: Blocked combo in execute returns error
// ═══════════════════════════════════════════════════════

#[tokio::test]
async fn test_screen_interact_execute_blocked_combo() {
    let tool = nexus_code::tools::screen_interact::ScreenInteractTool;
    let ctx = test_ctx();
    let result = tool
        .execute(json!({"action": "key", "combo": "ctrl+alt+Delete"}), &ctx)
        .await;
    assert!(!result.is_success());
    assert!(result.output.contains("Blocked key combo"));
}

// ═══════════════════════════════════════════════════════
// 15: Consent tier classification
// ═══════════════════════════════════════════════════════

#[test]
fn test_consent_tiers_for_computer_use() {
    let gate = nexus_code::governance::ConsentGate::new();

    // screen_capture = Tier2 (write-level)
    let tier = gate.classify("screen_capture");
    assert_eq!(tier, nexus_code::governance::ConsentTier::Tier2);

    // screen_interact = Tier3 (destructive)
    let tier = gate.classify("screen_interact");
    assert_eq!(tier, nexus_code::governance::ConsentTier::Tier3);

    // screen_analyze = Tier2
    let tier = gate.classify("screen_analyze");
    assert_eq!(tier, nexus_code::governance::ConsentTier::Tier2);
}

// ═══════════════════════════════════════════════════════
// 16: Capability mapping
// ═══════════════════════════════════════════════════════

#[test]
fn test_capability_for_tool_mapping() {
    assert_eq!(
        Capability::for_tool("screen_capture"),
        Some(Capability::ComputerUse)
    );
    assert_eq!(
        Capability::for_tool("screen_interact"),
        Some(Capability::ComputerUse)
    );
    assert_eq!(
        Capability::for_tool("screen_analyze"),
        Some(Capability::ComputerUse)
    );
}

// ═══════════════════════════════════════════════════════
// 17: Required capability returns ComputerUse
// ═══════════════════════════════════════════════════════

#[test]
fn test_required_capability_returns_computer_use() {
    let capture = nexus_code::tools::screen_capture::ScreenCaptureTool;
    assert_eq!(
        capture.required_capability(&json!({})),
        Some(Capability::ComputerUse)
    );

    let interact = nexus_code::tools::screen_interact::ScreenInteractTool;
    assert_eq!(
        interact.required_capability(&json!({})),
        Some(Capability::ComputerUse)
    );

    let analyze = nexus_code::tools::screen_analyze::ScreenAnalyzeTool;
    assert_eq!(
        analyze.required_capability(&json!({})),
        Some(Capability::ComputerUse)
    );
}

// ═══════════════════════════════════════════════════════
// 18: create_tool factory returns computer use tools
// ═══════════════════════════════════════════════════════

#[test]
fn test_create_tool_factory() {
    assert!(nexus_code::tools::create_tool("screen_capture").is_some());
    assert!(nexus_code::tools::create_tool("screen_interact").is_some());
    assert!(nexus_code::tools::create_tool("screen_analyze").is_some());
}

// ═══════════════════════════════════════════════════════
// 19: /qa slash command exists and requires computer use
// ═══════════════════════════════════════════════════════

#[test]
fn test_qa_slash_command_exists() {
    // /qa without computer use should return an error
    let result = nexus_code::commands::qa::execute("", false);
    match result {
        nexus_code::commands::CommandResult::Error(msg) => {
            assert!(msg.contains("Computer use is not active"));
        }
        _ => panic!("Expected Error when computer use is not active"),
    }

    // /qa with computer use should return an AgentPrompt
    let result = nexus_code::commands::qa::execute("", true);
    match result {
        nexus_code::commands::CommandResult::AgentPrompt(prompt) => {
            assert!(prompt.contains("Navigate to each page"));
        }
        _ => panic!("Expected AgentPrompt when computer use is active"),
    }
}

// ═══════════════════════════════════════════════════════
// 20: /computer-use on/off toggle works
// ═══════════════════════════════════════════════════════

#[tokio::test]
async fn test_computer_use_slash_command_toggle() {
    let config = nexus_code::config::NxConfig::default();
    let mut app = nexus_code::app::App::new(config).unwrap();

    // Initially disabled
    assert!(!app.is_computer_use_active());

    // /computer-use on
    let result = nexus_code::commands::computer_use_cmd::execute("on", &mut app).await;
    match result {
        nexus_code::commands::CommandResult::Output(msg) => {
            assert!(msg.contains("enabled"));
        }
        _ => panic!("Expected Output"),
    }
    assert!(app.is_computer_use_active());
    assert_eq!(app.tool_registry.list().len(), 14);

    // /computer-use off
    let result = nexus_code::commands::computer_use_cmd::execute("off", &mut app).await;
    match result {
        nexus_code::commands::CommandResult::Output(msg) => {
            assert!(msg.contains("disabled"));
        }
        _ => panic!("Expected Output"),
    }
    assert!(!app.is_computer_use_active());
    assert_eq!(app.tool_registry.list().len(), 11);
}

// ═══════════════════════════════════════════════════════
// 21: System prompt includes computer use section when enabled
// ══════════════════════���════════════════════════════════

#[test]
fn test_system_prompt_includes_computer_use() {
    let registry = ToolRegistry::with_defaults();
    let prompt = nexus_code::agent::build_system_prompt_with_computer_use(
        "You are Nexus Code.",
        &registry,
        true,
    );
    assert!(prompt.contains("Computer Use Mode"));
    assert!(prompt.contains("screen_capture"));
    assert!(prompt.contains("Autonomous Developer Workflow"));
    assert!(prompt.contains("Governance Rules"));
    assert!(prompt.contains("Quality Standard"));
}

// ═══════════════════════════════════════════════════════
// 22: System prompt excludes computer use section when disabled
// ═══════════════════════════════════════════════════════

#[test]
fn test_system_prompt_excludes_computer_use() {
    let registry = ToolRegistry::with_defaults();
    let prompt = nexus_code::agent::build_system_prompt_with_computer_use(
        "You are Nexus Code.",
        &registry,
        false,
    );
    assert!(!prompt.contains("Computer Use Mode"));
    assert!(!prompt.contains("Autonomous Developer Workflow"));
}

// ═══════════════════════════════════════════════════════
// 23: F3 panel renders correctly with computer use stats fields
// ═══════════════════════════════════════════════════════

#[test]
fn test_f3_panel_shows_computer_use_stats() {
    let config = nexus_code::config::NxConfig::default();
    let mut app = nexus_code::app::App::new(config).unwrap();
    app.enable_computer_use();

    // Verify TuiApp picks up computer_use_active state
    let tui = nexus_code::tui::TuiApp::new(&app);
    assert!(tui.computer_use_active);
    assert_eq!(tui.cu_screenshots, 0);
    assert_eq!(tui.cu_interactions, 0);
    assert_eq!(tui.cu_analyses, 0);
    assert_eq!(tui.cu_fixes_verified, 0);
    assert_eq!(tui.cu_fixes_total, 0);
    assert_eq!(tui.cu_current_page, "n/a");
    assert_eq!(tui.cu_last_action, "n/a");
}

// ═══════════════════════════════════════════════════════
// 24: Disable then re-enable computer use
// ═══════════════════════════════════════════════════════

#[test]
fn test_disable_then_reenable_computer_use() {
    let config = nexus_code::config::NxConfig::default();
    let mut app = nexus_code::app::App::new(config).unwrap();

    app.enable_computer_use();
    assert_eq!(app.tool_registry.list().len(), 14);
    assert!(app.is_computer_use_active());

    app.disable_computer_use();
    assert_eq!(app.tool_registry.list().len(), 11);
    assert!(!app.is_computer_use_active());

    // Re-enable should work
    app.enable_computer_use();
    assert_eq!(app.tool_registry.list().len(), 14);
    assert!(app.is_computer_use_active());
}

// ═══════════════════════════════════════════════════════
// 25: /qa page <name> works
// ═══════════════════════════════════════════════════════

#[test]
fn test_qa_page_command() {
    let result = nexus_code::commands::qa::execute("page Dashboard", true);
    match result {
        nexus_code::commands::CommandResult::AgentPrompt(prompt) => {
            assert!(prompt.contains("Dashboard"));
        }
        _ => panic!("Expected AgentPrompt"),
    }
}

// ═══════════════════════════════════════════════════════
// 26: /qa report works
// ═══════════════════════════════════════════════════════

#[test]
fn test_qa_report_command() {
    let result = nexus_code::commands::qa::execute("report", true);
    match result {
        nexus_code::commands::CommandResult::AgentPrompt(prompt) => {
            assert!(prompt.contains("Summarize"));
        }
        _ => panic!("Expected AgentPrompt"),
    }
}
