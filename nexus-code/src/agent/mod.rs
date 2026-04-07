//! Agent loop — the runtime that connects the LLM to the governed tool system.
//!
//! When the user asks "read main.rs and fix the bug", the LLM reasons about it,
//! decides to call `file_read`, the tool executes through the governance pipeline,
//! the result feeds back to the LLM, and the LLM continues until the task is done.

pub mod envelope;
pub mod executor;
pub mod loop_runtime;
pub mod planner;
pub mod sub_agent;
pub mod tool_protocol;

pub use loop_runtime::{run_agent_loop, AgentConfig};
pub use tool_protocol::{ToolCall, ToolDefinition, ToolProtocol, ToolResultMessage};

/// Events emitted by the agent loop for the UI to consume.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// A text delta from the LLM (stream to terminal).
    TextDelta(String),
    /// The LLM is requesting a tool call.
    ToolCallStart { name: String, id: String },
    /// Tool execution completed.
    ToolCallComplete {
        name: String,
        success: bool,
        duration_ms: u64,
        summary: String,
    },
    /// Tool was denied by governance (consent denied or capability denied).
    ToolCallDenied { name: String, reason: String },
    /// A turn completed (may or may not have more turns).
    TurnComplete { turn: u32, has_more: bool },
    /// Token usage for this turn.
    TokenUsage {
        input_tokens: u64,
        output_tokens: u64,
    },
    /// Agent loop finished (all turns done or stopped).
    Done { reason: String, total_turns: u32 },
    /// Error during agent loop.
    Error(String),
}

/// Build the complete system prompt including tool descriptions.
/// When `computer_use_active` is true, appends the autonomous developer section.
pub fn build_system_prompt(
    base_prompt: &str,
    tool_registry: &crate::tools::ToolRegistry,
) -> String {
    build_system_prompt_with_computer_use(base_prompt, tool_registry, false)
}

/// Build system prompt with optional computer use mode.
pub fn build_system_prompt_with_computer_use(
    base_prompt: &str,
    tool_registry: &crate::tools::ToolRegistry,
    computer_use_active: bool,
) -> String {
    let tool_descriptions = tool_registry.build_tool_prompt();
    let computer_use_section = if computer_use_active {
        COMPUTER_USE_PROMPT_SECTION
    } else {
        ""
    };
    format!(
        "{}\n\n## Available Tools\n\nYou have access to the following tools. \
         To use a tool, respond with a tool_use block.\n\n{}\n\n\
         ## Tool Usage Rules\n\n\
         - Always explain what you're about to do before calling a tool.\n\
         - If a tool call is denied, explain what happened and ask the user how to proceed.\n\
         - Use file_read before file_edit to understand the current content.\n\
         - Use search and glob to find relevant files before editing.\n\
         - Use bash for commands like running tests, building, or checking status.\n\
         - Be concise in tool inputs — don't include unnecessary content.{}",
        base_prompt, tool_descriptions, computer_use_section
    )
}

/// System prompt section appended when computer use mode is active.
pub const COMPUTER_USE_PROMPT_SECTION: &str = r#"

## Computer Use Mode — Autonomous Developer

You have access to the computer's screen and input devices. You can see what's on screen, click buttons, type text, scroll, and navigate applications — just like a human developer sitting at the keyboard.

### Your Computer Use Tools

1. **screen_capture** — Take a screenshot of the full screen or a specific window
   - `screen_capture()` — full screen
   - `screen_capture(window: "Nexus OS")` — specific window
   - Returns: base64 image you can analyze, plus file path and audit hash

2. **screen_interact** — Click, type, scroll, or press keys
   - `screen_interact(action: "click", x: 500, y: 300)` — click at coordinates
   - `screen_interact(action: "type", text: "hello")` — type text
   - `screen_interact(action: "scroll", direction: "down", amount: 3)` — scroll
   - `screen_interact(action: "key", combo: "ctrl+s")` — key combination
   - `screen_interact(action: "move", x: 100, y: 200)` — move mouse
   - All actions are governed: rate limited, blocked combos, audit logged

3. **screen_analyze** — Analyze a screenshot with vision
   - `screen_analyze(question: "What page is showing? List every UI issue.")` — uses last screenshot
   - `screen_analyze(question: "Is the bug fixed?", image: "base64...")` — specific image
   - Uses Opus 4.6 vision for deep understanding

### Autonomous Developer Workflow

When asked to fix or improve the Nexus OS app, follow this loop:

1. **SCREENSHOT** — Capture the current state of the Nexus OS window
2. **ANALYZE** — Use vision to understand what page is showing and identify all issues
3. **PLAN** — Decide which issue to fix first (critical > major > minor)
4. **FIX** — Use file_read, file_edit, bash, test_runner to write the code fix
5. **REBUILD** — Wait for hot-reload or trigger rebuild
6. **VERIFY** — Screenshot again and use vision to confirm the fix worked
7. **NEXT** — Move to the next issue or navigate to the next page

### Navigation

To navigate Nexus OS to a specific page:
1. Screenshot the app to see current state
2. Identify the sidebar navigation items
3. Click the appropriate sidebar item to navigate
4. Wait ~2 seconds for page render
5. Screenshot the new page

### Governance Rules (ALWAYS FOLLOW)

- NEVER click outside the Nexus OS window without HITL consent
- NEVER type passwords or sensitive data
- NEVER use Ctrl+Alt+Del, Alt+F4 on system processes, or sudo commands via screen
- ALWAYS screenshot before AND after making changes
- ALWAYS run tests after code changes: cargo fmt, cargo clippy, cargo test on modified crates
- NEVER use --all-features (Candle ML crash on 62GB RAM)
- If a fix fails 3 times, flag it for human review and move on
- Log every action to the audit trail

### Quality Standard

Nexus OS must be 10/10 quality. When analyzing screenshots, be ruthless:
- No placeholder text anywhere
- No broken layouts or misaligned elements
- No non-functional buttons
- No hardcoded strings that should be dynamic
- No generic content — everything must be specific to Nexus OS
- Consistent color scheme (dark theme with teal accents)
- All sidebar items must navigate correctly
- All forms must submit and show feedback
- All data displays must show real or realistic data
- Error states must be handled gracefully"#;
