//! Persistent budget tracking for the Nexus Builder.
//!
//! Stores provider budgets and build history in a JSON file at
//! `~/.nexus/builder_budget.json` so data survives app restarts.

use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::{Path, PathBuf};

// ── Bounds ─────────────────────────────────────────────────────────────────
//
// The desktop's `builder_record_build`, `builder_set_budget` and
// `builder_set_remaining` pass interface values straight through, and every
// write rewrites the whole file. So each value, the history and the file are
// bounded. A write outside a bound is refused rather than clamped, and no
// write deletes a record or replaces a file it cannot read.

/// Most build records the file holds; a full history refuses new records.
pub const MAX_BUILD_HISTORY: usize = 1_000;
/// Most providers that may have a budget entry.
pub const MAX_BUDGET_PROVIDERS: usize = 16;
/// Longest text field of a build record, in bytes.
pub const MAX_BUDGET_TEXT_BYTES: usize = 256;
/// Longest provider name, in bytes.
pub const MAX_PROVIDER_NAME_BYTES: usize = 32;
/// Largest dollar amount accepted for one build, one budget or one balance.
pub const MAX_BUDGET_USD: f64 = 1_000_000.0;
/// Largest elapsed time accepted for one build: one week, in seconds.
pub const MAX_BUILD_ELAPSED_SECONDS: f64 = 604_800.0;
/// Largest token or line count accepted for one build.
pub const MAX_BUILD_COUNT: usize = 1_000_000_000;
/// The budget file is never read or written beyond this size, in bytes.
pub const MAX_BUDGET_FILE_BYTES: u64 = 4 * 1024 * 1024;

fn check_text(field: &str, value: &str) -> Result<(), String> {
    if value.len() > MAX_BUDGET_TEXT_BYTES {
        return Err(format!(
            "{field} is longer than {MAX_BUDGET_TEXT_BYTES} bytes"
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{field} contains a control character"));
    }
    Ok(())
}

fn check_provider(provider: &str) -> Result<(), String> {
    let valid = (1..=MAX_PROVIDER_NAME_BYTES).contains(&provider.len())
        && provider
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte));
    if valid {
        Ok(())
    } else {
        Err(format!(
            "provider must be 1 to {MAX_PROVIDER_NAME_BYTES} ASCII letters, digits, '.', '_' or '-'"
        ))
    }
}

fn check_amount(field: &str, value: f64, max: f64) -> Result<(), String> {
    if value.is_finite() && (0.0..=max).contains(&value) {
        Ok(())
    } else {
        Err(format!("{field} must be a finite number from 0 to {max}"))
    }
}

fn check_count(field: &str, value: usize) -> Result<(), String> {
    if value <= MAX_BUILD_COUNT {
        Ok(())
    } else {
        Err(format!("{field} must be at most {MAX_BUILD_COUNT}"))
    }
}

impl BuildRecord {
    /// Refuse a record whose values are outside the store's bounds.
    pub fn validate(&self) -> Result<(), String> {
        check_text("project_name", &self.project_name)?;
        check_text("model_name", &self.model_name)?;
        check_provider(&self.provider)?;
        check_text("checkpoint_id", &self.checkpoint_id)?;
        check_text("timestamp", &self.timestamp)?;
        check_count("input_tokens", self.input_tokens)?;
        check_count("output_tokens", self.output_tokens)?;
        check_count("lines_generated", self.lines_generated)?;
        check_amount("cost_usd", self.cost_usd, MAX_BUDGET_USD)?;
        check_amount(
            "elapsed_seconds",
            self.elapsed_seconds,
            MAX_BUILD_ELAPSED_SECONDS,
        )?;
        Ok(())
    }
}

/// How reading the budget file failed.
enum ReadFailure {
    /// No file yet: the store starts from defaults.
    Missing,
    /// Present but unreadable, or larger than `MAX_BUDGET_FILE_BYTES`.
    Unusable(String),
}

/// Read at most `MAX_BUDGET_FILE_BYTES` of the file; a longer file is not
/// read further. The size check and the read use the same open file.
fn read_bounded(path: &Path) -> Result<String, ReadFailure> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(ReadFailure::Missing)
        }
        Err(error) => {
            return Err(ReadFailure::Unusable(format!(
                "budget file cannot be read: {error}"
            )))
        }
    };
    let mut contents = String::new();
    file.take(MAX_BUDGET_FILE_BYTES + 1)
        .read_to_string(&mut contents)
        .map_err(|error| ReadFailure::Unusable(format!("budget file cannot be read: {error}")))?;
    if contents.len() as u64 > MAX_BUDGET_FILE_BYTES {
        return Err(ReadFailure::Unusable(format!(
            "budget file is larger than {MAX_BUDGET_FILE_BYTES} bytes"
        )));
    }
    Ok(contents)
}

// ── Data Types ─────────────────────────────────────────────────────────────

/// A single build record with cost, token, and timing information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildRecord {
    pub project_name: String,
    pub model_name: String,
    pub provider: String,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub cost_usd: f64,
    pub elapsed_seconds: f64,
    pub lines_generated: usize,
    pub checkpoint_id: String,
    pub timestamp: String,
}

/// Budget allocation for a single LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderBudget {
    pub provider: String,
    pub initial_budget_usd: f64,
    pub spent_usd: f64,
}

/// Root data structure persisted to disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetData {
    pub budgets: Vec<ProviderBudget>,
    pub builds: Vec<BuildRecord>,
}

impl Default for BudgetData {
    fn default() -> Self {
        Self {
            budgets: vec![
                ProviderBudget {
                    provider: "anthropic".into(),
                    initial_budget_usd: 0.0,
                    spent_usd: 0.0,
                },
                ProviderBudget {
                    provider: "openai".into(),
                    initial_budget_usd: 0.0,
                    spent_usd: 0.0,
                },
            ],
            builds: Vec::new(),
        }
    }
}

/// Summary returned to the frontend for display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetStatus {
    pub anthropic_initial: f64,
    pub anthropic_spent: f64,
    pub anthropic_remaining: f64,
    pub openai_initial: f64,
    pub openai_spent: f64,
    pub openai_remaining: f64,
    pub total_builds: usize,
    pub avg_cost_per_build: f64,
    pub estimated_builds_remaining: usize,
}

// ── Provider Detection ─────────────────────────────────────────────────────

/// Detect the provider from a model name string.
pub fn detect_provider(model_name: &str) -> &'static str {
    let lower = model_name.to_lowercase();
    if lower.contains("claude")
        || lower.contains("haiku")
        || lower.contains("sonnet")
        || lower.contains("opus")
    {
        "anthropic"
    } else if lower.contains("gpt") {
        "openai"
    } else {
        "other"
    }
}

// ── Budget Tracker ─────────────────────────────────────────────────────────

/// Manages budget persistence via a JSON file at `~/.nexus/builder_budget.json`.
#[derive(Debug, Clone)]
pub struct BudgetTracker {
    path: PathBuf,
}

impl Default for BudgetTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl BudgetTracker {
    /// Create a tracker at `builder_budget.json` under the validated identity
    /// home. With none the path is empty: loading yields defaults and saving
    /// is refused, never falling back to the working directory (P0-002C5B).
    pub fn new() -> Self {
        Self {
            path: nexus_kernel::identity_home::nexus_state_path("builder_budget.json")
                .unwrap_or_default(),
        }
    }

    /// Create a tracker with a custom file path (useful for tests).
    #[cfg(test)]
    pub fn with_path(path: PathBuf) -> Self {
        Self { path }
    }

    /// Load budget data from disk for display, returning defaults if the
    /// file is absent, unreadable, larger than `MAX_BUDGET_FILE_BYTES` or
    /// not valid budget data.
    pub fn load(&self) -> BudgetData {
        if !self.path.is_absolute() {
            return BudgetData::default();
        }
        match read_bounded(&self.path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => BudgetData::default(),
        }
    }

    /// Load budget data for a write. Unlike [`Self::load`], a file that
    /// exists but is unreadable, too large or not valid budget data is an
    /// error: writing over it would destroy the records it holds.
    fn load_for_write(&self) -> Result<BudgetData, String> {
        if !self.path.is_absolute() {
            return Err("budget storage is unavailable: no valid identity home".into());
        }
        match read_bounded(&self.path) {
            Ok(contents) => serde_json::from_str(&contents).map_err(|error| {
                format!("budget file is not valid budget data and is left unchanged: {error}")
            }),
            Err(ReadFailure::Missing) => Ok(BudgetData::default()),
            Err(ReadFailure::Unusable(reason)) => Err(format!("{reason}; it is left unchanged")),
        }
    }

    /// Persist budget data to disk, creating parent directories as needed.
    fn save(&self, data: &BudgetData) -> Result<(), String> {
        if !self.path.is_absolute() {
            return Err("budget storage is unavailable: no valid identity home".into());
        }
        let json =
            serde_json::to_string_pretty(data).map_err(|e| format!("serialization error: {e}"))?;
        if json.len() as u64 > MAX_BUDGET_FILE_BYTES {
            return Err(format!(
                "budget file would exceed {MAX_BUDGET_FILE_BYTES} bytes; nothing was written"
            ));
        }
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create budget dir: {e}"))?;
        }
        std::fs::write(&self.path, json).map_err(|e| format!("failed to write budget file: {e}"))
    }

    /// Record a completed build, updating the provider's spent amount.
    ///
    /// Refused, with the file unchanged, when a value is out of bounds, the
    /// history holds `MAX_BUILD_HISTORY` records, or the spent total would
    /// stop being finite. No record is ever dropped to make room.
    pub fn record_build(&self, record: BuildRecord) -> Result<(), String> {
        record.validate()?;
        let mut data = self.load_for_write()?;
        if data.builds.len() >= MAX_BUILD_HISTORY {
            return Err(format!(
                "build history is full ({MAX_BUILD_HISTORY} records); no record was added"
            ));
        }

        // Update spent for the matching provider
        if let Some(budget) = data
            .budgets
            .iter_mut()
            .find(|b| b.provider == record.provider)
        {
            let spent = budget.spent_usd + record.cost_usd;
            if !spent.is_finite() {
                return Err("provider spend would not be a finite number".into());
            }
            budget.spent_usd = spent;
        }

        data.builds.push(record);
        self.save(&data)
    }

    /// Find a provider's budget, adding an entry while fewer than
    /// `MAX_BUDGET_PROVIDERS` exist.
    fn provider_entry<'a>(
        data: &'a mut BudgetData,
        provider: &str,
        initial_budget_usd: f64,
    ) -> Result<(&'a mut ProviderBudget, bool), String> {
        if let Some(index) = data.budgets.iter().position(|b| b.provider == provider) {
            return Ok((&mut data.budgets[index], false));
        }
        if data.budgets.len() >= MAX_BUDGET_PROVIDERS {
            return Err(format!(
                "at most {MAX_BUDGET_PROVIDERS} providers may have a budget"
            ));
        }
        data.budgets.push(ProviderBudget {
            provider: provider.to_string(),
            initial_budget_usd,
            spent_usd: 0.0,
        });
        let last = data.budgets.len() - 1;
        Ok((&mut data.budgets[last], true))
    }

    /// Set or update the initial budget for a provider.
    pub fn set_initial_budget(&self, provider: &str, amount: f64) -> Result<(), String> {
        check_provider(provider)?;
        check_amount("amount", amount, MAX_BUDGET_USD)?;
        let mut data = self.load_for_write()?;

        let (budget, _) = Self::provider_entry(&mut data, provider, amount)?;
        budget.initial_budget_usd = amount;

        self.save(&data)
    }

    /// Set the remaining balance for a provider by adjusting `spent_usd`.
    ///
    /// This lets users manually correct their balance when the tracked
    /// spend diverges from reality (e.g. API console shows different numbers).
    pub fn set_remaining(&self, provider: &str, remaining: f64) -> Result<(), String> {
        check_provider(provider)?;
        check_amount("remaining", remaining, MAX_BUDGET_USD)?;
        let mut data = self.load_for_write()?;

        // Provider not yet tracked: created with initial = remaining, spent = 0.
        let (budget, created) = Self::provider_entry(&mut data, provider, remaining)?;
        if !created {
            let spent = (budget.initial_budget_usd - remaining).max(0.0);
            if !spent.is_finite() {
                return Err("provider spend would not be a finite number".into());
            }
            budget.spent_usd = spent;
        }

        self.save(&data)
    }

    /// Return the last N build records (most recent last), capped at 50.
    pub fn get_build_history(&self) -> Vec<BuildRecord> {
        let data = self.load();
        let len = data.builds.len();
        let start = len.saturating_sub(50);
        data.builds[start..].to_vec()
    }

    /// Calculate a summary status for the frontend.
    pub fn get_budget_status(&self) -> BudgetStatus {
        let data = self.load();

        let anthropic = data
            .budgets
            .iter()
            .find(|b| b.provider == "anthropic")
            .cloned()
            .unwrap_or(ProviderBudget {
                provider: "anthropic".into(),
                initial_budget_usd: 0.0,
                spent_usd: 0.0,
            });

        let openai = data
            .budgets
            .iter()
            .find(|b| b.provider == "openai")
            .cloned()
            .unwrap_or(ProviderBudget {
                provider: "openai".into(),
                initial_budget_usd: 0.0,
                spent_usd: 0.0,
            });

        let total_builds = data.builds.len();

        // Average cost from the last 5 builds
        let recent: Vec<&BuildRecord> = data.builds.iter().rev().take(5).collect();
        let avg_cost_per_build = if recent.is_empty() {
            0.0
        } else {
            recent.iter().map(|b| b.cost_usd).sum::<f64>() / recent.len() as f64
        };

        let total_remaining = (anthropic.initial_budget_usd - anthropic.spent_usd).max(0.0)
            + (openai.initial_budget_usd - openai.spent_usd).max(0.0);

        let estimated_builds_remaining = if avg_cost_per_build > 0.0 {
            (total_remaining / avg_cost_per_build) as usize
        } else {
            0
        };

        BudgetStatus {
            anthropic_initial: anthropic.initial_budget_usd,
            anthropic_spent: anthropic.spent_usd,
            anthropic_remaining: (anthropic.initial_budget_usd - anthropic.spent_usd).max(0.0),
            openai_initial: openai.initial_budget_usd,
            openai_spent: openai.spent_usd,
            openai_remaining: (openai.initial_budget_usd - openai.spent_usd).max(0.0),
            total_builds,
            avg_cost_per_build,
            estimated_builds_remaining,
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p0_002c5b_budget_storage_needs_an_absolute_path() {
        for path in ["", "builder_budget.json", "relative/builder_budget.json"] {
            let tracker = BudgetTracker::with_path(PathBuf::from(path));
            assert_eq!(
                tracker.load().budgets.len(),
                BudgetData::default().budgets.len()
            );
            assert!(
                tracker.set_initial_budget("openai", 10.0).is_err(),
                "{path:?}"
            );
        }
        assert!(!std::path::Path::new("builder_budget.json").exists());
    }
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    // Per-test tempdir helper. Each call creates a unique top-level
    // directory under std::env::temp_dir(); no shared parent.
    // Cleanup: relies on /tmp being cleared by the OS or CI runner
    // between sessions. Long-running CI runners may accumulate
    // /tmp/nexus_budget_test_* directories — acceptable trade-off
    // vs the previous shared-parent design which caused
    // cross-UID permission failures (April 7 2026 fix).
    // TODO(nexus-builder): consider tempfile::TempDir for RAII cleanup
    // if /tmp accumulation becomes a problem.
    fn temp_tracker() -> BudgetTracker {
        let pid = std::process::id();
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("nexus_budget_test_{pid}_{counter}_{nanos}"));

        fs::create_dir_all(&dir).expect("temp_tracker: failed to create per-test tempdir");

        BudgetTracker::with_path(dir.join("budget.json"))
    }

    fn sample_record(name: &str, cost: f64) -> BuildRecord {
        BuildRecord {
            project_name: name.into(),
            model_name: "claude-sonnet-4-20250514".into(),
            provider: "anthropic".into(),
            input_tokens: 500,
            output_tokens: 2000,
            cost_usd: cost,
            elapsed_seconds: 12.3,
            lines_generated: 150,
            checkpoint_id: "ckpt-1".into(),
            timestamp: "2026-04-03T12:00:00Z".into(),
        }
    }

    #[test]
    fn default_loads_empty() {
        let tracker = temp_tracker();
        let status = tracker.get_budget_status();
        assert_eq!(status.total_builds, 0);
        assert_eq!(status.anthropic_initial, 0.0);
    }

    #[test]
    fn set_budget_persists() {
        let tracker = temp_tracker();
        tracker.set_initial_budget("anthropic", 10.0).unwrap();
        let status = tracker.get_budget_status();
        assert_eq!(status.anthropic_initial, 10.0);
        assert_eq!(status.anthropic_remaining, 10.0);
    }

    #[test]
    fn record_build_updates_spent() {
        let tracker = temp_tracker();
        tracker.set_initial_budget("anthropic", 10.0).unwrap();
        tracker
            .record_build(sample_record("test-site", 0.05))
            .unwrap();

        let status = tracker.get_budget_status();
        assert_eq!(status.total_builds, 1);
        assert!((status.anthropic_spent - 0.05).abs() < 1e-10);
        assert!((status.anthropic_remaining - 9.95).abs() < 1e-10);
    }

    #[test]
    fn build_history_caps_at_50() {
        let tracker = temp_tracker();
        for i in 0..60 {
            tracker
                .record_build(BuildRecord {
                    project_name: format!("proj-{i}"),
                    checkpoint_id: format!("ckpt-{i}"),
                    ..sample_record("", 0.01)
                })
                .unwrap();
        }
        let history = tracker.get_build_history();
        assert_eq!(history.len(), 50);
        assert_eq!(history[0].project_name, "proj-10");
    }

    #[test]
    fn detect_provider_works() {
        assert_eq!(detect_provider("claude-sonnet-4-20250514"), "anthropic");
        assert_eq!(detect_provider("claude-3-haiku"), "anthropic");
        assert_eq!(detect_provider("gpt-4o"), "openai");
        assert_eq!(detect_provider("llama-3.1-70b"), "other");
    }

    #[test]
    fn estimated_builds_remaining() {
        let tracker = temp_tracker();
        tracker.set_initial_budget("anthropic", 1.0).unwrap();
        for _ in 0..5 {
            tracker.record_build(sample_record("site", 0.10)).unwrap();
        }
        let status = tracker.get_budget_status();
        // Spent 0.50, remaining 0.50, avg 0.10 → 5 builds remaining
        assert_eq!(status.estimated_builds_remaining, 5);
    }

    #[test]
    fn corrupt_file_returns_defaults() {
        let tracker = temp_tracker();
        fs::create_dir_all(tracker.path.parent().unwrap()).unwrap();
        fs::write(&tracker.path, "not valid json!!!").unwrap();
        let status = tracker.get_budget_status();
        assert_eq!(status.total_builds, 0);
    }

    // ── P0-FG K: resource bounds ────────────────────────────────────────────

    fn file_bytes(tracker: &BudgetTracker) -> Vec<u8> {
        fs::read(&tracker.path).unwrap()
    }

    fn write_data(tracker: &BudgetTracker, data: &BudgetData) {
        fs::write(&tracker.path, serde_json::to_string_pretty(data).unwrap()).unwrap();
    }

    #[test]
    fn p0_fg_k_out_of_bound_records_are_refused_and_change_nothing() {
        let tracker = temp_tracker();
        tracker.set_initial_budget("anthropic", 10.0).unwrap();
        tracker.record_build(sample_record("site", 0.05)).unwrap();
        let before = file_bytes(&tracker);

        let long = "x".repeat(MAX_BUDGET_TEXT_BYTES + 1);
        let refused: Vec<(&str, BuildRecord)> = vec![
            (
                "NaN cost",
                BuildRecord {
                    cost_usd: f64::NAN,
                    ..sample_record("s", 0.0)
                },
            ),
            (
                "infinite cost",
                BuildRecord {
                    cost_usd: f64::INFINITY,
                    ..sample_record("s", 0.0)
                },
            ),
            ("overflowing cost", sample_record("s", 1.7e308)),
            ("negative cost", sample_record("s", -0.01)),
            (
                "cost above bound",
                sample_record("s", MAX_BUDGET_USD * 1.000_001),
            ),
            (
                "NaN elapsed",
                BuildRecord {
                    elapsed_seconds: f64::NAN,
                    ..sample_record("s", 0.1)
                },
            ),
            (
                "negative elapsed",
                BuildRecord {
                    elapsed_seconds: -1.0,
                    ..sample_record("s", 0.1)
                },
            ),
            (
                "elapsed above bound",
                BuildRecord {
                    elapsed_seconds: MAX_BUILD_ELAPSED_SECONDS + 1.0,
                    ..sample_record("s", 0.1)
                },
            ),
            (
                "tokens above bound",
                BuildRecord {
                    input_tokens: MAX_BUILD_COUNT + 1,
                    ..sample_record("s", 0.1)
                },
            ),
            (
                "lines above bound",
                BuildRecord {
                    lines_generated: usize::MAX,
                    ..sample_record("s", 0.1)
                },
            ),
            ("long project name", sample_record(&long, 0.1)),
            (
                "long model name",
                BuildRecord {
                    model_name: long.clone(),
                    ..sample_record("s", 0.1)
                },
            ),
            (
                "control character",
                BuildRecord {
                    timestamp: "2026-09-28\nforged".into(),
                    ..sample_record("s", 0.1)
                },
            ),
            (
                "empty provider",
                BuildRecord {
                    provider: String::new(),
                    ..sample_record("s", 0.1)
                },
            ),
            (
                "provider grammar",
                BuildRecord {
                    provider: "open ai".into(),
                    ..sample_record("s", 0.1)
                },
            ),
            (
                "long provider",
                BuildRecord {
                    provider: "p".repeat(MAX_PROVIDER_NAME_BYTES + 1),
                    ..sample_record("s", 0.1)
                },
            ),
        ];
        for (case, record) in refused {
            assert!(tracker.record_build(record).is_err(), "{case}");
            assert_eq!(file_bytes(&tracker), before, "{case} changed the file");
        }

        for (case, result) in [
            (
                "NaN budget",
                tracker.set_initial_budget("anthropic", f64::NAN),
            ),
            (
                "negative budget",
                tracker.set_initial_budget("anthropic", -1.0),
            ),
            (
                "huge budget",
                tracker.set_initial_budget("anthropic", 1e300),
            ),
            (
                "bad provider",
                tracker.set_initial_budget("bad provider", 1.0),
            ),
            ("NaN balance", tracker.set_remaining("anthropic", f64::NAN)),
            (
                "infinite balance",
                tracker.set_remaining("anthropic", f64::INFINITY),
            ),
            ("negative balance", tracker.set_remaining("anthropic", -5.0)),
        ] {
            assert!(result.is_err(), "{case}");
            assert_eq!(file_bytes(&tracker), before, "{case} changed the file");
        }
    }

    #[test]
    fn p0_fg_k_bound_values_themselves_are_accepted() {
        let tracker = temp_tracker();
        let edge = "é".repeat(MAX_BUDGET_TEXT_BYTES / 2); // 256 bytes, multi-byte chars
        tracker
            .record_build(BuildRecord {
                project_name: edge.clone(),
                model_name: edge.clone(),
                provider: "p".repeat(MAX_PROVIDER_NAME_BYTES),
                input_tokens: MAX_BUILD_COUNT,
                output_tokens: 0,
                cost_usd: MAX_BUDGET_USD,
                elapsed_seconds: MAX_BUILD_ELAPSED_SECONDS,
                lines_generated: MAX_BUILD_COUNT,
                checkpoint_id: String::new(),
                timestamp: edge,
            })
            .unwrap();
        tracker.set_initial_budget("openai", 0.0).unwrap();
        tracker
            .set_initial_budget("openai", MAX_BUDGET_USD)
            .unwrap();
        tracker.set_remaining("openai", MAX_BUDGET_USD).unwrap();
        tracker.set_remaining("openai", 0.0).unwrap();
        assert_eq!(tracker.get_build_history().len(), 1);
    }

    #[test]
    fn p0_fg_k_a_full_history_refuses_records_and_deletes_none() {
        let tracker = temp_tracker();
        let data = BudgetData {
            builds: (0..MAX_BUILD_HISTORY)
                .map(|i| BuildRecord {
                    project_name: format!("proj-{i}"),
                    ..sample_record("", 0.01)
                })
                .collect(),
            ..BudgetData::default()
        };
        write_data(&tracker, &data);
        let before = file_bytes(&tracker);

        let error = tracker
            .record_build(sample_record("one-too-many", 0.01))
            .expect_err("a full history must refuse");
        assert!(error.contains("history is full"), "{error}");
        assert_eq!(file_bytes(&tracker), before);
        assert_eq!(tracker.load().builds.len(), MAX_BUILD_HISTORY);
        assert_eq!(tracker.load().builds[0].project_name, "proj-0");
        // Budget edits still work on a full history.
        tracker.set_initial_budget("anthropic", 5.0).unwrap();
        assert_eq!(tracker.load().builds.len(), MAX_BUILD_HISTORY);
    }

    #[test]
    fn p0_fg_k_writes_never_replace_a_file_they_cannot_read() {
        // Before the bounds, spending past f64::MAX wrote `null`, the next
        // load fell back to defaults and the next write replaced the whole
        // history. Now an unparsable, oversized or unreadable file is left as
        // it is; display still falls back to defaults.
        let tracker = temp_tracker();
        let history = format!(
            "{{\"budgets\":[{{\"provider\":\"anthropic\",\"initial_budget_usd\":1.0,\
             \"spent_usd\":null}}],\"builds\":[{}]}}",
            serde_json::to_string(&sample_record("kept", 0.5)).unwrap()
        );
        let mut oversized = serde_json::to_string(&BudgetData::default()).unwrap();
        oversized.push_str(&" ".repeat(MAX_BUDGET_FILE_BYTES as usize));

        for (case, contents) in [
            ("unparsable", history.into_bytes()),
            ("oversized", oversized.into_bytes()),
            ("not UTF-8", vec![0xff, 0xfe, 0x00, 0x7b]),
        ] {
            fs::write(&tracker.path, &contents).unwrap();
            for (op, result) in [
                ("record", tracker.record_build(sample_record("new", 0.1))),
                ("budget", tracker.set_initial_budget("anthropic", 3.0)),
                ("balance", tracker.set_remaining("anthropic", 1.0)),
            ] {
                let error = result.expect_err(case);
                assert!(error.contains("left unchanged"), "{case}/{op}: {error}");
                assert_eq!(fs::read(&tracker.path).unwrap(), contents, "{case}/{op}");
            }
            assert_eq!(tracker.get_budget_status().total_builds, 0, "{case}");
        }

        // A directory where the file belongs cannot be read or replaced.
        fs::remove_file(&tracker.path).unwrap();
        fs::create_dir(&tracker.path).unwrap();
        assert!(tracker.set_initial_budget("anthropic", 3.0).is_err());
        assert!(tracker.path.is_dir());
    }

    #[test]
    fn p0_fg_k_provider_entries_are_bounded() {
        let tracker = temp_tracker();
        // The default file already holds anthropic and openai.
        for i in 0..(MAX_BUDGET_PROVIDERS - 2) {
            tracker
                .set_initial_budget(&format!("provider-{i}"), 1.0)
                .unwrap();
        }
        let before = file_bytes(&tracker);
        assert!(tracker.set_initial_budget("one-more", 1.0).is_err());
        assert!(tracker.set_remaining("one-more", 1.0).is_err());
        assert_eq!(file_bytes(&tracker), before);
        // Existing providers stay editable.
        tracker.set_initial_budget("provider-0", 2.0).unwrap();
        tracker.set_remaining("anthropic", 0.0).unwrap();
        assert_eq!(tracker.load().budgets.len(), MAX_BUDGET_PROVIDERS);
    }

    #[test]
    fn p0_fg_k_a_worst_case_file_within_the_bounds_fits_the_cap() {
        // Longest fields, each doubled by JSON escaping, at every bound.
        let text = "\"".repeat(MAX_BUDGET_TEXT_BYTES);
        let record = BuildRecord {
            project_name: text.clone(),
            model_name: text.clone(),
            provider: "p".repeat(MAX_PROVIDER_NAME_BYTES),
            input_tokens: MAX_BUILD_COUNT,
            output_tokens: MAX_BUILD_COUNT,
            cost_usd: MAX_BUDGET_USD,
            elapsed_seconds: MAX_BUILD_ELAPSED_SECONDS,
            lines_generated: MAX_BUILD_COUNT,
            checkpoint_id: text.clone(),
            timestamp: text,
        };
        record.validate().unwrap();
        let data = BudgetData {
            budgets: (0..MAX_BUDGET_PROVIDERS)
                .map(|i| ProviderBudget {
                    provider: format!("{i:0>width$}", width = MAX_PROVIDER_NAME_BYTES),
                    initial_budget_usd: MAX_BUDGET_USD,
                    spent_usd: MAX_BUDGET_USD * MAX_BUILD_HISTORY as f64,
                })
                .collect(),
            builds: vec![record; MAX_BUILD_HISTORY],
        };
        let size = serde_json::to_string_pretty(&data).unwrap().len() as u64;
        assert!(size <= MAX_BUDGET_FILE_BYTES, "{size} bytes");
    }
}
