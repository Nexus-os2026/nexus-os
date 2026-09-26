//! Private retained directory identity; never a credential or serialized state.
//! `File` owns a close-on-exec Unix fd / non-inheritable Windows handle.
use std::fs::{File, Metadata, OpenOptions};
use std::io;
use std::path::Path;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum IdentityError {
    Changed,
    Unavailable,
}

impl From<io::Error> for IdentityError {
    fn from(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::NotFound | io::ErrorKind::NotADirectory => Self::Changed,
            _ => Self::Unavailable,
        }
    }
}

pub(super) struct DirectoryIdentity {
    handle: File,
}

impl DirectoryIdentity {
    pub(super) fn capture(path: &Path) -> Result<Self, IdentityError> {
        check_path(path)?;
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            use windows_sys::Win32::Storage::FileSystem::{
                FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
            };
            options
                .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE);
        }
        let witness = Self {
            handle: options.open(path)?,
        };
        identity(&witness.handle)?;
        check_path(path)?;
        Ok(witness)
    }

    pub(super) fn validate(&self, path: &Path) -> Result<(), IdentityError> {
        let current = Self::capture(path)?;
        if identity(&self.handle)? != identity(&current.handle)? {
            return Err(IdentityError::Changed);
        }
        Ok(())
    }

    /// Binds an already opened directory handle to this retained identity by
    /// comparing native identities directly; no pathname is re-resolved.
    pub(super) fn validate_handle(&self, handle: &File) -> Result<(), IdentityError> {
        if identity(&self.handle)? != identity(handle)? {
            return Err(IdentityError::Changed);
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn handle(&self) -> &File {
        &self.handle
    }
}

fn check_path(path: &Path) -> Result<(), IdentityError> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !path.is_absolute()
        || !metadata.is_dir()
        || redirected(&metadata)
        || path.canonicalize()? != path
    {
        return Err(IdentityError::Changed);
    }
    Ok(())
}

#[cfg(unix)]
fn redirected(metadata: &Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(windows)]
fn redirected(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(unix)]
fn identity(file: &File) -> Result<(u64, u64), IdentityError> {
    use std::os::unix::fs::MetadataExt;
    let metadata = file.metadata()?;
    if !metadata.is_dir() {
        return Err(IdentityError::Changed);
    }
    Ok((metadata.dev(), metadata.ino()))
}

#[cfg(windows)]
fn identity(file: &File) -> Result<(u64, [u8; 16]), IdentityError> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FileIdInfo, GetFileInformationByHandleEx, FILE_ID_INFO,
    };
    if !file.metadata()?.is_dir() {
        return Err(IdentityError::Changed);
    }
    let mut info = std::mem::MaybeUninit::<FILE_ID_INFO>::uninit();
    // SAFETY: File keeps the handle alive. The output buffer has the exact type
    // and size required by FileIdInfo, and is read only after API success.
    let success = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileIdInfo,
            info.as_mut_ptr().cast(),
            std::mem::size_of::<FILE_ID_INFO>() as u32,
        )
    };
    if success == 0 {
        return Err(IdentityError::Unavailable);
    }
    // SAFETY: the successful call initialized the entire FILE_ID_INFO buffer.
    let info = unsafe { info.assume_init() };
    Ok((info.VolumeSerialNumber, info.FileId.Identifier))
}
