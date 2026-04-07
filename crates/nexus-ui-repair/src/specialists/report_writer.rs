//! Markdown bug report writer. Phase 1.1 stub: computes the target
//! path under `self.root`, runs it through the ACL (the I-2 enforcement
//! gate), and returns the path without actually writing the file. The
//! deterministic-vs-LLM split (v1.1 §5.0) lands in Phase 1.4.

use std::path::PathBuf;

use crate::governance::acl::Acl;

/// A scout bug report. Phase 1.1 keeps this minimal — `bugs` is just a
/// list of free-form strings until the §5.0 contract is implemented.
pub struct BugReport {
    pub page: String,
    pub bugs: Vec<String>,
}

/// Markdown report writer. Owns the root directory under which reports
/// are placed.
pub struct ReportWriter {
    pub root: PathBuf,
}

impl ReportWriter {
    /// Compute the target path for a report and gate it through the
    /// ACL. Phase 1.1 does not write the file — it only verifies that
    /// the path *would* be permitted, then returns it.
    pub fn write(&self, report: &BugReport, acl: &Acl) -> crate::Result<PathBuf> {
        let slug: String = report
            .page
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let target = self.root.join(format!("{}.md", slug));
        acl.check_write(&target)?;
        Ok(target)
    }
}
