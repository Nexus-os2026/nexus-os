// P0 scanner forms copied unchanged except dependency-free hex formatting.
use std::path::{Path, PathBuf};
pub struct Canaries {
    pub c: String,
    pub cl: String,
}
impl Canaries {
    pub fn both(&self) -> [(&str, &str); 2] {
        [("C", &self.c), ("Cl", &self.cl)]
    }
}
const STD64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const URL64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Base64 with padding, in `alphabet`.
pub(crate) fn base64(bytes: &[u8], alphabet: &[u8; 64]) -> String {
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(alphabet[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub(crate) fn base64_std(bytes: &[u8]) -> String {
    base64(bytes, STD64)
}

pub(crate) fn base64_url(bytes: &[u8]) -> String {
    base64(bytes, URL64)
}

/// The base64 characters that depend on `text` alone when it is encoded
/// after `offset` (0, 1 or 2) other bytes: what any larger encoded value
/// that holds `text` contains.
fn base64_core(text: &[u8], offset: usize, alphabet: &[u8; 64]) -> String {
    let mut padded = vec![0u8; offset];
    padded.extend_from_slice(text);
    let encoded = base64(&padded, alphabet);
    let trimmed = encoded.trim_end_matches('=');
    let skip = [0, 2, 3][offset];
    let mut core: Vec<char> = trimmed.chars().skip(skip).collect();
    if !padded.len().is_multiple_of(3) {
        core.pop();
    }
    core.into_iter().collect()
}

pub(crate) fn percent_all(text: &str, upper: bool) -> String {
    text.bytes()
        .map(|b| {
            if upper {
                format!("%{b:02X}")
            } else {
                format!("%{b:02x}")
            }
        })
        .collect()
}

pub(crate) fn json_u(text: &str, upper: bool) -> String {
    text.chars()
        .map(|c| {
            if upper {
                format!("\\u{:04X}", c as u32)
            } else {
                format!("\\u{:04x}", c as u32)
            }
        })
        .collect()
}

/// Four fragments of `text`, each at least 8 characters, that together
/// cover it (overlapping when `text` is shorter than 32).
pub(crate) fn fragments(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let len = n.div_ceil(4).max(8).min(n);
    (0..4)
        .map(|i| {
            let start = if n <= len { 0 } else { (n - len) * i / 3 };
            chars[start..start + len].iter().collect()
        })
        .collect()
}

/// Every form the scanner looks for, named: raw, upper and lower case,
/// hex of the bytes, base64 (standard and URL-safe, at each of the three
/// alignments), percent-encoded, JSON `\u`-escaped, and each fragment of
/// the raw, upper and lower forms.
pub(crate) fn needles(canary: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = vec![
        ("raw".into(), canary.to_string()),
        ("upper".into(), canary.to_uppercase()),
        ("lower".into(), canary.to_lowercase()),
        (
            "hex".into(),
            canary.bytes().map(|b| format!("{b:02x}")).collect(),
        ),
        (
            "HEX".into(),
            canary.bytes().map(|b| format!("{b:02X}")).collect(),
        ),
        ("percent".into(), percent_all(canary, true)),
        ("percent-lower".into(), percent_all(canary, false)),
        ("json-u".into(), json_u(canary, false)),
        ("json-U".into(), json_u(canary, true)),
    ];
    for offset in 0..3 {
        out.push((
            format!("base64+{offset}"),
            base64_core(canary.as_bytes(), offset, STD64),
        ));
        out.push((
            format!("base64url+{offset}"),
            base64_core(canary.as_bytes(), offset, URL64),
        ));
    }
    for (case, text) in [
        ("raw", canary.to_string()),
        ("upper", canary.to_uppercase()),
        ("lower", canary.to_lowercase()),
    ] {
        for (i, fragment) in fragments(&text).into_iter().enumerate() {
            out.push((format!("fragment-{case}-{i}"), fragment));
        }
    }
    out
}

/// What of `canaries` `haystack` holds, as `<where>: <canary>/<form>`.
pub(crate) fn hits_in(haystack: &[u8], place: &str, canaries: &Canaries) -> Vec<String> {
    let text = String::from_utf8_lossy(haystack);
    let mut hits = Vec::new();
    for (name, canary) in canaries.both() {
        for (form, needle) in needles(canary) {
            if text.contains(needle.as_str()) {
                hits.push(format!("{place}: {name}/{form}"));
            }
        }
    }
    hits
}

/// Every file under `dir`, recursively, except the subtree `skip`: a
/// store's `-wal` and `-shm` files are read like any other.
pub(crate) fn files_under(dir: &Path, skip: Option<&Path>) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for path in paths {
        if skip.is_some_and(|s| path == s) {
            continue;
        }
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.is_dir() {
            out.extend(files_under(&path, skip));
        } else if let Ok(bytes) = std::fs::read(&path) {
            out.push((path, bytes));
        }
    }
    out
}

/// The scan of a directory: every file under it but `skip`.
pub(crate) fn scan_dir(dir: &Path, skip: Option<&Path>, canaries: &Canaries) -> Vec<String> {
    files_under(dir, skip)
        .into_iter()
        .flat_map(|(path, bytes)| {
            let place = path
                .strip_prefix(dir)
                .unwrap_or(&path)
                .display()
                .to_string();
            hits_in(&bytes, &place, canaries)
        })
        .collect()
}

/// Scan all Home files except Mother's private blobs and the single exact
/// candidate retained by the broker for same-session undo. Never exempt the
/// staging directory: metadata, extra files, WAL and SHM remain covered.
pub(crate) fn scan_home(
    home: &Path,
    candidate: &str,
    content: &[u8],
    canaries: &Canaries,
) -> Vec<String> {
    let mut retained = 0;
    files_under(home, Some(&home.join("private")))
        .into_iter()
        .flat_map(|(path, bytes)| {
            let relative = path.strip_prefix(home).unwrap();
            let parts: Vec<_> = relative.iter().collect();
            let candidate_payload = parts.len() == 5
                && parts[0] == "broker"
                && parts[1] == "files"
                && parts[2] == "coding-runs"
                && parts[3]
                    .to_str()
                    .is_some_and(|s| uuid::Uuid::parse_str(s).is_ok_and(|id| id.to_string() == s))
                && parts[4] == candidate
                && std::fs::symlink_metadata(&path)
                    .unwrap()
                    .file_type()
                    .is_file()
                && bytes == content;
            if candidate_payload {
                retained += 1;
                assert!(retained <= 1, "only one legitimate retained candidate");
                vec![]
            } else {
                hits_in(&bytes, &relative.display().to_string(), canaries)
            }
        })
        .collect()
}
