//! P0-002C4D2 build-time manifest generation for the packaged Builder
//! toolchain. Compiled into the desktop build script (and unit tests only).
//!
//! It accepts only an assembled tree the runtime verifier would accept for
//! the build target: regular files and directories (no links, reparse points,
//! special files or empty directories) under the shared contract, containing
//! the Nexus Node executable and entry, no npm, and native binaries only where
//! expected and only for the target OS and architecture. The manifest is
//! rendered from the exact bytes present.
use super::contract::{node_executable, valid_listing, ENTRY_MODULE};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::io::Read;
use std::path::Path;

/// The manifest schema this generator emits (the verifier's `SCHEMA`).
pub(super) const SCHEMA: u32 = 1;

/// The embedded manifest of a build without a packaged toolchain.
pub(super) const ABSENT: &str =
    "const PRODUCTION_MANIFEST: Option<&ToolchainManifest<'static>> = None;\n";

// Header bytes read to classify a file as a native binary.
const HEADER_BYTES: usize = 4096;

/// The release targets a packaged toolchain exists for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Target {
    LinuxX64Gnu,
    WindowsX64Msvc,
    MacosArm64,
}

impl Target {
    /// From Cargo's `CARGO_CFG_TARGET_{OS,ARCH,ENV}`.
    pub(super) fn from_cargo(os: &str, arch: &str, env: &str) -> Option<Self> {
        match (os, arch, env) {
            ("linux", "x86_64", "gnu") => Some(Self::LinuxX64Gnu),
            ("windows", "x86_64", "msvc") => Some(Self::WindowsX64Msvc),
            ("macos", "aarch64", "") => Some(Self::MacosArm64),
            _ => None,
        }
    }

    /// The verifier's target identity (`os`, `arch`, `env`).
    pub(super) fn identity(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::LinuxX64Gnu => ("linux", "x86_64", "gnu"),
            Self::WindowsX64Msvc => ("windows", "x86_64", "msvc"),
            Self::MacosArm64 => ("macos", "aarch64", "none"),
        }
    }

    fn node(self) -> &'static str {
        node_executable(self == Self::WindowsX64Msvc)
    }
}

pub(super) struct PackagedFile {
    pub(super) path: String,
    pub(super) size: u64,
    pub(super) sha256: [u8; 32],
}

/// The complete checked manifest source for an assembled tree.
pub(super) fn generate(root: &Path, target: Target) -> Result<String, String> {
    let files = collect(root)?;
    check_target(root, &files, target)?;
    Ok(render(&files, target))
}

/// Every regular file under `root`, sorted by path bytes, with exact sizes
/// and SHA-256 digests, satisfying the shared manifest contract.
pub(super) fn collect(root: &Path) -> Result<Vec<PackagedFile>, String> {
    let metadata = std::fs::symlink_metadata(root).map_err(|e| format!("toolchain root: {e}"))?;
    if !metadata.is_dir() || is_link(&metadata) {
        return Err("toolchain root is not a plain directory".into());
    }
    let mut files = Vec::new();
    walk(root, "", &mut files)?;
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let listing: Vec<(&str, u64)> = files.iter().map(|f| (f.path.as_str(), f.size)).collect();
    if !valid_listing(&listing) {
        return Err("toolchain tree violates the manifest grammar or limits".into());
    }
    Ok(files)
}

fn walk(dir: &Path, prefix: &str, files: &mut Vec<PackagedFile>) -> Result<(), String> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{prefix:?}: {e}"))? {
        let name = entry.map_err(|e| format!("{prefix:?}: {e}"))?.file_name();
        names.push(
            name.into_string()
                .map_err(|_| format!("non-UTF-8 name under {prefix:?}"))?,
        );
    }
    if names.is_empty() {
        return Err(format!("empty directory {prefix:?}"));
    }
    names.sort();
    for name in names {
        let relative = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        let path = dir.join(&name);
        let metadata =
            std::fs::symlink_metadata(&path).map_err(|e| format!("{relative:?}: {e}"))?;
        if is_link(&metadata) {
            return Err(format!("link or reparse point {relative:?}"));
        } else if metadata.is_dir() {
            walk(&path, &relative, files)?;
        } else if metadata.is_file() {
            let (size, sha256) =
                digest(&path, metadata.len()).map_err(|e| format!("{relative:?}: {e}"))?;
            files.push(PackagedFile {
                path: relative,
                size,
                sha256,
            });
        } else {
            return Err(format!("unsupported entry {relative:?}"));
        }
    }
    Ok(())
}

fn is_link(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    metadata.file_type().is_symlink()
}

fn digest(path: &Path, expected: u64) -> std::io::Result<(u64, [u8; 32])> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    let mut size = 0u64;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        size += read as u64;
        hasher.update(&buffer[..read]);
    }
    if size != expected {
        return Err(std::io::Error::other("file changed while hashing"));
    }
    Ok((size, hasher.finalize().into()))
}

/// Target-specific closure rules: the Node executable and entry present, no
/// npm, and every native binary (the Node executable and `.node` addons only)
/// built for the target.
pub(super) fn check_target(
    root: &Path,
    files: &[PackagedFile],
    target: Target,
) -> Result<(), String> {
    for required in [target.node(), ENTRY_MODULE] {
        if !files.iter().any(|file| file.path == required) {
            return Err(format!("{required} missing"));
        }
    }
    for file in files {
        let components = file.path.split('/');
        if components
            .clone()
            .any(|c| matches!(c, "npm" | "npx" | "corepack"))
        {
            return Err(format!("package manager in toolchain: {}", file.path));
        }
        if target == Target::LinuxX64Gnu && components.clone().any(|c| c.contains("musl")) {
            return Err(format!("musl artifact for a glibc target: {}", file.path));
        }
        let header = header(root, &file.path).map_err(|e| format!("{}: {e}", file.path))?;
        let binary = Binary::classify(&header);
        let role = if file.path == target.node() {
            Role::Executable
        } else if file.path.ends_with(".node") || binary != Binary::Other {
            Role::Addon
        } else {
            continue;
        };
        if !binary.fits(target, role) {
            return Err(format!(
                "native binary not built for the target: {}",
                file.path
            ));
        }
    }
    Ok(())
}

fn header(root: &Path, relative: &str) -> std::io::Result<Vec<u8>> {
    let path = relative
        .split('/')
        .fold(root.to_path_buf(), |p, c| p.join(c));
    let mut header = Vec::with_capacity(HEADER_BYTES);
    std::fs::File::open(path)?
        .take(HEADER_BYTES as u64)
        .read_to_end(&mut header)?;
    Ok(header)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    Executable,
    Addon,
}

/// The native-binary facts the target rules need.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Binary {
    /// 64-bit little-endian ELF: `e_machine`, `e_type`.
    Elf {
        machine: u16,
        kind: u16,
    },
    /// PE32+: `Machine`, DLL characteristic.
    Pe {
        machine: u16,
        dll: bool,
    },
    /// Thin 64-bit little-endian Mach-O: `cputype`, `filetype`.
    MachO {
        cpu: u32,
        kind: u32,
    },
    /// Universal Mach-O: every slice `cputype`.
    Universal {
        cpus: Vec<u32>,
    },
    /// Any other native format (32-bit, big-endian, unknown machine layout).
    Foreign,
    Other,
}

const EM_X86_64: u16 = 62;
const ET_EXEC: u16 = 2;
const ET_DYN: u16 = 3;
const IMAGE_FILE_MACHINE_AMD64: u16 = 0x8664;
const CPU_TYPE_ARM64: u32 = 0x0100_000c;
const MH_EXECUTE: u32 = 2;
const MH_DYLIB: u32 = 6;
const MH_BUNDLE: u32 = 8;

impl Binary {
    fn classify(h: &[u8]) -> Self {
        let u16le = |at: usize| h.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
        let u32le = |at: usize| {
            h.get(at..at + 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        };
        let u32be = |at: usize| {
            h.get(at..at + 4)
                .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
        };
        match h.get(..4) {
            Some([0x7f, b'E', b'L', b'F']) => match (h.get(4), h.get(5), u16le(16), u16le(18)) {
                (Some(2), Some(1), Some(kind), Some(machine)) => Self::Elf { machine, kind },
                _ => Self::Foreign,
            },
            Some([0xcf, 0xfa, 0xed, 0xfe]) => match (u32le(4), u32le(12)) {
                (Some(cpu), Some(kind)) => Self::MachO { cpu, kind },
                _ => Self::Foreign,
            },
            Some(
                [0xce, 0xfa, 0xed, 0xfe] | [0xfe, 0xed, 0xfa, 0xce] | [0xfe, 0xed, 0xfa, 0xcf],
            ) => Self::Foreign,
            Some([0xca, 0xfe, 0xba, 0xbe] | [0xca, 0xfe, 0xba, 0xbf]) => {
                let stride = if h[3] == 0xbf { 32 } else { 20 };
                let count = u32be(4).unwrap_or(0) as usize;
                // A Java class file shares this magic; its "count" is a version.
                if count == 0 || count > 16 {
                    return Self::Other;
                }
                (0..count)
                    .map(|i| u32be(8 + i * stride))
                    .collect::<Option<Vec<_>>>()
                    .map_or(Self::Foreign, |cpus| Self::Universal { cpus })
            }
            Some([b'M', b'Z', ..]) => {
                let Some(offset) = u32le(0x3c).map(|o| o as usize) else {
                    return Self::Other;
                };
                if h.get(offset..offset + 4) != Some(b"PE\0\0") {
                    return Self::Other;
                }
                match (u16le(offset + 4), u16le(offset + 22), u16le(offset + 24)) {
                    (Some(machine), Some(characteristics), Some(0x20b)) => Self::Pe {
                        machine,
                        dll: characteristics & 0x2000 != 0,
                    },
                    _ => Self::Foreign,
                }
            }
            _ => Self::Other,
        }
    }

    fn fits(&self, target: Target, role: Role) -> bool {
        match (target, role, self) {
            (Target::LinuxX64Gnu, Role::Executable, Self::Elf { machine, kind }) => {
                *machine == EM_X86_64 && (*kind == ET_EXEC || *kind == ET_DYN)
            }
            (Target::LinuxX64Gnu, Role::Addon, Self::Elf { machine, kind }) => {
                *machine == EM_X86_64 && *kind == ET_DYN
            }
            (Target::WindowsX64Msvc, role, Self::Pe { machine, dll }) => {
                *machine == IMAGE_FILE_MACHINE_AMD64 && *dll == (role == Role::Addon)
            }
            (Target::MacosArm64, Role::Executable, Self::MachO { cpu, kind }) => {
                *cpu == CPU_TYPE_ARM64 && *kind == MH_EXECUTE
            }
            (Target::MacosArm64, Role::Addon, Self::MachO { cpu, kind }) => {
                *cpu == CPU_TYPE_ARM64 && (*kind == MH_BUNDLE || *kind == MH_DYLIB)
            }
            (Target::MacosArm64, Role::Addon, Self::Universal { cpus }) => {
                cpus.contains(&CPU_TYPE_ARM64)
            }
            _ => false,
        }
    }
}

/// The embedded manifest source (paths are contract-checked ASCII).
pub(super) fn render(files: &[PackagedFile], target: Target) -> String {
    let (os, arch, env) = target.identity();
    let mut out = String::with_capacity(256 + files.len() * 240);
    out.push_str(
        "// @generated by the nexus-desktop-backend build script from the assembled\n\
         // packaged Builder toolchain (P0-002C4D2). Do not edit.\n",
    );
    let _ = writeln!(
        out,
        "const PRODUCTION_MANIFEST: Option<&ToolchainManifest<'static>> = Some(&ToolchainManifest {{"
    );
    let _ = writeln!(out, "    schema: {SCHEMA},");
    let _ = writeln!(
        out,
        "    target: ToolchainTarget {{ os: {os:?}, arch: {arch:?}, env: {env:?} }},"
    );
    out.push_str("    files: &[\n");
    for file in files {
        let _ = write!(
            out,
            "        ManifestFile {{ path: {:?}, size: {}, sha256: [",
            file.path, file.size
        );
        for (index, byte) in file.sha256.iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            let _ = write!(out, "0x{byte:02x}");
        }
        out.push_str("] },\n");
    }
    out.push_str("    ],\n});\n");
    out
}

#[cfg(test)]
mod tests;
