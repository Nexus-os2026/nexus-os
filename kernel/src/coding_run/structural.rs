//! The one P1A-002 verifier class: in-process structural verification.
//!
//! It spawns nothing and runs no project code. It re-reads the staging tree
//! through descriptor-anchored handles (it does not trust in-memory state),
//! builds the candidate manifest and checks it against the frozen scopes and
//! the base manifest. A pass means only "the candidate is structurally within
//! policy"; it says nothing about semantic correctness or tests.

use sha2::{Digest, Sha256};

use super::fsops::{DirHandle, EntryKind};
use super::manifest::{put_bytes, Manifest, ManifestEntry, ManifestHash};
use super::scope::{is_git_metadata, validate_component, RelPath, RunScopes};
use super::RunId;

const PROFILE_DOMAIN: &[u8] = b"nexus.coding_run.structural_profile.v1";
const BINDING_DOMAIN: &[u8] = b"nexus.coding_run.structural_binding.v1";

/// Caps and policy of the structural verifier (first-wedge constants).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructuralProfile {
    pub version: u64,
    pub max_files: u64,
    pub max_total_bytes: u64,
    pub max_file_bytes: u64,
}

impl StructuralProfile {
    pub const V1: Self = Self {
        version: 1,
        max_files: 10_000,
        max_total_bytes: 64 * 1024 * 1024,
        max_file_bytes: 8 * 1024 * 1024,
    };

    /// Text policy for edited or created files: valid UTF-8 without NUL bytes.
    pub(crate) fn text_ok(content: &[u8]) -> bool {
        !content.contains(&0) && std::str::from_utf8(content).is_ok()
    }

    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        put_bytes(&mut hasher, PROFILE_DOMAIN);
        hasher.update(self.version.to_be_bytes());
        hasher.update(self.max_files.to_be_bytes());
        hasher.update(self.max_total_bytes.to_be_bytes());
        hasher.update(self.max_file_bytes.to_be_bytes());
        put_bytes(&mut hasher, b"changed-files:utf8-without-nul");
        hasher.finalize().into()
    }
}

/// A structural policy violation found in the candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuralViolation {
    GitMetadataPresent(String),
    SymlinkEntry(String),
    SpecialEntry(String),
    UnscopedName(String),
    UnreadableEntry(String),
    OutsideReadScope(String),
    ChangeOutsideWriteScope(String),
    ProtectedInputChanged(String),
    Deleted(String),
    NotText(String),
    FileCountCapExceeded,
    TotalBytesCapExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuralOutcome {
    Passed,
    Rejected(Vec<StructuralViolation>),
}

/// A structural result bound to the run, the base, the exact candidate bytes
/// and the verifier profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralVerification {
    pub run_id: RunId,
    pub base_manifest_hash: ManifestHash,
    pub candidate_manifest_hash: ManifestHash,
    pub profile_hash: [u8; 32],
    pub outcome: StructuralOutcome,
}

impl StructuralVerification {
    pub fn passed(&self) -> bool {
        self.outcome == StructuralOutcome::Passed
    }

    /// Canonical hash of (run id, base hash, candidate hash, profile hash,
    /// outcome).
    pub fn binding_hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        put_bytes(&mut hasher, BINDING_DOMAIN);
        hasher.update(self.run_id.0.as_bytes());
        hasher.update(self.base_manifest_hash.bytes());
        hasher.update(self.candidate_manifest_hash.bytes());
        hasher.update(self.profile_hash);
        hasher.update([u8::from(self.passed())]);
        hasher.finalize().into()
    }
}

/// Scan the staging tree into a candidate manifest, collecting violations.
pub(crate) fn scan_staging(
    root: &DirHandle,
    profile: &StructuralProfile,
    changed_text: &mut Vec<(RelPath, bool)>,
    base: &Manifest,
) -> (Manifest, Vec<StructuralViolation>) {
    let mut manifest = Manifest::default();
    let mut violations = Vec::new();
    let mut files = 0u64;
    let mut bytes = 0u64;
    walk(
        root,
        &mut Vec::new(),
        profile,
        base,
        &mut manifest,
        &mut violations,
        changed_text,
        &mut files,
        &mut bytes,
    );
    (manifest, violations)
}

#[allow(clippy::too_many_arguments)]
fn walk(
    dir: &DirHandle,
    prefix: &mut Vec<String>,
    profile: &StructuralProfile,
    base: &Manifest,
    manifest: &mut Manifest,
    violations: &mut Vec<StructuralViolation>,
    changed_text: &mut Vec<(RelPath, bool)>,
    files: &mut u64,
    bytes: &mut u64,
) {
    let display = |prefix: &Vec<String>, name: &str| {
        let mut parts = prefix.clone();
        parts.push(name.to_string());
        parts.join("/")
    };
    let names = match dir.entries() {
        Ok(names) => names,
        Err(_) => {
            violations.push(StructuralViolation::UnreadableEntry(prefix.join("/")));
            return;
        }
    };
    for name in names {
        if is_git_metadata(&name) {
            violations.push(StructuralViolation::GitMetadataPresent(display(
                prefix, &name,
            )));
            continue;
        }
        if validate_component(&name).is_err() {
            violations.push(StructuralViolation::UnscopedName(display(prefix, &name)));
            continue;
        }
        match dir.kind(&name) {
            Ok(EntryKind::Directory) => match dir.open_subdir(&name) {
                Ok(child) => {
                    prefix.push(name);
                    walk(
                        &child,
                        prefix,
                        profile,
                        base,
                        manifest,
                        violations,
                        changed_text,
                        files,
                        bytes,
                    );
                    prefix.pop();
                }
                Err(_) => {
                    violations.push(StructuralViolation::UnreadableEntry(display(prefix, &name)))
                }
            },
            Ok(EntryKind::Regular) => {
                let mut components = prefix.clone();
                components.push(name.clone());
                let Ok(path) = RelPath::from_components(components) else {
                    violations.push(StructuralViolation::UnscopedName(display(prefix, &name)));
                    continue;
                };
                match dir.read_regular(&name, profile.max_file_bytes) {
                    Ok(content) => {
                        *files += 1;
                        *bytes += content.len() as u64;
                        let entry = ManifestEntry::of(&content);
                        if base.get(&path) != Some(&entry) {
                            changed_text.push((path.clone(), StructuralProfile::text_ok(&content)));
                        }
                        manifest.insert(path, entry);
                    }
                    Err(_) => {
                        violations.push(StructuralViolation::UnreadableEntry(path.as_string()))
                    }
                }
            }
            Ok(EntryKind::Symlink) => {
                violations.push(StructuralViolation::SymlinkEntry(display(prefix, &name)))
            }
            Ok(EntryKind::Special) => {
                violations.push(StructuralViolation::SpecialEntry(display(prefix, &name)))
            }
            Ok(EntryKind::Missing) | Err(_) => {
                violations.push(StructuralViolation::UnreadableEntry(display(prefix, &name)))
            }
        }
    }
    if *files > profile.max_files
        && !violations.contains(&StructuralViolation::FileCountCapExceeded)
    {
        violations.push(StructuralViolation::FileCountCapExceeded);
    }
    if *bytes > profile.max_total_bytes
        && !violations.contains(&StructuralViolation::TotalBytesCapExceeded)
    {
        violations.push(StructuralViolation::TotalBytesCapExceeded);
    }
}

/// Compare a scanned candidate with the base under the frozen scopes.
pub(crate) fn check_against_base(
    scopes: &RunScopes,
    base: &Manifest,
    candidate: &Manifest,
    changed_text: &[(RelPath, bool)],
    violations: &mut Vec<StructuralViolation>,
) {
    for (path, entry) in candidate.entries() {
        let text = path.as_string();
        if !scopes.read().covers(path) {
            violations.push(StructuralViolation::OutsideReadScope(text));
            continue;
        }
        if base.get(path) != Some(entry) {
            if scopes.protected().covers(path) {
                violations.push(StructuralViolation::ProtectedInputChanged(text));
            } else if !scopes.write().covers(path) {
                violations.push(StructuralViolation::ChangeOutsideWriteScope(text));
            }
        }
    }
    for path in base.entries().keys() {
        if candidate.get(path).is_none() {
            let text = path.as_string();
            if scopes.protected().covers(path) {
                violations.push(StructuralViolation::ProtectedInputChanged(text));
            } else {
                violations.push(StructuralViolation::Deleted(text));
            }
        }
    }
    for (path, is_text) in changed_text {
        if !is_text {
            violations.push(StructuralViolation::NotText(path.as_string()));
        }
    }
}
