use super::*;
use std::ffi::OsStr;
use std::fs::OpenOptions;
use std::marker::PhantomData;
use std::mem::size_of;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::ExitStatusExt;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::{
    DuplicateHandle, DUPLICATE_SAME_ACCESS, ERROR_INSUFFICIENT_BUFFER, HANDLE,
    INVALID_HANDLE_VALUE, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::Globalization::CompareStringOrdinal;
use windows_sys::Win32::System::JobObjects::{
    CreateJobObjectW, JobObjectBasicAccountingInformation, JobObjectExtendedLimitInformation,
    QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
    JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;
use windows_sys::Win32::System::Threading::{
    CreateProcessW, DeleteProcThreadAttributeList, GetCurrentProcess, GetExitCodeProcess,
    InitializeProcThreadAttributeList, UpdateProcThreadAttribute, WaitForSingleObject,
    CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT, LPPROC_THREAD_ATTRIBUTE_LIST,
    PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_HANDLE_LIST, PROC_THREAD_ATTRIBUTE_JOB_LIST,
    STARTF_USESTDHANDLES, STARTUPINFOEXW,
};

pub(super) struct Child {
    // All native handles are private RAII owners, never inheritable themselves.
    job: OwnedHandle,
    process: OwnedHandle,
    id: u32,
    stdout: Option<ResourceReader>,
    stderr: Option<ResourceReader>,
    termination_requested: bool,
}

impl Child {
    pub(super) fn spawn(
        spec: &ResourceSpawnSpec,
        _limits: &ResourceLimits,
    ) -> Result<Self, ResourceLimitError> {
        Self::spawn_with_environment(spec, None)
    }

    pub(super) fn spawn_actuator(
        spec: &ResourceSpawnSpec,
        _limits: &ResourceLimits,
        policy: &ActuatorEnvironment,
    ) -> Result<Self, ResourceLimitError> {
        let environment =
            actuator_environment(spec, policy).map_err(ResourceLimitError::SpawnFailed)?;
        // Keep canonical authority in spec/environment validation. Some native
        // runtimes (Node included) cannot resolve relative modules with a verbatim
        // cwd. Use an identity-checked Win32 spelling only for process creation.
        let mut process_spec = spec.clone();
        process_spec.current_dir = actuator_working_directory(&spec.current_dir)
            .map_err(ResourceLimitError::SpawnFailed)?;
        Self::spawn_with_environment(&process_spec, Some(&environment))
    }

    fn spawn_with_environment(
        spec: &ResourceSpawnSpec,
        environment: Option<&[u16]>,
    ) -> Result<Self, ResourceLimitError> {
        let (application, mut command_line) =
            command_line(spec).map_err(ResourceLimitError::SpawnFailed)?;
        let cwd =
            wide_nul(spec.current_dir.as_os_str()).map_err(ResourceLimitError::SpawnFailed)?;
        // SAFETY: null security attributes create a non-inheritable, unnamed job.
        let raw_job = unsafe { CreateJobObjectW(null(), null()) };
        if raw_job.is_null() {
            return Err(ResourceLimitError::ContainmentSetupFailed(
                io::Error::last_os_error(),
            ));
        }
        // SAFETY: uniquely owned valid handle returned by CreateJobObjectW.
        let job = unsafe { OwnedHandle::from_raw_handle(raw_job) };
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: correctly sized initialized structure, valid live job handle.
        if unsafe {
            SetInformationJobObject(
                job.as_raw_handle(),
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        } == 0
        {
            return Err(ResourceLimitError::ContainmentSetupFailed(
                io::Error::last_os_error(),
            ));
        }

        let stdin = input_handle(spec.stdin).map_err(ResourceLimitError::SpawnFailed)?;
        let (stdout, stdout_reader) =
            output_handle(spec.stdout, false).map_err(ResourceLimitError::SpawnFailed)?;
        let (stderr, stderr_reader) =
            output_handle(spec.stderr, true).map_err(ResourceLimitError::SpawnFailed)?;
        let jobs = [job.as_raw_handle()];
        let handles = [
            stdin.as_raw_handle(),
            stdout.as_raw_handle(),
            stderr.as_raw_handle(),
        ];
        let mut attributes =
            Attributes::new().map_err(ResourceLimitError::ContainmentSetupFailed)?;
        attributes
            .handles(PROC_THREAD_ATTRIBUTE_JOB_LIST, &jobs)
            .map_err(ResourceLimitError::ContainmentSetupFailed)?;
        attributes
            .handles(PROC_THREAD_ATTRIBUTE_HANDLE_LIST, &handles)
            .map_err(ResourceLimitError::ContainmentSetupFailed)?;
        let mut startup = STARTUPINFOEXW::default();
        startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput = stdin.as_raw_handle();
        startup.StartupInfo.hStdOutput = stdout.as_raw_handle();
        startup.StartupInfo.hStdError = stderr.as_raw_handle();
        startup.lpAttributeList = attributes.as_ptr();
        let mut info = PROCESS_INFORMATION::default();
        // SAFETY: all pointers/storage and attribute values remain live until
        // after CreateProcessW and DeleteProcThreadAttributeList. Only the three
        // stdio handles are inherited. The job is associated AT creation; any
        // unsupported attribute/nested-job policy fails before workload runs.
        let created = unsafe {
            CreateProcessW(
                application.as_ptr(),
                command_line.as_mut_ptr(),
                null(),
                null(),
                1,
                EXTENDED_STARTUPINFO_PRESENT
                    | if environment.is_some() {
                        CREATE_UNICODE_ENVIRONMENT
                    } else {
                        0
                    },
                environment.map_or(null(), |block| block.as_ptr().cast()),
                cwd.as_ptr(),
                &startup.StartupInfo,
                &mut info,
            )
        };
        if created == 0 {
            return Err(ResourceLimitError::SpawnFailed(io::Error::last_os_error()));
        }
        // SAFETY: CreateProcessW succeeded and returned two uniquely owned handles.
        let process = unsafe { OwnedHandle::from_raw_handle(info.hProcess) };
        let thread = unsafe { OwnedHandle::from_raw_handle(info.hThread) };
        drop(thread);
        drop(attributes);
        // Close parent copies immediately, otherwise pipe EOF can never arrive.
        drop((stdin, stdout, stderr));
        Ok(Self {
            job,
            process,
            id: info.dwProcessId,
            stdout: stdout_reader,
            stderr: stderr_reader,
            termination_requested: false,
        })
    }

    pub(super) fn id(&self) -> u32 {
        self.id
    }
    pub(super) fn take_stdout(&mut self) -> Option<ResourceReader> {
        self.stdout.take()
    }
    pub(super) fn take_stderr(&mut self) -> Option<ResourceReader> {
        self.stderr.take()
    }

    pub(super) fn poll_exit(&mut self) -> Result<Option<ExitStatus>, ResourceLimitError> {
        // A zero-time wait observes completion without releasing either handle.
        // It also distinguishes a live process from exit code STILL_ACTIVE (259).
        match unsafe { WaitForSingleObject(self.process.as_raw_handle(), 0) } {
            WAIT_OBJECT_0 => {
                let mut code = 0;
                if unsafe { GetExitCodeProcess(self.process.as_raw_handle(), &mut code) } == 0 {
                    return Err(ResourceLimitError::ObservationFailed(
                        io::Error::last_os_error(),
                    ));
                }
                Ok(Some(ExitStatus::from_raw(code)))
            }
            WAIT_TIMEOUT => Ok(None),
            WAIT_FAILED => Err(ResourceLimitError::ObservationFailed(
                io::Error::last_os_error(),
            )),
            _ => Err(ResourceLimitError::ObservationFailed(io::Error::other(
                "unexpected process wait result",
            ))),
        }
    }

    pub(super) fn request_termination(&mut self) -> Result<(), ResourceLimitError> {
        if !self.termination_requested {
            // SAFETY: this is the retained private job, never a looked-up PID.
            if unsafe { TerminateJobObject(self.job.as_raw_handle(), 1) } == 0 {
                return Err(ResourceLimitError::TerminationFailed(
                    io::Error::last_os_error(),
                ));
            }
            self.termination_requested = true;
        }
        Ok(())
    }

    pub(super) fn try_finalize(&mut self) -> Result<Option<ExitStatus>, ResourceLimitError> {
        debug_assert!(self.termination_requested);
        let Some(status) = self.poll_exit()? else {
            return Ok(None);
        };
        // Job termination is asynchronous. Confirm all members have completed
        // before returning success; this queries accounting, never process IDs.
        let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        if unsafe {
            QueryInformationJobObject(
                self.job.as_raw_handle(),
                JobObjectBasicAccountingInformation,
                (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                null_mut(),
            )
        } == 0
        {
            return Err(ResourceLimitError::TerminationFailed(
                io::Error::last_os_error(),
            ));
        }
        Ok((accounting.ActiveProcesses == 0).then_some(status))
    }
}

fn ordinary_component(name: &OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    if name.is_empty()
        || name.ends_with(['.', ' '])
        || name
            .chars()
            .any(|c| c <= '\u{1f}' || r#"<>:"/\|?*"#.contains(c))
    {
        return false;
    }
    let upper = name.to_ascii_uppercase();
    let stem = upper.split('.').next().unwrap().trim_end_matches(' ');
    if matches!(stem, "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$") {
        return false;
    }
    !["COM", "LPT"].iter().any(|prefix| {
        stem.strip_prefix(prefix).is_some_and(|suffix| {
            matches!(
                suffix,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    })
}

fn actuator_working_directory(canonical: &std::path::Path) -> io::Result<PathBuf> {
    use std::path::{Component, Prefix};
    let invalid = || {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "workspace has no equivalent Win32 runtime path",
        )
    };
    let mut components = canonical.components();
    let Some(Component::Prefix(prefix)) = components.next() else {
        return Err(invalid());
    };
    let mut ordinary = match prefix.kind() {
        Prefix::VerbatimDisk(letter) => PathBuf::from(format!("{}:\\", char::from(letter))),
        Prefix::VerbatimUNC(server, share)
            if ordinary_component(server) && ordinary_component(share) =>
        {
            let mut prefix = OsString::from(r"\\");
            prefix.push(server);
            prefix.push(r"\");
            prefix.push(share);
            prefix.push(r"\");
            PathBuf::from(prefix)
        }
        _ => return Err(invalid()),
    };
    if components.next() != Some(Component::RootDir) {
        return Err(invalid());
    }
    for component in components {
        match component {
            Component::Normal(name) if ordinary_component(name) => ordinary.push(name),
            _ => return Err(invalid()),
        }
    }
    // Reject normalization changes, stale/missing roots, and different targets.
    // No raw caller spelling is ever used as a fallback.
    if ordinary.canonicalize()?.as_path() != canonical {
        return Err(invalid());
    }
    Ok(ordinary)
}

// Environment blocks use Windows' ordinal uppercase table, not locale or Rust
// Unicode folding. Insertion is fallible so a native comparison error denies spawn.
fn compare_names(left: &[u16], right: &[u16]) -> io::Result<std::cmp::Ordering> {
    let length = |s: &[u16]| {
        i32::try_from(s.len())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "environment name too long"))
    };
    let result = unsafe {
        CompareStringOrdinal(
            left.as_ptr(),
            length(left)?,
            right.as_ptr(),
            length(right)?,
            1,
        )
    };
    match result {
        1 => Ok(std::cmp::Ordering::Less),
        2 => Ok(std::cmp::Ordering::Equal),
        3 => Ok(std::cmp::Ordering::Greater),
        _ => Err(io::Error::last_os_error()),
    }
}

fn environment_block(
    inherited: impl IntoIterator<Item = (OsString, OsString)>,
    overrides: Vec<(OsString, OsString)>,
) -> io::Result<Vec<u16>> {
    let mut entries: Vec<(Vec<u16>, Vec<u16>)> = Vec::new();
    for (replace, pairs) in [
        (false, inherited.into_iter().collect::<Vec<_>>()),
        (true, overrides),
    ] {
        for (name, value) in pairs {
            let name: Vec<_> = name.encode_wide().collect();
            let value: Vec<_> = value.encode_wide().collect();
            // Windows' inherited per-drive current directories are special =C:
            // entries. Only preserve that exact form; overrides never create it.
            let drive_entry = !replace
                && name.len() == 3
                && name[0] == b'=' as u16
                && ((b'A' as u16..=b'Z' as u16).contains(&name[1])
                    || (b'a' as u16..=b'z' as u16).contains(&name[1]))
                && name[2] == b':' as u16;
            if name.is_empty()
                || name.contains(&0)
                || value.contains(&0)
                || (name.contains(&(b'=' as u16)) && !drive_entry)
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid child environment entry",
                ));
            }
            let mut index = entries.len();
            for (i, (existing, _)) in entries.iter().enumerate() {
                match compare_names(&name, existing)? {
                    std::cmp::Ordering::Less => {
                        index = i;
                        break;
                    }
                    std::cmp::Ordering::Equal if replace => {
                        index = i;
                        entries.remove(i);
                        break;
                    }
                    std::cmp::Ordering::Equal => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "ambiguous inherited environment names",
                        ))
                    }
                    std::cmp::Ordering::Greater => {}
                }
            }
            entries.insert(index, (name, value));
        }
    }
    let mut block = Vec::new();
    for (name, value) in entries {
        block.extend(name);
        block.push(b'=' as u16);
        block.extend(value);
        block.push(0);
    }
    if block.is_empty() {
        block.push(0);
    }
    block.push(0);
    Ok(block)
}

fn actuator_environment(
    spec: &ResourceSpawnSpec,
    policy: &ActuatorEnvironment,
) -> io::Result<Vec<u16>> {
    let cwd = &spec.current_dir;
    if !cwd.is_absolute() || !cwd.is_dir() || cwd.canonicalize()? != *cwd {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "actuator cwd must be canonical",
        ));
    }
    let (ActuatorEnvironment::Shell { path } | ActuatorEnvironment::InlineCode { path }) = policy;
    if path.is_empty() || std::env::split_paths(path).any(|p| !p.is_absolute()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "actuator PATH must contain absolute directories",
        ));
    }
    let mut overrides = vec![(OsString::from("PATH"), path.clone())];
    if matches!(policy, ActuatorEnvironment::InlineCode { .. }) {
        for name in ["HOME", "TMPDIR", "USERPROFILE", "TEMP", "TMP"] {
            overrides.push((name.into(), cwd.as_os_str().to_owned()));
        }
        // Windows names are case-insensitive. Emit lowercase proxy names so
        // explicitly requested POSIX-compatible runtimes also see their usual
        // case-sensitive variables, without creating duplicate Windows names.
        for name in ["http_proxy", "https_proxy"] {
            overrides.push((name.into(), "http://0.0.0.0:0".into()));
        }
        overrides.push(("no_proxy".into(), "".into()));
    }
    environment_block(std::env::vars_os(), overrides)
}

/// Owns aligned attribute storage and borrows the arrays referenced by Windows.
/// Drop runs before those arrays/handles disappear, on success and every error.
struct Attributes<'a> {
    storage: Vec<usize>,
    values: PhantomData<&'a [HANDLE]>,
}

impl<'a> Attributes<'a> {
    fn new() -> io::Result<Self> {
        let mut bytes = 0;
        let result = unsafe { InitializeProcThreadAttributeList(null_mut(), 2, 0, &mut bytes) };
        let error = io::Error::last_os_error();
        if result != 0
            || error.raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER as i32)
            || bytes == 0
        {
            return Err(error);
        }
        let mut storage = vec![0usize; bytes.div_ceil(size_of::<usize>())];
        if unsafe {
            InitializeProcThreadAttributeList(storage.as_mut_ptr().cast(), 2, 0, &mut bytes)
        } == 0
        {
            return Err(io::Error::last_os_error()); // storage freed; list not initialized
        }
        Ok(Self {
            storage,
            values: PhantomData,
        })
    }
    fn as_ptr(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.storage.as_mut_ptr().cast()
    }
    fn handles(&mut self, attribute: u32, values: &'a [HANDLE]) -> io::Result<()> {
        if unsafe {
            UpdateProcThreadAttribute(
                self.as_ptr(),
                0,
                attribute as usize,
                values.as_ptr().cast(),
                std::mem::size_of_val(values),
                null_mut(),
                null(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}
impl Drop for Attributes<'_> {
    fn drop(&mut self) {
        // SAFETY: initialized list whose allocation and borrowed values are live.
        unsafe { DeleteProcThreadAttributeList(self.as_ptr()) };
    }
}

fn inheritable_copy(handle: HANDLE) -> io::Result<OwnedHandle> {
    let mut copy = null_mut();
    // SAFETY: caller retains the source. The duplicate is independently owned.
    if unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            handle,
            GetCurrentProcess(),
            &mut copy,
            0,
            1,
            DUPLICATE_SAME_ACCESS,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { OwnedHandle::from_raw_handle(copy) })
}
fn null_handle(input: bool) -> io::Result<OwnedHandle> {
    let file = OpenOptions::new()
        .read(input)
        .write(!input)
        .open(r"\\.\NUL")?;
    inheritable_copy(file.as_raw_handle())
}
fn inherited_handle(handle: HANDLE, input: bool) -> io::Result<OwnedHandle> {
    // GUI/detached parents may have no console stream. Represent that as NUL;
    // never DuplicateHandle(INVALID_HANDLE_VALUE), which aliases the current
    // process pseudo-handle and could leak parent process access into the child.
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        null_handle(input)
    } else {
        inheritable_copy(handle)
    }
}
fn input_handle(mode: ResourceStdin) -> io::Result<OwnedHandle> {
    match mode {
        ResourceStdin::Inherit => inherited_handle(io::stdin().as_raw_handle(), true),
        ResourceStdin::Null => null_handle(true),
    }
}
fn output_handle(
    mode: ResourceOutput,
    stderr: bool,
) -> io::Result<(OwnedHandle, Option<ResourceReader>)> {
    match mode {
        ResourceOutput::Inherit => Ok((
            if stderr {
                inherited_handle(io::stderr().as_raw_handle(), false)?
            } else {
                inherited_handle(io::stdout().as_raw_handle(), false)?
            },
            None,
        )),
        ResourceOutput::Null => Ok((null_handle(false)?, None)),
        ResourceOutput::Piped => {
            // std owns pipe creation/closure; no extra windows-sys feature needed.
            let (reader, writer) = io::pipe()?;
            let child = inheritable_copy(writer.as_raw_handle())?;
            Ok((child, Some(Box::new(reader))))
        }
    }
}

fn wide_nul(value: &OsStr) -> io::Result<Vec<u16>> {
    let mut wide: Vec<u16> = value.encode_wide().collect();
    if wide.contains(&0) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "interior NUL"));
    }
    wide.push(0);
    Ok(wide)
}

fn system_cmd() -> io::Result<OsString> {
    let mut path = vec![0u16; 260];
    loop {
        let len = unsafe { GetSystemDirectoryW(path.as_mut_ptr(), path.len() as u32) } as usize;
        if len == 0 {
            return Err(io::Error::last_os_error());
        }
        if len < path.len() {
            path.truncate(len);
            let mut path = PathBuf::from(OsString::from_wide(&path));
            path.push("cmd.exe");
            return Ok(path.into_os_string());
        }
        path.resize(len + 1, 0);
    }
}

fn command_line(spec: &ResourceSpawnSpec) -> io::Result<(Vec<u16>, Vec<u16>)> {
    let (program, args) = match &spec.program {
        ResourceProgram::Executable { program, args } => {
            // Explicit executable paths are sufficient for this narrow API.
            // Resolve relative paths against the requested cwd, never PATH.
            let path = std::path::absolute(spec.current_dir.join(program))?;
            (path.into_os_string(), args.clone())
        }
        ResourceProgram::Shell(script) => (
            system_cmd()?,
            vec![OsString::from("/C"), OsString::from(script)],
        ),
    };
    let application = wide_nul(&program)?;
    if application.contains(&(b'"' as u16)) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "quote in executable path",
        ));
    }
    let mut line = vec![b'"' as u16];
    line.extend_from_slice(&application[..application.len() - 1]);
    line.push(b'"' as u16);
    for arg in args {
        line.push(b' ' as u16);
        append_argument(&mut line, &arg)?;
    }
    line.push(0);
    Ok((application, line))
}

/// Matches std::process::Command's regular-argument encoding, including the
/// legacy `.args(["/C", command])` terminal tail. Do not switch to raw_arg or
/// introduce /S, /D, or a different cmd quoting contract during this repair.
fn append_argument(line: &mut Vec<u16>, arg: &OsStr) -> io::Result<()> {
    let wide = wide_nul(arg)?;
    let wide = &wide[..wide.len() - 1];
    let quoted = wide.is_empty() || wide.iter().any(|c| *c == 32 || *c == 9);
    if quoted {
        line.push(34);
    }
    let mut slashes = 0;
    for &c in wide {
        if c == 92 {
            slashes += 1;
        } else {
            if c == 34 {
                line.extend(std::iter::repeat_n(92, slashes + 1));
            }
            slashes = 0;
        }
        line.push(c);
    }
    if quoted {
        line.extend(std::iter::repeat_n(92, slashes));
        line.push(34);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::System::Threading::GetProcessId;

    fn pairs(values: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
        values
            .iter()
            .map(|(key, value)| ((*key).into(), (*value).into()))
            .collect()
    }

    #[test]
    fn actuator_runtime_directory_retains_canonical_identity() {
        let root = tempfile::Builder::new()
            .prefix("runtime path ü ")
            .tempdir()
            .unwrap();
        let canonical = root.path().canonicalize().unwrap();
        let ordinary = actuator_working_directory(&canonical).unwrap();
        assert_eq!(ordinary.canonicalize().unwrap(), canonical);
        assert!(
            matches!(ordinary.components().next(), Some(std::path::Component::Prefix(p)) if matches!(p.kind(), std::path::Prefix::Disk(_)))
        );
        assert!(actuator_working_directory(&canonical.join("missing")).is_err());
        assert!(actuator_working_directory(&canonical.join("..")).is_err());
        assert!(actuator_working_directory(std::path::Path::new("relative")).is_err());
    }

    #[test]
    fn runtime_path_rejects_components_that_change_under_win32_normalization() {
        for name in [
            "NUL",
            "con.txt",
            "COM1",
            "LPT³.txt",
            "CONIN$",
            "con .txt",
            "trailing.",
            "trailing ",
            "a:b",
            "..",
            ".",
            "a?b",
        ] {
            assert!(!ordinary_component(OsStr::new(name)), "{name}");
        }
        for name in ["workspace ü", ".config", "COM10", "normal.txt"] {
            assert!(ordinary_component(OsStr::new(name)), "{name}");
        }
    }

    #[test]
    fn child_environment_is_sorted_and_overrides_case_aliases() {
        let block = environment_block(
            pairs(&[
                ("z", "last"),
                ("Path", "old"),
                ("=C:", "C:\\old"),
                ("ä", "unicode"),
            ]),
            pairs(&[("PATH", "new"), ("Ä", "replaced"), ("a", "first")]),
        )
        .unwrap();
        let text = String::from_utf16(&block).unwrap();
        assert_eq!(
            text,
            "=C:=C:\\old\0a=first\0PATH=new\0z=last\0Ä=replaced\0\0"
        );
        assert_eq!(environment_block(Vec::new(), Vec::new()).unwrap(), [0, 0]);
    }

    #[test]
    fn child_environment_rejects_invalid_and_ambiguous_entries() {
        for pair in [
            ("", "value"),
            ("bad=name", "value"),
            ("name\0", "value"),
            ("name", "bad\0value"),
            ("=BAD", "value"),
        ] {
            assert!(environment_block(pairs(&[pair]), Vec::new()).is_err());
        }
        assert!(environment_block(pairs(&[("Path", "a"), ("PATH", "b")]), Vec::new()).is_err());
        assert!(environment_block(Vec::new(), pairs(&[("=C:", "value")])).is_err());
    }

    #[test]
    fn actuator_environment_requires_canonical_workspace_and_absolute_path() {
        let root = tempfile::tempdir().unwrap();
        let mut spec = ResourceSpawnSpec {
            program: ResourceProgram::Executable {
                program: "unused.exe".into(),
                args: vec![],
            },
            current_dir: root.path().canonicalize().unwrap(),
            stdin: ResourceStdin::Null,
            stdout: ResourceOutput::Piped,
            stderr: ResourceOutput::Piped,
        };
        for path in ["", ".", "C:\\valid;.", "C:\\bad\0value"] {
            assert!(
                actuator_environment(&spec, &ActuatorEnvironment::Shell { path: path.into() })
                    .is_err()
            );
        }
        spec.current_dir.push("missing");
        assert!(actuator_environment(
            &spec,
            &ActuatorEnvironment::InlineCode {
                path: "C:\\valid".into()
            }
        )
        .is_err());
    }

    #[test]
    fn absent_console_handles_never_duplicate_the_parent_process() {
        for absent in [null_mut(), INVALID_HANDLE_VALUE] {
            for input in [true, false] {
                let handle = inherited_handle(absent, input).unwrap();
                // A NUL stream is not a process handle. In particular -1 must
                // not be duplicated as the current process pseudo-handle.
                assert_eq!(unsafe { GetProcessId(handle.as_raw_handle()) }, 0);
            }
        }
    }
}
