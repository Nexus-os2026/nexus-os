//! The owner's review of a verified candidate (Phase One, P1-05).
//!
//! A [`Review`] is computed by the backend from the base manifest and the
//! bytes captured when each file was first edited, and from the verified
//! candidate re-read out of staging (whose manifest must still hash to the
//! verified candidate hash). It never shells out to `git diff`.
//!
//! A review is data. It grants nothing; it names exactly what an owner
//! approval would bind to ([`ReviewBinding`]: run, base manifest hash,
//! candidate manifest hash, structural profile hash). Rendering is bounded
//! per file and in total, so a model-controlled candidate cannot produce an
//! unbounded diff.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use super::manifest::{put_bytes, ManifestEntry, ManifestHash};
use super::scope::RelPath;
use super::RunId;

const BINDING_DOMAIN: &[u8] = b"nexus.coding_run.review_binding.v1";

/// Largest file (either side) that is diffed as text.
pub const MAX_DIFF_FILE_BYTES: usize = 256 * 1024;
/// Largest rendered diff for one file.
pub const MAX_DIFF_BYTES_PER_FILE: usize = 64 * 1024;
/// Largest rendered diff for the whole review.
pub const MAX_DIFF_BYTES_TOTAL: usize = 512 * 1024;
/// Largest line-comparison table (lines × lines) before a changed region is
/// shown as a whole removal and addition.
const MAX_LCS_CELLS: usize = 2_000_000;
const CONTEXT_LINES: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Create,
    Replace,
    Delete,
}

/// Why a file's textual diff is not shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffOmitted {
    NotText,
    TooLarge,
    BudgetExhausted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextDiff {
    /// A unified diff; `truncated` if cut at the per-file or total bound.
    Unified {
        text: String,
        truncated: bool,
    },
    Omitted(DiffOmitted),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    pub path: RelPath,
    pub kind: ChangeKind,
    pub old: Option<ManifestEntry>,
    pub new: Option<ManifestEntry>,
    pub diff: TextDiff,
}

/// Exactly what an owner approval binds to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReviewBinding {
    pub run_id: RunId,
    pub base_manifest_hash: ManifestHash,
    pub candidate_manifest_hash: ManifestHash,
    pub profile_hash: [u8; 32],
}

impl ReviewBinding {
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        put_bytes(&mut hasher, BINDING_DOMAIN);
        hasher.update(self.run_id.0.as_bytes());
        hasher.update(self.base_manifest_hash.bytes());
        hasher.update(self.candidate_manifest_hash.bytes());
        hasher.update(self.profile_hash);
        hasher.finalize().into()
    }
}

/// A backend-computed, bounded review of a verified candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Review {
    pub binding: ReviewBinding,
    pub changes: Vec<FileChange>,
}

impl Review {
    pub fn count(&self, kind: ChangeKind) -> usize {
        self.changes.iter().filter(|c| c.kind == kind).count()
    }
}

/// Text made safe to display: control characters (other than line breaks
/// and tabs in multi-line text) and invisible or direction-changing
/// formatting characters (bidirectional overrides, isolates and marks,
/// zero-width characters, the byte-order mark) are shown as visible
/// `⟨U+XXXX⟩` escapes, so a file name or candidate line cannot display as
/// something other than what it is.
pub fn display_safe(text: &str, multiline: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        let invisible = matches!(
            c,
            '\u{061C}'
                | '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{2069}'
                | '\u{FEFF}'
        );
        let kept_control = multiline && (c == '\n' || c == '\t');
        if invisible || (c.is_control() && !kept_control) {
            out.push_str(&format!("⟨U+{:04X}⟩", c as u32));
        } else {
            out.push(c);
        }
    }
    out
}

/// Build the review. `old` holds the base bytes of changed base files and
/// `new` the candidate bytes of changed files; both were hash-checked by the
/// caller against the base and candidate manifests.
pub(crate) fn build(
    binding: ReviewBinding,
    base: &BTreeMap<RelPath, ManifestEntry>,
    candidate: &BTreeMap<RelPath, ManifestEntry>,
    old: &BTreeMap<RelPath, Vec<u8>>,
    new: &BTreeMap<RelPath, Vec<u8>>,
) -> Review {
    let mut changes = Vec::new();
    let mut budget = MAX_DIFF_BYTES_TOTAL;
    let mut paths: Vec<&RelPath> = base.keys().chain(candidate.keys()).collect();
    paths.sort();
    paths.dedup();
    for path in paths {
        let (before, after) = (base.get(path), candidate.get(path));
        let kind = match (before, after) {
            (Some(a), Some(b)) if a == b => continue,
            (Some(_), Some(_)) => ChangeKind::Replace,
            (None, Some(_)) => ChangeKind::Create,
            (Some(_), None) => ChangeKind::Delete,
            (None, None) => continue,
        };
        let empty = Vec::new();
        let old_bytes = if kind == ChangeKind::Create {
            Some(&empty)
        } else {
            old.get(path)
        };
        let new_bytes = if kind == ChangeKind::Delete {
            Some(&empty)
        } else {
            new.get(path)
        };
        let diff = match (old_bytes, new_bytes) {
            (Some(a), Some(b)) => render(path, a, b, &mut budget),
            _ => TextDiff::Omitted(DiffOmitted::NotText),
        };
        changes.push(FileChange {
            path: path.clone(),
            kind,
            old: before.cloned(),
            new: after.cloned(),
            diff,
        });
    }
    Review { binding, changes }
}

fn render(path: &RelPath, old: &[u8], new: &[u8], budget: &mut usize) -> TextDiff {
    if old.len() > MAX_DIFF_FILE_BYTES || new.len() > MAX_DIFF_FILE_BYTES {
        return TextDiff::Omitted(DiffOmitted::TooLarge);
    }
    let (Ok(old), Ok(new)) = (std::str::from_utf8(old), std::str::from_utf8(new)) else {
        return TextDiff::Omitted(DiffOmitted::NotText);
    };
    if *budget == 0 {
        return TextDiff::Omitted(DiffOmitted::BudgetExhausted);
    }
    let limit = MAX_DIFF_BYTES_PER_FILE.min(*budget);
    let (text, truncated) = unified(&path.as_string(), old, new, limit);
    *budget -= text.len();
    TextDiff::Unified { text, truncated }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Keep,
    Remove,
    Add,
}

/// Line operations turning `a` into `b`: common prefix and suffix, and an
/// LCS of the middle when it is small enough (otherwise the middle is one
/// removal followed by one addition, still an exact diff).
fn line_ops(a: &[&str], b: &[&str]) -> Vec<(Op, usize, usize)> {
    let prefix = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (am, bm) = (&a[prefix..a.len() - suffix], &b[prefix..b.len() - suffix]);
    let mut ops: Vec<(Op, usize, usize)> = (0..prefix).map(|i| (Op::Keep, i, i)).collect();
    let (n, m) = (am.len(), bm.len());
    if n.saturating_mul(m) <= MAX_LCS_CELLS && n > 0 && m > 0 {
        // lcs[i][j] = LCS length of am[i..], bm[j..].
        let mut lcs = vec![0u32; (n + 1) * (m + 1)];
        let at = |i: usize, j: usize| i * (m + 1) + j;
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                lcs[at(i, j)] = if am[i] == bm[j] {
                    lcs[at(i + 1, j + 1)] + 1
                } else {
                    lcs[at(i + 1, j)].max(lcs[at(i, j + 1)])
                };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < n || j < m {
            if i < n && j < m && am[i] == bm[j] {
                ops.push((Op::Keep, prefix + i, prefix + j));
                i += 1;
                j += 1;
            } else if i < n && (j == m || lcs[at(i + 1, j)] >= lcs[at(i, j + 1)]) {
                ops.push((Op::Remove, prefix + i, prefix + j));
                i += 1;
            } else {
                ops.push((Op::Add, prefix + i, prefix + j));
                j += 1;
            }
        }
    } else {
        ops.extend((0..n).map(|i| (Op::Remove, prefix + i, prefix)));
        ops.extend((0..m).map(|j| (Op::Add, prefix + n, prefix + j)));
    }
    ops.extend((0..suffix).map(|k| (Op::Keep, a.len() - suffix + k, b.len() - suffix + k)));
    ops
}

/// A unified diff with three context lines, cut at `limit` bytes.
fn unified(path: &str, old: &str, new: &str, limit: usize) -> (String, bool) {
    let a: Vec<&str> = old.split_inclusive('\n').collect();
    let b: Vec<&str> = new.split_inclusive('\n').collect();
    let ops = line_ops(&a, &b);
    let mut out = format!("--- a/{path}\n+++ b/{path}\n");
    let changed: Vec<usize> = ops
        .iter()
        .enumerate()
        .filter(|(_, op)| op.0 != Op::Keep)
        .map(|(k, _)| k)
        .collect();
    let mut k = 0;
    while k < changed.len() {
        // Group changes whose context overlaps into one hunk.
        let start = changed[k].saturating_sub(CONTEXT_LINES);
        let mut end = changed[k];
        while k < changed.len() && changed[k] <= end + 2 * CONTEXT_LINES {
            end = changed[k];
            k += 1;
        }
        let end = (end + CONTEXT_LINES + 1).min(ops.len());
        let hunk = &ops[start..end];
        let old_start = hunk.iter().find(|o| o.0 != Op::Add).map(|o| o.1);
        let new_start = hunk.iter().find(|o| o.0 != Op::Remove).map(|o| o.2);
        let old_len = hunk.iter().filter(|o| o.0 != Op::Add).count();
        let new_len = hunk.iter().filter(|o| o.0 != Op::Remove).count();
        out.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            old_start.map_or(hunk[0].1, |s| s + 1),
            old_len,
            new_start.map_or(hunk[0].2, |s| s + 1),
            new_len
        ));
        for (op, i, j) in hunk {
            let (sign, line) = match op {
                Op::Keep => (' ', a[*i]),
                Op::Remove => ('-', a[*i]),
                Op::Add => ('+', b[*j]),
            };
            out.push(sign);
            out.push_str(line);
            if !line.ends_with('\n') {
                out.push_str("\n\\ No newline at end of file\n");
            }
            if out.len() > limit {
                let mut cut = limit;
                while !out.is_char_boundary(cut) {
                    cut -= 1;
                }
                out.truncate(cut);
                return (out, true);
            }
        }
    }
    (out, false)
}

#[cfg(test)]
pub(crate) fn unified_for_test(path: &str, old: &str, new: &str, limit: usize) -> (String, bool) {
    unified(path, old, new, limit)
}
