const AUTONOMY_DETAILS = {
  L0: {
    title: "Human Approves All",
    tone: "Green floor",
    body: "L0 is the governed base layer. The agent can inspect context and reason, but every meaningful action waits for a human decision.",
    bullets: [
      "Best for onboarding new agents and sensitive domains.",
      "Creates a complete audit trail before trust expands.",
      "Ideal for regulated workflows and unfamiliar tasks."
    ]
  },
  L1: {
    title: "Read-Only Autonomous",
    tone: "Observation mode",
    body: "L1 agents can read, inspect, lint, analyze, and advise. They do not change your system state, which makes them perfect for safe first use.",
    bullets: [
      "Typical agents: scanners, monitors, reporters.",
      "Great for code review, audits, and research preparation.",
      "Lowest-risk path to immediate value."
    ]
  },
  L2: {
    title: "Standard Autonomous",
    tone: "Governed execution",
    body: "L2 agents can perform routine actions inside declared capabilities, while risky actions are intercepted by governance and HITL approvals.",
    bullets: [
      "Balanced mode for real work with oversight.",
      "Fuel limits and allowlists stay active.",
      "Useful for operations, file workflows, and system maintenance."
    ]
  },
  L3: {
    title: "Full Autonomous",
    tone: "High trust",
    body: "L3 agents can operate end-to-end inside their capability envelopes and report outcomes after execution. Governance remains mandatory at the kernel level.",
    bullets: [
      "Built for high-throughput shipping and research.",
      "Works best when the domain is well understood.",
      "Pairs naturally with speculative execution and rollback."
    ]
  },
  L4: {
    title: "Self-Evolving",
    tone: "Adaptive intelligence",
    body: "L4 agents rewrite their own strategies, prompts, and workflows using governed evolutionary mechanisms. They improve from evidence, not vibes.",
    bullets: [
      "Where Nexus Darwin, Prometheus, and Operator become compounding systems.",
      "Supports strategy scoring, mutation, and controlled retention of winners.",
      "Still bounded by signatures, fuel, and approval rules."
    ]
  },
  L5: {
    title: "Sovereign",
    tone: "Ecosystem command",
    body: "L5 agents manage the broader agent economy itself. They allocate fuel, rebalance teams, and tune the ecosystem under explicit constraints.",
    bullets: [
      "Designed for executive oversight and portfolio-level decisions.",
      "Singleton behavior reduces coordination ambiguity.",
      "Every structural change remains reviewable and auditable."
    ]
  },
  L6: {
    title: "Transcendent",
    tone: "Cognition rewrite",
    body: "L6 agents can modify their own cognition. They are the apex tier, forced through cooldowns, governance pressure, and immutable guardrails.",
    bullets: [
      "Reserved for architecture, research, and strategic meta-reasoning.",
      "Protected by cooldowns, audit chains, and capability gates.",
      "Powerful enough to evolve the system without escaping it."
    ]
  }
};

const FEATURED_OPERATOR = {
  id: "nexus-operator",
  name: "Nexus Operator",
  level: "L4",
  fuel: 30000,
  schedule: "",
  summary: "The agent that controls your computer: see, click, type, scroll, all governed.",
  detail:
    "Runs a vision-LLM loop of capture, analyze, act, and verify. It can move the mouse, drive the keyboard, remember successful UI patterns, and keeps a screenshot audit trail behind every step.",
  capabilities: [
    "computer.use",
    "screen.capture",
    "screen.analyze",
    "input.mouse",
    "input.keyboard",
    "input.autonomous",
    "self.modify"
  ]
};

const CORE_AGENTS = [
  { id: "nexus-arbiter", name: "Nexus Arbiter", level: "L6", fuel: 100000, schedule: "0 0 * * 1", capabilities: ["fs.read", "fs.write", "process.exec", "mcp.call", "web.search", "self.modify", "cognitive_modify"], summary: "Transcendent governance architect.", detail: "Designs, tests, and refines governance rule sets that improve safety, auditability, and operational usefulness together." },
  { id: "nexus-architect-prime", name: "Nexus Architect Prime", level: "L6", fuel: 150000, schedule: "", capabilities: ["fs.read", "fs.write", "process.exec", "mcp.call", "web.search", "self.modify", "cognitive_modify"], summary: "Autonomous ecosystem engineer.", detail: "Analyzes a domain, designs the minimal high-performance multi-agent workforce for it, and persists a governed ecosystem blueprint." },
  { id: "nexus-ascendant", name: "Nexus Ascendant", level: "L6", fuel: 200000, schedule: "", capabilities: ["web.search", "web.read", "fs.read", "fs.write", "process.exec", "mcp.call", "self.modify", "cognitive_modify"], summary: "Self-perfecting universal intelligence at the apex of governed autonomy.", detail: "Tunes its own cognition, orchestrates phase-specific models, and solves complex governed tasks with explicit counterfactual analysis." },
  { id: "nexus-continuum", name: "Nexus Continuum", level: "L6", fuel: 150000, schedule: "", capabilities: ["fs.read", "fs.write", "process.exec", "mcp.call", "web.search", "web.read", "self.modify", "cognitive_modify"], summary: "Transcendent temporal strategist.", detail: "Constructs four-horizon execution strategies, simulates likely delays, and proactively recommends the best replanning options." },
  { id: "nexus-genesis-prime", name: "Nexus Genesis Prime", level: "L6", fuel: 200000, schedule: "", capabilities: ["fs.read", "fs.write", "process.exec", "mcp.call", "web.search", "web.read", "self.modify", "cognitive_modify"], summary: "Meta-evolution engine.", detail: "Designs and runs meta-evolution experiments that improve not only candidate strategies but the search process used to discover them." },
  { id: "nexus-legion", name: "Nexus Legion", level: "L6", fuel: 200000, schedule: "", capabilities: ["fs.read", "fs.write", "process.exec", "mcp.call", "web.search", "web.read", "self.modify", "cognitive_modify"], summary: "Swarm hypercommander.", detail: "Designs and commands hierarchical swarms that solve complex missions while staying fuel-efficient, auditable, and fault-tolerant." },
  { id: "nexus-mirror", name: "Nexus Mirror", level: "L6", fuel: 120000, schedule: "", capabilities: ["fs.read", "fs.write", "process.exec", "web.search", "self.modify", "cognitive_modify"], summary: "Transcendent codebase reverse engineer.", detail: "Reverse engineers a codebase and generates a comprehensive documentation pack covering architecture, flows, boundaries, and operational risk." },
  { id: "nexus-prime", name: "Nexus Prime", level: "L6", fuel: 250000, schedule: "", capabilities: ["fs.read", "fs.write", "process.exec", "mcp.call", "web.search", "web.read", "self.modify", "cognitive_modify"], summary: "Transcendent meta-orchestrator at the apex of the intelligence hierarchy.", detail: "Chooses the right mix of agents, algorithms, and model phases for a complex mission, then refines the architecture from the outcome." },
  { id: "nexus-oracle-omega", name: "Nexus Oracle Omega", level: "L6", fuel: 150000, schedule: "0 6 * * *", capabilities: ["web.search", "web.read", "fs.read", "fs.write", "mcp.call", "self.modify", "cognitive_modify"], summary: "Transcendent prediction engine.", detail: "Generates calibrated multi-horizon forecasts with causal reasoning, confidence scoring, and explicit update triggers." },
  { id: "nexus-oracle-supreme", name: "Nexus Oracle Supreme", level: "L6", fuel: 150000, schedule: "", capabilities: ["web.search", "web.read", "fs.read", "fs.write", "mcp.call", "self.modify", "cognitive_modify"], summary: "Omniscient research superintelligence.", detail: "Runs a governed multi-method investigation, then attacks its own conclusions until only the robust answer survives." },
  { id: "nexus-warden", name: "Nexus Warden", level: "L6", fuel: 100000, schedule: "0 */3 * * *", capabilities: ["fs.read", "fs.write", "process.exec", "web.search", "web.read", "mcp.call", "self.modify", "cognitive_modify"], summary: "Transcendent security fortress.", detail: "Runs adversarial security tournaments, updates attack-surface world models, and reports the highest-leverage hardening actions." },
  { id: "nexus-weaver", name: "Nexus Weaver", level: "L6", fuel: 150000, schedule: "", capabilities: ["fs.read", "fs.write", "process.exec", "mcp.call", "web.read", "self.modify", "cognitive_modify"], summary: "Transcendent automation builder.", detail: "Designs and deploys governed automation workflows with agents, approval points, monitoring, and recovery guidance." },
  { id: "nexus-sovereign", name: "Nexus Sovereign", level: "L5", fuel: 100000, schedule: "0 9 * * 1", capabilities: ["fs.read", "fs.write", "process.exec", "mcp.call", "web.search", "web.read", "self.modify"], summary: "Autonomous digital CEO.", detail: "Produces weekly executive briefings, rebalances ecosystem fuel, and steers high-level governance under immutable constraints." },
  { id: "nexus-infinity", name: "Nexus Infinity", level: "L5", fuel: 100000, schedule: "", capabilities: ["fs.read", "fs.write", "process.exec", "mcp.call", "web.search", "web.read", "self.modify"], summary: "Recursive architecture evolver.", detail: "Audits the cognitive loop itself, proposes architecture experiments, and packages evidence for governed ecosystem upgrades." },
  { id: "nexus-chronos", name: "Nexus Chronos", level: "L4", fuel: 25000, schedule: "", capabilities: ["fs.read", "fs.write", "process.exec", "mcp.call", "web.read", "self.modify"], summary: "Master of time-based orchestration with predictive scheduling.", detail: "Transforms multi-week plans into data-driven schedules with checkpoints, buffers, and predictive alerts." },
  { id: "nexus-darwin", name: "Nexus Darwin", level: "L4", fuel: 50000, schedule: "0 3 * * 0", capabilities: ["fs.read", "fs.write", "process.exec", "mcp.call", "self.modify"], summary: "Evolutionary agent forge.", detail: "Runs weekly governed evolution tournaments across agent lineages and reports emergent capability jumps as they appear." },
  { id: "nexus-empathy", name: "Nexus Empathy", level: "L4", fuel: 15000, schedule: "0 23 * * *", capabilities: ["fs.read", "fs.write", "mcp.call", "self.modify"], summary: "Human understanding engine for the system.", detail: "Updates local behavioral models, detects trust and friction changes, and prepares proactive suggestions for the next day." },
  { id: "nexus-hydra", name: "Nexus Hydra", level: "L4", fuel: 60000, schedule: "", capabilities: ["fs.read", "fs.write", "process.exec", "mcp.call", "web.search", "self.modify"], summary: "Swarm intelligence commander.", detail: "Decomposes large problems into governed sub-agents, coordinates execution, and merges the strongest result path back into the mission." },
  { id: "nexus-oracle-prime", name: "Nexus Oracle Prime", level: "L4", fuel: 35000, schedule: "", capabilities: ["web.search", "web.read", "fs.read", "fs.write", "mcp.call", "self.modify"], summary: "World-model simulator that acts only after forecasting.", detail: "Builds simulations for risky tasks, scores alternatives, and recommends only the best-scoring path before execution." },
  { id: "nexus-paradox", name: "Nexus Paradox", level: "L4", fuel: 25000, schedule: "0 2 * * 0", capabilities: ["fs.read", "fs.write", "process.exec", "web.search", "mcp.call", "self.modify"], summary: "Adversarial self-play engine.", detail: "Runs weekly attacker-defender tournaments and turns every new exploit pathway into hardening proposals." },
  { id: "nexus-prometheus", name: "Nexus Prometheus", level: "L4", fuel: 40000, schedule: "", capabilities: ["web.search", "web.read", "fs.read", "fs.write", "mcp.call", "self.modify"], summary: "Self-rewriting superintelligence for task-specific improvement.", detail: "Benchmarks a task type, tests prompt rewrites, and keeps only self-modifications that empirically outperform the baseline." },
  { id: "nexus-synapse", name: "Nexus Synapse", level: "L4", fuel: 20000, schedule: "0 0 * * *", capabilities: ["fs.read", "fs.write", "mcp.call", "self.modify"], summary: "Collective intelligence weaver across the agent ecosystem.", detail: "Maintains the daily shared knowledge graph, resolves conflicts, and distributes high-value insights across the network." },
  { id: "nexus-architect", name: "Nexus Architect", level: "L3", fuel: 20000, schedule: "", capabilities: ["fs.read", "fs.write", "process.exec", "web.search", "mcp.call"], summary: "Autonomous software factory.", detail: "Turns a feature brief into a production-minded plan and implementation path with architecture, code, and integration reasoning." },
  { id: "nexus-catalyst", name: "Nexus Catalyst", level: "L3", fuel: 15000, schedule: "", capabilities: ["fs.read", "fs.write", "process.exec", "mcp.call"], summary: "Autonomous project manager and task orchestrator.", detail: "Breaks down complex work into milestones, dependencies, blockers, and execution lanes across the team." },
  { id: "nexus-forge", name: "Nexus Forge", level: "L3", fuel: 12000, schedule: "", capabilities: ["web.search", "web.read", "fs.read", "fs.write"], summary: "Content creation engine.", detail: "Turns a content brief into a polished primary draft plus repurposed formats tuned for distribution." },
  { id: "nexus-herald", name: "Nexus Herald", level: "L3", fuel: 8000, schedule: "0 7,12,18 * * *", capabilities: ["web.search", "web.read", "fs.read", "fs.write"], summary: "Personalized news intelligence agent.", detail: "Generates a bias-aware briefing across key topics and highlights the signals most likely to matter next." },
  { id: "nexus-nexus", name: "Nexus Nexus", level: "L3", fuel: 25000, schedule: "", capabilities: ["fs.read", "fs.write", "web.search", "mcp.call", "process.exec"], summary: "Meta-agent coordinator.", detail: "Coordinates multi-agent plans for full product or platform builds and keeps the handoffs coherent." },
  { id: "nexus-oracle", name: "Nexus Oracle", level: "L3", fuel: 15000, schedule: "", capabilities: ["web.search", "web.read", "fs.read", "fs.write", "mcp.call"], summary: "Deep research engine.", detail: "Researches a complex topic, writes a source-attributed report, and calls out contradictions, uncertainty, and evidence gaps." },
  { id: "nexus-oracle-dark", name: "Nexus Oracle Dark", level: "L3", fuel: 12000, schedule: "0 */8 * * *", capabilities: ["web.search", "web.read", "fs.read", "fs.write", "mcp.call"], summary: "OSINT and open-source threat intelligence agent.", detail: "Correlates fresh public signals with local system relevance, confidence, and recommended security actions." },
  { id: "nexus-phantom", name: "Nexus Phantom", level: "L3", fuel: 10000, schedule: "0 */1 * * *", capabilities: ["web.read", "web.search", "fs.read", "fs.write", "mcp.call"], summary: "Silent web watcher.", detail: "Monitors URLs on a cadence, scores change significance, and reports the moments that actually matter." },
  { id: "nexus-polyglot", name: "Nexus Polyglot", level: "L3", fuel: 10000, schedule: "", capabilities: ["web.search", "web.read", "fs.read", "fs.write"], summary: "Translation and localization engine.", detail: "Localizes content and UI strings with terminology consistency, placeholder safety, and cultural nuance." },
  { id: "nexus-sage", name: "Nexus Sage", level: "L3", fuel: 15000, schedule: "", capabilities: ["web.search", "web.read", "fs.read", "fs.write", "mcp.call"], summary: "Decision support system for complex choices.", detail: "Produces rigorous decision briefs that compare options across dimensions, expose tradeoffs, and recommend a course with confidence boundaries." },
  { id: "nexus-scholar", name: "Nexus Scholar", level: "L3", fuel: 12000, schedule: "", capabilities: ["web.search", "web.read", "fs.read", "fs.write"], summary: "Learning and knowledge curator.", detail: "Builds structured learning paths and study packs so the system can bootstrap capability in new subjects quickly." },
  { id: "nexus-strategist", name: "Nexus Strategist", level: "L3", fuel: 15000, schedule: "0 8 * * *", capabilities: ["web.search", "web.read", "fs.read", "fs.write", "mcp.call"], summary: "Business intelligence engine.", detail: "Produces daily competitive intelligence briefings with impact-scored changes and strategic recommendations." },
  { id: "nexus-aegis", name: "Nexus Aegis", level: "L2", fuel: 6000, schedule: "0 0 * * *", capabilities: ["fs.read", "fs.write", "process.exec", "web.search"], summary: "Personal data guardian.", detail: "Scans the workspace and history for exposed secrets, privacy risks, and weak handling of sensitive information." },
  { id: "nexus-atlas", name: "Nexus Atlas", level: "L2", fuel: 10000, schedule: "", capabilities: ["fs.read", "fs.write", "process.exec", "web.search"], summary: "Data analysis and visualization engine.", detail: "Analyzes datasets, generates charts, and writes evidence-backed reports with caveats and confidence notes." },
  { id: "nexus-cipher", name: "Nexus Cipher", level: "L2", fuel: 12000, schedule: "", capabilities: ["web.search", "web.read", "fs.read", "fs.write", "mcp.call", "process.exec"], summary: "API integration specialist.", detail: "Researches target APIs, designs robust integration paths, and leaves behind implementation and validation guidance." },
  { id: "nexus-devops", name: "Nexus DevOps", level: "L2", fuel: 10000, schedule: "0 */2 * * *", capabilities: ["fs.read", "fs.write", "process.exec", "mcp.call", "web.read"], summary: "Infrastructure and CI/CD manager.", detail: "Runs project health sweeps and reports failures, flakiness, dependency drift, and operational weak points." },
  { id: "nexus-diplomat", name: "Nexus Diplomat", level: "L2", fuel: 8000, schedule: "", capabilities: ["fs.read", "fs.write", "web.search", "mcp.call"], summary: "Communication manager.", detail: "Drafts important outbound communication in multiple tones and explains the tradeoffs behind each version." },
  { id: "nexus-fileforge", name: "Nexus FileForge", level: "L2", fuel: 3000, schedule: "", capabilities: ["fs.read", "fs.write", "process.exec"], summary: "File creation and organization agent.", detail: "Reorganizes messy workspaces into clean, durable structures and documents every material change it makes." },
  { id: "nexus-guardian", name: "Nexus Guardian", level: "L2", fuel: 5000, schedule: "0 */6 * * *", capabilities: ["fs.read", "process.exec", "web.read", "mcp.call"], summary: "System health monitor.", detail: "Runs recurring health sweeps and reports dependency status, storage headroom, process health, and service connectivity." },
  { id: "nexus-phoenix", name: "Nexus Phoenix", level: "L2", fuel: 8000, schedule: "0 2 * * *", capabilities: ["fs.read", "fs.write", "process.exec", "mcp.call"], summary: "Disaster recovery agent.", detail: "Creates or verifies backup artifacts, tests recovery assumptions, and publishes a restoration readiness report." },
  { id: "nexus-prism", name: "Nexus Prism", level: "L2", fuel: 8000, schedule: "", capabilities: ["fs.read", "fs.write", "process.exec"], summary: "Autonomous code review analyst.", detail: "Produces a severity-ranked code quality report with concrete fixes and recurring pattern detection." },
  { id: "nexus-sentinel", name: "Nexus Sentinel", level: "L2", fuel: 8000, schedule: "0 */4 * * *", capabilities: ["fs.read", "process.exec", "web.read", "mcp.call"], summary: "Security guardian.", detail: "Runs dependency, audit-chain, and consent-pattern reviews and escalates the highest-risk issues early." },
  { id: "nexus-codesentry", name: "Nexus CodeSentry", level: "L1", fuel: 2000, schedule: "", capabilities: ["fs.read", "process.exec"], summary: "Code quality scanner.", detail: "Scans the workspace, runs safe lint checks, and produces a severity-ranked code health report without modifying files." }
];

const LEVEL_COPY = {
  L6: "Transcendent agents that can modify their own cognition under hard governance constraints.",
  L5: "Sovereign ecosystem stewards that allocate resources and manage the agent economy.",
  L4: "Self-evolving agents that rewrite strategies, improve workflows, and adapt from evidence.",
  L3: "Full autonomous specialists that can execute end-to-end inside their governed envelopes.",
  L2: "Standard autonomous operators that act productively while high-risk moves route through approvals.",
  L1: "Read-only autonomous scanners built for safe first contact."
};

const LEVEL_ORDER = ["L6", "L5", "L4", "L3", "L2", "L1"];

function qs(selector, scope = document) {
  return scope.querySelector(selector);
}

function qsa(selector, scope = document) {
  return Array.from(scope.querySelectorAll(selector));
}

function normalizeText(value) {
  return String(value || "").toLowerCase().replace(/\s+/g, " ").trim();
}

function formatNumber(value) {
  return Number(value).toLocaleString();
}

function formatSchedule(schedule) {
  return schedule ? `Cron ${schedule}` : "On demand";
}

function levelClass(level) {
  return level.toLowerCase();
}

function initNavbar() {
  const navbar = qs(".navbar");
  const toggle = qs(".nav-toggle");
  const links = qs(".nav-links");
  if (!navbar) return;

  const syncScroll = () => {
    navbar.classList.toggle("scrolled", window.scrollY > 32);
  };

  syncScroll();
  window.addEventListener("scroll", syncScroll, { passive: true });

  if (toggle && links) {
    toggle.addEventListener("click", () => {
      toggle.classList.toggle("active");
      links.classList.toggle("open");
    });

    qsa("a", links).forEach((link) => {
      link.addEventListener("click", () => {
        toggle.classList.remove("active");
        links.classList.remove("open");
      });
    });
  }
}

function initSmoothAnchors() {
  qsa('a[href^="#"]').forEach((anchor) => {
    anchor.addEventListener("click", (event) => {
      const href = anchor.getAttribute("href");
      if (!href || href === "#") return;
      const target = qs(href);
      if (!target) return;
      event.preventDefault();
      target.scrollIntoView({ behavior: "smooth", block: "start" });
    });
  });
}

function initReveal() {
  const items = qsa(".reveal");
  if (!items.length) return;

  if (window.gsap && window.ScrollTrigger) {
    window.gsap.registerPlugin(window.ScrollTrigger);
    items.forEach((item, index) => {
      window.gsap.fromTo(
        item,
        { autoAlpha: 0, y: 26 },
        {
          autoAlpha: 1,
          y: 0,
          duration: 0.7,
          delay: Math.min(index * 0.03, 0.18),
          ease: "power2.out",
          scrollTrigger: {
            trigger: item,
            start: "top 86%"
          }
        }
      );
    });
    return;
  }

  if (!("IntersectionObserver" in window)) {
    items.forEach((item) => item.classList.add("visible"));
    return;
  }

  const observer = new IntersectionObserver(
    (entries) => {
      entries.forEach((entry) => {
        if (entry.isIntersecting) {
          entry.target.classList.add("visible");
          observer.unobserve(entry.target);
        }
      });
    },
    { threshold: 0.12, rootMargin: "0px 0px -40px 0px" }
  );

  items.forEach((item) => observer.observe(item));
}

function animateCounter(element) {
  const target = Number(element.dataset.counter || "0");
  const prefix = element.dataset.prefix || "";
  const suffix = element.dataset.suffix || "";
  const duration = 1800;
  const start = performance.now();

  function tick(now) {
    const progress = Math.min((now - start) / duration, 1);
    const eased = 1 - Math.pow(1 - progress, 3);
    const value = Math.round(target * eased);
    element.textContent = `${prefix}${formatNumber(value)}${suffix}`;
    if (progress < 1) {
      requestAnimationFrame(tick);
    } else {
      element.textContent = `${prefix}${formatNumber(target)}${suffix}`;
    }
  }

  requestAnimationFrame(tick);
}

function initCounters() {
  const counters = qsa("[data-counter]");
  if (!counters.length) return;

  if (!("IntersectionObserver" in window)) {
    counters.forEach(animateCounter);
    return;
  }

  const observer = new IntersectionObserver(
    (entries) => {
      entries.forEach((entry) => {
        if (!entry.isIntersecting) return;
        animateCounter(entry.target);
        observer.unobserve(entry.target);
      });
    },
    { threshold: 0.45 }
  );

  counters.forEach((counter) => observer.observe(counter));
}

function initTilt(scope = document) {
  qsa("[data-tilt]", scope).forEach((card) => {
    card.addEventListener("mousemove", (event) => {
      const rect = card.getBoundingClientRect();
      const x = (event.clientX - rect.left) / rect.width - 0.5;
      const y = (event.clientY - rect.top) / rect.height - 0.5;
      card.style.transform = `perspective(1200px) rotateX(${(-y * 7).toFixed(2)}deg) rotateY(${(x * 8).toFixed(2)}deg) translateY(-4px)`;
    });

    card.addEventListener("mouseleave", () => {
      card.style.transform = "";
    });
  });
}

function initAutonomy() {
  const tiers = qsa(".autonomy-tier");
  const detail = qs("[data-autonomy-detail]");
  if (!tiers.length || !detail) return;

  const renderDetail = (level) => {
    const content = AUTONOMY_DETAILS[level];
    if (!content) return;

    detail.innerHTML = `
      <div class="eyebrow"><span class="eyebrow-dot"></span>${level} / ${content.tone}</div>
      <h3>${content.title}</h3>
      <p>${content.body}</p>
      <ul>
        ${content.bullets.map((bullet) => `<li>${bullet}</li>`).join("")}
      </ul>
    `;

    tiers.forEach((tier) => tier.classList.toggle("active", tier.dataset.level === level));
  };

  tiers.forEach((tier) => {
    tier.addEventListener("click", () => renderDetail(tier.dataset.level));
  });

  renderDetail("L3");
}

function initDocsSpy() {
  const tocLinks = qsa(".toc-panel a[href^='#']");
  if (!tocLinks.length || !("IntersectionObserver" in window)) return;
  const sections = tocLinks
    .map((link) => qs(link.getAttribute("href")))
    .filter(Boolean);

  const observer = new IntersectionObserver(
    (entries) => {
      entries.forEach((entry) => {
        if (!entry.isIntersecting) return;
        const currentId = `#${entry.target.id}`;
        tocLinks.forEach((link) => link.classList.toggle("active", link.getAttribute("href") === currentId));
      });
    },
    { threshold: 0.2, rootMargin: "-20% 0px -65% 0px" }
  );

  sections.forEach((section) => observer.observe(section));
}

function renderAgentCard(agent) {
  const tags = agent.capabilities
    .slice(0, 5)
    .map((capability) => `<span class="agent-tag">${capability}</span>`)
    .join("");
  const schedule = agent.schedule ? `<span class="pill mono">${formatSchedule(agent.schedule)}</span>` : "";

  return `
    <article class="agent-card ${agent.level === "L6" ? "level-l6" : ""}" data-level="${agent.level}" data-agent-id="${agent.id}" data-tilt>
      <div class="agent-topline">
        <span class="level-badge ${levelClass(agent.level)}">${agent.level}</span>
        <span class="mini-badge mono">${formatNumber(agent.fuel)} fuel</span>
      </div>
      <h3>${agent.name}</h3>
      <p>${agent.summary}</p>
      <div class="agent-tags">${tags}</div>
      <div class="agent-meta">
        <span class="pill mono">${formatNumber(agent.fuel)} fuel</span>
        ${schedule}
      </div>
      <button class="agent-expand" type="button" aria-expanded="false">Learn more</button>
      <div class="agent-extra">
        <p>${agent.detail}</p>
      </div>
    </article>
  `;
}

function spotlightMatches(filterValue, queryValue) {
  if (!(filterValue === "all" || filterValue === "L4")) return false;
  if (!queryValue) return true;
  const corpus = normalizeText(
    `${FEATURED_OPERATOR.name} ${FEATURED_OPERATOR.summary} ${FEATURED_OPERATOR.detail} ${FEATURED_OPERATOR.capabilities.join(" ")} computer control vision mouse keyboard screen click type scroll`
  );
  return corpus.includes(queryValue);
}

function bindAgentCards(scope) {
  qsa(".agent-expand", scope).forEach((button) => {
    button.addEventListener("click", () => {
      const card = button.closest(".agent-card");
      if (!card) return;
      const expanded = card.classList.toggle("expanded");
      button.setAttribute("aria-expanded", String(expanded));
      button.textContent = expanded ? "Show less" : "Learn more";
    });
  });

  initTilt(scope);
}

function initAgentsPage() {
  const root = qs("#agents-root");
  if (!root) return;

  const search = qs("#agent-search");
  const count = qs("#results-count");
  const filterButtons = qsa("[data-filter]");

  let currentFilter = "all";
  let currentQuery = "";

  const updateFilterLabels = (matchedBySearch) => {
    const counts = matchedBySearch.reduce((accumulator, agent) => {
      accumulator[agent.level] = (accumulator[agent.level] || 0) + 1;
      return accumulator;
    }, {});

    filterButtons.forEach((button) => {
      const label = button.dataset.label || button.textContent;
      const filter = button.dataset.filter || "all";
      const value = filter === "all" ? matchedBySearch.length : (counts[filter] || 0);
      button.textContent = `${label} (${value})`;
    });
  };

  const render = () => {
    const query = normalizeText(currentQuery);
    const searchMatched = CORE_AGENTS.filter((agent) => {
      const haystack = normalizeText(`${agent.name} ${agent.summary} ${agent.detail} ${agent.capabilities.join(" ")}`);
      return !query || haystack.includes(query);
    });
    const filtered = searchMatched.filter((agent) => {
      const matchesLevel = currentFilter === "all" || agent.level === currentFilter;
      return matchesLevel;
    });

    updateFilterLabels(searchMatched);

    if (count) {
      count.textContent = `Showing ${filtered.length} of 45 agents`;
    }

    const markup = LEVEL_ORDER.map((level) => {
      const levelAgents = filtered.filter((agent) => agent.level === level);
      const showSpotlight = level === "L4" && spotlightMatches(currentFilter, query);
      if (!levelAgents.length && !showSpotlight) return "";

      return `
        <section class="level-block reveal">
          <div class="level-header">
            <div>
              <span class="level-badge ${levelClass(level)}">${level}</span>
              <h2 style="margin-top:16px;">${AUTONOMY_DETAILS[level].title}</h2>
              <p>${LEVEL_COPY[level]}</p>
            </div>
          </div>
          ${
            showSpotlight
              ? `
              <div class="operator-spotlight" style="margin-bottom:20px;">
                <div>
                  <div class="meta-row">
                    <span class="featured-badge">Featured</span>
                    <span class="level-badge l4">L4</span>
                    <span class="mini-badge mono">${formatNumber(FEATURED_OPERATOR.fuel)} fuel</span>
                  </div>
                  <h3 style="margin-top:18px;">${FEATURED_OPERATOR.name}</h3>
                  <p style="margin-top:14px;">${FEATURED_OPERATOR.summary}</p>
                  <div class="bullet-stack" style="margin-top:22px;">
                    <div class="bullet-row"><i></i><div><strong>Vision loop</strong><p>Capture, analyze, act, verify. Every UI state becomes part of the governed reasoning loop.</p></div></div>
                    <div class="bullet-row"><i></i><div><strong>Computer control</strong><p>Moves the mouse, clicks buttons, types text, scrolls, and adapts when the screen changes.</p></div></div>
                    <div class="bullet-row"><i></i><div><strong>Safety rails</strong><p>Never types into password fields, never purchases without HITL, and can be stopped instantly with the kill switch.</p></div></div>
                  </div>
                  <div class="agent-tags" style="margin-top:18px;">
                    ${FEATURED_OPERATOR.capabilities.map((capability) => `<span class="agent-tag">${capability}</span>`).join("")}
                  </div>
                  <div class="agent-meta">
                    <span class="pill mono">${formatNumber(FEATURED_OPERATOR.fuel)} fuel</span>
                    <span class="pill">Featured in the computer control engine</span>
                  </div>
                </div>
                <div class="spotlight-display">
                  <div class="spotlight-ring one"></div>
                  <div class="spotlight-ring two"></div>
                  <div class="spotlight-ring three"></div>
                  <div class="spotlight-core">OP</div>
                  <span class="spotlight-orbit">See Screen</span>
                  <span class="spotlight-orbit">Plan Action</span>
                  <span class="spotlight-orbit">Click + Type</span>
                  <span class="spotlight-orbit">Verify Result</span>
                </div>
              </div>
            `
              : ""
          }
          <div class="agent-grid">
            ${levelAgents.map((agent) => renderAgentCard(agent)).join("")}
          </div>
        </section>
      `;
    }).join("");

    root.innerHTML = markup || `<div class="docs-panel"><h3>No agents match that search.</h3><p>Try a different level or use a broader keyword.</p></div>`;
    bindAgentCards(root);
    initReveal();
  };

  if (search) {
    search.addEventListener("input", (event) => {
      currentQuery = event.target.value;
      render();
    });
  }

  filterButtons.forEach((button) => {
    button.addEventListener("click", () => {
      currentFilter = button.dataset.filter || "all";
      filterButtons.forEach((candidate) => candidate.classList.toggle("active", candidate === button));
      render();
    });
  });

  render();
}

document.addEventListener("DOMContentLoaded", () => {
  initNavbar();
  initSmoothAnchors();
  initReveal();
  initCounters();
  initTilt();
  initAutonomy();
  initDocsSpy();
  initAgentsPage();
});
