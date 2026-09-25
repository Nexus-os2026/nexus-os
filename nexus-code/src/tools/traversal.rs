//! Filesystem enumeration for ordinary tools. Never follows descendant symlinks.
use super::ToolContext;
use crate::error::NxError;
use std::path::{Path, PathBuf};

pub(super) fn entries(ctx: &ToolContext, path: &Path) -> Result<Vec<std::fs::DirEntry>, NxError> {
    let path = ctx.resolve_workspace_path(path)?;
    contained_entries(ctx, &path)
}

pub(super) fn contained_entries(
    ctx: &ToolContext,
    path: &Path,
) -> Result<Vec<std::fs::DirEntry>, NxError> {
    let path = ctx.resolve_contained_path(path)?;
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        // No target is opened for symlinks or special files. Explicitly requested
        // paths are resolved separately by the tool before traversal starts.
        if kind.is_file() || kind.is_dir() {
            entries.push(entry);
        }
    }
    entries.sort_by_key(|e| e.file_name());
    Ok(entries)
}

pub(super) fn paths(
    ctx: &ToolContext,
    base: &Path,
    skip_hidden: bool,
    include_directories: bool,
) -> Result<Vec<PathBuf>, NxError> {
    let base = ctx.resolve_workspace_path(base)?;
    if base.is_file() {
        return Ok(vec![base]);
    }
    let mut files = if include_directories {
        vec![base.clone()]
    } else {
        Vec::new()
    };
    let mut directories = vec![base];
    while let Some(directory) = directories.pop() {
        for entry in entries(ctx, &directory)? {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if skip_hidden
                && (name.starts_with('.')
                    || matches!(name.as_ref(), "target" | "node_modules" | "__pycache__"))
            {
                continue;
            }
            let path = ctx.resolve_workspace_path(&entry.path())?;
            if entry.file_type()?.is_dir() {
                if include_directories {
                    files.push(path.clone());
                }
                directories.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}
