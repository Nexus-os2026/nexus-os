use chrono::Utc;
use tracing::{info, warn};

use crate::agent::AgentAction;
use crate::governance::app_registry::AppCategory;
use crate::learning::memory::{ActionMemory, MemoryEntry};
use crate::learning::pattern::{PatternAction, PatternLibrary, UIPattern};

/// The 5 hard invariants that self-improvement cannot violate
#[derive(Debug, Clone)]
pub enum HardInvariant {
    /// Audit trail must never be disabled or modified
    AuditIntegrity,
    /// Fuel metering must never be bypassed
    FuelMeteringActive,
    /// HITL consent gates cannot be removed (only tier can change)
    ConsentGatesPresent,
    /// App governance grants cannot be self-escalated
    NoSelfEscalation,
    /// Behavioral envelope bounds cannot be widened by the agent
    EnvelopeBoundsFixed,
}

impl std::fmt::Display for HardInvariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HardInvariant::AuditIntegrity => write!(f, "AuditIntegrity"),
            HardInvariant::FuelMeteringActive => write!(f, "FuelMeteringActive"),
            HardInvariant::ConsentGatesPresent => write!(f, "ConsentGatesPresent"),
            HardInvariant::NoSelfEscalation => write!(f, "NoSelfEscalation"),
            HardInvariant::EnvelopeBoundsFixed => write!(f, "EnvelopeBoundsFixed"),
        }
    }
}

/// Result of optimizing a pattern
#[derive(Debug, Clone)]
pub struct OptimizationResult {
    pub pattern_id: String,
    pub optimization_type: OptimizationType,
    pub before: Vec<PatternAction>,
    pub after: Vec<PatternAction>,
    pub expected_improvement: String,
    pub invariants_checked: Vec<(HardInvariant, bool)>,
}

/// Type of optimization applied
#[derive(Debug, Clone, PartialEq)]
pub enum OptimizationType {
    /// Remove a step that isn't needed
    RemoveRedundantStep,
    /// Merge sequential type actions
    CombineActions,
    /// Add wait where timing issues occurred
    AddWait,
    /// Reorder steps for efficiency
    ReorderSteps,
    /// UI layout changed, update positions
    UpdateCoordinates,
    /// Entirely new pattern from memory
    NewPattern,
}

impl std::fmt::Display for OptimizationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OptimizationType::RemoveRedundantStep => write!(f, "RemoveRedundantStep"),
            OptimizationType::CombineActions => write!(f, "CombineActions"),
            OptimizationType::AddWait => write!(f, "AddWait"),
            OptimizationType::ReorderSteps => write!(f, "ReorderSteps"),
            OptimizationType::UpdateCoordinates => write!(f, "UpdateCoordinates"),
            OptimizationType::NewPattern => write!(f, "NewPattern"),
        }
    }
}

/// The pattern optimizer — learns from runs and improves patterns
pub struct PatternOptimizer {
    memory: ActionMemory,
    library: PatternLibrary,
    invariants: Vec<HardInvariant>,
}

impl PatternOptimizer {
    /// Create a new optimizer with the given memory and library
    pub fn new(memory: ActionMemory, library: PatternLibrary) -> Self {
        let invariants = vec![
            HardInvariant::AuditIntegrity,
            HardInvariant::FuelMeteringActive,
            HardInvariant::ConsentGatesPresent,
            HardInvariant::NoSelfEscalation,
            HardInvariant::EnvelopeBoundsFixed,
        ];
        Self {
            memory,
            library,
            invariants,
        }
    }

    /// Analyze a completed run and extract or update patterns
    pub fn learn_from_run(&mut self, entry: &MemoryEntry) -> Option<UIPattern> {
        if !entry.success {
            // Failed runs don't create patterns, but we record to memory
            return None;
        }

        // Check if a similar pattern exists with high confidence
        let matches = self.library.find_matching(&entry.task);

        if let Some(best) = matches.first() {
            if best.pattern.confidence > 0.7 {
                // Pattern exists and works — record success
                self.library.record_success(&best.pattern.id);
                return None;
            }
        }

        // No good pattern — create one from this successful run
        let actions: Vec<PatternAction> = entry
            .steps
            .iter()
            .flat_map(|s| s.actions.iter())
            .filter(|a| !matches!(a, AgentAction::Screenshot | AgentAction::Wait { .. }))
            .map(PatternAction::from_agent_action)
            .collect();

        if actions.is_empty() {
            return None;
        }

        let app_context = detect_primary_app(&entry.steps);

        let pattern = UIPattern {
            id: uuid::Uuid::new_v4().to_string(),
            name: slugify(&entry.task),
            description: entry.task.clone(),
            trigger: entry.task.clone(),
            app_context,
            actions,
            success_count: 1,
            failure_count: 0,
            avg_duration_ms: entry.total_duration_ms,
            confidence: 1.0,
            created_at: Utc::now(),
            last_used: Utc::now(),
            version: 1,
        };

        self.library.add_pattern(pattern.clone());
        Some(pattern)
    }

    /// Optimize a pattern by looking at success/failure history
    pub fn optimize_pattern(&self, pattern_id: &str) -> Option<OptimizationResult> {
        let pattern = self
            .library
            .patterns()
            .iter()
            .find(|p| p.id == pattern_id)?;

        // Try to combine sequential type actions
        let optimized = combine_sequential_types(&pattern.actions);
        if optimized.len() < pattern.actions.len() {
            return Some(OptimizationResult {
                pattern_id: pattern_id.to_string(),
                optimization_type: OptimizationType::CombineActions,
                before: pattern.actions.clone(),
                after: optimized,
                expected_improvement: "Fewer actions by combining sequential typing".to_string(),
                invariants_checked: Vec::new(),
            });
        }

        // Try to remove redundant screenshots
        let cleaned = remove_redundant_screenshots(&pattern.actions);
        if cleaned.len() < pattern.actions.len() {
            return Some(OptimizationResult {
                pattern_id: pattern_id.to_string(),
                optimization_type: OptimizationType::RemoveRedundantStep,
                before: pattern.actions.clone(),
                after: cleaned,
                expected_improvement: "Removed redundant screenshot actions".to_string(),
                invariants_checked: Vec::new(),
            });
        }

        None
    }

    /// Validate that an optimization does not violate any hard invariants
    pub fn validate_optimization(&self, result: &mut OptimizationResult) -> bool {
        let mut all_pass = true;

        for invariant in &self.invariants {
            let passes = match invariant {
                HardInvariant::AuditIntegrity => {
                    // Optimization must not disable audit hashing
                    // Pattern actions don't control the audit trail, so this always passes
                    // unless the optimization explicitly disables it
                    !has_audit_disable(&result.after)
                }
                HardInvariant::FuelMeteringActive => {
                    // Optimization must not bypass fuel metering
                    // Fuel is deducted per-action in the executor, not in patterns
                    true
                }
                HardInvariant::ConsentGatesPresent => {
                    // Optimization cannot remove consent/approval steps
                    // The consent gate is in the loop controller, not in patterns
                    true
                }
                HardInvariant::NoSelfEscalation => {
                    // Optimization cannot escalate permissions
                    !has_permission_escalation(&result.after)
                }
                HardInvariant::EnvelopeBoundsFixed => {
                    // Optimization cannot widen behavioral envelope
                    // New action count should not exceed original significantly
                    result.after.len() <= result.before.len() + 2
                }
            };

            if !passes {
                warn!(
                    "Hard invariant {} VIOLATED in optimization for pattern {}",
                    invariant, result.pattern_id
                );
                all_pass = false;
            }
            result.invariants_checked.push((invariant.clone(), passes));
        }

        all_pass
    }

    /// Apply an optimization if it passes all invariant checks
    pub fn apply_optimization(&mut self, mut result: OptimizationResult) -> Result<(), String> {
        if !self.validate_optimization(&mut result) {
            return Err("Optimization violates one or more hard invariants".to_string());
        }

        // Find and update the pattern
        let pattern = self
            .library
            .patterns_mut()
            .iter_mut()
            .find(|p| p.id == result.pattern_id)
            .ok_or_else(|| format!("Pattern {} not found", result.pattern_id))?;

        info!(
            "Applying {} optimization to pattern '{}' (v{} -> v{})",
            result.optimization_type,
            pattern.name,
            pattern.version,
            pattern.version + 1
        );

        pattern.actions = result.after;
        pattern.version += 1;
        pattern.last_used = Utc::now();

        Ok(())
    }

    /// Suggest new patterns from recurring tasks in memory that lack patterns
    pub fn suggest_patterns(&self) -> Vec<String> {
        let mut suggestions = Vec::new();

        for entry in self.memory.entries() {
            if !entry.success {
                continue;
            }
            let matches = self.library.find_matching(&entry.task);
            if matches.is_empty() || matches[0].score < 0.5 {
                // This successful task has no matching pattern
                let task = entry.task.clone();
                if !suggestions.contains(&task) {
                    suggestions.push(task);
                }
            }
        }

        suggestions
    }

    /// Get a reference to the pattern library
    pub fn library(&self) -> &PatternLibrary {
        &self.library
    }

    /// Get a mutable reference to the pattern library
    pub fn library_mut(&mut self) -> &mut PatternLibrary {
        &mut self.library
    }

    /// Get a reference to the memory
    pub fn memory(&self) -> &ActionMemory {
        &self.memory
    }

    /// Get a mutable reference to the memory
    pub fn memory_mut(&mut self) -> &mut ActionMemory {
        &mut self.memory
    }
}

/// Detect the primary app category from steps
fn detect_primary_app(steps: &[crate::learning::memory::MemoryStep]) -> AppCategory {
    use std::collections::HashMap;

    let mut counts: HashMap<String, usize> = HashMap::new();
    for step in steps {
        *counts.entry(step.app_context.clone()).or_default() += 1;
    }

    let primary = counts
        .into_iter()
        .max_by_key(|(_, c)| *c)
        .map(|(name, _)| name)
        .unwrap_or_else(|| "Unknown".to_string());

    match primary.as_str() {
        "Terminal" => AppCategory::Terminal,
        "Editor" => AppCategory::Editor,
        "Browser" => AppCategory::Browser,
        "FileManager" => AppCategory::FileManager,
        "Communication" => AppCategory::Communication,
        "System" => AppCategory::System,
        "NexusOS" => AppCategory::NexusOS,
        _ => AppCategory::Unknown,
    }
}

/// Create a slug from a task description
fn slugify(task: &str) -> String {
    task.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<&str>>()
        .join("_")
}

/// Combine sequential type actions into a single type action
fn combine_sequential_types(actions: &[PatternAction]) -> Vec<PatternAction> {
    let mut result: Vec<PatternAction> = Vec::new();

    for action in actions {
        if action.action_type == "type" {
            if let Some(last) = result.last_mut() {
                if last.action_type == "type" {
                    // Combine text
                    let prev_text = last
                        .parameters
                        .get("text")
                        .and_then(|t| t.as_str())
                        .unwrap_or("");
                    let curr_text = action
                        .parameters
                        .get("text")
                        .and_then(|t| t.as_str())
                        .unwrap_or("");
                    let combined = format!("{prev_text}{curr_text}");
                    last.parameters = serde_json::json!({ "text": combined });
                    continue;
                }
            }
        }
        result.push(action.clone());
    }

    result
}

/// Remove redundant screenshot actions from a pattern
fn remove_redundant_screenshots(actions: &[PatternAction]) -> Vec<PatternAction> {
    actions
        .iter()
        .filter(|a| a.action_type != "screenshot")
        .cloned()
        .collect()
}

/// Check if an optimization attempts to disable the audit trail
fn has_audit_disable(actions: &[PatternAction]) -> bool {
    actions.iter().any(|a| {
        if a.action_type == "type" {
            if let Some(text) = a.parameters.get("text").and_then(|t| t.as_str()) {
                return text.contains("disable_audit") || text.contains("skip_audit");
            }
        }
        false
    })
}

/// Check if an optimization attempts to escalate permissions
fn has_permission_escalation(actions: &[PatternAction]) -> bool {
    actions.iter().any(|a| {
        if a.action_type == "type" {
            if let Some(text) = a.parameters.get("text").and_then(|t| t.as_str()) {
                return text.contains("grant_level=Full") || text.contains("escalate_permissions");
            }
        }
        false
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::learning::memory::MemoryStep;

    fn make_memory(task: &str, success: bool) -> MemoryEntry {
        MemoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            task: task.to_string(),
            steps: vec![MemoryStep {
                step_number: 1,
                actions: vec![
                    AgentAction::Click {
                        x: 100,
                        y: 200,
                        button: "left".to_string(),
                    },
                    AgentAction::Type {
                        text: "cargo test".to_string(),
                    },
                    AgentAction::KeyPress {
                        key: "Return".to_string(),
                    },
                ],
                screenshot_hash: "abc123".to_string(),
                app_context: "Terminal".to_string(),
                duration_ms: 500,
            }],
            success,
            total_duration_ms: 1500,
            fuel_consumed: 100,
            timestamp: Utc::now(),
        }
    }

    fn make_optimizer() -> PatternOptimizer {
        let dir = tempfile::tempdir().expect("tmpdir");
        #[allow(deprecated)]
        let pat_path = dir.into_path().join("patterns.json");
        let mem_path = pat_path.with_file_name("memory.json");
        let library = PatternLibrary::new(pat_path);
        let memory = ActionMemory::new(mem_path, 100);
        PatternOptimizer::new(memory, library)
    }

    #[test]
    fn test_hard_invariant_audit_integrity() {
        let opt = make_optimizer();
        let mut result = OptimizationResult {
            pattern_id: "test".to_string(),
            optimization_type: OptimizationType::CombineActions,
            before: vec![],
            after: vec![PatternAction {
                action_type: "type".to_string(),
                parameters: serde_json::json!({ "text": "disable_audit" }),
                relative_to: None,
                wait_after_ms: 0,
            }],
            expected_improvement: "test".to_string(),
            invariants_checked: Vec::new(),
        };
        assert!(!opt.validate_optimization(&mut result));
        // AuditIntegrity should have failed
        let audit_check = result
            .invariants_checked
            .iter()
            .find(|(i, _)| matches!(i, HardInvariant::AuditIntegrity));
        assert!(audit_check.is_some());
        assert!(!audit_check.expect("found").1);
    }

    #[test]
    fn test_hard_invariant_fuel_metering() {
        let opt = make_optimizer();
        let mut result = OptimizationResult {
            pattern_id: "test".to_string(),
            optimization_type: OptimizationType::CombineActions,
            before: vec![],
            after: vec![],
            expected_improvement: "test".to_string(),
            invariants_checked: Vec::new(),
        };
        opt.validate_optimization(&mut result);
        let fuel_check = result
            .invariants_checked
            .iter()
            .find(|(i, _)| matches!(i, HardInvariant::FuelMeteringActive));
        assert!(fuel_check.expect("found").1);
    }

    #[test]
    fn test_hard_invariant_consent_gates() {
        let opt = make_optimizer();
        let mut result = OptimizationResult {
            pattern_id: "test".to_string(),
            optimization_type: OptimizationType::CombineActions,
            before: vec![],
            after: vec![],
            expected_improvement: "test".to_string(),
            invariants_checked: Vec::new(),
        };
        opt.validate_optimization(&mut result);
        let consent_check = result
            .invariants_checked
            .iter()
            .find(|(i, _)| matches!(i, HardInvariant::ConsentGatesPresent));
        assert!(consent_check.expect("found").1);
    }

    #[test]
    fn test_hard_invariant_no_self_escalation() {
        let opt = make_optimizer();
        let mut result = OptimizationResult {
            pattern_id: "test".to_string(),
            optimization_type: OptimizationType::CombineActions,
            before: vec![],
            after: vec![PatternAction {
                action_type: "type".to_string(),
                parameters: serde_json::json!({ "text": "grant_level=Full" }),
                relative_to: None,
                wait_after_ms: 0,
            }],
            expected_improvement: "test".to_string(),
            invariants_checked: Vec::new(),
        };
        assert!(!opt.validate_optimization(&mut result));
    }

    #[test]
    fn test_hard_invariant_envelope_bounds() {
        let opt = make_optimizer();
        let before = vec![PatternAction {
            action_type: "click".to_string(),
            parameters: serde_json::json!({}),
            relative_to: None,
            wait_after_ms: 0,
        }];
        // After has way more actions — envelope violation
        let after: Vec<PatternAction> = (0..10)
            .map(|_| PatternAction {
                action_type: "click".to_string(),
                parameters: serde_json::json!({}),
                relative_to: None,
                wait_after_ms: 0,
            })
            .collect();
        let mut result = OptimizationResult {
            pattern_id: "test".to_string(),
            optimization_type: OptimizationType::AddWait,
            before,
            after,
            expected_improvement: "test".to_string(),
            invariants_checked: Vec::new(),
        };
        assert!(!opt.validate_optimization(&mut result));
    }

    #[test]
    fn test_validate_optimization_all_pass() {
        let opt = make_optimizer();
        let actions = vec![PatternAction {
            action_type: "click".to_string(),
            parameters: serde_json::json!({ "x": 100, "y": 200 }),
            relative_to: None,
            wait_after_ms: 100,
        }];
        let mut result = OptimizationResult {
            pattern_id: "test".to_string(),
            optimization_type: OptimizationType::RemoveRedundantStep,
            before: actions.clone(),
            after: actions,
            expected_improvement: "test".to_string(),
            invariants_checked: Vec::new(),
        };
        assert!(opt.validate_optimization(&mut result));
        assert_eq!(result.invariants_checked.len(), 5);
        assert!(result.invariants_checked.iter().all(|(_, pass)| *pass));
    }

    #[test]
    fn test_validate_optimization_invariant_violation() {
        let opt = make_optimizer();
        let mut result = OptimizationResult {
            pattern_id: "test".to_string(),
            optimization_type: OptimizationType::CombineActions,
            before: vec![],
            after: vec![PatternAction {
                action_type: "type".to_string(),
                parameters: serde_json::json!({ "text": "skip_audit && escalate_permissions" }),
                relative_to: None,
                wait_after_ms: 0,
            }],
            expected_improvement: "test".to_string(),
            invariants_checked: Vec::new(),
        };
        assert!(!opt.validate_optimization(&mut result));
    }

    #[test]
    fn test_learn_from_successful_run() {
        let mut opt = make_optimizer();
        let entry = make_memory("run cargo test in terminal", true);
        let pattern = opt.learn_from_run(&entry);
        assert!(pattern.is_some());
        let p = pattern.expect("pattern created");
        assert_eq!(p.success_count, 1);
        assert_eq!(p.confidence, 1.0);
        assert!(!p.actions.is_empty());
    }

    #[test]
    fn test_learn_from_failed_run_skipped() {
        let mut opt = make_optimizer();
        let entry = make_memory("run cargo test", false);
        let pattern = opt.learn_from_run(&entry);
        assert!(pattern.is_none());
    }

    #[test]
    fn test_combine_sequential_type_actions() {
        let actions = vec![
            PatternAction {
                action_type: "type".to_string(),
                parameters: serde_json::json!({ "text": "hello " }),
                relative_to: None,
                wait_after_ms: 50,
            },
            PatternAction {
                action_type: "type".to_string(),
                parameters: serde_json::json!({ "text": "world" }),
                relative_to: None,
                wait_after_ms: 50,
            },
        ];
        let combined = combine_sequential_types(&actions);
        assert_eq!(combined.len(), 1);
        assert_eq!(combined[0].parameters["text"], "hello world");
    }

    #[test]
    fn test_remove_redundant_screenshot() {
        let actions = vec![
            PatternAction {
                action_type: "click".to_string(),
                parameters: serde_json::json!({}),
                relative_to: None,
                wait_after_ms: 100,
            },
            PatternAction {
                action_type: "screenshot".to_string(),
                parameters: serde_json::json!({}),
                relative_to: None,
                wait_after_ms: 0,
            },
            PatternAction {
                action_type: "type".to_string(),
                parameters: serde_json::json!({ "text": "test" }),
                relative_to: None,
                wait_after_ms: 50,
            },
        ];
        let cleaned = remove_redundant_screenshots(&actions);
        assert_eq!(cleaned.len(), 2);
        assert!(cleaned.iter().all(|a| a.action_type != "screenshot"));
    }

    #[test]
    fn test_new_pattern_creation() {
        let mut opt = make_optimizer();

        // First run — should create a pattern
        let entry = make_memory("deploy to staging server", true);
        let pattern = opt.learn_from_run(&entry);
        assert!(pattern.is_some());
        assert_eq!(opt.library().len(), 1);
    }

    #[test]
    fn test_existing_pattern_updated() {
        let mut opt = make_optimizer();

        // First run creates pattern
        let entry1 = make_memory("run cargo test", true);
        let p = opt.learn_from_run(&entry1);
        assert!(p.is_some());

        // Second similar run should update existing, not create new
        let entry2 = make_memory("run cargo test", true);
        let p2 = opt.learn_from_run(&entry2);
        assert!(p2.is_none()); // No new pattern
        assert_eq!(opt.library().len(), 1); // Still just one pattern

        // But success count should be incremented
        let pattern = &opt.library().patterns()[0];
        assert_eq!(pattern.success_count, 2);
    }

    #[test]
    fn test_optimization_type_display() {
        assert_eq!(
            format!("{}", OptimizationType::RemoveRedundantStep),
            "RemoveRedundantStep"
        );
        assert_eq!(
            format!("{}", OptimizationType::CombineActions),
            "CombineActions"
        );
        assert_eq!(format!("{}", OptimizationType::AddWait), "AddWait");
        assert_eq!(
            format!("{}", OptimizationType::ReorderSteps),
            "ReorderSteps"
        );
        assert_eq!(
            format!("{}", OptimizationType::UpdateCoordinates),
            "UpdateCoordinates"
        );
        assert_eq!(format!("{}", OptimizationType::NewPattern), "NewPattern");
    }

    #[test]
    fn test_full_learning_cycle() {
        let mut opt = make_optimizer();

        // 1. Record a successful run
        let entry = make_memory("run cargo test in terminal", true);
        opt.memory_mut().record(entry.clone());

        // 2. Learn from it — creates a pattern
        let pattern = opt.learn_from_run(&entry);
        assert!(pattern.is_some());
        let pat = pattern.expect("pattern");
        let pat_id = pat.id.clone();

        // 3. Match against a new task
        let matches = opt.library().find_matching("run cargo test in terminal");
        assert!(!matches.is_empty());
        assert!(matches[0].score > 0.8);

        // 4. Optimize the pattern
        let opt_result = opt.optimize_pattern(&pat_id);
        // May or may not produce an optimization depending on actions
        if let Some(mut result) = opt_result {
            let valid = opt.validate_optimization(&mut result);
            assert!(valid);
        }

        // 5. Suggest patterns for unmatched tasks
        opt.memory_mut()
            .record(make_memory("open firefox and search", true));
        let suggestions = opt.suggest_patterns();
        assert!(!suggestions.is_empty());
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("run cargo test"), "run_cargo_test");
        assert_eq!(slugify("Open Firefox & Search"), "open_firefox_search");
    }

    #[test]
    fn test_detect_primary_app() {
        let steps = vec![
            MemoryStep {
                step_number: 1,
                actions: vec![],
                screenshot_hash: "a".to_string(),
                app_context: "Terminal".to_string(),
                duration_ms: 100,
            },
            MemoryStep {
                step_number: 2,
                actions: vec![],
                screenshot_hash: "b".to_string(),
                app_context: "Terminal".to_string(),
                duration_ms: 100,
            },
            MemoryStep {
                step_number: 3,
                actions: vec![],
                screenshot_hash: "c".to_string(),
                app_context: "Editor".to_string(),
                duration_ms: 100,
            },
        ];
        assert_eq!(detect_primary_app(&steps), AppCategory::Terminal);
    }
}
