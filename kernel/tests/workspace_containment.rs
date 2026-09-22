//! P0-002: exercise the real filesystem actuator against disposable host files.
use nexus_kernel::actuators::types::Actuator;
use nexus_kernel::actuators::{ActuatorContext, ActuatorError, GovernedFilesystem};
use nexus_kernel::autonomy::AutonomyLevel;
use nexus_kernel::cognitive::PlannedAction;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

struct Fixture {
    _temp: TempDir,
    workspace: PathBuf,
    outside: PathBuf,
    context: ActuatorContext,
}

impl Fixture {
    fn new() -> Self {
        let temp = TempDir::new().unwrap();
        let workspace = temp.path().join("nexus-work");
        let outside = temp.path().join("nexus-work-evil");
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret.txt"), "outside evidence").unwrap();
        let context = ActuatorContext {
            agent_id: "containment-test".into(),
            agent_name: "containment-test".into(),
            working_dir: workspace.clone(),
            autonomy_level: AutonomyLevel::L3,
            capabilities: ["fs.read".into(), "fs.write".into()].into_iter().collect(),
            fuel_remaining: 1000.0,
            egress_allowlist: vec![],
            action_review_engine: None,
            hitl_approved: true,
        };
        Self {
            _temp: temp,
            workspace,
            outside,
            context,
        }
    }

    fn read(&self, path: &Path) -> Result<String, ActuatorError> {
        GovernedFilesystem
            .execute(
                &PlannedAction::FileRead {
                    path: path.to_str().unwrap().into(),
                },
                &self.context,
            )
            .map(|result| result.output)
    }

    fn write(&self, path: &Path) -> Result<(), ActuatorError> {
        GovernedFilesystem
            .execute(
                &PlannedAction::FileWrite {
                    path: path.to_str().unwrap().into(),
                    content: "inside content".into(),
                },
                &self.context,
            )
            .map(|_| ())
    }

    fn assert_outside_untouched(&self) {
        assert_eq!(
            std::fs::read_to_string(self.outside.join("secret.txt")).unwrap(),
            "outside evidence"
        );
        assert_eq!(std::fs::read_dir(&self.outside).unwrap().count(), 1);
    }

    fn deny_read_and_write(&self, path: &Path) {
        let read = self.read(path);
        let write = self.write(path);
        // Check actual state even when the operation incorrectly returned Ok.
        self.assert_outside_untouched();
        assert!(
            matches!(read, Err(ActuatorError::PathTraversal(_))),
            "read: {read:?}"
        );
        assert!(
            matches!(write, Err(ActuatorError::PathTraversal(_))),
            "write: {write:?}"
        );
    }
}

#[test]
fn relative_and_absolute_inside_access_succeeds() {
    let f = Fixture::new();
    f.write(Path::new("normal.txt")).unwrap();
    assert_eq!(f.read(Path::new("normal.txt")).unwrap(), "inside content");
    let absolute = f.workspace.join("absolute.txt");
    f.write(&absolute).unwrap();
    assert_eq!(f.read(&absolute).unwrap(), "inside content");
    f.assert_outside_untouched();
}

#[test]
fn absolute_outside_and_workspace_prefix_confusion_are_denied() {
    let f = Fixture::new();
    f.deny_read_and_write(&f.outside.join("secret.txt"));
}

#[test]
fn parent_traversal_is_denied_without_modifying_outside_file() {
    let f = Fixture::new();
    f.deny_read_and_write(Path::new("../nexus-work-evil/secret.txt"));
}

#[test]
fn nested_traversal_is_denied_without_creating_outside_directories() {
    let f = Fixture::new();
    std::fs::create_dir(f.workspace.join("normal")).unwrap();
    let result = f.write(Path::new("normal/../../nexus-work-evil/new/deep/file.txt"));
    f.assert_outside_untouched();
    assert!(matches!(result, Err(ActuatorError::PathTraversal(_))));
}

#[test]
fn new_nested_file_inside_workspace_succeeds() {
    let f = Fixture::new();
    f.write(Path::new("new/deep/file.txt")).unwrap();
    assert_eq!(
        std::fs::read_to_string(f.workspace.join("new/deep/file.txt")).unwrap(),
        "inside content"
    );
    f.assert_outside_untouched();
}

#[test]
fn missing_read_does_not_create_parent_directories() {
    let f = Fixture::new();
    let result = f.read(Path::new("missing/deep/file.txt"));
    assert!(matches!(result, Err(ActuatorError::IoError(_))));
    assert!(!f.workspace.join("missing").exists());
}

#[test]
fn media_output_paths_are_denied_before_provider_execution() {
    use nexus_kernel::actuators::{ImageGenActuator, TtsActuator};
    let mut f = Fixture::new();
    f.context
        .capabilities
        .extend(["image.generate".into(), "tts.generate".into()]);
    let output_path = f
        .outside
        .join("new/deep/output.txt")
        .to_str()
        .unwrap()
        .to_string();
    // An unsupported provider cannot perform I/O even if path checking regresses.
    // PathTraversal proves the containment check ran before provider dispatch.
    let image = ImageGenActuator.execute(
        &PlannedAction::ImageGenerate {
            prompt: "fixture".into(),
            output_path: output_path.clone(),
            provider: Some("p0-002-no-provider".into()),
            model: None,
            size: None,
        },
        &f.context,
    );
    let speech = TtsActuator.execute(
        &PlannedAction::TextToSpeech {
            text: "fixture".into(),
            output_path,
            provider: Some("p0-002-no-provider".into()),
            voice: None,
            model: None,
        },
        &f.context,
    );
    f.assert_outside_untouched();
    assert!(matches!(image, Err(ActuatorError::PathTraversal(_))));
    assert!(matches!(speech, Err(ActuatorError::PathTraversal(_))));
}

#[test]
fn containment_error_does_not_disclose_resolved_host_path() {
    let f = Fixture::new();
    let error = f.read(&f.outside.join("secret.txt")).unwrap_err();
    assert!(matches!(error, ActuatorError::PathTraversal(_)));
    assert!(!error.to_string().contains(f.outside.to_str().unwrap()));
    assert!(!error.to_string().contains("secret.txt"));
    assert!(matches!(
        f.read(Path::new("absent.txt")),
        Err(ActuatorError::IoError(_))
    ));
}

#[cfg(unix)]
#[test]
fn symlink_file_escape_is_denied() {
    let f = Fixture::new();
    std::os::unix::fs::symlink(
        f.outside.join("secret.txt"),
        f.workspace.join("escape-file"),
    )
    .unwrap();
    f.deny_read_and_write(Path::new("escape-file"));
}

#[cfg(unix)]
#[test]
fn symlink_directory_escape_and_new_targets_are_denied() {
    let f = Fixture::new();
    std::os::unix::fs::symlink(&f.outside, f.workspace.join("escape-dir")).unwrap();
    f.deny_read_and_write(Path::new("escape-dir/secret.txt"));
    for path in ["escape-dir/new.txt", "escape-dir/new/deep/file.txt"] {
        let result = f.write(Path::new(path));
        f.assert_outside_untouched();
        assert!(
            matches!(result, Err(ActuatorError::PathTraversal(_))),
            "{result:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn dangling_symlink_escape_is_denied() {
    let f = Fixture::new();
    std::os::unix::fs::symlink(f.outside.join("new.txt"), f.workspace.join("dangling")).unwrap();
    let result = f.write(Path::new("dangling"));
    f.assert_outside_untouched();
    assert!(matches!(result, Err(ActuatorError::PathTraversal(_))));
}

#[cfg(unix)]
#[test]
fn symlinks_resolving_inside_workspace_remain_allowed() {
    let f = Fixture::new();
    std::fs::create_dir(f.workspace.join("normal")).unwrap();
    std::fs::write(f.workspace.join("normal/file.txt"), "before").unwrap();
    std::os::unix::fs::symlink("normal/file.txt", f.workspace.join("inside-file")).unwrap();
    std::os::unix::fs::symlink("normal", f.workspace.join("inside-dir")).unwrap();
    f.write(Path::new("inside-file")).unwrap();
    assert_eq!(f.read(Path::new("inside-file")).unwrap(), "inside content");
    f.write(Path::new("inside-dir/new/deep/file.txt")).unwrap();
    assert_eq!(
        std::fs::read_to_string(f.workspace.join("normal/new/deep/file.txt")).unwrap(),
        "inside content"
    );
    f.assert_outside_untouched();
}

#[test]
fn existing_relative_resolution_never_creates_root_or_target() {
    use nexus_kernel::workspace::{resolve_existing_relative, resolve_path};
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap().join("fresh");
    assert!(resolve_existing_relative(&root, Path::new("nested/file")).is_err());
    assert!(!root.exists());
    // The original resolver retains its initialization and absolute-path contract.
    assert_eq!(
        resolve_path(&root, &root.join("file")).unwrap(),
        root.join("file")
    );
    assert_eq!(
        resolve_existing_relative(&root, Path::new("nested/file")).unwrap(),
        root.join("nested/file")
    );
    assert!(!root.join("nested").exists());
    std::fs::remove_dir(&root).unwrap();
    assert!(resolve_existing_relative(&root, Path::new("file")).is_err());
    assert!(!root.exists());
}

#[test]
fn existing_relative_resolution_rejects_nonrelative_targets_and_file_roots() {
    use nexus_kernel::workspace::resolve_existing_relative;
    let f = Fixture::new();
    let root = f.workspace.canonicalize().unwrap();
    for target in [
        Path::new("../nexus-work-evil/secret.txt"),
        f.outside.as_path(),
        root.as_path(),
        Path::new("a/../../secret"),
        Path::new(""),
    ] {
        assert!(resolve_existing_relative(&root, target).is_err());
    }
    let file = root.join("not-a-directory");
    std::fs::write(&file, "evidence").unwrap();
    assert!(resolve_existing_relative(&file, Path::new("child")).is_err());
    f.assert_outside_untouched();
}

#[cfg(unix)]
#[test]
fn existing_relative_resolution_rejects_symlink_escape_and_root_repointing() {
    use nexus_kernel::workspace::resolve_existing_relative;
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let root = f.workspace.canonicalize().unwrap();
    symlink(&f.outside, root.join("escape")).unwrap();
    assert!(resolve_existing_relative(&root, Path::new("escape/new/file")).is_err());
    std::fs::remove_file(root.join("escape")).unwrap();
    std::fs::remove_dir(&root).unwrap();
    symlink(&f.outside, &root).unwrap();
    assert!(resolve_existing_relative(&root, Path::new("file")).is_err());
    f.assert_outside_untouched();
}
