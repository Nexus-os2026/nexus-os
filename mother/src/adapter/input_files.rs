//! M3-E M-1b (rulings M3E-R1..R4): the bounded reads behind the host's
//! page inputs, and the running image's digest. Mother's adapter is the one
//! place that touches the host filesystem; the host keeps only parsing.
//!
//! An input is opened once, read-only, `O_NOFOLLOW | O_NONBLOCK |
//! O_CLOEXEC` (a final symbolic link is refused; a FIFO is never waited on),
//! checked regular by `fstat` on that descriptor, and read from it to at
//! most `cap + 1` bytes; metadata length is never trusted. The digest is
//! over exactly the retained bytes; nothing is reread by path. Parent
//! directory links are followed, and a device is opened before `fstat`
//! refuses it (recorded limitations, M3E-R3).
//!
//! The image is reached through `/proc/self/exe`, which names the file this
//! process was executed from even after its pathname is replaced; argv[0]
//! and pathnames are never used. It is streamed to SHA-256, at most
//! [`IMAGE_MAX_BYTES`], and must cover exactly its `fstat` size.
//!
//! Data only: this reads what the Owner named; it is not a Files capability
//! and grants nothing. Refusals are fixed codes, never content or OS text.

use nix::errno::Errno;
use nix::fcntl::OFlag;
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

/// Why a bounded read was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadRefusal {
    /// Missing, unopenable or unstattable.
    Unavailable,
    /// The final path component is a symbolic link.
    Link,
    /// A directory, FIFO, device or socket.
    NotRegular,
    /// More bytes than the cap.
    TooLarge,
    /// A read error on the opened descriptor.
    Unreadable,
}

/// A file opened without following its final link, checked regular.
pub struct OpenedFile {
    file: File,
    cap: usize,
}

impl fmt::Debug for OpenedFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenedFile")
            .field("cap", &self.cap)
            .finish_non_exhaustive()
    }
}

/// Open `path` for a bounded read of at most `cap` bytes.
pub fn open(path: &Path, cap: usize) -> Result<OpenedFile, ReadRefusal> {
    let flags = OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(flags.bits())
        .open(path)
        .map_err(|e| {
            if e.raw_os_error() == Some(Errno::ELOOP as i32) {
                ReadRefusal::Link
            } else {
                ReadRefusal::Unavailable
            }
        })?;
    let meta = file.metadata().map_err(|_| ReadRefusal::Unavailable)?;
    if !meta.file_type().is_file() {
        return Err(ReadRefusal::NotRegular);
    }
    Ok(OpenedFile { file, cap })
}

impl OpenedFile {
    /// Read at most `cap + 1` bytes from this descriptor.
    pub fn read(self) -> Result<InputBytes, ReadRefusal> {
        let mut bytes = Vec::new();
        (&self.file)
            .take(self.cap as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| ReadRefusal::Unreadable)?;
        if bytes.len() > self.cap {
            return Err(ReadRefusal::TooLarge);
        }
        let sha256 = Sha256::digest(&bytes).into();
        Ok(InputBytes { bytes, sha256 })
    }
}

/// One file's complete bytes and their SHA-256. `Debug` shows neither.
pub struct InputBytes {
    bytes: Vec<u8>,
    sha256: [u8; 32],
}

impl fmt::Debug for InputBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InputBytes")
            .field("bytes", &self.bytes.len())
            .finish_non_exhaustive()
    }
}

impl InputBytes {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn sha256(&self) -> [u8; 32] {
        self.sha256
    }
}

/// The largest image hashed: 1 GiB (ruling M3E-R2 5).
pub const IMAGE_MAX_BYTES: u64 = 1 << 30;

/// The running image's digest could not be established.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageUnavailable;

/// SHA-256 of the executable image this process runs.
pub fn running_image_sha256() -> Result<[u8; 32], ImageUnavailable> {
    #[cfg(test)]
    if let Some(path) = tests::SOURCE.with(|s| s.get()) {
        return hash_image(Path::new(path));
    }
    hash_image(Path::new("/proc/self/exe"))
}

fn hash_image(path: &Path) -> Result<[u8; 32], ImageUnavailable> {
    let file = File::open(path).map_err(|_| ImageUnavailable)?;
    let meta = file.metadata().map_err(|_| ImageUnavailable)?;
    if !meta.file_type().is_file() || meta.len() > IMAGE_MAX_BYTES {
        return Err(ImageUnavailable);
    }
    let mut hasher = Sha256::new();
    let mut reader = (&file).take(IMAGE_MAX_BYTES + 1);
    let mut buffer = vec![0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(_) => return Err(ImageUnavailable),
        };
        total += n as u64;
        hasher.update(&buffer[..n]);
    }
    if total != meta.len() || total > IMAGE_MAX_BYTES {
        return Err(ImageUnavailable);
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
        assert_eq!(running_image_sha256(), Ok(expected), "running image");
        // Missing, a directory and a size-0 /proc file with content (whose
        // stream disagrees with its fstat size) are each unavailable.
        for source in ["/nonexistent/m3e-u1-image", "/", "/proc/self/status"] {
            SOURCE.with(|s| s.set(Some(source)));
            let result = running_image_sha256();
            SOURCE.with(|s| s.set(None));
            assert_eq!(result, Err(ImageUnavailable), "{source}");
        }
    }
}
