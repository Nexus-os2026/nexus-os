//! Filesystem-based checkpoint manager for the Builder.
//!
//! Each checkpoint is a copy of the generated file(s) in `{project_dir}/checkpoints/cp_NNN/`
//! with a `metadata.json` sidecar. The active version lives in `{project_dir}/current/`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Metadata for a single checkpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Sequential ID: "cp_001", "cp_002", etc.
    pub id: String,
    /// ISO-8601 timestamp.
    pub timestamp: String,
    /// Human-readable description (e.g. "Initial build" or "User: move nav to sidebar").
    pub description: String,
    /// Cost of the generation that produced this version (0.0 for rollbacks).
    pub cost: f64,
    /// Which checkpoint this was derived from.
    pub parent_id: Option<String>,
    /// Line count of the main output file.
    pub lines: usize,
    /// Character count of the main output file.
    pub chars: usize,
}

/// Manages checkpoint storage for a single project.
pub struct CheckpointManager {
    project_dir: PathBuf,
}

/// Maximum checkpoints per project. After this, oldest non-initial checkpoints are pruned.
const MAX_CHECKPOINTS: usize = 50;

impl CheckpointManager {
    /// Create a new checkpoint manager for a project directory.
    ///
    /// The directory structure is:
    /// ```text
    /// {project_dir}/
    ///   current/          ← active version (what the preview shows)
    ///     index.html
    ///   checkpoints/
    ///     cp_001/          ← auto-saved before each iteration
    ///       index.html
    ///       metadata.json
    ///     cp_002/
    ///       ...
    /// ```
    pub fn new(project_dir: &Path) -> Self {
        Self {
            project_dir: project_dir.to_path_buf(),
        }
    }

    /// Save the current state as a new checkpoint.
    ///
    /// If `current/` doesn't exist but `index.html` exists directly in the
    /// project dir (pre-checkpoint build), auto-initializes the structure.
    pub fn save_checkpoint(&self, description: &str, cost: f64) -> Result<Checkpoint, String> {
        let current_dir = self.project_dir.join("current");
        if !current_dir.exists() {
            // Auto-initialize from a direct index.html if it exists
            let direct_html = self.project_dir.join("index.html");
            if direct_html.exists() {
                std::fs::create_dir_all(&current_dir)
                    .map_err(|e| format!("failed to create current dir: {e}"))?;
                copy_build_files(&self.project_dir, &current_dir)?;
            } else {
                return Err("No current build to checkpoint".to_string());
            }
        }

        let checkpoints_dir = self.project_dir.join("checkpoints");
        std::fs::create_dir_all(&checkpoints_dir)
            .map_err(|e| format!("failed to create checkpoints dir: {e}"))?;

        let next_id = self.next_checkpoint_id();
        let cp_dir = checkpoints_dir.join(&next_id);
        std::fs::create_dir_all(&cp_dir)
            .map_err(|e| format!("failed to create checkpoint dir: {e}"))?;

        // Copy current/ contents to checkpoint dir
        copy_dir_contents(&current_dir, &cp_dir)?;

        // Count lines/chars of the main file
        let index_path = cp_dir.join("index.html");
        let (lines, chars) = if index_path.exists() {
            let content = std::fs::read_to_string(&index_path).unwrap_or_default();
            (content.lines().count(), content.len())
        } else {
            (0, 0)
        };

        // Determine parent
        let parent_id = self.latest_checkpoint_id();

        let metadata = Checkpoint {
            id: next_id.clone(),
            timestamp: now_iso8601(),
            description: description.to_string(),
            cost,
            parent_id,
            lines,
            chars,
        };

        // Write metadata
        let meta_json = serde_json::to_string_pretty(&metadata)
            .map_err(|e| format!("failed to serialize metadata: {e}"))?;
        std::fs::write(cp_dir.join("metadata.json"), meta_json)
            .map_err(|e| format!("failed to write metadata: {e}"))?;

        // Prune old checkpoints if over limit
        self.prune_old_checkpoints();

        Ok(metadata)
    }

    /// Rollback: replace current/ with checkpoint contents.
    ///
    /// Auto-saves current state before rollback so the rollback itself is reversible.
    pub fn rollback(&self, checkpoint_id: &str) -> Result<Checkpoint, String> {
        let cp_dir = self.project_dir.join("checkpoints").join(checkpoint_id);
        if !cp_dir.exists() {
            return Err(format!("Checkpoint {} not found", checkpoint_id));
        }

        let current_dir = self.project_dir.join("current");

        // Auto-save current state before rollback (so rollback is reversible)
        if current_dir.exists() {
            let _ = self.save_checkpoint("Auto-save before rollback", 0.0);
        }

        // Replace current/ with checkpoint contents
        if current_dir.exists() {
            clear_dir(&current_dir)?;
        } else {
            std::fs::create_dir_all(&current_dir)
                .map_err(|e| format!("failed to create current dir: {e}"))?;
        }
        copy_dir_contents(&cp_dir, &current_dir)?;

        // Don't copy metadata.json to current/
        let meta_in_current = current_dir.join("metadata.json");
        if meta_in_current.exists() {
            let _ = std::fs::remove_file(meta_in_current);
        }

        // Read and return the checkpoint metadata
        let meta_path = cp_dir.join("metadata.json");
        let meta_str = std::fs::read_to_string(&meta_path)
            .map_err(|e| format!("failed to read checkpoint metadata: {e}"))?;
        let metadata: Checkpoint = serde_json::from_str(&meta_str)
            .map_err(|e| format!("failed to parse checkpoint metadata: {e}"))?;

        Ok(metadata)
    }

    /// List all checkpoints for the timeline view, sorted by ID (chronological).
    pub fn list_checkpoints(&self) -> Vec<Checkpoint> {
        let checkpoints_dir = self.project_dir.join("checkpoints");
        if !checkpoints_dir.exists() {
            return Vec::new();
        }

        let mut checkpoints = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&checkpoints_dir) {
            for entry in entries.flatten() {
                let meta_path = entry.path().join("metadata.json");
                if meta_path.exists() {
                    if let Ok(meta_str) = std::fs::read_to_string(&meta_path) {
                        if let Ok(cp) = serde_json::from_str::<Checkpoint>(&meta_str) {
                            checkpoints.push(cp);
                        }
                    }
                }
            }
        }

        checkpoints.sort_by(|a, b| a.id.cmp(&b.id));
        checkpoints
    }

    /// Read the current HTML from current/index.html.
    ///
    /// Falls back to reading index.html directly from the project dir
    /// if the checkpoint structure hasn't been initialized yet.
    pub fn read_current_html(&self) -> Result<String, String> {
        let current_path = self.project_dir.join("current").join("index.html");
        if current_path.exists() {
            return std::fs::read_to_string(&current_path)
                .map_err(|e| format!("failed to read current/index.html: {e}"));
        }
        // Fallback: read directly from project dir (pre-checkpoint builds)
        let direct_path = self.project_dir.join("index.html");
        if direct_path.exists() {
            return std::fs::read_to_string(&direct_path)
                .map_err(|e| format!("failed to read index.html: {e}"));
        }
        Err("No index.html found in project directory".to_string())
    }

    /// Write HTML to current/index.html (used after iteration generates new output).
    pub fn write_current_html(&self, html: &str) -> Result<(), String> {
        let current_dir = self.project_dir.join("current");
        std::fs::create_dir_all(&current_dir)
            .map_err(|e| format!("failed to create current dir: {e}"))?;
        std::fs::write(current_dir.join("index.html"), html)
            .map_err(|e| format!("failed to write current/index.html: {e}"))
    }

    /// Initialize the project directory with the first build output.
    ///
    /// Copies the build output to `current/` and creates `cp_001` (initial build).
    /// Handles the case where `build_output_dir == project_dir` by skipping
    /// the `current/` and `checkpoints/` infrastructure directories.
    pub fn init_from_build(
        &self,
        build_output_dir: &Path,
        cost: f64,
    ) -> Result<Checkpoint, String> {
        let current_dir = self.project_dir.join("current");
        std::fs::create_dir_all(&current_dir)
            .map_err(|e| format!("failed to create current dir: {e}"))?;

        // Copy build output files to current/, skipping checkpoint infrastructure
        copy_build_files(build_output_dir, &current_dir)?;

        // Save as cp_001 (initial build)
        self.save_checkpoint("Initial build", cost)
    }

    fn next_checkpoint_id(&self) -> String {
        let existing = self.list_checkpoints();
        let next_num = existing.len() + 1;
        format!("cp_{:03}", next_num)
    }

    fn latest_checkpoint_id(&self) -> Option<String> {
        let existing = self.list_checkpoints();
        existing.last().map(|cp| cp.id.clone())
    }

    fn prune_old_checkpoints(&self) {
        let checkpoints = self.list_checkpoints();
        if checkpoints.len() <= MAX_CHECKPOINTS {
            return;
        }

        // Keep cp_001 (initial build) and the most recent ones
        let to_remove = checkpoints.len() - MAX_CHECKPOINTS;
        for cp in checkpoints.iter().skip(1).take(to_remove) {
            let cp_dir = self.project_dir.join("checkpoints").join(&cp.id);
            let _ = std::fs::remove_dir_all(cp_dir);
        }
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────

/// Copy build output files, skipping checkpoint infrastructure directories.
/// Used by `init_from_build` to handle the case where `src == project_dir`.
fn copy_build_files(src: &Path, dst: &Path) -> Result<(), String> {
    let entries =
        std::fs::read_dir(src).map_err(|e| format!("failed to read dir {}: {e}", src.display()))?;

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip checkpoint infrastructure directories
        if name_str == "current" || name_str == "checkpoints" {
            continue;
        }

        let file_type = entry
            .file_type()
            .map_err(|e| format!("failed to get file type: {e}"))?;
        let dst_path = dst.join(&name);

        if file_type.is_file() {
            std::fs::copy(entry.path(), &dst_path)
                .map_err(|e| format!("failed to copy file: {e}"))?;
        } else if file_type.is_dir() {
            std::fs::create_dir_all(&dst_path).map_err(|e| format!("failed to create dir: {e}"))?;
            copy_dir_contents(&entry.path(), &dst_path)?;
        }
    }
    Ok(())
}

fn copy_dir_contents(src: &Path, dst: &Path) -> Result<(), String> {
    let entries =
        std::fs::read_dir(src).map_err(|e| format!("failed to read dir {}: {e}", src.display()))?;

    for entry in entries.flatten() {
        let file_type = entry
            .file_type()
            .map_err(|e| format!("failed to get file type: {e}"))?;
        let dst_path = dst.join(entry.file_name());

        if file_type.is_file() {
            std::fs::copy(entry.path(), &dst_path)
                .map_err(|e| format!("failed to copy file: {e}"))?;
        } else if file_type.is_dir() {
            std::fs::create_dir_all(&dst_path).map_err(|e| format!("failed to create dir: {e}"))?;
            copy_dir_contents(&entry.path(), &dst_path)?;
        }
    }
    Ok(())
}

fn clear_dir(dir: &Path) -> Result<(), String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("failed to read dir {}: {e}", dir.display()))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            std::fs::remove_dir_all(&path).map_err(|e| format!("failed to remove dir: {e}"))?;
        } else {
            std::fs::remove_file(&path).map_err(|e| format!("failed to remove file: {e}"))?;
        }
    }
    Ok(())
}

fn now_iso8601() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Simple ISO-8601 without chrono dependency
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Approximate date calculation (good enough for timestamps)
    let mut y = 1970i64;
    let mut remaining_days = days as i64;
    loop {
        let days_in_year = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
            366
        } else {
            365
        };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let month_days: [i64; 12] = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 0usize;
    for (i, &md) in month_days.iter().enumerate() {
        if remaining_days < md {
            m = i;
            break;
        }
        remaining_days -= md;
    }

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m + 1,
        remaining_days + 1,
        hours,
        minutes,
        seconds
    )
}

// ─── Iteration Prompt ─────────────────────────────────────────────────────

/// System prompt for iteration requests (different from initial build).
pub const ITERATION_SYSTEM_PROMPT: &str = "\
You are a code editor. The user wants a specific change applied to their \
existing website. Apply ONLY the requested change. Do NOT regenerate the \
site from scratch. Preserve all existing code, styles, structure, and \
functionality exactly as-is except for the specific change requested. \
Output the COMPLETE updated HTML file. Output ONLY HTML code — no \
explanations, no markdown fences, no commentary.";

/// Build the prompt for an iteration request.
///
/// The change request is placed BEFORE the HTML so the LLM sees the
/// instruction first and applies it to the code that follows.
pub fn build_iteration_prompt(current_html: &str, user_request: &str) -> String {
    format!(
        "CHANGE REQUESTED: {user_request}\n\n\
         Apply this change to the following HTML. Output the complete updated file.\n\
         Preserve ALL existing code, styles, and structure — only modify what is \
         necessary for the requested change.\n\n\
         ```html\n{current_html}\n```"
    )
}

// ─── Project Metadata ─────────────────────────────────────────────────────

/// Metadata for a builder project, saved alongside the build output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub id: String,
    pub name: String,
    pub prompt: String,
    pub model: String,
    pub created_at: String,
    pub updated_at: String,
    pub versions: usize,
    pub total_cost: f64,
    pub lines: usize,
}

/// Save project metadata to `{project_dir}/project.json`.
pub fn save_project_meta(project_dir: &Path, meta: &ProjectMeta) -> Result<(), String> {
    let path = project_dir.join("project.json");
    let json = serde_json::to_string_pretty(meta).map_err(|e| format!("serialize: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("write project.json: {e}"))
}

/// Load project metadata from `{project_dir}/project.json`.
pub fn load_project_meta(project_dir: &Path) -> Option<ProjectMeta> {
    let path = project_dir.join("project.json");
    let contents = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&contents).ok()
}

/// List all projects in `~/.nexus/builds/` that have a `project.json`.
/// Returns sorted by `updated_at` descending (most recent first).
pub fn list_projects() -> Vec<ProjectMeta> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let builds_dir = std::path::PathBuf::from(home).join(".nexus").join("builds");
    let mut projects = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&builds_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                if let Some(meta) = load_project_meta(&entry.path()) {
                    projects.push(meta);
                }
            }
        }
    }

    projects.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    projects
}

/// Delete a project directory entirely.
pub fn delete_project(project_id: &str) -> Result<(), String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let project_dir = std::path::PathBuf::from(home)
        .join(".nexus")
        .join("builds")
        .join(project_id);

    if !project_dir.exists() {
        return Err(format!("project {project_id} not found"));
    }
    std::fs::remove_dir_all(&project_dir).map_err(|e| format!("delete: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "nexus-checkpoint-test-{}-{}",
            name,
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn setup_current(project_dir: &Path) {
        let current = project_dir.join("current");
        fs::create_dir_all(&current).unwrap();
        fs::write(
            current.join("index.html"),
            "<!DOCTYPE html><html><body>Hello</body></html>",
        )
        .unwrap();
    }

    #[test]
    fn test_save_checkpoint() {
        let dir = test_dir("save");
        setup_current(&dir);
        let mgr = CheckpointManager::new(&dir);

        let cp = mgr.save_checkpoint("Initial build", 0.05).unwrap();
        assert_eq!(cp.id, "cp_001");
        assert_eq!(cp.description, "Initial build");
        assert!((cp.cost - 0.05).abs() < 1e-10);
        assert!(cp.lines > 0);
        assert!(cp.chars > 0);
        assert!(cp.parent_id.is_none());

        // Verify files exist
        let cp_dir = dir.join("checkpoints").join("cp_001");
        assert!(cp_dir.join("index.html").exists());
        assert!(cp_dir.join("metadata.json").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_sequential_checkpoints() {
        let dir = test_dir("sequential");
        setup_current(&dir);
        let mgr = CheckpointManager::new(&dir);

        let cp1 = mgr.save_checkpoint("First", 0.05).unwrap();
        let cp2 = mgr.save_checkpoint("Second", 0.03).unwrap();
        let cp3 = mgr.save_checkpoint("Third", 0.04).unwrap();

        assert_eq!(cp1.id, "cp_001");
        assert_eq!(cp2.id, "cp_002");
        assert_eq!(cp3.id, "cp_003");
        assert_eq!(cp2.parent_id.as_deref(), Some("cp_001"));
        assert_eq!(cp3.parent_id.as_deref(), Some("cp_002"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_list_checkpoints() {
        let dir = test_dir("list");
        setup_current(&dir);
        let mgr = CheckpointManager::new(&dir);

        mgr.save_checkpoint("First", 0.05).unwrap();
        mgr.save_checkpoint("Second", 0.03).unwrap();

        let list = mgr.list_checkpoints();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, "cp_001");
        assert_eq!(list[1].id, "cp_002");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_rollback() {
        let dir = test_dir("rollback");
        setup_current(&dir);
        let mgr = CheckpointManager::new(&dir);

        // Save initial state
        mgr.save_checkpoint("Initial", 0.05).unwrap();

        // Modify current
        fs::write(
            dir.join("current").join("index.html"),
            "<!DOCTYPE html><html><body>Modified</body></html>",
        )
        .unwrap();
        mgr.save_checkpoint("Modified", 0.03).unwrap();

        // Rollback to cp_001
        let rolled_back = mgr.rollback("cp_001").unwrap();
        assert_eq!(rolled_back.id, "cp_001");

        // Verify current/ contains original content
        let current_html = mgr.read_current_html().unwrap();
        assert!(current_html.contains("Hello"));
        assert!(!current_html.contains("Modified"));

        // Verify auto-save created a new checkpoint (cp_003)
        let list = mgr.list_checkpoints();
        assert_eq!(list.len(), 3);
        assert_eq!(list[2].description, "Auto-save before rollback");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_rollback_nonexistent() {
        let dir = test_dir("rollback-bad");
        setup_current(&dir);
        let mgr = CheckpointManager::new(&dir);

        let result = mgr.rollback("cp_999");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_init_from_build() {
        let dir = test_dir("init");
        let build_output = test_dir("init-build-output");
        fs::write(
            build_output.join("index.html"),
            "<!DOCTYPE html><html><body>Built</body></html>",
        )
        .unwrap();

        let mgr = CheckpointManager::new(&dir);
        let cp = mgr.init_from_build(&build_output, 0.12).unwrap();

        assert_eq!(cp.id, "cp_001");
        assert_eq!(cp.description, "Initial build");

        // Verify current/ exists
        let current_html = mgr.read_current_html().unwrap();
        assert!(current_html.contains("Built"));

        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&build_output);
    }

    #[test]
    fn test_write_current_html() {
        let dir = test_dir("write");
        let mgr = CheckpointManager::new(&dir);

        mgr.write_current_html("<html>New content</html>").unwrap();
        let html = mgr.read_current_html().unwrap();
        assert_eq!(html, "<html>New content</html>");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_iteration_prompt() {
        let prompt = build_iteration_prompt(
            "<html><body>Hello</body></html>",
            "change the background to blue",
        );
        // Change request should appear BEFORE the HTML
        let change_pos = prompt.find("CHANGE REQUESTED").unwrap();
        let html_pos = prompt.find("<html><body>Hello</body></html>").unwrap();
        assert!(
            change_pos < html_pos,
            "change request must come before HTML in prompt"
        );
        assert!(prompt.contains("change the background to blue"));
        assert!(prompt.contains("complete updated file"));
    }

    #[test]
    fn test_now_iso8601() {
        let ts = now_iso8601();
        assert!(ts.contains('T'));
        assert!(ts.ends_with('Z'));
        assert!(ts.starts_with("20")); // valid for 2000-2099
    }
}
