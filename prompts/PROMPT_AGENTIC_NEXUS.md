# NEXUS OS AGENTIC SYSTEM — Implementation Plan
# From infrastructure to living autonomous agents
# 5 Phases, each one Claude Code session

---

## PHASE 1: THE AGENT LOOP (Session 1 — This makes agents ALIVE)

The core problem: Nexus OS has 53 agent DEFINITIONS but no agent RUNTIME LOOP.
OpenClaw works because it has one simple loop: receive → think → act → observe → repeat.
We need the same loop, but governed.

```
PROMPT FOR CLAUDE CODE:

Read CLAUDE.md for project context. Then implement the Agent Runtime Loop.

THE PROBLEM: Nexus OS agents are configs, not running processes. We need 
a runtime loop that makes agents autonomous — they wake up, think, act, 
observe results, and loop. Like OpenClaw but governed.

THE ARCHITECTURE:

┌─────────────────────────────────────────────────────┐
│                 AGENT RUNTIME LOOP                    │
│                                                       │
│  ┌──────┐   ┌───────┐   ┌──────┐   ┌─────────┐     │
│  │PERCEIVE│→│ REASON │→│ PLAN  │→│  ACT     │     │
│  │        │  │(LLM)  │  │       │  │(execute) │     │
│  └──────┘   └───────┘   └──────┘   └─────────┘     │
│      ↑                                    │          │
│      │         ┌──────────┐               │          │
│      └─────────│ OBSERVE  │←──────────────┘          │
│                │(results) │                           │
│                └──────────┘                           │
│                                                       │
│  GOVERNANCE LAYER (wraps every step):                 │
│  Capability Check → Fuel → Audit → HITL → Sandbox    │
└─────────────────────────────────────────────────────┘

STEP 1: Create crate nexus-agent-runtime (or modify nexus-cognitive)

The AgentLoop struct:

pub struct AgentLoop {
    agent_id: AgentId,
    agent_config: AgentConfig,
    llm_provider: Arc<dyn LlmProvider>,  // Flash Inference
    tool_registry: Arc<ToolRegistry>,     // available actions
    governance: Arc<GovernancePipeline>,
    memory: AgentMemory,                  // persistent across loops
    state: AgentState,                    // running/paused/stopped
}

impl AgentLoop {
    pub async fn run(&mut self) -> Result<()> {
        loop {
            // 1. PERCEIVE — gather inputs
            let perception = self.perceive().await?;
            
            // 2. REASON — ask LLM what to do
            let reasoning = self.reason(&perception).await?;
            
            // 3. PLAN — break into steps
            let plan = self.plan(&reasoning).await?;
            
            // 4. GOVERNANCE CHECK — validate plan
            let approved_plan = self.governance.validate(&plan).await?;
            
            // 5. ACT — execute each step
            for step in approved_plan.steps {
                let result = self.execute_step(&step).await?;
                
                // 6. OBSERVE — check result
                let observation = self.observe(&result).await?;
                
                // 7. LEARN — update memory
                self.memory.record(step, result, observation).await?;
            }
            
            // 8. REFLECT — did we achieve goal?
            let reflection = self.reflect().await?;
            
            if reflection.goal_achieved || reflection.should_pause {
                break;
            }
            
            // Governance: deduct fuel for this loop iteration
            self.governance.deduct_fuel(self.agent_id, 1).await?;
        }
        Ok(())
    }
}

STEP 2: The perceive() function gathers inputs from:
- Scheduled triggers (cron fired, time-based)
- External events (webhook received, file changed)  
- Messages from other agents (A2A inbox)
- User commands (from the UI)
- System state (CPU, memory, disk, network)

STEP 3: The reason() function calls Flash Inference:
- Builds a prompt with: agent persona + current goal + perception + memory
- Calls the local LLM via FlashProvider
- Parses the response into structured ActionPlan

Use this EXACT prompt template for the LLM:
"""
You are {agent_name}, a {agent_role} agent in Nexus OS.
Your current goal: {goal}

Current observations:
{perception}

Your memory of past actions:
{memory_summary}

Available tools:
{tool_list}

Based on the above, decide your next action. Respond in JSON:
{
  "thinking": "your reasoning",
  "action": "tool_name",
  "parameters": { ... },
  "expected_outcome": "what you expect to happen"
}
"""

STEP 4: The execute_step() function dispatches to tools:
- shell_execute → run a command (sandboxed)
- browser_navigate → open URL
- browser_click → click element
- file_read / file_write → filesystem access
- http_request → call an API
- send_message → communicate with user or other agent
- llm_query → ask the LLM a sub-question

Each tool call goes through the governance pipeline:
  capability_check → fuel_check → audit_log → [hitl_if_needed] → sandbox → execute

STEP 5: Wire Flash Inference as the default LLM provider:
- AgentLoop should accept any LlmProvider, but default to FlashProvider
- Use the "balanced" model (Qwen 35B at 5 tok/s) for agent reasoning
- Fall back to "fast" model (Gemma 2B) for simple classification tasks

STEP 6: Persist agent memory to SQLite:
- agent_memories table: (agent_id, timestamp, action, result, reflection)
- Load last N memories on startup for context
- Summarize old memories to prevent context overflow

STEP 7: Add a Tauri command to start/stop agents:
- agent_start(agent_id) → spawns AgentLoop in a tokio task
- agent_stop(agent_id) → signals the loop to stop
- agent_status(agent_id) → returns current state

STEP 8: Basic test — create a "System Monitor" agent that:
- Wakes up every 60 seconds
- Reads CPU, RAM, disk usage
- If any metric exceeds 90%, logs a warning
- Stores observations in memory
- After 5 observations, summarizes trends using the LLM

This is the simplest possible agent but proves the full loop works:
perceive(system metrics) → reason(LLM) → act(log warning) → observe(result)

DO NOT build all tools. Just build:
- shell_execute (sandboxed)
- file_read / file_write
- llm_query (Flash Inference)
- send_notification (to UI)

Test by starting the System Monitor agent and watching it run 5 loops.

cargo fmt && cargo clippy on modified crates -- -D warnings
```

---

## PHASE 2: THE TOOL SYSTEM (Session 2 — Real computer actions)

```
PROMPT FOR CLAUDE CODE:

Read CLAUDE.md. The Agent Runtime Loop from Phase 1 is working.
Now build the real tool system so agents can DO things.

TOOL 1: BROWSER AUTOMATION (test live)
Wire the computer-control crate for real browser automation.
The agent needs to:
- Open a URL in a browser
- Read page content (DOM text extraction)
- Click elements by CSS selector
- Fill form fields
- Take screenshots
- Wait for elements to appear

Use one of these approaches (check what's available):
a) headless chromium via chromiumoxide crate
b) WebDriver protocol to control Firefox/Chrome
c) Shell out to puppeteer/playwright via Node.js

Test: Agent opens https://news.ycombinator.com, reads top 5 story 
titles, and returns them as structured data.

TOOL 2: HTTP/API CLIENT
- Make HTTP requests (GET, POST, PUT, DELETE)
- Parse JSON responses
- Handle authentication headers
- Rate limiting per domain

Test: Agent calls GitHub API to list recent commits on nexus-os repo.

TOOL 3: FILE SYSTEM (sandboxed)
- Read files within allowed directories
- Write files within allowed directories  
- List directory contents
- Watch for file changes (inotify)

Sandbox: agents can ONLY access directories listed in their capability 
config. Attempting to access /etc/passwd should fail with CapabilityDenied.

TOOL 4: COMMUNICATION
- Send notification to user (Tauri event → frontend toast)
- Send message to another agent (A2A protocol)
- Post to webhook URL

TOOL 5: CODE EXECUTION (sandboxed)
- Execute Python/Rust/Node.js code in WASM sandbox
- Capture stdout/stderr
- Timeout after N seconds
- No network access from sandbox

All tools register in a ToolRegistry. The agent's LLM sees the list 
of available tools and their parameter schemas (like OpenAI function calling).

Test the full chain:
1. Start an agent
2. Agent uses browser tool to read HN front page
3. Agent uses LLM to summarize the stories
4. Agent uses file_write to save summary to ~/agent-output/hn-summary.md
5. Agent sends notification to user: "HN summary ready"

cargo fmt && cargo clippy on modified crates -- -D warnings
```

---

## PHASE 3: THE SCHEDULER + HEARTBEAT (Session 3 — Agents wake up on their own)

```
PROMPT FOR CLAUDE CODE:

Read CLAUDE.md. Agent Loop and Tools are working from Phase 1 & 2.
Now make agents wake up AUTONOMOUSLY — no human trigger needed.

OpenClaw calls this the "heartbeat." We call it the Nexus Scheduler.

STEP 1: Activate the existing scheduler crate (nexus-scheduler).
Wire it to actually spawn AgentLoop instances on triggers:

Trigger types:
- Cron: "every 5 minutes", "every hour", "daily at 9am"
- Event: "when file changes in ~/Documents"
- Webhook: "when POST arrives at /api/webhook/agent-name"
- Interval: "every 300 seconds"
- Startup: "run once when Nexus OS starts"

STEP 2: Create a scheduler_config.toml that users can edit:

[[agents]]
name = "system-monitor"
trigger = { type = "interval", seconds = 60 }
enabled = true
fuel_budget = 100
autonomy_level = "L3"

[[agents]]
name = "hn-reader"
trigger = { type = "cron", schedule = "0 9 * * *" }  # 9am daily
enabled = true
fuel_budget = 50
autonomy_level = "L4"

[[agents]]
name = "inbox-manager"
trigger = { type = "interval", seconds = 300 }  # every 5 min
enabled = true
fuel_budget = 200
autonomy_level = "L4"

STEP 3: On Nexus OS startup, load scheduler_config.toml and start all 
enabled agents. Each runs in its own tokio task.

STEP 4: Add Scheduler page in the frontend:
- Show running agents with status (active/idle/error)
- Toggle agents on/off
- See last run time, next run time
- View agent logs (last 20 actions)
- Add/edit agent schedules

STEP 5: Implement the "heartbeat" — proactive agent behavior:
- Between scheduled runs, agents can set "watchers"
- Watchers fire when conditions are met (file changed, new email, etc.)
- The agent then runs an unscheduled loop to handle the event

STEP 6: Test by scheduling 3 agents:
- System Monitor: every 60 seconds, checks system health
- HN Reader: every hour, reads top stories, saves summary
- File Watcher: watches ~/Documents, summarizes new files

Let them run for 10 minutes. Verify they execute autonomously,
governance pipeline logs every action, fuel is consumed correctly.

cargo fmt && cargo clippy on modified crates -- -D warnings
```

---

## PHASE 4: WEALTH GENERATION AGENT (Session 4 — The money maker)

```
PROMPT FOR CLAUDE CODE:

Read CLAUDE.md. Agent Loop, Tools, and Scheduler are working.
Now build the first agent that GENERATES REVENUE.

THE AGENT: Content Creator (L4 Autonomous)

Strategy: 
1. Agent monitors trending topics (HN, Reddit, Twitter/X)
2. Researches each topic using browser + LLM
3. Writes high-quality articles using Flash Inference (35B model)
4. Publishes to: 
   - A local blog (generated static site)
   - GitHub Pages (free hosting)
   - Medium (via API)
5. Includes affiliate links for products mentioned
6. Tracks which articles get traffic
7. Evolves strategy based on what performs best (Darwin Core)

IMPLEMENTATION:

Step 1: TrendScanner tool
- Reads HN, Reddit r/technology, r/programming front pages
- Extracts trending topics with engagement scores
- Uses LLM to classify: "Is this topic monetizable?"
- Returns top 3 opportunities

Step 2: ResearchEngine tool  
- For each trending topic, reads 5-10 related articles
- Extracts key facts, quotes, data points
- Builds a research document the LLM can reference

Step 3: ContentWriter tool
- Uses Flash Inference (35B model) to write articles
- Follows SEO best practices (keywords, headers, meta)
- Generates 1000-2000 word articles
- Includes affiliate links for relevant products
- Creates matching social media posts (Twitter/X, LinkedIn)

Step 4: Publisher tool
- Generates static HTML for the article
- Commits to a GitHub Pages repo
- Posts to Medium via API (if configured)
- Posts social media snippets

Step 5: Analytics tool
- Tracks page views (simple counter via GitHub API)
- Monitors which topics drive traffic
- Reports daily: articles published, estimated views, revenue

Step 6: Evolution (Darwin Core)
- After 7 days, evaluate which content strategy works best
- Generate new strategies based on successful patterns
- Adversarial arena tests new strategies before deployment

GOVERNANCE:
- Fuel budget: 500 per day (limits API calls and compute)
- HITL gate: Agent must get approval before publishing (first 7 days)
- After 7 days with good behavior, auto-approve publishing
- All articles logged in audit trail
- No financial transactions without explicit HITL approval

SCHEDULE:
- Trend scan: every 2 hours
- Article writing: twice daily (10am, 3pm)
- Publishing: after each article, with HITL approval
- Analytics: daily at 11pm
- Evolution: weekly on Sunday

TEST:
1. Agent scans HN for trends
2. Picks the best topic
3. Researches it (reads 5 articles)
4. Writes a high-quality article
5. Saves it to ~/agent-output/articles/
6. Sends notification: "New article ready for review"
7. User approves → agent publishes to GitHub Pages

This is NOT a demo. This is a real content business run by an agent.

cargo fmt && cargo clippy on modified crates -- -D warnings
```

---

## PHASE 5: MULTI-AGENT COLLABORATION (Session 5 — The agent economy)

```
PROMPT FOR CLAUDE CODE:

Read CLAUDE.md. Single agents are running autonomously. Now make 
them work TOGETHER as a team.

THE SYSTEM: Agent Team for Full Business Automation

TEAM STRUCTURE:
┌─────────────────────────────────────────────────┐
│              DIRECTOR AGENT (L6)                 │
│  Oversees strategy, allocates resources          │
│  Uses 397B model for strategic decisions         │
└────────────────────┬────────────────────────────┘
                     │
        ┌────────────┼────────────────┐
        │            │                │
   ┌────▼────┐  ┌───▼────┐   ┌──────▼──────┐
   │RESEARCHER│  │WRITER  │   │PUBLISHER    │
   │(L4)     │  │(L4)    │   │(L3)         │
   │Finds    │  │Creates │   │Posts content │
   │topics   │  │content │   │Tracks stats  │
   └─────────┘  └────────┘   └─────────────┘

COMMUNICATION:
- Director sends tasks to workers via A2A protocol
- Workers report results back to Director
- Director evaluates performance and adjusts strategy
- All communication logged in audit trail

WORKFLOW:
1. Director wakes up (daily at 8am)
2. Director asks Researcher: "Find today's opportunities"
3. Researcher scans trends, returns top 5 topics
4. Director selects best 2, assigns to Writer
5. Writer researches and writes 2 articles
6. Director reviews articles (uses 397B for quality check)
7. Director assigns approved articles to Publisher
8. Publisher posts to all channels
9. Director reviews daily analytics at 11pm
10. Director adjusts strategy for tomorrow

CONFLICT RESOLUTION:
- If Writer and Researcher disagree on a topic, Director decides
- If any agent exceeds fuel budget, Director reallocates from others
- If any agent fails 3 times, Director escalates to user (HITL)

IMPLEMENTATION:
1. Director agent config with team_members list
2. A2A task assignment: director.assign_task(worker, task)
3. A2A result reporting: worker.report_result(director, result)
4. Shared team memory: director can read all workers' memories
5. Team dashboard in frontend showing all agents and their status

TEST:
Run the full team for one cycle:
- Director assigns research task
- Researcher returns topics
- Director assigns writing task  
- Writer produces article
- Director approves and assigns publishing
- Publisher saves to ~/agent-output/
- Director sends summary notification to user

This proves multi-agent collaboration works end-to-end.

cargo fmt && cargo clippy on modified crates -- -D warnings
```

---

## EXECUTION ORDER

| Phase | Session | What it proves | Time estimate |
|-------|---------|---------------|---------------|
| 1 | Next session | Agents can think and act autonomously | 3-4 hours |
| 2 | Session after | Agents can use browser, files, APIs | 3-4 hours |
| 3 | Session after | Agents wake up on their own | 2-3 hours |
| 4 | Session after | Agents generate revenue | 3-4 hours |
| 5 | Session after | Agents work as a team | 3-4 hours |

After Phase 5, Nexus OS is a REAL autonomous agent system — not infrastructure, 
not configs, not demos. Real agents doing real work, governed and audited.

Better than OpenClaw because every action is governed.
Better than LangGraph because it runs locally with 671B intelligence.
Better than CrewAI because it has computer control and financial safety.
Better than all of them because agents EVOLVE via Darwin Core.

The user says "Here is $1000. Generate $5000."
The Director agent plans the strategy.
The team executes.
Every dollar tracked.
Every decision auditable.
That's Nexus OS.
