//! Closed, typed relative paths and the three immutable run scopes.
//!
//! A [`RelPath`] is data: parsing one grants nothing. It is always relative,
//! has only normal components, never names `.git` (in any letter case) and
//! never uses the staging temp-name prefix. Scopes are frozen when a run is
//! created: [`RunScopes`] has no setters and enforces
//! `WriteScope ⊆ ReadScope` and `ProtectedInputs ⊆ ReadScope`.

use thiserror::Error;

/// Reserved prefix for the staging area's own temporary files. Project paths
/// may not use it, so a leftover temporary file is never a scoped path.
pub(crate) const TEMP_PREFIX: &str = ".nexus-coding-run-tmp-";

const MAX_COMPONENT_BYTES: usize = 255;
const MAX_PATH_BYTES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ScopeError {
    #[error("path must be relative")]
    Absolute,
    #[error("path is empty or has an empty component")]
    Empty,
    #[error("path has a `.` or `..` component")]
    Traversal,
    #[error("path names `.git`")]
    GitMetadata,
    #[error("path has a forbidden character")]
    ForbiddenCharacter,
    #[error("path or component is too long")]
    TooLong,
    #[error("path uses the reserved staging prefix")]
    Reserved,
    #[error("write scope is not within the read scope")]
    WriteOutsideRead,
    #[error("protected inputs are not within the read scope")]
    ProtectedOutsideRead,
    #[error("read scope is empty")]
    EmptyReadScope,
}

/// A validated relative path inside a project or staging root.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RelPath {
    components: Vec<String>,
}

impl RelPath {
    pub fn parse(text: &str) -> Result<Self, ScopeError> {
        if text.starts_with('/') {
            return Err(ScopeError::Absolute);
        }
        if text.len() > MAX_PATH_BYTES {
            return Err(ScopeError::TooLong);
        }
        if text.is_empty() {
            return Err(ScopeError::Empty);
        }
        let components = text
            .split('/')
            .map(|part| validate_component(part).map(str::to_string))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { components })
    }

    pub(crate) fn from_components(components: Vec<String>) -> Result<Self, ScopeError> {
        if components.is_empty() {
            return Err(ScopeError::Empty);
        }
        for part in &components {
            validate_component(part)?;
        }
        let path = Self { components };
        if path.as_string().len() > MAX_PATH_BYTES {
            return Err(ScopeError::TooLong);
        }
        Ok(path)
    }

    pub fn components(&self) -> &[String] {
        &self.components
    }

    /// `/`-joined form, used for manifests and ledger facts.
    pub fn as_string(&self) -> String {
        self.components.join("/")
    }

    /// Whether `self` equals `prefix` or lies beneath it.
    pub fn starts_with(&self, prefix: &RelPath) -> bool {
        self.components.len() >= prefix.components.len()
            && self.components[..prefix.components.len()] == prefix.components[..]
    }

    pub(crate) fn parent_and_name(&self) -> (&[String], &str) {
        let (name, parents) = self
            .components
            .split_last()
            .expect("RelPath always has a component");
        (parents, name)
    }
}

/// Validate one path component: a normal, non-reserved name.
pub(crate) fn validate_component(part: &str) -> Result<&str, ScopeError> {
    if part.is_empty() {
        return Err(ScopeError::Empty);
    }
    if part == "." || part == ".." {
        return Err(ScopeError::Traversal);
    }
    if part.eq_ignore_ascii_case(".git") {
        return Err(ScopeError::GitMetadata);
    }
    if part.len() > MAX_COMPONENT_BYTES {
        return Err(ScopeError::TooLong);
    }
    if part
        .chars()
        .any(|c| c == '/' || c == '\\' || c == '\0' || c.is_control())
    {
        return Err(ScopeError::ForbiddenCharacter);
    }
    if part.starts_with(TEMP_PREFIX) {
        return Err(ScopeError::Reserved);
    }
    Ok(part)
}

/// Whether a directory entry name is Git metadata that is never staged.
pub(crate) fn is_git_metadata(name: &str) -> bool {
    name.eq_ignore_ascii_case(".git")
}

/// One scope entry: a single file, a subtree, or the whole project.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScopeEntry {
    File(RelPath),
    Tree(RelPath),
    WholeProject,
}

/// A frozen set of scope entries.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScopeSet {
    entries: Vec<ScopeEntry>,
}

impl ScopeSet {
    pub fn new(entries: impl IntoIterator<Item = ScopeEntry>) -> Self {
        let mut entries: Vec<_> = entries.into_iter().collect();
        entries.sort();
        entries.dedup();
        Self { entries }
    }

    pub fn entries(&self) -> &[ScopeEntry] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Whether a file path lies within this scope.
    pub fn covers(&self, path: &RelPath) -> bool {
        self.entries.iter().any(|entry| match entry {
            ScopeEntry::File(file) => file == path,
            ScopeEntry::Tree(tree) => path.starts_with(tree),
            ScopeEntry::WholeProject => true,
        })
    }

    /// Whether an entry of another scope lies entirely within this scope.
    fn covers_entry(&self, entry: &ScopeEntry) -> bool {
        match entry {
            ScopeEntry::File(file) => self.covers(file),
            ScopeEntry::Tree(tree) => self.entries.iter().any(|own| match own {
                ScopeEntry::Tree(own) => tree.starts_with(own),
                ScopeEntry::WholeProject => true,
                ScopeEntry::File(_) => false,
            }),
            ScopeEntry::WholeProject => self.entries.contains(&ScopeEntry::WholeProject),
        }
    }

    fn contains_scope(&self, other: &ScopeSet) -> bool {
        other.entries.iter().all(|entry| self.covers_entry(entry))
    }

    /// Unambiguous encoding for hashing and ledger facts.
    pub(crate) fn describe(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|entry| match entry {
                ScopeEntry::File(path) => format!("file:{}", path.as_string()),
                ScopeEntry::Tree(path) => format!("tree:{}", path.as_string()),
                ScopeEntry::WholeProject => "project".to_string(),
            })
            .collect()
    }
}

/// The three immutable scopes of a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunScopes {
    read: ScopeSet,
    write: ScopeSet,
    protected: ScopeSet,
}

impl RunScopes {
    /// Freeze the scopes. Write and protected scopes must lie within the read
    /// scope; the read scope must not be empty.
    pub fn new(read: ScopeSet, write: ScopeSet, protected: ScopeSet) -> Result<Self, ScopeError> {
        if read.is_empty() {
            return Err(ScopeError::EmptyReadScope);
        }
        if !read.contains_scope(&write) {
            return Err(ScopeError::WriteOutsideRead);
        }
        if !read.contains_scope(&protected) {
            return Err(ScopeError::ProtectedOutsideRead);
        }
        Ok(Self {
            read,
            write,
            protected,
        })
    }

    pub fn read(&self) -> &ScopeSet {
        &self.read
    }

    pub fn write(&self) -> &ScopeSet {
        &self.write
    }

    pub fn protected(&self) -> &ScopeSet {
        &self.protected
    }

    /// A path the run may edit: in the write scope and not protected.
    pub fn editable(&self, path: &RelPath) -> bool {
        self.write.covers(path) && !self.protected.covers(path)
    }
}
