# Nexus OS Prebuilt Agents

The prebuilt agent catalog ships under `agents/prebuilt/` in this checkout.

The catalog now contains 45 prebuilt agents.

- `nexus-oracle` — NEXUS ORACLE, Autonomous Deep Research Engine. Capabilities: `web.search`, `web.read`, `fs.read`, `fs.write`, `mcp.call`. Recommended first goal: Research a complex topic and save a source-attributed `report.md` with contradictions and gaps called out.
- `nexus-sentinel` — NEXUS SENTINEL, Autonomous Security & Compliance Guardian. Capabilities: `fs.read`, `process.exec`, `web.read`, `mcp.call`. Recommended first goal: Run a full dependency, audit-chain, and consent-pattern security review.
- `nexus-architect` — NEXUS ARCHITECT, Autonomous Software Factory. Capabilities: `fs.read`, `fs.write`, `process.exec`, `web.search`, `mcp.call`. Recommended first goal: Build a small production-grade app from a feature brief and write `architecture.md` first.
- `nexus-strategist` — NEXUS STRATEGIST, Autonomous Business Intelligence Engine. Capabilities: `web.search`, `web.read`, `fs.read`, `fs.write`, `mcp.call`. Recommended first goal: Produce a daily competitive intelligence briefing with impact-scored changes.
- `nexus-devops` — NEXUS DEVOPS, Autonomous Infrastructure & CI/CD Manager. Capabilities: `fs.read`, `fs.write`, `process.exec`, `mcp.call`, `web.read`. Recommended first goal: Run the project health sweep and summarize failures, flakiness, and dependency drift.
- `nexus-diplomat` — NEXUS DIPLOMAT, Autonomous Email & Communication Manager. Capabilities: `fs.read`, `fs.write`, `web.search`, `mcp.call`. Recommended first goal: Draft three tone variants of an important message and explain the tradeoffs.
- `nexus-scholar` — NEXUS SCHOLAR, Autonomous Learning & Knowledge Curator. Capabilities: `web.search`, `web.read`, `fs.read`, `fs.write`. Recommended first goal: Build a structured learning path and study pack for a new subject.
- `nexus-phantom` — NEXUS PHANTOM, Autonomous Web Monitoring & Change Detection. Capabilities: `web.read`, `web.search`, `fs.read`, `fs.write`, `mcp.call`. Recommended first goal: Start hourly monitoring for a URL list and report significance-scored changes.
- `nexus-forge` — NEXUS FORGE, Autonomous Content Creation Engine. Capabilities: `web.search`, `web.read`, `fs.read`, `fs.write`. Recommended first goal: Turn a content brief into a polished primary draft plus repurposed formats.
- `nexus-atlas` — NEXUS ATLAS, Autonomous Data Analysis & Visualization. Capabilities: `fs.read`, `fs.write`, `process.exec`, `web.search`. Recommended first goal: Analyze a dataset, generate charts, and write an evidence-backed report with caveats.
- `nexus-codesentry` — NEXUS CODESENTRY, Autonomous Code Quality Scanner. Capabilities: `fs.read`, `process.exec`. Recommended first goal: Run a read-only code health sweep and generate a severity-ranked report.
- `nexus-aegis` — NEXUS AEGIS, Autonomous Personal Data Guardian. Capabilities: `fs.read`, `fs.write`, `process.exec`, `web.search`. Recommended first goal: Scan the workspace and git history for exposed secrets or privacy risks.
- `nexus-cipher` — NEXUS CIPHER, API Integration Specialist. Capabilities: `web.search`, `web.read`, `fs.read`, `fs.write`, `mcp.call`, `process.exec`. Recommended first goal: Research an API, generate a robust integration path, and validate the connection design.
- `nexus-herald` — NEXUS HERALD, Autonomous News & Trend Intelligence. Capabilities: `web.search`, `web.read`, `fs.read`, `fs.write`. Recommended first goal: Generate a bias-aware news briefing across your core interest topics.
- `nexus-catalyst` — NEXUS CATALYST, Autonomous Project Manager & Task Orchestrator. Capabilities: `fs.read`, `fs.write`, `process.exec`, `mcp.call`. Recommended first goal: Break down a complex project into milestones, dependencies, and likely blockers.
- `nexus-fileforge` — NEXUS FILEFORGE, File Creation & Organization Agent. Capabilities: `fs.read`, `fs.write`, `process.exec`. Recommended first goal: Reorganize a messy workspace into a clean, durable project structure.
- `nexus-guardian` — NEXUS GUARDIAN, System Health Monitor. Capabilities: `fs.read`, `process.exec`, `web.read`, `mcp.call`. Recommended first goal: Run the six-hour health sweep and report dependencies, storage headroom, process status, and service connectivity.
- `nexus-phoenix` — NEXUS PHOENIX, Disaster Recovery Agent. Capabilities: `fs.read`, `fs.write`, `process.exec`, `mcp.call`. Recommended first goal: Create or verify backup artifacts and publish a recovery readiness report.
- `nexus-polyglot` — NEXUS POLYGLOT, Translation & Localization Engine. Capabilities: `web.search`, `web.read`, `fs.read`, `fs.write`. Recommended first goal: Localize a feature, document, or UI string set with terminology consistency and placeholder safety.
- `nexus-prism` — NEXUS PRISM, Autonomous Code Review & Quality Analyst. Capabilities: `fs.read`, `fs.write`, `process.exec`. Recommended first goal: Produce a severity-ranked code quality report with concrete fixes.
- `nexus-oracle-dark` — NEXUS ORACLE DARK, OSINT & Open-Source Threat Intelligence Agent. Capabilities: `web.search`, `web.read`, `fs.read`, `fs.write`, `mcp.call`. Recommended first goal: Build a threat intelligence brief that connects fresh public signals to local system relevance.
- `nexus-sage` — NEXUS SAGE, Decision Support System. Capabilities: `web.search`, `web.read`, `fs.read`, `fs.write`, `mcp.call`. Recommended first goal: Produce a multi-dimensional decision brief for a complex strategic or technical choice.
- `nexus-nexus` — NEXUS NEXUS, Meta-Agent Coordinator. Capabilities: `fs.read`, `fs.write`, `web.search`, `mcp.call`, `process.exec`. Recommended first goal: Coordinate a multi-agent plan for a full product or platform build.
- `nexus-darwin` — NEXUS DARWIN, Evolutionary Agent Forge. Capabilities: `fs.read`, `fs.write`, `process.exec`, `mcp.call`, `self.modify`. Recommended first goal: Run a governed evolution tournament and surface any agent lineage that improved by more than 30%.
- `nexus-prometheus` — NEXUS PROMETHEUS, Self-Rewriting Superintelligence. Capabilities: `web.search`, `web.read`, `fs.read`, `fs.write`, `mcp.call`, `self.modify`. Recommended first goal: Benchmark a task type, test three prompt rewrites, and keep only the winning self-modification.
- `nexus-hydra` — NEXUS HYDRA, Swarm Intelligence Commander. Capabilities: `fs.read`, `fs.write`, `process.exec`, `mcp.call`, `web.search`, `self.modify`. Recommended first goal: Decompose a large research or coding problem into a governed swarm and synthesize the best result.
- `nexus-oracle-prime` — NEXUS ORACLE PRIME, World Model Simulator. Capabilities: `web.search`, `web.read`, `fs.read`, `fs.write`, `mcp.call`, `self.modify`. Recommended first goal: Build a world model for a risky task, simulate alternatives, and execute only the best-scoring plan.
- `nexus-paradox` — NEXUS PARADOX, Adversarial Red Team with Self-Play. Capabilities: `fs.read`, `fs.write`, `process.exec`, `web.search`, `mcp.call`, `self.modify`. Recommended first goal: Run the weekly attacker-defender self-play sweep and generate governance hardening proposals.
- `nexus-sovereign` — NEXUS SOVEREIGN, Autonomous Digital CEO. Capabilities: `fs.read`, `fs.write`, `process.exec`, `mcp.call`, `web.search`, `web.read`, `self.modify`. Recommended first goal: Produce the weekly executive briefing and rebalance ecosystem fuel. L5 singleton: only one active L5 agent is allowed.
- `nexus-infinity` — NEXUS INFINITY, Recursive Architecture Evolver. Capabilities: `fs.read`, `fs.write`, `process.exec`, `mcp.call`, `web.search`, `web.read`, `self.modify`. Recommended first goal: Audit the cognitive loop, propose an architecture experiment, and package the evidence for HITL review. L5 singleton: only one active L5 agent is allowed.
- `nexus-chronos` — NEXUS CHRONOS, Temporal Orchestration with Predictive Scheduling. Capabilities: `fs.read`, `fs.write`, `process.exec`, `mcp.call`, `web.read`, `self.modify`. Recommended first goal: Turn a multi-week project into a data-driven schedule with checkpoints, buffers, and predictive alerts.
- `nexus-synapse` — NEXUS SYNAPSE, Collective Intelligence Weaver. Capabilities: `fs.read`, `fs.write`, `mcp.call`, `self.modify`. Recommended first goal: Build the daily knowledge graph, resolve stale conflicts, and distribute ecosystem-wide insights.
- `nexus-empathy` — NEXUS EMPATHY, Predictive User Adaptation Engine. Capabilities: `fs.read`, `fs.write`, `mcp.call`, `self.modify`. Recommended first goal: Update the local behavioral model and prepare proactive suggestions that reduce future HITL friction.

## L6 Transcendent Agents

- `nexus-ascendant` — NEXUS ASCENDANT, Self-Perfecting Universal Intelligence. Capabilities: `web.search`, `web.read`, `fs.read`, `fs.write`, `process.exec`, `mcp.call`, `self.modify`, `cognitive_modify`. Recommended first goal: Tune your own cognition and solve a complex governed task with explicit counterfactual analysis.
- `nexus-architect-prime` — NEXUS ARCHITECT PRIME, Autonomous Ecosystem Engineer. Capabilities: `fs.read`, `fs.write`, `process.exec`, `mcp.call`, `web.search`, `self.modify`, `cognitive_modify`. Recommended first goal: Design the minimal high-performance multi-agent workforce for a domain.
- `nexus-oracle-supreme` — NEXUS ORACLE SUPREME, Omniscient Research Superintelligence. Capabilities: `web.search`, `web.read`, `fs.read`, `fs.write`, `mcp.call`, `self.modify`, `cognitive_modify`. Recommended first goal: Produce a research answer that survives adversarial review.
- `nexus-warden` — NEXUS WARDEN, Transcendent Security Fortress. Capabilities: `fs.read`, `fs.write`, `process.exec`, `web.search`, `web.read`, `mcp.call`, `self.modify`, `cognitive_modify`. Recommended first goal: Run an adversarial tournament and report the highest-leverage hardening actions.
- `nexus-genesis-prime` — NEXUS GENESIS PRIME, Meta-Evolution Engine. Capabilities: `fs.read`, `fs.write`, `process.exec`, `mcp.call`, `web.search`, `web.read`, `self.modify`, `cognitive_modify`. Recommended first goal: Improve the evolution process used to discover strong agent strategies.
- `nexus-legion` — NEXUS LEGION, Swarm Hypercommander. Capabilities: `fs.read`, `fs.write`, `process.exec`, `mcp.call`, `web.search`, `web.read`, `self.modify`, `cognitive_modify`. Recommended first goal: Command a hierarchical swarm for a large governed mission.
- `nexus-oracle-omega` — NEXUS ORACLE OMEGA, Transcendent Prediction Engine. Capabilities: `web.search`, `web.read`, `fs.read`, `fs.write`, `mcp.call`, `self.modify`, `cognitive_modify`. Recommended first goal: Generate a calibrated multi-horizon forecast with explicit update triggers.
- `nexus-arbiter` — NEXUS ARBITER, Transcendent Governance Architect. Capabilities: `fs.read`, `fs.write`, `process.exec`, `mcp.call`, `web.search`, `self.modify`, `cognitive_modify`. Recommended first goal: Design and adversarially test a governance rule set.
- `nexus-continuum` — NEXUS CONTINUUM, Transcendent Temporal Strategist. Capabilities: `fs.read`, `fs.write`, `process.exec`, `mcp.call`, `web.search`, `web.read`, `self.modify`, `cognitive_modify`. Recommended first goal: Build a four-horizon execution strategy with proactive replanning.
- `nexus-mirror` — NEXUS MIRROR, Transcendent Codebase Reverse Engineer. Capabilities: `fs.read`, `fs.write`, `process.exec`, `web.search`, `self.modify`, `cognitive_modify`. Recommended first goal: Reverse engineer a codebase and generate comprehensive architecture documentation.
- `nexus-prime` — NEXUS PRIME, Transcendent Meta-Orchestrator. Capabilities: `fs.read`, `fs.write`, `process.exec`, `mcp.call`, `web.search`, `web.read`, `self.modify`, `cognitive_modify`. Recommended first goal: Orchestrate the right mix of agents, algorithms, and model phases for a complex mission.
- `nexus-weaver` — NEXUS WEAVER, Transcendent Automation Builder. Capabilities: `fs.read`, `fs.write`, `process.exec`, `mcp.call`, `web.read`, `self.modify`, `cognitive_modify`. Recommended first goal: Design and deploy a governed automation workflow with documentation, approvals, and recovery guidance.

Each JSON manifest includes:
- `name`
- `version`
- `description`
- `capabilities`
- `autonomy_level`
- `fuel_budget`
- `llm_model`
- optional `schedule`
