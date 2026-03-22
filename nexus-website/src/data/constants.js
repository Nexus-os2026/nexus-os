// Real data fetched from GitLab and GitHub APIs on 2026-03-21
// These serve as fallback values when APIs are unavailable

export const GITLAB_PROJECT = {
  id: 80071116,
  name: 'nexus-os',
  description: 'What if your OS was built for AI agents, not humans? Nexus OS: a governed AI agent operating system with 53 agents, 397 commands, 47 self-evolving genomes, and 12 Gen-3 systems. Rust kernel + Tauri 2.0, WASM-sandboxed execution, DID/Ed25519 cryptographic identity, hash-chained audit trails, and human-in-the-loop governance. 100% local-first. Air-gappable. No cloud. No data leaves your machine.',
  web_url: 'https://gitlab.com/nexaiceo/nexus-os',
  star_count: 0,
  forks_count: 0,
  last_activity_at: '2026-03-21T01:34:50.008Z',
  default_branch: 'main',
  created_at: '2026-03-07T23:24:59.151Z',
  topics: ['Agent-os', 'Anti-cloud', 'Rust Lang', 'ai-agents', 'desktop-app', 'llm', 'local-first', 'rust', 'security', 'tauri'],
};

export const GITLAB_LANGUAGES = {
  Rust: 70.57,
  TSX: 17.37,
  CSS: 6.04,
  Python: 3.02,
  TypeScript: 1.40,
};

export const COMMIT_COUNT = 263;

export const STATS = {
  rustLines: '224K',
  tests: '3,521',
  agents: 53,
  commands: 397,
  nimModels: 93,
  panics: 0,
  genomes: 47,
  gen3Systems: 12,
  desktopPages: 50,
  crates: 33,
};

export const VERSION = 'v9.3.0';

export const COMPETITORS = {
  langgraph: {
    name: 'LangGraph',
    repo: 'langchain-ai/langgraph',
    stars: 27052,
    license: 'MIT',
    language: 'Python',
    description: 'Build resilient language agents as graphs.',
    forks: 4657,
  },
  crewai: {
    name: 'CrewAI',
    repo: 'crewAIInc/crewAI',
    stars: 46745,
    license: 'MIT',
    language: 'Python',
    description: 'Framework for orchestrating role-playing, autonomous AI agents.',
    forks: 6319,
  },
  autogen: {
    name: 'AutoGen',
    repo: 'microsoft/autogen',
    stars: 55959,
    license: 'CC-BY-4.0',
    language: 'Python',
    description: 'A programming framework for agentic AI.',
    forks: 8426,
  },
  openai_agents: {
    name: 'OpenAI Agents',
    repo: 'openai/openai-agents-python',
    stars: 20171,
    license: 'MIT',
    language: 'Python',
    description: 'A lightweight, powerful framework for multi-agent workflows.',
    forks: 3304,
  },
};

export const AUTONOMY_LEVELS = [
  { level: 'L0', name: 'Inert', description: 'Agent is disabled. No actions permitted. Safe storage state.', color: '#4a5568' },
  { level: 'L1', name: 'Suggest', description: 'Agent can analyze and suggest actions. Human decides everything. Read-only access to data.', color: '#8892a4' },
  { level: 'L2', name: 'Act with Approval', description: 'Agent proposes actions, human approves each one. HITL gate required for all mutations.', color: '#00f5ff' },
  { level: 'L3', name: 'Act then Report', description: 'Agent executes within bounded scope, reports afterward. Post-action review with rollback capability.', color: '#7c3aed' },
  { level: 'L4', name: 'Autonomous Bounded', description: 'Full autonomy within strict capability boundaries. Anomaly detection triggers automatic pause.', color: '#00ff88' },
  { level: 'L5', name: 'Full Autonomy', description: 'Unrestricted within declared capabilities. Only kernel override can intervene. Reserved for proven agents.', color: '#ff6b2b' },
  { level: 'L6', name: 'Transcendent', description: 'Self-evolving agent with Darwin Core integration. Can modify own genome. Highest governance scrutiny.', color: '#ff0055' },
];

export const GOVERNANCE_PIPELINE = [
  { step: 1, name: 'Capability Check', description: 'Verify agent holds required capability tokens. Cryptographically signed, scoped permissions checked against TOML manifest.' },
  { step: 2, name: 'Fuel Reserve', description: 'Check fuel budget before execution. Each operation has a cost table entry. Insufficient fuel blocks the action.' },
  { step: 3, name: 'Adversarial Arena', description: 'Darwin Core adversarial testing. Red-team agents challenge the proposed action for safety and correctness.' },
  { step: 4, name: 'HITL Gate', description: 'Human-in-the-loop approval for Tier1+ operations. tokio::sync::Notify-based consent flow with configurable timeout.' },
  { step: 5, name: 'WASM Sandbox', description: 'Execute in wasmtime isolation. Memory-limited, capability-restricted sandbox. ~5ms creation time.' },
  { step: 6, name: 'Output Firewall', description: 'Scan agent output for policy violations, injection attempts, and harmful content before delivery.' },
  { step: 7, name: 'PII Redaction', description: 'Automated detection and redaction at LLM gateway boundary. Regex + Luhn + NER pattern matching.' },
  { step: 8, name: 'Audit Trail', description: 'Append to hash-chained, append-only SQLite log. SHA-256 integrity chain. Tamper-evident by design.' },
  { step: 9, name: 'Fuel Commit', description: 'Deduct actual fuel cost from agent budget. Record consumption metrics. Update governance dashboard.' },
];

export const FEATURES_DEEP = [
  {
    id: 'governance',
    title: 'Governance Kernel',
    subtitle: 'Capability ACL, Fuel Metering, Audit Trails, HITL Gates',
    description: 'The kernel is the root of all trust. Every agent action passes through capability-based access control with cryptographically signed permission tokens. Fuel metering ensures no agent can consume unbounded resources. Hash-chained audit trails provide tamper-evident logging. Human-in-the-loop gates enforce approval for sensitive operations.',
    stats: [
      { label: 'Kernel Modules', value: '26' },
      { label: 'Capability Check', value: '<1ms' },
      { label: 'Audit Write', value: '~2ms' },
    ],
    code: `// Every action goes through the governance pipeline
let permit = kernel.check_capability(&agent_id, &cap)?;
kernel.reserve_fuel(&agent_id, cost)?;
let result = kernel.execute_sandboxed(action).await?;
kernel.audit_trail.append_event(event)?;
kernel.commit_fuel(&agent_id, actual_cost)?;`,
  },
  {
    id: 'darwin',
    title: 'Darwin Core',
    subtitle: 'Adversarial Arena, Swarm Coordination, Evolution Engine',
    description: 'Agents evolve through Darwinian selection. The Adversarial Arena pits agent variants against each other. The Swarm Coordinator orchestrates multi-agent collaboration. The Plan Evolution Engine breeds better strategies through genetic crossover, mutation, and fitness-based selection across 47 genomes.',
    stats: [
      { label: 'Genomes', value: '47' },
      { label: 'Fitness Proven', value: '35/40 to 39/40' },
      { label: 'Generations', value: 'Multi-gen breeding' },
    ],
    code: `// Darwin Core evolves agent populations
let arena = AdversarialArena::new(config);
let results = arena.compete(agent_variants).await?;
let evolved = evolution_engine.breed(
    &results.top_performers,
    CrossoverStrategy::Uniform,
    MutationRate::Adaptive(0.05),
)?;`,
  },
  {
    id: 'wasm',
    title: 'WASM Sandbox',
    subtitle: 'Wasmtime Isolation with Memory Limits',
    description: 'Every agent executes inside a WASM sandbox powered by wasmtime. Memory-limited, capability-restricted, with ~5ms creation time. Agents cannot access the host filesystem, network, or other agents without explicit capability grants. Sandbox escape is a reported vulnerability class.',
    stats: [
      { label: 'Sandbox Creation', value: '~5ms' },
      { label: 'Isolation', value: 'Full memory' },
      { label: 'Engine', value: 'Wasmtime' },
    ],
    code: `// WASM sandbox with strict capability limits
let sandbox = WasmSandbox::new(WasmConfig {
    max_memory: 64 * 1024 * 1024, // 64MB
    max_fuel: 1_000_000,
    capabilities: agent.manifest.capabilities(),
    timeout: Duration::from_secs(30),
})?;
let output = sandbox.execute(wasm_module).await?;`,
  },
  {
    id: 'flash',
    title: 'Flash Inference',
    subtitle: 'Local GGUF Model Inference with Full Governance',
    description: 'Run LLM inference locally via llama.cpp FFI. Load GGUF models directly on your machine. Every inference call goes through the full governance pipeline: capability check, fuel reserve, output firewall, PII redaction, and audit trail. Zero data leaves your device.',
    stats: [
      { label: 'NIM Models', value: '93' },
      { label: 'LLM Providers', value: '6' },
      { label: 'Data Leaked', value: '0 bytes' },
    ],
    code: `// Flash inference with full governance
let model = FlashEngine::load("mistral-7b.gguf")?;
let response = kernel.governed_inference(
    &agent_id,
    InferenceRequest {
        model: model.id(),
        prompt: redacted_prompt,
        max_tokens: 2048,
    },
).await?; // Audit trail recorded automatically`,
  },
  {
    id: 'mcp',
    title: 'MCP + A2A Protocols',
    subtitle: 'Governed Inter-Agent Communication',
    description: 'Model Context Protocol and Agent-to-Agent communication with full governance overlay. Every message between agents passes through capability checks and audit logging. Agents can only communicate with explicitly authorized peers. Protocol-level encryption for all inter-agent traffic.',
    stats: [
      { label: 'Protocol', value: 'MCP + A2A' },
      { label: 'Encryption', value: 'Ed25519' },
      { label: 'Auth', value: 'Mutual capability' },
    ],
    code: `// Governed agent-to-agent communication
let channel = conductor.open_channel(
    &sender_id, &receiver_id,
    ChannelPolicy::Encrypted,
)?;
channel.send(Message::new(payload))
    .with_capability(Cap::AgentComm)
    .audit_logged()
    .await?;`,
  },
  {
    id: 'scheduler',
    title: 'Background Scheduler',
    subtitle: 'Cron, Webhook, Event, Interval, One-Shot Triggers',
    description: 'Schedule agent tasks with five trigger types: cron expressions, webhook endpoints, event listeners, interval timers, and one-shot delayed execution. All scheduled tasks inherit the scheduling agent\'s governance scope. Failed tasks are automatically retried with exponential backoff.',
    stats: [
      { label: 'Trigger Types', value: '5' },
      { label: 'Retry', value: 'Exponential backoff' },
      { label: 'Governance', value: 'Inherited scope' },
    ],
    code: `// Background scheduler with governance
scheduler.register(Task {
    trigger: Trigger::Cron("0 */6 * * *"),
    agent_id: agent.id(),
    action: Action::HealthCheck,
    governance: GovernanceScope::Inherited,
    retry: RetryPolicy::ExponentialBackoff {
        max_retries: 3,
        base_delay: Duration::from_secs(60),
    },
})?;`,
  },
  {
    id: 'ghost',
    title: 'Ghost Protocol',
    subtitle: 'P2P Sync, Ed25519 Identity, Distributed Mesh',
    description: 'Sovereign identity with Ed25519 key pairs bound to hardware. Zero-knowledge proofs for agent authentication without revealing identity details. Distributed mesh networking enables multi-machine consciousness with agent migration and shared knowledge graphs.',
    stats: [
      { label: 'Identity', value: 'DID/Ed25519' },
      { label: 'Auth', value: 'ZK Proofs' },
      { label: 'Network', value: 'P2P Mesh' },
    ],
    code: `// Sovereign identity with ZK authentication
let identity = SovereignIdentity::generate(
    KeyType::Ed25519,
    HardwareBinding::OsKeyring,
)?;
let proof = identity.create_zk_proof(
    &challenge,
    ProofScope::AgentAuthentication,
)?;
mesh.announce(identity.did(), proof).await?;`,
  },
  {
    id: 'connectors',
    title: '9 Integration Adapters',
    subtitle: 'Slack, Teams, Discord, GitHub, Jira, and More',
    description: 'Pre-built connectors for enterprise communication and DevOps platforms. Each connector operates under the governance pipeline: capability checks, audit logging, and PII redaction apply to all outbound messages. Webhook support for real-time event ingestion.',
    stats: [
      { label: 'Connectors', value: '9' },
      { label: 'Governance', value: 'Full pipeline' },
      { label: 'Direction', value: 'Bidirectional' },
    ],
    code: `// Governed Slack connector
let slack = SlackConnector::new(config)?;
slack.send_message(
    &channel,
    message.pii_redacted(), // Auto-redacted
).with_capability(Cap::SlackWrite)
 .audit_logged()
 .await?;`,
  },
  {
    id: 'enterprise',
    title: 'Enterprise Auth',
    subtitle: 'OIDC, Multi-Tenancy, Metering',
    description: 'Enterprise-grade authentication with OIDC/SSO integration. Multi-tenant architecture with complete data isolation between tenants. Per-tenant metering tracks resource consumption. Six RBAC roles from viewer to super-admin. AES-256-GCM encryption at rest.',
    stats: [
      { label: 'RBAC Roles', value: '6' },
      { label: 'Encryption', value: 'AES-256-GCM' },
      { label: 'Auth', value: 'OIDC/SSO' },
    ],
    code: `// Enterprise multi-tenant setup
let tenant = TenantManager::create(TenantConfig {
    name: "acme-corp",
    auth: AuthProvider::OIDC(oidc_config),
    encryption: EncryptionAt::Rest(Aes256Gcm),
    isolation: DataIsolation::Complete,
    metering: MeteringPolicy::PerAgent,
})?;`,
  },
  {
    id: 'persistence',
    title: 'Persistent Governance',
    subtitle: 'SQLite WAL, Hash-Chain Audit, Crash Recovery',
    description: 'All governance state persists to SQLite with Write-Ahead Logging for crash recovery. The audit trail uses SHA-256 hash chaining: each entry references the previous hash, making tampering detectable. Append-only design ensures no entry can be modified or deleted.',
    stats: [
      { label: 'Storage', value: 'SQLite WAL' },
      { label: 'Hash', value: 'SHA-256 chain' },
      { label: 'Recovery', value: 'Crash-safe' },
    ],
    code: `// Hash-chained audit trail
let prev_hash = audit_trail.latest_hash()?;
let entry = AuditEntry {
    timestamp: Utc::now(),
    agent_id: agent.id(),
    action: action.description(),
    result: outcome,
    prev_hash, // Tamper-evident chain
    hash: sha256(&[prev_hash, &payload]),
};
audit_trail.append(entry)?; // Append-only`,
  },
];

export const ARCHITECTURE_LAYERS = [
  { name: 'Presentation', description: 'React + TypeScript, 50 desktop pages, 397 Tauri commands', color: '#00f5ff', crates: ['app (React/TypeScript)'], tech: 'React 18, Tauri 2.0, TypeScript' },
  { name: 'Orchestration', description: 'Nexus Conductor, DAG scheduling, A2A/MCP protocols', color: '#00d4ff', crates: ['nexus-conductor', 'nexus-mcp', 'nexus-a2a'], tech: 'DAG Scheduler, Protocol Handlers' },
  { name: 'Agent Layer', description: '53 agents with TOML manifests, autonomy L0-L6', color: '#3b82f6', crates: ['nexus-agents (9 crates)', 'nexus-sdk'], tech: 'Agent Runtime, SDK' },
  { name: 'Evolution', description: 'Darwin Core: adversarial arena, swarm, genome breeding', color: '#7c3aed', crates: ['nexus-darwin', 'nexus-genome'], tech: 'Genetic Algorithms, Arena' },
  { name: 'Governance Kernel', description: 'Capability ACL, fuel, audit trail, HITL, PII redaction', color: '#9333ea', crates: ['nexus-kernel (26 modules)'], tech: 'Core Runtime, SQLite WAL' },
  { name: 'Sandbox', description: 'WASM isolation via wasmtime, ~5ms creation', color: '#a855f7', crates: ['nexus-sandbox'], tech: 'Wasmtime, Memory Limits' },
  { name: 'Infrastructure', description: 'Flash inference, Ghost Protocol, connectors, persistence', color: '#c084fc', crates: ['nexus-flash', 'nexus-ghost', 'nexus-connectors'], tech: 'llama.cpp, Ed25519, SQLite' },
  { name: 'LLM Providers', description: '6 providers, 93 NIM models, 5 routing strategies', color: '#e879f9', crates: ['nexus-llm-router', 'nexus-nim'], tech: 'Ollama, NVIDIA NIM, OpenAI, Anthropic' },
];

export const ROADMAP_DATA = [
  {
    version: 'v1.0 - v3.0',
    title: 'Foundation to Hardened Platform',
    date: 'Early 2026',
    status: 'completed',
    items: [
      'Kernel foundation with hash-chained audit',
      'Governance primitives: capability ACL, fuel model, HITL',
      'Autonomy Levels L0-L5, HITL Tier0-Tier3',
      'PII redaction, hardware security stubs',
      'Desktop app via Tauri 2.0',
    ],
  },
  {
    version: 'v4.0',
    title: 'Governed Distributed Agent Platform',
    date: 'March 2026',
    status: 'completed',
    items: [
      'Cross-node replication and quorum execution',
      'Federated audit trails',
      'Agent marketplace with Ed25519 verification',
      'Plugin SDK and enterprise RBAC (6 roles)',
      'Multi-agent collaboration and delegation',
    ],
  },
  {
    version: 'v5.0',
    title: 'Production Ready',
    date: 'March 2026',
    status: 'completed',
    items: [
      'WASM sandbox, TCP transport, 24 CLI commands',
      'Desktop UI: Command Center, Audit Timeline, Marketplace',
      'Full documentation suite',
      'End-to-end integration tests',
    ],
  },
  {
    version: 'v7.0',
    title: 'The Complete Operating System',
    date: 'March 2026',
    status: 'completed',
    items: [
      '15 built-in governed applications',
      'Code Editor, Design Studio, Terminal, File Manager',
      'Database Manager, API Client, Notes, Email',
      'Project Manager, Media Studio, System Monitor',
      '33 desktop pages, 1,175 tests',
    ],
  },
  {
    version: 'v9.0',
    title: 'Gen-3 Living OS',
    date: 'March 17, 2026',
    status: 'completed',
    items: [
      '12 Gen-3 systems: Agent DNA Genome, Genesis Protocol',
      'Consciousness Kernel, Dream Forge, Temporal Engine',
      'Immune System, Cognitive Filesystem, Agent Civilization',
      'Sovereign Identity, Distributed Mesh, Self-Rewriting Kernel',
      '2,997+ Rust tests, 47 agent genomes',
    ],
  },
  {
    version: 'v9.3.0',
    title: 'Current Release',
    date: 'March 21, 2026',
    status: 'current',
    items: [
      'All 5 audit gaps closed: persistence, scheduler, connectors',
      'HITL UI and Darwin Core governance complete',
      'Flash Inference engine for local GGUF models',
      'Background agent scheduler with 5 trigger types',
      'Clippy clean, zero warnings',
    ],
  },
  {
    version: 'v9.x',
    title: 'Enterprise Foundation',
    date: 'March-April 2026',
    status: 'in_progress',
    items: [
      'SSO/OIDC authentication integration',
      'OpenTelemetry observability',
      'Docker and Helm deployment',
      'AES-256-GCM encryption at rest',
      'Bug fixes and stability improvements',
    ],
  },
  {
    version: 'v10.0',
    title: 'Enterprise Scale',
    date: 'May-June 2026',
    status: 'planned',
    items: [
      'Horizontal scaling and high availability',
      'Admin console and fleet management',
      'Enterprise integrations: Slack, Teams, Jira, ServiceNow',
      'SOC 2 Type II and ISO 27001 certification',
      'HIPAA compliance framework',
    ],
  },
  {
    version: 'v11.0',
    title: 'Developer Ecosystem',
    date: 'Q3 2026',
    status: 'planned',
    items: [
      'Agent SDK for Rust, Python, TypeScript',
      'Genome marketplace for community-bred agents',
      'Mobile companion app',
      'Visual agent builder',
    ],
  },
  {
    version: 'v12.0',
    title: 'Enterprise Premium',
    date: 'Q4 2026',
    status: 'planned',
    items: [
      'SLA guarantees and premium support',
      'FedRAMP and FIPS 140-2 certification',
      'Governance policy marketplace',
      'Industry-specific vertical packages',
    ],
  },
  {
    version: '2027+',
    title: 'Long-Term Vision',
    date: '2027 and beyond',
    status: 'planned',
    items: [
      'Nexus OS Cloud managed service',
      'Agent App Store',
      'Industry verticals (healthcare, finance, defense)',
      'Standards body participation',
    ],
  },
];

export const CHANGELOG_DATA = [
  {
    version: 'v9.3.0',
    date: 'March 21, 2026',
    latest: true,
    sections: {
      Added: [
        'Flash Inference engine: local GGUF model inference with full governance pipeline',
        'Background agent scheduler with cron, webhook, event, interval, one-shot triggers',
      ],
      Fixed: [
        'All 5 audit gaps closed: persistence, scheduler, connectors, HITL UI, Darwin Core',
        'Clippy needless_borrows in webhook HMAC and main.rs db path',
        'Graceful skip for Ollama-dependent tests when model unavailable',
      ],
    },
  },
  {
    version: 'v9.0.0',
    date: 'March 17, 2026',
    sections: {
      Added: [
        'Agent DNA Genome: genetic breeding, crossover, mutation, 47 genomes',
        'Genesis Protocol: agents create agents, gap detection, multi-generation',
        'Consciousness Kernel: agent internal states (confidence, fatigue, curiosity)',
        'Dream Forge: overnight autonomous work, replay, experiment, consolidate',
        'Temporal Engine: parallel timeline forking, checkpoint rollback',
        'Immune System: threat detection, antibody spawning, adversarial arena',
        'Cognitive Filesystem: semantic understanding, knowledge graph, NL queries',
        'Agent Civilization: parliament, economy, elections, dispute resolution',
        'Sovereign Identity: Ed25519 keys, ZK proofs, hardware-bound identity',
        'Distributed Mesh: multi-machine consciousness, agent migration',
        'Self-Rewriting Kernel: performance profiling, LLM-generated patches',
        'Computer Omniscience: screen understanding, intent prediction',
      ],
      Changed: [
        'Chat pipeline: end-to-end streaming with Ollama + NVIDIA NIM',
        '47/47 agents verified via smoke test',
        'Agent self-improvement proven (35/40 to 39/40)',
      ],
      Fixed: [
        'Output firewall false positives',
        'Startup crash (async agent loading)',
        'Vite dev server configuration',
      ],
    },
  },
  {
    version: 'v8.1.0',
    date: 'March 16, 2026',
    sections: {
      Fixed: [
        'Comprehensive audit: all pages wired, all systems verified, zero dead code',
        '10 missing agents added (total 45)',
      ],
    },
  },
  {
    version: 'v8.0.0',
    date: 'March 16, 2026',
    sections: {
      Added: [
        'Cognitive loop and HITL approval flow fully working',
        'Docker support',
        'README refresh with 45 agents',
        'Production ready status',
      ],
    },
  },
  {
    version: 'v7.0.0',
    date: 'March 2026',
    sections: {
      Added: [
        '15 built-in governed applications',
        'Code Editor: Monaco, 50+ languages, Git, agent-assisted coding',
        'Design Studio: AI canvas, 29 components, React/HTML preview',
        'Terminal: 30+ commands, 18 blocked patterns, HITL',
        'File Manager: grid/list, drag-drop, encrypted vault',
        'Database Manager: SQLite/PostgreSQL/MySQL, visual query builder',
        'API Client: 7 HTTP methods, collections, governed key vault',
        'Notes App: markdown, folders, tags, agent auto-creates',
        'Email Client: IMAP/SMTP, threading, agent drafts with HITL',
        'Project Manager: Kanban, sprints, agent-estimated complexity',
        'Media Studio: image editor, AI generation, OCR',
        'System Monitor: CPU/RAM/GPU/disk/network, per-agent breakdown',
        'App Store: Ed25519 verification, ratings, developer portal',
        'AI Chat Hub: 9 models, side-by-side comparison, voice chat',
        'Deploy Pipeline: Vercel/Netlify/Cloudflare, rollback, HITL',
        'Learning Center: 7 courses, 6 challenges, XP leveling',
      ],
    },
  },
  {
    version: 'v5.0.0',
    date: 'March 2026',
    sections: {
      Added: [
        'WASM-ready sandbox, TCP transport, 24 CLI commands',
        'Desktop UI: Command Center, Audit Timeline, Marketplace',
        'Full documentation: Architecture, SDK, Deployment, Security',
        'E2E integration tests',
      ],
    },
  },
  {
    version: 'v4.0.0',
    date: 'March 2026',
    sections: {
      Added: [
        'Criterion benchmarks, replay evidence bundles, circuit breaker',
        'Cross-node replication, quorum execution, federated audit',
        'Plugin SDK, enterprise RBAC (6 roles), SOC2 compliance',
        'Multi-agent collaboration, capability delegation, adaptive governance',
        '90 test suites, zero failures',
      ],
    },
  },
];

export const SECURITY_LAYERS = [
  { layer: 7, name: 'Output Firewall', description: 'Scan outputs for policy violations, injection attempts, and harmful content.' },
  { layer: 6, name: 'PII Redaction', description: 'Automated detection and redaction. Regex + Luhn + NER at LLM gateway boundary.' },
  { layer: 5, name: 'HITL Consent Gates', description: 'Human approval for Tier1+ operations. Configurable timeout and escalation.' },
  { layer: 4, name: 'Fuel Metering', description: 'Cost table per operation. Budget checked before execution, committed after.' },
  { layer: 3, name: 'Capability ACL', description: 'Cryptographically signed permission tokens. Scoped to specific agent and action.' },
  { layer: 2, name: 'WASM Sandbox', description: 'Wasmtime isolation. Memory-limited, capability-restricted execution environment.' },
  { layer: 1, name: 'Agent Identity', description: 'DID/Ed25519 keypairs. Hardware-bound identity with zero-knowledge proofs.' },
  { layer: 0, name: 'Audit Trail', description: 'SHA-256 hash-chained, append-only SQLite log. Tamper-evident by design.' },
];

export const AGENT_CATEGORIES = [
  {
    name: 'Cognitive',
    agents: [
      { name: 'planner-agent', level: 'L3', description: 'Strategic planning and task decomposition' },
      { name: 'reasoner-agent', level: 'L3', description: 'Logical reasoning and inference chains' },
      { name: 'memory-agent', level: 'L2', description: 'Long-term knowledge retrieval and consolidation' },
      { name: 'meta-cognitive-agent', level: 'L4', description: 'Self-monitoring and strategy adjustment' },
      { name: 'dream-agent', level: 'L4', description: 'Overnight consolidation and precomputation' },
      { name: 'temporal-agent', level: 'L3', description: 'Timeline forking and checkpoint management' },
      { name: 'consciousness-agent', level: 'L4', description: 'Internal state management and empathic interface' },
    ],
  },
  {
    name: 'Creative',
    agents: [
      { name: 'designer-agent', level: 'L2', description: 'UI/UX design and visual generation' },
      { name: 'writer-agent', level: 'L2', description: 'Content creation and copywriting' },
      { name: 'web-builder-agent', level: 'L3', description: 'Full-stack web application generation' },
      { name: 'media-agent', level: 'L2', description: 'Image editing, OCR, and media processing' },
      { name: 'screen-poster-agent', level: 'L2', description: 'Visual content and social media assets' },
    ],
  },
  {
    name: 'Technical',
    agents: [
      { name: 'coder-agent', level: 'L3', description: 'Code generation, review, and refactoring' },
      { name: 'devops-agent', level: 'L3', description: 'CI/CD pipeline management and deployment' },
      { name: 'self-improve-agent', level: 'L6', description: 'Self-evolving code improvement (Darwin Core)' },
      { name: 'debugger-agent', level: 'L3', description: 'Automated bug detection and fix proposals' },
      { name: 'architect-agent', level: 'L2', description: 'System architecture and design patterns' },
      { name: 'database-agent', level: 'L3', description: 'Schema design, query optimization, migration' },
      { name: 'api-agent', level: 'L3', description: 'API design, testing, and documentation' },
      { name: 'terminal-agent', level: 'L3', description: 'Governed shell command execution' },
      { name: 'deploy-agent', level: 'L3', description: 'Production deployment with HITL approval' },
      { name: 'kernel-rewrite-agent', level: 'L6', description: 'Self-rewriting kernel patches with rollback' },
    ],
  },
  {
    name: 'Security',
    agents: [
      { name: 'audit-agent', level: 'L2', description: 'Compliance auditing and policy verification' },
      { name: 'immune-agent', level: 'L4', description: 'Threat detection and antibody spawning' },
      { name: 'privacy-agent', level: 'L2', description: 'PII detection, redaction, and privacy scanning' },
      { name: 'firewall-agent', level: 'L4', description: 'Output filtering and injection prevention' },
      { name: 'identity-agent', level: 'L3', description: 'Ed25519 key management and ZK proofs' },
      { name: 'compliance-agent', level: 'L2', description: 'EU AI Act and SOC2 conformity checks' },
    ],
  },
  {
    name: 'Communication',
    agents: [
      { name: 'slack-agent', level: 'L2', description: 'Slack workspace integration and messaging' },
      { name: 'teams-agent', level: 'L2', description: 'Microsoft Teams connector' },
      { name: 'discord-agent', level: 'L2', description: 'Discord server management and bot' },
      { name: 'email-agent', level: 'L3', description: 'IMAP/SMTP governed email with HITL' },
      { name: 'telegram-agent', level: 'L2', description: 'Telegram bot and remote control' },
      { name: 'github-agent', level: 'L3', description: 'GitHub/GitLab issue and PR management' },
      { name: 'jira-agent', level: 'L2', description: 'Jira ticket management and sprint planning' },
      { name: 'webhook-agent', level: 'L3', description: 'Inbound/outbound webhook processing' },
    ],
  },
  {
    name: 'Specialized',
    agents: [
      { name: 'research-agent', level: 'L3', description: 'Deep research and information synthesis' },
      { name: 'data-agent', level: 'L3', description: 'Data analysis and transformation pipelines' },
      { name: 'workflow-agent', level: 'L3', description: 'Multi-step workflow orchestration' },
      { name: 'scheduler-agent', level: 'L3', description: 'Task scheduling and cron management' },
      { name: 'marketplace-agent', level: 'L2', description: 'Agent marketplace browsing and installation' },
      { name: 'learning-agent', level: 'L3', description: 'Adaptive learning and course generation' },
      { name: 'genesis-agent', level: 'L5', description: 'Agent creation and gap detection' },
      { name: 'omniscience-agent', level: 'L4', description: 'Screen understanding and intent prediction' },
      { name: 'civilization-agent', level: 'L5', description: 'Parliament, economy, and governance DAO' },
      { name: 'cognitive-fs-agent', level: 'L3', description: 'Semantic file understanding and NL queries' },
    ],
  },
];

export const COMPARISON_CAPABILITIES = [
  { name: 'Open Source', nexus: true, langgraph: true, crewai: true, autogen: true, openai: true },
  { name: 'License', nexus: 'MIT', langgraph: 'MIT', crewai: 'MIT', autogen: 'CC-BY-4.0', openai: 'MIT' },
  { name: 'Primary Language', nexus: 'Rust', langgraph: 'Python', crewai: 'Python', autogen: 'Python', openai: 'Python' },
  { name: 'Local-First / Offline', nexus: true, langgraph: false, crewai: false, autogen: false, openai: false },
  { name: 'Air-Gappable', nexus: true, langgraph: false, crewai: false, autogen: false, openai: false },
  { name: 'Desktop Native App', nexus: true, langgraph: false, crewai: false, autogen: false, openai: false },
  { name: 'Governance Kernel', nexus: true, langgraph: false, crewai: false, autogen: false, openai: false },
  { name: 'HITL Approval Gates', nexus: true, langgraph: true, crewai: true, autogen: true, openai: true },
  { name: 'Hash-Chained Audit', nexus: true, langgraph: false, crewai: false, autogen: false, openai: false },
  { name: 'Cryptographic Agent Identity', nexus: true, langgraph: false, crewai: false, autogen: false, openai: false },
  { name: 'Self-Evolving Agents', nexus: true, langgraph: false, crewai: false, autogen: false, openai: false },
  { name: 'WASM Sandbox', nexus: true, langgraph: false, crewai: false, autogen: false, openai: false },
  { name: 'Fuel Metering', nexus: true, langgraph: false, crewai: false, autogen: false, openai: false },
  { name: 'Multi-LLM Support', nexus: true, langgraph: true, crewai: true, autogen: true, openai: false },
  { name: 'Enterprise Auth (OIDC)', nexus: true, langgraph: false, crewai: true, autogen: false, openai: false },
  { name: 'EU AI Act Conformity', nexus: true, langgraph: false, crewai: false, autogen: false, openai: false },
];

export const EU_AI_ACT_ARTICLES = [
  { article: 'Article 9', title: 'Risk Management', status: 'Implemented', description: 'Continuous risk assessment through governance kernel, adversarial arena testing, and fuel metering.' },
  { article: 'Article 10', title: 'Data Governance', status: 'Implemented', description: 'PII redaction at gateway, cognitive filesystem with semantic understanding, privacy scanning.' },
  { article: 'Article 11', title: 'Technical Documentation', status: 'Implemented', description: '477-line architecture document, security policy, API reference, SDK documentation.' },
  { article: 'Article 12', title: 'Record-Keeping', status: 'Implemented', description: 'Hash-chained audit trails with SHA-256 integrity. Append-only, tamper-evident design.' },
  { article: 'Article 13', title: 'Transparency', status: 'Implemented', description: 'Open source MIT license. Full source available. Agent manifests declare all capabilities.' },
  { article: 'Article 14', title: 'Human Oversight', status: 'Implemented', description: 'HITL gates for Tier1+ operations. Autonomy levels L0-L6 with configurable approval flows.' },
  { article: 'Article 15', title: 'Accuracy & Robustness', status: 'Implemented', description: '3,521 tests, Darwin Core adversarial testing, immune system threat detection.' },
];

export const SOC2_CONTROLS = [
  { id: 'CC1-CC2', title: 'Control Environment & Communication', status: 'Implemented' },
  { id: 'CC3', title: 'Risk Assessment', status: 'Implemented' },
  { id: 'CC4-CC5', title: 'Monitoring & Control Activities', status: 'Implemented' },
  { id: 'CC6', title: 'Logical & Physical Access', status: 'Implemented' },
  { id: 'CC7', title: 'System Operations', status: 'Implemented' },
  { id: 'CC8', title: 'Change Management', status: 'Implemented' },
  { id: 'CC9', title: 'Risk Mitigation', status: 'In Progress' },
  { id: 'A1', title: 'Availability', status: 'In Progress' },
  { id: 'PI1', title: 'Processing Integrity', status: 'Implemented' },
  { id: 'C1', title: 'Confidentiality', status: 'Implemented' },
  { id: 'P1-P8', title: 'Privacy', status: 'Implemented' },
];
