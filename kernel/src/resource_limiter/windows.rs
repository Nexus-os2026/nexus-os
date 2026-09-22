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
    EXTENDED_STARTUPINFO_PRESENT, LPPROC_THREAD_ATTRIBUTE_LIST, PROCESS_INFORMATION,
    PROC_THREAD_ATTRIBUTE_HANDLE_LIST, PROC_THREAD_ATTRIBUTE_JOB_LIST, STARTF_USESTDHANDLES,
    STARTUPINFOEXW,
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
                EXTENDED_STARTUPINFO_PRESENT,
                null(),
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
