use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io;
use std::mem::{size_of, size_of_val};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{FromRawHandle, RawHandle};
use std::path::Path;
use std::ptr::{null, null_mut};
use std::thread;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{
    CloseHandle, SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE,
    WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicAccountingInformation,
    JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
    TerminateJobObject, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_ACTIVE_PROCESS,
    JOB_OBJECT_LIMIT_JOB_MEMORY, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows_sys::Win32::System::Pipes::CreatePipe;
use windows_sys::Win32::System::Threading::{
    CreateProcessW, DeleteProcThreadAttributeList, GetExitCodeProcess,
    InitializeProcThreadAttributeList, ResumeThread, UpdateProcThreadAttribute,
    WaitForSingleObject, CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT,
    EXTENDED_STARTUPINFO_PRESENT, PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
    STARTF_USESTDHANDLES, STARTUPINFOEXW, STARTUPINFOW,
};

const MAX_ACTIVE_PROCESSES: u32 = 64;
const MAX_JOB_MEMORY_BYTES: usize = 2 * 1024 * 1024 * 1024;
const PROCESS_EXIT_CODE_STILL_ACTIVE: u32 = 259;

pub struct SpawnRequest<'a> {
    pub executable: &'a OsStr,
    pub arguments: &'a [OsString],
    pub current_dir: &'a Path,
    pub environment: &'a [(OsString, OsString)],
}

pub struct SpawnedProcess {
    pub child: ManagedChild,
    pub stdin: File,
    pub stdout: File,
    pub stderr: File,
}

pub struct ManagedChild {
    process: OwnedHandle,
    job: OwnedHandle,
    process_id: u32,
}

impl ManagedChild {
    pub fn process_id(&self) -> u32 {
        self.process_id
    }

    pub fn try_wait(&self) -> io::Result<Option<i32>> {
        // SAFETY: `process` is an owned live process handle retained for this call.
        let wait = unsafe { WaitForSingleObject(self.process.0, 0) };
        match wait {
            WAIT_TIMEOUT => Ok(None),
            WAIT_OBJECT_0 => {
                let mut code = 0_u32;
                // SAFETY: `process` is valid and `code` is writable for one u32.
                if unsafe { GetExitCodeProcess(self.process.0, &mut code) } == 0 {
                    return Err(io::Error::last_os_error());
                }
                if code == PROCESS_EXIT_CODE_STILL_ACTIVE {
                    Err(io::Error::other("signaled process reported STILL_ACTIVE"))
                } else {
                    Ok(Some(code as i32))
                }
            }
            _ => Err(io::Error::last_os_error()),
        }
    }

    pub fn active_process_count(&self) -> io::Result<u32> {
        let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        // SAFETY: `job` is an owned job handle and `accounting` is a writable value
        // of the information class requested below.
        let ok = unsafe {
            QueryInformationJobObject(
                self.job.0,
                JobObjectBasicAccountingInformation,
                (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(accounting.ActiveProcesses)
    }

    pub fn wait_for_tree_empty(&self, timeout: Duration) -> io::Result<bool> {
        let deadline = Instant::now() + timeout;
        loop {
            if self.active_process_count()? == 0 {
                return Ok(true);
            }
            if Instant::now() >= deadline {
                return Ok(false);
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    pub fn terminate_tree(&self, timeout: Duration) -> io::Result<bool> {
        if self.active_process_count()? == 0 {
            return Ok(true);
        }
        // SAFETY: `job` is an owned job handle. Termination is limited to processes
        // assigned to this Job Object by `spawn` before the initial thread resumed.
        if unsafe { TerminateJobObject(self.job.0, 1) } == 0 {
            return Ok(self.active_process_count().is_ok_and(|count| count == 0));
        }
        let deadline = Instant::now() + timeout;
        loop {
            match self.active_process_count() {
                Ok(0) => return Ok(true),
                Ok(_) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(5));
                }
                Ok(_) => return Ok(false),
                Err(error) => return Err(error),
            }
        }
    }
}

impl Drop for ManagedChild {
    fn drop(&mut self) {
        let _ = self.terminate_tree(Duration::from_millis(250));
        self.job.close();
        self.process.close();
    }
}

pub fn spawn(request: SpawnRequest<'_>) -> io::Result<SpawnedProcess> {
    validate_input(&request)?;
    let stdin_pipe = Pipe::new(false)?;
    let stdout_pipe = Pipe::new(true)?;
    let stderr_pipe = Pipe::new(true)?;
    let job = create_job()?;
    let attributes = HandleAttributeList::new()?;
    let mut child_handles = [stdin_pipe.child.0, stdout_pipe.child.0, stderr_pipe.child.0];
    // SAFETY: the attribute list is initialized, aligned, and alive through CreateProcessW;
    // `child_handles` is alive and contains exactly the inheritable child pipe endpoints.
    let updated = unsafe {
        UpdateProcThreadAttribute(
            attributes.pointer,
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
            child_handles.as_mut_ptr().cast(),
            size_of_val(&child_handles),
            null_mut(),
            null_mut(),
        )
    };
    if updated == 0 {
        return Err(io::Error::last_os_error());
    }

    let executable = wide_z(request.executable)?;
    let mut command_line = build_command_line(request.executable, request.arguments)?;
    let current_dir = wide_path(request.current_dir)?;
    let mut environment = build_environment(request.environment)?;
    let mut startup = STARTUPINFOEXW::default();
    startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = stdin_pipe.child.0;
    startup.StartupInfo.hStdOutput = stdout_pipe.child.0;
    startup.StartupInfo.hStdError = stderr_pipe.child.0;
    startup.lpAttributeList = attributes.pointer;
    let mut information = PROCESS_INFORMATION::default();
    let flags = CREATE_SUSPENDED
        | CREATE_UNICODE_ENVIRONMENT
        | EXTENDED_STARTUPINFO_PRESENT
        | CREATE_NO_WINDOW;
    // SAFETY: all strings/blocks are NUL-terminated and alive during the call;
    // startup handle values are restricted with PROC_THREAD_ATTRIBUTE_HANDLE_LIST.
    // The process starts suspended so it cannot execute before job assignment.
    let created = unsafe {
        CreateProcessW(
            executable.as_ptr(),
            command_line.as_mut_ptr(),
            null(),
            null(),
            1,
            flags,
            environment.as_mut_ptr().cast(),
            current_dir.as_ptr(),
            (&startup.StartupInfo as *const STARTUPINFOW).cast(),
            &mut information,
        )
    };
    if created == 0 {
        return Err(io::Error::last_os_error());
    }
    let process = OwnedHandle::new(information.hProcess)?;
    let thread = OwnedHandle::new(information.hThread)?;
    let mut rollback = SpawnRollback {
        job: &job,
        process: &process,
        committed: false,
    };
    // SAFETY: both handles are owned above; the process's initial thread is still suspended.
    if unsafe { AssignProcessToJobObject(job.0, process.0) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `thread` is the initial thread returned by CreateProcessW and is still suspended.
    if unsafe { ResumeThread(thread.0) } == u32::MAX {
        return Err(io::Error::last_os_error());
    }
    rollback.committed = true;
    drop(rollback);
    drop(thread);
    drop(attributes);
    drop(stdin_pipe.child);
    drop(stdout_pipe.child);
    drop(stderr_pipe.child);
    let child = ManagedChild {
        process,
        job,
        process_id: information.dwProcessId,
    };
    Ok(SpawnedProcess {
        child,
        stdin: stdin_pipe.parent.into_file(),
        stdout: stdout_pipe.parent.into_file(),
        stderr: stderr_pipe.parent.into_file(),
    })
}

struct SpawnRollback<'a> {
    job: &'a OwnedHandle,
    process: &'a OwnedHandle,
    committed: bool,
}

impl Drop for SpawnRollback<'_> {
    fn drop(&mut self) {
        if !self.committed {
            // SAFETY: both handles remain owned and open until rollback completes.
            let _ = unsafe { TerminateJobObject(self.job.0, 1) };
            // SAFETY: process is an owned handle; this covers failure before job assignment.
            let _ = unsafe {
                windows_sys::Win32::System::Threading::TerminateProcess(self.process.0, 1)
            };
            // SAFETY: the process handle remains owned until this rollback guard drops.
            let _ = unsafe { WaitForSingleObject(self.process.0, 1_000) };
        }
    }
}

fn create_job() -> io::Result<OwnedHandle> {
    // SAFETY: null creates an unnamed Job Object with the current process security context.
    let job = unsafe { CreateJobObjectW(null(), null()) };
    let job = OwnedHandle::new(job)?;
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
        | JOB_OBJECT_LIMIT_JOB_MEMORY;
    limits.BasicLimitInformation.ActiveProcessLimit = MAX_ACTIVE_PROCESSES;
    limits.JobMemoryLimit = MAX_JOB_MEMORY_BYTES;
    // SAFETY: `limits` matches the requested information class and remains alive for the call.
    if unsafe {
        SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(job)
}

struct Pipe {
    parent: OwnedHandle,
    child: OwnedHandle,
}

impl Pipe {
    fn new(parent_reads: bool) -> io::Result<Self> {
        let mut read = null_mut();
        let mut write = null_mut();
        let security = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: null_mut(),
            bInheritHandle: 1,
        };
        // SAFETY: output pointers reference writable HANDLEs; security enables inheritance only
        // for the two pipe ends, and the non-child end is made non-inheritable immediately.
        if unsafe { CreatePipe(&mut read, &mut write, &security, 16 * 1024) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let read = OwnedHandle::new(read)?;
        let write = OwnedHandle::new(write)?;
        let (parent, child) = if parent_reads {
            (read, write)
        } else {
            (write, read)
        };
        // SAFETY: `parent` is a valid owned pipe endpoint; this only clears its inherit flag.
        if unsafe { SetHandleInformation(parent.0, HANDLE_FLAG_INHERIT, 0) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { parent, child })
    }
}

struct OwnedHandle(HANDLE);

impl OwnedHandle {
    fn new(handle: HANDLE) -> io::Result<Self> {
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self(handle))
        }
    }

    fn close(&mut self) {
        if !self.0.is_null() {
            // SAFETY: this wrapper uniquely owns the handle and closes it at most once.
            let _ = unsafe { CloseHandle(self.0) };
            self.0 = null_mut();
        }
    }

    fn into_file(self) -> File {
        let mut owned = self;
        let handle = std::mem::replace(&mut owned.0, null_mut());
        drop(owned);
        // SAFETY: ownership of a unique CreatePipe endpoint is transferred to File.
        unsafe { File::from_raw_handle(handle as RawHandle) }
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        self.close();
    }
}

struct HandleAttributeList {
    storage: Vec<usize>,
    pointer: windows_sys::Win32::System::Threading::LPPROC_THREAD_ATTRIBUTE_LIST,
    initialized: bool,
}

impl HandleAttributeList {
    fn new() -> io::Result<Self> {
        let mut required = 0_usize;
        // SAFETY: this first call intentionally queries the required size with a null list.
        let _ = unsafe { InitializeProcThreadAttributeList(null_mut(), 1, 0, &mut required) };
        if required == 0 {
            return Err(io::Error::last_os_error());
        }
        let words = required.div_ceil(size_of::<usize>());
        let mut storage = vec![0_usize; words];
        let pointer = storage.as_mut_ptr().cast();
        // SAFETY: storage is sufficiently sized and usize-aligned for the opaque attribute list.
        if unsafe { InitializeProcThreadAttributeList(pointer, 1, 0, &mut required) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            storage,
            pointer,
            initialized: true,
        })
    }
}

impl Drop for HandleAttributeList {
    fn drop(&mut self) {
        if self.initialized {
            // SAFETY: the list was initialized in this struct and its backing storage is alive.
            unsafe { DeleteProcThreadAttributeList(self.pointer) };
            self.initialized = false;
        }
        let _ = &self.storage;
    }
}

fn validate_input(request: &SpawnRequest<'_>) -> io::Result<()> {
    if request.executable.is_empty()
        || request.executable.encode_wide().any(|unit| unit == 0)
        || request.executable.to_string_lossy().ends_with(".cmd")
        || request.executable.to_string_lossy().ends_with(".bat")
        || !request.current_dir.is_absolute()
        || request
            .arguments
            .iter()
            .any(|argument| argument.encode_wide().any(|unit| unit == 0))
        || request.environment.iter().any(|(key, value)| {
            key.is_empty()
                || key
                    .encode_wide()
                    .any(|unit| unit == 0 || unit == b'=' as u16)
                || value.encode_wide().any(|unit| unit == 0)
        })
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid Windows process input",
        ));
    }
    Ok(())
}

fn wide_z(value: &OsStr) -> io::Result<Vec<u16>> {
    let mut output = value.encode_wide().collect::<Vec<_>>();
    if output.contains(&0) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "embedded NUL"));
    }
    output.push(0);
    Ok(output)
}

fn wide_path(path: &Path) -> io::Result<Vec<u16>> {
    wide_z(path.as_os_str())
}

fn build_command_line(executable: &OsStr, arguments: &[OsString]) -> io::Result<Vec<u16>> {
    let mut values = Vec::with_capacity(arguments.len() + 1);
    values.push(executable.to_os_string());
    values.extend(arguments.iter().cloned());
    let mut output = Vec::new();
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            output.push(b' ' as u16);
        }
        append_quoted_argument(&mut output, value)?;
    }
    output.push(0);
    if output.len() > 32_767 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "command line too long",
        ));
    }
    Ok(output)
}

fn append_quoted_argument(output: &mut Vec<u16>, argument: &OsStr) -> io::Result<()> {
    let units = argument.encode_wide().collect::<Vec<_>>();
    if units.contains(&0) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "embedded NUL"));
    }
    output.push(b'"' as u16);
    let mut backslashes = 0_usize;
    for unit in units {
        if unit == b'\\' as u16 {
            backslashes += 1;
            continue;
        }
        if unit == b'"' as u16 {
            output.extend(std::iter::repeat_n(b'\\' as u16, backslashes * 2 + 1));
            output.push(unit);
        } else {
            output.extend(std::iter::repeat_n(b'\\' as u16, backslashes));
            output.push(unit);
        }
        backslashes = 0;
    }
    output.extend(std::iter::repeat_n(b'\\' as u16, backslashes * 2));
    output.push(b'"' as u16);
    Ok(())
}

fn build_environment(environment: &[(OsString, OsString)]) -> io::Result<Vec<u16>> {
    let mut entries = environment.iter().collect::<Vec<_>>();
    entries.sort_by_key(|(key, _)| key.to_string_lossy().to_ascii_uppercase());
    let mut block = Vec::new();
    for (key, value) in entries {
        let mut key_wide = key.encode_wide().collect::<Vec<_>>();
        let mut value_wide = value.encode_wide().collect::<Vec<_>>();
        if key_wide
            .iter()
            .any(|unit| *unit == 0 || *unit == b'=' as u16)
            || value_wide.contains(&0)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid environment entry",
            ));
        }
        block.append(&mut key_wide);
        block.push(b'=' as u16);
        block.append(&mut value_wide);
        block.push(0);
    }
    if block.is_empty() {
        block.push(0);
    }
    block.push(0);
    if block.len() > 32_767 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "environment block too large",
        ));
    }
    Ok(block)
}
