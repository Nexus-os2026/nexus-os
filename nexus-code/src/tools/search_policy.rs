//! Project-local privacy filtering over Nexus-owned traversal.
//!
//! Precedence: .rgignore > .ignore > .gitignore > .git/info/exclude;
//! deeper files win within a source type, and the last matching rule wins.
//! Ignored directories are pruned, so a rule inside one cannot reopen it.
//! Unlike ambient rg discovery, policies apply even without a Git repository,
//! never come from above W/global configuration/external gitdirs, and cannot
//! be overridden by an explicit search path or the search include filter.
//! Symlinked, unreadable or invalid local policy fails closed. Policy file
//! reads themselves must also pass blocked_paths/max_file_scope enforcement.
use super::{traversal, ToolContext};
use crate::error::NxError;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};

#[derive(Clone, Default)]
struct Policy {
    sources: [Vec<Gitignore>; 4],
}

fn policy_error(reason: &str) -> NxError {
    NxError::CapabilityDenied {
        capability: "path.policy".into(),
        reason: format!("project-local ignore policy {reason}"),
    }
}

fn policy_resolution_error(error: NxError) -> NxError {
    if error.is_filesystem_denial() {
        error
    } else {
        policy_error("could not be safely resolved")
    }
}

fn regular_metadata(ctx: &ToolContext, path: &Path) -> Result<Option<std::fs::Metadata>, NxError> {
    // Resolve before inspecting an effective target; do not follow policy
    // symlinks even when their target happens to be inside the workspace.
    let resolved = ctx
        .resolve_contained_path(path)
        .map_err(policy_resolution_error)?;
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err(ToolContext::workspace_denied(
            "symlinked project ignore sources are not allowed",
        )),
        Ok(meta) => {
            debug_assert_eq!(resolved, path);
            Ok(Some(meta))
        }
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(_) => Err(policy_error("could not be inspected")),
    }
}

impl Policy {
    fn load(&mut self, ctx: &ToolContext, directory: &Path) -> Result<(), NxError> {
        let directory = ctx
            .resolve_contained_path(directory)
            .map_err(policy_resolution_error)?;
        // A .git *file* is not followed: its gitdir could be outside authority.
        let git = directory.join(".git");
        if regular_metadata(ctx, &git)?.is_some_and(|m| m.is_dir()) {
            let info = git.join("info");
            if regular_metadata(ctx, &info)?.is_some_and(|m| m.is_dir()) {
                self.add(ctx, &directory, &info.join("exclude"), 0)?;
            }
        }
        for (name, rank) in [(".gitignore", 1), (".ignore", 2), (".rgignore", 3)] {
            self.add(ctx, &directory, &directory.join(name), rank)?;
        }
        Ok(())
    }

    fn add(
        &mut self,
        ctx: &ToolContext,
        base: &Path,
        file: &Path,
        rank: usize,
    ) -> Result<(), NxError> {
        let Some(meta) = regular_metadata(ctx, file)? else {
            return Ok(());
        };
        if !meta.is_file() {
            return Err(policy_error("source must be a regular file"));
        }
        // No WalkBuilder, Gitignore::global, build_global, or automatic add().
        // The matcher receives only contents Nexus has authorized to read.
        let file = ctx
            .resolve_workspace_path(file)
            .map_err(policy_resolution_error)?;
        let contents = std::fs::read_to_string(&file)
            .map_err(|_| policy_error("could not be read as text"))?;
        let mut builder = GitignoreBuilder::new(base);
        for line in contents.trim_start_matches('\u{feff}').lines() {
            builder
                .add_line(Some(file.clone()), line)
                .map_err(|_| policy_error("could not be parsed"))?;
        }
        self.sources[rank].push(
            builder
                .build()
                .map_err(|_| policy_error("could not be compiled"))?,
        );
        Ok(())
    }

    fn allows(&self, path: &Path, is_dir: bool, explicit: bool) -> bool {
        for source in self.sources.iter().rev() {
            for matcher in source.iter().rev() {
                match matcher.matched(path, is_dir) {
                    ignore::Match::Ignore(_) => return false,
                    ignore::Match::Whitelist(_) => return true,
                    ignore::Match::None => {}
                }
            }
        }
        explicit || !hidden(path)
    }
}

fn hidden(path: &Path) -> bool {
    if path
        .file_name()
        .is_some_and(|n| n.to_string_lossy().starts_with('.'))
    {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if path.metadata().is_ok_and(|m| m.file_attributes() & 2 != 0) {
            return true;
        }
    }
    false
}

// Explicit file arguments disable rg's recursive binary filtering. Use the
// same full-file NUL check for both backends, even if a match precedes the NUL.
// No file bytes or canonical paths are included in these errors.
fn text_file(ctx: &ToolContext, path: &Path) -> Result<bool, NxError> {
    let path = ctx.resolve_workspace_path(path)?;
    let mut file = std::fs::File::open(path)
        .map_err(|_| NxError::Io(std::io::Error::other("search candidate could not be read")))?;
    let mut buffer = [0; 16 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|_| {
            NxError::Io(std::io::Error::other("search candidate could not be read"))
        })?;
        if count == 0 {
            return Ok(true);
        }
        if buffer[..count].contains(&0) {
            return Ok(false);
        }
    }
}

pub(super) fn files(ctx: &ToolContext, base: &Path) -> Result<Vec<PathBuf>, NxError> {
    let root = ctx.workspace_root()?;
    let base = ctx.resolve_contained_path(base)?;
    let mut policy = Policy::default();
    policy.load(ctx, &root)?;
    // Load ancestors only from W downward, including for an explicit subpath.
    // Validate/filter each ancestor before reading policies below it.
    let mut ancestor = root.clone();
    for component in base
        .strip_prefix(&root)
        .expect("contained path")
        .components()
    {
        ancestor.push(component);
        let ancestor = ctx.resolve_contained_path(&ancestor)?;
        let is_dir = ancestor.is_dir();
        if !policy.allows(&ancestor, is_dir, true) {
            return Ok(vec![]);
        }
        ctx.resolve_workspace_path(&ancestor)?;
        if is_dir {
            policy.load(ctx, &ancestor)?;
        }
    }
    if base.is_file() {
        return Ok(if text_file(ctx, &base)? {
            vec![base]
        } else {
            vec![]
        });
    }
    let mut files = Vec::new();
    let mut directories = vec![(base, policy)];
    while let Some((directory, policy)) = directories.pop() {
        for entry in traversal::contained_entries(ctx, &directory)? {
            let path = ctx.resolve_contained_path(&entry.path())?;
            let is_dir = entry.file_type()?.is_dir();
            if !policy.allows(&path, is_dir, false) {
                continue;
            }
            let path = ctx.resolve_workspace_path(&path)?;
            if is_dir {
                let mut child = policy.clone();
                child.load(ctx, &path)?;
                directories.push((path, child));
            } else if text_file(ctx, &path)? {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}
