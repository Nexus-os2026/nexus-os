use super::*;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::{symlink, PermissionsExt};

/// A disposable directory tree: `base/ws/area`, with `ws` and `area`
/// retained like a workspace's.
struct Tree {
    base: PathBuf,
    ws: Dir,
    area: Dir,
}

impl Tree {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "nexus-p2e-{tag}-{}-{}",
            std::process::id(),
            sys::random_hex(4).unwrap()
        ));
        std::fs::create_dir_all(base.join("ws/area")).unwrap();
        let base_fd =
            sys::open_fixed(&path_cstring(&base), libc::O_RDONLY | libc::O_DIRECTORY).unwrap();
        let ws = Dir::open_at(base_fd.as_fd(), c"ws").unwrap();
        let area = Dir::open_at(ws.fd(), c"area").unwrap();
        Self { base, ws, area }
    }

    fn area_path(&self) -> PathBuf {
        self.base.join("ws/area")
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

fn path_cstring(path: &Path) -> CString {
    CString::new(path.as_os_str().as_bytes()).unwrap()
}

fn listing(path: &Path) -> Vec<std::ffi::OsString> {
    let mut names: Vec<_> = std::fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    names.sort();
    names
}

#[test]
fn p2e_removal_handles_everything_a_verifier_can_leave() {
    let tree = Tree::new("adversarial");
    let area = tree.area_path();
    // Nesting far deeper than the descriptor bound.
    let mut deep = area.join("deep");
    for _ in 0..(MAX_REMOVAL_DEPTH * 4) {
        deep = deep.join("d");
    }
    std::fs::create_dir_all(&deep).unwrap();
    std::fs::write(deep.join("leaf"), b"x").unwrap();
    // Directories the owner cannot read, list or write into.
    for (name, mode) in [("locked", 0o000), ("readonly", 0o500), ("noexec", 0o600)] {
        let dir = area.join(name);
        std::fs::create_dir_all(dir.join("inner")).unwrap();
        std::fs::write(dir.join("inner/file"), b"x").unwrap();
        std::fs::write(dir.join("file"), b"x").unwrap();
        std::fs::set_permissions(dir.join("inner"), std::fs::Permissions::from_mode(mode)).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(mode)).unwrap();
    }
    // Names that are not UTF-8, a file without permissions, hard links.
    let odd = area.join(std::ffi::OsString::from_vec(b"odd-\xff\xfe".to_vec()));
    std::fs::create_dir(&odd).unwrap();
    std::fs::write(
        odd.join(std::ffi::OsString::from_vec(b"\x01\x80name".to_vec())),
        b"x",
    )
    .unwrap();
    std::fs::write(area.join("unreadable"), b"x").unwrap();
    std::fs::set_permissions(
        area.join("unreadable"),
        std::fs::Permissions::from_mode(0o000),
    )
    .unwrap();
    std::fs::write(area.join("linked"), b"x").unwrap();
    std::fs::hard_link(area.join("linked"), area.join("linked-too")).unwrap();
    // More entries than one listing buffer holds.
    let many = area.join("many");
    std::fs::create_dir(&many).unwrap();
    for i in 0..3000 {
        std::fs::write(many.join(format!("a-rather-long-file-name-{i:06}")), b"").unwrap();
    }

    empty(&tree.area, &tree.ws).unwrap();
    assert!(listing(&area).is_empty());
    // Deep subtrees were moved up into the workspace; emptying it removes
    // them too.
    assert!(listing(&tree.base.join("ws"))
        .iter()
        .any(|name| name.to_string_lossy().starts_with(".nexus-remove-")));
    empty(&tree.ws, &tree.ws).unwrap();
    assert!(listing(&tree.base.join("ws")).is_empty());
}

#[test]
fn p2e_removal_never_follows_a_symlink_or_leaves_the_tree() {
    let tree = Tree::new("symlink");
    let outside = tree.base.join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("sentinel"), b"keep").unwrap();
    symlink(&outside, tree.area_path().join("link-to-dir")).unwrap();
    symlink(
        outside.join("sentinel"),
        tree.area_path().join("link-to-file"),
    )
    .unwrap();
    empty(&tree.area, &tree.ws).unwrap();
    assert!(listing(&tree.area_path()).is_empty());
    assert_eq!(std::fs::read(outside.join("sentinel")).unwrap(), b"keep");
}

#[test]
fn p2e_a_replaced_directory_is_never_removed() {
    let tree = Tree::new("replaced");
    std::fs::write(tree.area_path().join("inside"), b"x").unwrap();
    // The retained area is moved away and another directory takes its name.
    std::fs::rename(tree.area_path(), tree.base.join("ws/area-moved")).unwrap();
    std::fs::create_dir(tree.area_path()).unwrap();
    std::fs::write(tree.area_path().join("sentinel"), b"keep").unwrap();
    // The retained directory is emptied through its descriptor…
    empty(&tree.area, &tree.ws).unwrap();
    assert!(listing(&tree.base.join("ws/area-moved")).is_empty());
    // …but the name now refers to another directory, which is kept.
    assert!(remove_dir_if_identity(tree.ws.fd(), c"area", &tree.area).is_err());
    assert_eq!(
        std::fs::read(tree.area_path().join("sentinel")).unwrap(),
        b"keep"
    );
    assert!(!tree.area.removed().unwrap());
    // An empty directory at the name is not the retained one either.
    std::fs::remove_file(tree.area_path().join("sentinel")).unwrap();
    assert!(remove_dir_if_identity(tree.ws.fd(), c"area", &tree.area).is_err());
    assert!(tree.area_path().is_dir(), "the replacement was kept");
    // Put back, it is removed and the removal is confirmed.
    std::fs::remove_dir_all(tree.area_path()).unwrap();
    std::fs::rename(tree.base.join("ws/area-moved"), tree.area_path()).unwrap();
    remove_dir_if_identity(tree.ws.fd(), c"area", &tree.area).unwrap();
    assert!(tree.area.removed().unwrap());
    // Removing it again is confirmed without touching any name.
    remove_dir_if_identity(tree.ws.fd(), c"area", &tree.area).unwrap();
}

#[test]
fn p2e_areas_have_their_designed_roles() {
    let names: std::collections::BTreeSet<_> = Area::ALL.iter().map(|a| a.name()).collect();
    assert_eq!(names.len(), Area::ALL.len());
    for area in Area::ALL {
        assert_eq!(area.c_name().to_bytes(), area.name().as_bytes());
        let expected = match area {
            Area::Input => Role::CandidateInput,
            Area::Target => Role::Target,
            _ => Role::Scratch,
        };
        assert_eq!(area.role(), expected, "{area:?}");
    }
    // The candidate is read only; only the build output may execute.
    use crate::policy::fs_access::{EXECUTE, MAKE_REG, REMOVE_FILE, WRITE_FILE};
    let input = Area::Input.role().rights();
    assert_eq!(input & (WRITE_FILE | MAKE_REG | REMOVE_FILE | EXECUTE), 0);
    for area in Area::ALL {
        let executes = area.role().rights() & EXECUTE != 0;
        assert_eq!(executes, area == Area::Target, "{area:?}");
    }
}

#[test]
fn p2e_a_listing_is_complete_across_buffers() {
    let tree = Tree::new("listing");
    for i in 0..2500 {
        std::fs::write(
            tree.area_path()
                .join(format!("entry-with-a-long-name-{i:05}")),
            b"",
        )
        .unwrap();
    }
    let first = sys::directory_batch(tree.area.fd()).unwrap();
    assert!(!first.is_empty());
    assert!(!sys::directory_is_empty(tree.area.fd()).unwrap());
    empty(&tree.area, &tree.ws).unwrap();
    assert!(sys::directory_is_empty(tree.area.fd()).unwrap());
}

/// XA-L-01: the location the desktop reserves against project registration
/// is exactly where the workspaces live, derived once from the real uid. The
/// retained root is compared only where it already exists, so the test
/// creates nothing in the runtime directory.
#[test]
fn p2e_the_reserved_workspaces_path_is_the_derived_root() {
    assert_eq!(
        workspaces_path_of(1000),
        PathBuf::from("/run/user/1000/nexus-verifier")
    );
    // SAFETY: getuid has no preconditions.
    let uid = unsafe { libc::getuid() };
    assert_eq!(workspaces_path(), workspaces_path_of(uid));
    if workspaces_path().is_dir() {
        if let Ok(root) = WorkspaceRoot::derive() {
            assert_eq!(root.path, workspaces_path());
        }
    }
}
