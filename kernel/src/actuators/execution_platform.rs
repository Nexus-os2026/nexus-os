//! Windows execution support. Ordinary commands never require a Unix shell.
//! Installed runtimes are explicit optional tools, resolved from backend PATH.
use super::types::ActuatorError;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

fn absolute_search_path(path: &OsStr) -> Result<(Vec<PathBuf>, OsString), ActuatorError> {
    let directories: Vec<_> = std::env::split_paths(path)
        .filter(|directory| directory.is_absolute())
        .collect();
    let sanitized = std::env::join_paths(&directories)
        .map_err(|error| ActuatorError::IoError(format!("invalid executable PATH: {error}")))?;
    Ok((directories, sanitized))
}

fn resolve_executable(program: &str, directories: &[PathBuf]) -> Result<PathBuf, ActuatorError> {
    // Callers already enforce command/language allowlists. This additional
    // boundary prevents path input, batch files and implicit shell routing.
    if program.is_empty()
        || !program
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-+".contains(&byte))
    {
        return Err(ActuatorError::CommandBlocked(
            "expected a bare native executable name".into(),
        ));
    }
    for directory in directories.iter().filter(|path| path.is_absolute()) {
        let candidate = directory.join(format!("{program}.exe"));
        match candidate.metadata() {
            Ok(metadata) if metadata.is_file() => {
                let resolved = candidate.canonicalize().map_err(|error| {
                    ActuatorError::IoError(format!("resolve executable '{program}': {error}"))
                })?;
                if !resolved
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
                {
                    return Err(ActuatorError::CommandBlocked(
                        "resolved executable must be a native .exe file".into(),
                    ));
                }
                return Ok(resolved);
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(ActuatorError::IoError(format!(
                    "inspect executable '{program}': {error}"
                )));
            }
        }
    }
    Err(ActuatorError::IoError(format!(
        "native executable '{program}.exe' is not installed on the backend PATH"
    )))
}

#[cfg(windows)]
pub(super) fn runtime_name(runtime: &str) -> Result<&str, ActuatorError> {
    Ok(match runtime {
        "python3" => "python",
        "node" => "node",
        // Bash is optional and only used for an explicit Bash code request.
        "bash" => "bash",
        _ => return Err(ActuatorError::CommandBlocked("unsupported runtime".into())),
    })
}

#[cfg(windows)]
pub(super) fn execute(
    program: &str,
    args: &[OsString],
    workspace: &Path,
    inline_code: bool,
    timeout: std::time::Duration,
    output_limit: usize,
) -> Result<std::process::Output, ActuatorError> {
    use crate::resource_limiter::*;
    let path = std::env::var_os("PATH")
        .ok_or_else(|| ActuatorError::IoError("backend executable PATH is missing".into()))?;
    let (directories, path) = absolute_search_path(&path)?;
    let program = if program.eq_ignore_ascii_case("python3") {
        "python"
    } else {
        program
    };
    let executable = resolve_executable(program, &directories)?;
    let root = workspace
        .canonicalize()
        .map_err(|error| ActuatorError::IoError(format!("resolve workspace: {error}")))?;
    let spec = ResourceSpawnSpec {
        program: ResourceProgram::Executable {
            program: executable.into(),
            args: args.to_vec(),
        },
        current_dir: root,
        stdin: ResourceStdin::Null,
        stdout: ResourceOutput::Piped,
        stderr: ResourceOutput::Piped,
    };
    let environment = if inline_code {
        ActuatorEnvironment::InlineCode { path }
    } else {
        ActuatorEnvironment::Shell { path }
    };
    execute_owned(&spec, &environment, timeout, output_limit)
}

#[cfg(windows)]
fn drain(
    reader: crate::resource_limiter::ResourceReader,
    limit: usize,
) -> std::io::Result<std::sync::mpsc::Receiver<std::io::Result<Vec<u8>>>> {
    use std::io::Read;
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("nexus-actuator-output".into())
        .spawn(move || {
            let mut reader = reader;
            let result = (|| {
                let mut output = Vec::new();
                let mut buffer = [0u8; 8192];
                loop {
                    let count = match reader.read(&mut buffer) {
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                        result => result?,
                    };
                    if count == 0 {
                        break;
                    }
                    let keep = count.min(limit.saturating_sub(output.len()));
                    output.extend_from_slice(&buffer[..keep]);
                    // Continue draining after the cap to prevent pipe backpressure.
                }
                Ok(output)
            })();
            let _ = send.send(result);
        })?;
    Ok(receive)
}

#[cfg(windows)]
fn execute_owned(
    spec: &crate::resource_limiter::ResourceSpawnSpec,
    environment: &crate::resource_limiter::ActuatorEnvironment,
    timeout: std::time::Duration,
    output_limit: usize,
) -> Result<std::process::Output, ActuatorError> {
    use crate::resource_limiter::*;
    use std::time::{Duration, Instant};
    let error = |e| ActuatorError::IoError(format!("contained execution: {e}"));
    let mut child = ResourceLimiter::default()
        .spawn_actuator(spec, environment)
        .map_err(|e| error(e.to_string()))?;
    let readers = (|| {
        let stdout = child
            .take_stdout()
            .ok_or_else(|| std::io::Error::other("missing stdout pipe"))?;
        let stderr = child
            .take_stderr()
            .ok_or_else(|| std::io::Error::other("missing stderr pipe"))?;
        // One sentinel byte lets the caller report truncation, still with a
        // strict per-stream memory cap independent of the child's output volume.
        let capture_limit = output_limit.saturating_add(1);
        Ok::<_, std::io::Error>((drain(stdout, capture_limit)?, drain(stderr, capture_limit)?))
    })();
    let outcome = if readers.is_ok() {
        let deadline = Instant::now() + timeout;
        loop {
            match child.poll_exit() {
                Ok(Some(status)) => break Ok(status),
                Err(e) => break Err(error(e.to_string())),
                Ok(None) if Instant::now() >= deadline => {
                    break Err(ActuatorError::CommandTimeout {
                        seconds: timeout.as_secs(),
                    })
                }
                Ok(None) => std::thread::sleep(
                    Duration::from_millis(5)
                        .min(deadline.saturating_duration_since(Instant::now())),
                ),
            }
        }
    } else {
        Err(error("failed to start output readers".into()))
    };
    // Cleanup errors take precedence over success, command failure or timeout.
    // The retained job is finalized even if the root exited naturally.
    let cleanup_deadline = Instant::now() + Duration::from_secs(5);
    child
        .terminate_and_reap(cleanup_deadline)
        .map_err(|e| error(e.to_string()))?;
    let (stdout, stderr) = readers.map_err(|e| error(e.to_string()))?;
    let collect = |reader: std::sync::mpsc::Receiver<std::io::Result<Vec<u8>>>| {
        reader
            .recv_timeout(cleanup_deadline.saturating_duration_since(Instant::now()))
            .map_err(|e| error(format!("output cleanup: {e}")))?
            .map_err(|e| error(format!("output read: {e}")))
    };
    // Bounded receives only; never join a reader after failed containment cleanup.
    let stdout = collect(stdout)?;
    let stderr = collect(stderr)?;
    Ok(std::process::Output {
        status: outcome?,
        stdout,
        stderr,
    })
}

pub(super) fn bounded(mut output: String, limit: usize) -> String {
    if output.len() > limit {
        const MARKER: &str = "\n... [output truncated]";
        let mut end = limit.saturating_sub(MARKER.len());
        while !output.is_char_boundary(end) {
            end -= 1;
        }
        output.truncate(end);
        output.push_str(&MARKER[..MARKER.len().min(limit)]);
    }
    output
}

/// Minimal native basic commands, after the actuator's capability and allowlist
/// checks. Unsupported ls options fail explicitly; this is not a POSIX shell.
pub(super) fn basic_command(
    program: &str,
    args: &[String],
    workspace: &Path,
    output_limit: usize,
) -> Result<Option<String>, ActuatorError> {
    let program = program.to_ascii_lowercase();
    if !matches!(program.as_str(), "echo" | "pwd" | "ls") {
        return Ok(None);
    }
    let root = workspace
        .canonicalize()
        .map_err(|error| ActuatorError::IoError(format!("resolve working directory: {error}")))?;
    if !root.is_dir() {
        return Err(ActuatorError::IoError(
            "working directory is not a directory".into(),
        ));
    }
    let output = match program.as_str() {
        "echo" => {
            let (args, newline) = if args.first().is_some_and(|arg| arg == "-n") {
                (&args[1..], "")
            } else {
                (args, "\n")
            };
            format!("{}{newline}", args.join(" "))
        }
        "pwd" if args.is_empty() => format!("{}\n", root.display()),
        "pwd" => {
            return Err(ActuatorError::CommandBlocked(
                "native pwd takes no arguments".into(),
            ))
        }
        "ls" => {
            let mut show_hidden = false;
            let mut options = true;
            let mut requested = None;
            for arg in args {
                if options && arg == "--" {
                    options = false;
                } else if options && arg.starts_with('-') {
                    if arg.len() < 2 || !arg[1..].chars().all(|c| matches!(c, 'a' | 'A' | '1')) {
                        return Err(ActuatorError::CommandBlocked(
                            "native Windows ls supports -a, -A, -1 and one workspace directory"
                                .into(),
                        ));
                    }
                    show_hidden |= arg.contains('a') || arg.contains('A');
                } else if requested.replace(arg.as_str()).is_some() {
                    return Err(ActuatorError::CommandBlocked(
                        "native Windows ls accepts one workspace directory".into(),
                    ));
                }
            }
            let directory = super::filesystem::GovernedFilesystem::resolve_safe_path(
                &root,
                requested.unwrap_or("."),
            )?;
            let entries = std::fs::read_dir(directory)
                .map_err(|error| ActuatorError::IoError(format!("list directory: {error}")))?;
            let mut names = Vec::new();
            let mut size = 0;
            for entry in entries {
                let entry = entry.map_err(|error| ActuatorError::IoError(error.to_string()))?;
                let name = entry.file_name().to_string_lossy().into_owned();
                if !show_hidden && name.starts_with('.') {
                    continue;
                }
                size += name.len() + 1;
                names.push(name);
                if size > output_limit {
                    break;
                }
            }
            names.sort();
            if names.is_empty() {
                String::new()
            } else {
                format!("{}\n", names.join("\n"))
            }
        }
        _ => unreachable!(),
    };
    Ok(Some(bounded(output, output_limit)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executable_resolution_never_uses_relative_or_empty_path_entries() {
        let directory = tempfile::tempdir().unwrap();
        let native = directory.path().join("tool.exe");
        std::fs::write(&native, "fixture").unwrap();
        let path = std::env::join_paths([Path::new(""), Path::new("."), directory.path()]).unwrap();
        let (directories, sanitized) = absolute_search_path(&path).unwrap();
        assert_eq!(directories, [directory.path()]);
        assert_eq!(
            std::env::split_paths(&sanitized).collect::<Vec<_>>(),
            directories
        );
        assert_eq!(
            resolve_executable("tool", &directories).unwrap(),
            native.canonicalize().unwrap()
        );
        assert!(resolve_executable("tool", &[PathBuf::from(".")]).is_err());
        assert!(resolve_executable("tool", &[]).is_err());
        for program in ["../tool", "tool.exe", "tool.cmd", "C:\\tool", "tool /c", ""] {
            assert!(matches!(
                resolve_executable(program, &directories),
                Err(ActuatorError::CommandBlocked(_))
            ));
        }
    }

    #[cfg(unix)]
    #[test]
    fn executable_symlink_cannot_route_to_a_batch_file() {
        let directory = tempfile::tempdir().unwrap();
        let batch = directory.path().join("tool.cmd");
        std::fs::write(&batch, "fixture").unwrap();
        std::os::unix::fs::symlink(&batch, directory.path().join("tool.exe")).unwrap();
        assert!(matches!(
            resolve_executable("tool", &[directory.path().to_path_buf()]),
            Err(ActuatorError::CommandBlocked(_))
        ));
    }

    #[test]
    fn missing_runtime_and_non_file_candidates_fail_closed() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("bash.exe")).unwrap();
        std::fs::write(directory.path().join("bash.cmd"), "fixture").unwrap();
        let paths = [directory.path().to_path_buf()];
        assert!(resolve_executable("bash", &paths).is_err());
        assert!(resolve_executable("python", &paths).is_err());
    }

    #[test]
    fn basic_commands_need_no_external_tools_and_keep_metacharacters_literal() {
        let directory = tempfile::tempdir().unwrap();
        let args = vec![
            "a & echo injected".into(),
            "$(touch owned)".into(),
            "\"quoted\"".into(),
            "%PATH%".into(),
        ];
        assert_eq!(
            basic_command("echo", &args, directory.path(), 4096)
                .unwrap()
                .unwrap(),
            format!("{}\n", args.join(" "))
        );
        assert!(!directory.path().join("owned").exists());
        std::fs::write(directory.path().join("test.txt"), "data").unwrap();
        assert_eq!(
            basic_command("ls", &[], directory.path(), 4096)
                .unwrap()
                .unwrap(),
            "test.txt\n"
        );
        assert_eq!(
            basic_command("pwd", &[], directory.path(), 4096)
                .unwrap()
                .unwrap()
                .trim(),
            directory.path().canonicalize().unwrap().to_string_lossy()
        );
        assert!(basic_command("echo", &[], &directory.path().join("missing"), 4096).is_err());
    }

    #[test]
    fn native_listing_preserves_workspace_containment_and_rejects_unknown_flags() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("workspace");
        let outside = directory.path().join("outside");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(&outside).unwrap();
        for args in [
            vec!["../outside".into()],
            vec![outside.to_string_lossy().into_owned()],
        ] {
            assert!(matches!(
                basic_command("ls", &args, &root, 4096),
                Err(ActuatorError::PathTraversal(_))
            ));
        }
        assert!(matches!(
            basic_command("ls", &["-R".into()], &root, 4096),
            Err(ActuatorError::CommandBlocked(_))
        ));
        std::fs::write(root.join(".hidden"), "data").unwrap();
        assert_eq!(
            basic_command("ls", &["-a1".into()], &root, 4096)
                .unwrap()
                .unwrap(),
            ".hidden\n"
        );
    }

    #[test]
    fn native_output_limit_preserves_utf8() {
        let output = bounded("😀".repeat(100), 65);
        assert!(output.len() <= 65);
        assert!(output.ends_with("[output truncated]"));
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;
    use crate::resource_limiter::*;
    use std::sync::OnceLock;
    use std::time::{Duration, Instant};

    // Finite, native Rust fixture: no Bash/Python/Git runner dependency. rustc is
    // the test toolchain, and the compiled executable stays alive in this TempDir.
    fn fixture() -> &'static Path {
        static FIXTURE: OnceLock<tempfile::TempDir> = OnceLock::new();
        FIXTURE.get_or_init(|| {
            let dir = tempfile::tempdir().unwrap();
            let source = dir.path().join("fixture.rs");
            std::fs::write(&source, r#"
use std::{env, fs, io::{self, Write}, process::{Command, Stdio}, thread, time::{Duration, Instant}};
fn main() {
    let args: Vec<_> = env::args().skip(1).collect();
    match args[0].as_str() {
        "inspect" => {
            let mut report = vec![fs::canonicalize(env::current_dir().unwrap()).unwrap().to_string_lossy().into_owned()];
            for key in ["PATH", "HOME", "TMPDIR", "USERPROFILE", "TEMP", "TMP", "http_proxy", "https_proxy", "no_proxy"] {
                report.push(env::var(key).unwrap());
            }
            report.extend(args[1..].iter().cloned());
            fs::write("report.txt", report.join("\0")).unwrap();
        }
        "output" => {
            io::stdout().write_all(&vec![b'o'; 262144]).unwrap();
            io::stderr().write_all(&vec![b'e'; 262144]).unwrap();
            std::process::exit(7);
        }
        "descendant" => {
            println!("descendant ready"); io::stdout().flush().unwrap();
            fs::write("ready", "ready").unwrap();
            thread::sleep(Duration::from_secs(30));
        }
        "tree-exit" | "tree-timeout" => {
            let _child = Command::new(env::current_exe().unwrap()).arg("descendant")
                .stdin(Stdio::null()).stdout(Stdio::inherit()).stderr(Stdio::inherit()).spawn().unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            while !std::path::Path::new("ready").exists() {
                assert!(Instant::now() < deadline, "descendant startup deadline");
                thread::sleep(Duration::from_millis(5));
            }
            if args[0] == "tree-timeout" { thread::sleep(Duration::from_secs(30)); }
        }
        _ => panic!("unknown mode"),
    }
}
"#).unwrap();
            let output = std::process::Command::new("rustc").arg(&source).arg("-o")
                .arg(dir.path().join("fixture.exe")).output().unwrap();
            assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
            dir
        }).path()
    }

    fn run(
        root: &Path,
        args: &[&str],
        inline: bool,
        timeout: Duration,
        limit: usize,
    ) -> Result<std::process::Output, ActuatorError> {
        let spec = ResourceSpawnSpec {
            program: ResourceProgram::Executable {
                program: fixture().join("fixture.exe").canonicalize().unwrap().into(),
                args: args.iter().map(OsString::from).collect(),
            },
            current_dir: root.canonicalize().unwrap(),
            stdin: ResourceStdin::Null,
            stdout: ResourceOutput::Piped,
            stderr: ResourceOutput::Piped,
        };
        let path = std::env::join_paths([fixture()]).unwrap();
        let environment = if inline {
            ActuatorEnvironment::InlineCode { path }
        } else {
            ActuatorEnvironment::Shell { path }
        };
        execute_owned(&spec, &environment, timeout, limit)
    }

    #[test]
    fn native_child_gets_fixed_environment_and_literal_arguments_without_parent_mutation() {
        let dir = tempfile::Builder::new()
            .prefix("nexus workspace space ")
            .tempdir()
            .unwrap();
        let keys = [
            "PATH",
            "HOME",
            "TMPDIR",
            "USERPROFILE",
            "TEMP",
            "TMP",
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "NO_PROXY",
        ];
        let before: Vec<_> = keys.iter().map(std::env::var_os).collect();
        let args = [
            "inspect",
            "space argument",
            "a&echo injected",
            "$(touch owned)",
            "quote\"slash\\",
            "",
            "%PATH%",
        ];
        let output = run(dir.path(), &args, true, Duration::from_secs(10), 4096).unwrap();
        assert!(output.status.success());
        let report = std::fs::read_to_string(dir.path().join("report.txt")).unwrap();
        let actual: Vec<_> = report.split('\0').collect();
        let root = dir
            .path()
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert_eq!(actual[0], root);
        assert_eq!(
            actual[1],
            std::env::join_paths([fixture()]).unwrap().to_string_lossy()
        );
        for value in &actual[2..7] {
            assert_eq!(*value, root);
        }
        assert_eq!(&actual[7..10], ["http://0.0.0.0:0", "http://0.0.0.0:0", ""]);
        assert_eq!(&actual[10..], &args[1..]);
        assert_eq!(
            keys.iter().map(std::env::var_os).collect::<Vec<_>>(),
            before
        );
        assert!(!dir.path().join("owned").exists());
    }

    #[test]
    fn native_output_is_drained_past_cap_and_preserves_nonzero_status() {
        let dir = tempfile::tempdir().unwrap();
        let output = run(
            dir.path(),
            &["output"],
            false,
            Duration::from_secs(10),
            4096,
        )
        .unwrap();
        assert_eq!(output.status.code(), Some(7));
        assert_eq!(output.stdout, vec![b'o'; 4097]);
        assert_eq!(output.stderr, vec![b'e'; 4097]);
        assert!(bounded(String::from_utf8(output.stdout).unwrap(), 4096)
            .ends_with("[output truncated]"));
    }

    #[test]
    fn native_adapter_propagates_setup_and_cleanup_failures() {
        let dir = tempfile::tempdir().unwrap();
        let mut spec = ResourceSpawnSpec {
            program: ResourceProgram::Executable {
                program: fixture().join("fixture.exe").canonicalize().unwrap().into(),
                args: vec!["descendant".into()],
            },
            current_dir: dir.path().canonicalize().unwrap(),
            stdin: ResourceStdin::Null,
            stdout: ResourceOutput::Piped,
            stderr: ResourceOutput::Piped,
        };
        let environment = ActuatorEnvironment::Shell {
            path: std::env::join_paths([fixture()]).unwrap(),
        };
        let mut child = ResourceLimiter::default()
            .spawn_actuator(&spec, &environment)
            .unwrap();
        assert!(matches!(
            child.terminate_and_reap(Instant::now()),
            Err(ResourceLimitError::CleanupDeadlineExceeded)
        ));
        child
            .terminate_and_reap(Instant::now() + Duration::from_secs(5))
            .unwrap();
        spec.program = ResourceProgram::Executable {
            program: dir.path().join("missing.exe").into(),
            args: vec![],
        };
        assert!(matches!(
            execute_owned(&spec, &environment, Duration::from_secs(5), 4096),
            Err(ActuatorError::IoError(_))
        ));
        spec.current_dir.push("missing");
        assert!(matches!(
            ResourceLimiter::default().spawn_actuator(&spec, &environment),
            Err(ResourceLimitError::SpawnFailed(_))
        ));
    }

    #[test]
    fn native_adapter_cleans_descendants_after_natural_root_exit() {
        let dir = tempfile::tempdir().unwrap();
        let start = Instant::now();
        let output = run(
            dir.path(),
            &["tree-exit"],
            false,
            Duration::from_secs(10),
            4096,
        )
        .unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("descendant ready"));
        assert!(start.elapsed() < Duration::from_secs(15));
    }

    #[test]
    fn native_adapter_cleans_descendants_on_timeout() {
        let dir = tempfile::tempdir().unwrap();
        // Initialize the compiled fixture before starting the execution deadline.
        fixture();
        let start = Instant::now();
        assert!(matches!(
            run(
                dir.path(),
                &["tree-timeout"],
                false,
                Duration::from_secs(5),
                4096
            ),
            Err(ActuatorError::CommandTimeout { seconds: 5 })
        ));
        assert!(dir.path().join("ready").exists());
        assert!(start.elapsed() < Duration::from_secs(12));
    }
}
