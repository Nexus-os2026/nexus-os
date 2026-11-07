//! M3-E M-1: the SHA-256 of the running host image (design step 0).
//!
//! The image is reached through `/proc/self/exe`, which the kernel resolves
//! to the file this process was executed from, even after its pathname is
//! replaced or removed; argv[0] and `current_exe()`'s pathname are never
//! used. The digest is streamed from that one descriptor, bounded by
//! [`SELF_IDENTITY_MAX_BYTES`], and must cover exactly the size fstat
//! reports. Anything else is [`SelfIdentityUnavailable`]: no made-up hash.
//! Linux-only. Data-only (M3E-R2 1(a)): not called from hands yet.

use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{ErrorKind, Read};
use std::path::Path;

/// The largest image hashed: 1 GiB (ruling M3E-R2 5).
pub const SELF_IDENTITY_MAX_BYTES: u64 = 1 << 30;

const SELF_EXE: &str = "/proc/self/exe";

/// The running image's digest could not be established.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelfIdentityUnavailable;

impl SelfIdentityUnavailable {
    pub fn code(&self) -> &'static str {
        "self_identity_unavailable"
    }
}

/// SHA-256 of the executable image this process runs.
pub fn running_binary_sha256() -> Result<[u8; 32], SelfIdentityUnavailable> {
    #[cfg(test)]
    if let Some(path) = tests::SOURCE.with(|s| s.get()) {
        return hash_image(Path::new(path));
    }
    hash_image(Path::new(SELF_EXE))
}

fn hash_image(path: &Path) -> Result<[u8; 32], SelfIdentityUnavailable> {
    let file = File::open(path).map_err(|_| SelfIdentityUnavailable)?;
    let meta = file.metadata().map_err(|_| SelfIdentityUnavailable)?;
    if !meta.file_type().is_file() || meta.len() > SELF_IDENTITY_MAX_BYTES {
        return Err(SelfIdentityUnavailable);
    }
    let mut hasher = Sha256::new();
    let mut reader = (&file).take(SELF_IDENTITY_MAX_BYTES + 1);
    let mut buffer = vec![0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(_) => return Err(SelfIdentityUnavailable),
        };
        total += n as u64;
        hasher.update(&buffer[..n]);
    }
    if total != meta.len() || total > SELF_IDENTITY_MAX_BYTES {
        return Err(SelfIdentityUnavailable);
    }
    Ok(hasher.finalize().into())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::cell::Cell;

    thread_local! {
        pub(crate) static SOURCE: Cell<Option<&'static str>> = const { Cell::new(None) };
    }

    #[test]
    fn m3e_u1_running_binary_identity_unavailable() {
        let image = std::fs::read(std::env::current_exe().unwrap()).unwrap();
        let expected: [u8; 32] = Sha256::digest(&image).into();
        assert_eq!(running_binary_sha256(), Ok(expected), "running image");
        // Missing, a directory and a size-0 /proc file with content (whose
        // stream disagrees with its fstat size) are each unavailable.
        for source in ["/nonexistent/m3e-u1-image", "/", "/proc/self/status"] {
            SOURCE.with(|s| s.set(Some(source)));
            let result = running_binary_sha256();
            SOURCE.with(|s| s.set(None));
            assert_eq!(result, Err(SelfIdentityUnavailable), "{source}");
        }
        assert_eq!(SelfIdentityUnavailable.code(), "self_identity_unavailable");
    }
}
