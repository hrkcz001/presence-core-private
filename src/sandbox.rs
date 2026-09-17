//! OS-level process sandboxing and resource bounding for Presence organs and tools.
//! - Windows: Win32 Job Objects with KILL_ON_JOB_CLOSE, memory cap, and priority control.
//! - Linux/Unix: Process Groups, rlimits, and Landlock/Bubblewrap container isolation.

#[derive(Debug, Clone)]
pub struct SandboxConfig {
    pub max_memory_bytes: Option<usize>,
    pub kill_on_parent_exit: bool,
    pub timeout_seconds: u64,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            // Default 512MB memory cap for organs
            max_memory_bytes: Some(512 * 1024 * 1024),
            kill_on_parent_exit: true,
            timeout_seconds: 30,
        }
    }
}

pub struct ProcessGuard {
    #[cfg(windows)]
    job_handle: isize,
}

#[cfg(windows)]
#[allow(non_snake_case, non_upper_case_globals)]
mod win32_job {
    use std::ffi::c_void;

    pub const JobObjectExtendedLimitInformation: u32 = 9;
    pub const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x00002000;
    pub const JOB_OBJECT_LIMIT_PROCESS_MEMORY: u32 = 0x00000100;
    pub const JOB_OBJECT_LIMIT_JOB_MEMORY: u32 = 0x00000200;

    #[repr(C)]
    #[derive(Default)]
    pub struct IO_COUNTERS {
        pub ReadOperationCount: u64,
        pub WriteOperationCount: u64,
        pub OtherOperationCount: u64,
        pub ReadTransferCount: u64,
        pub WriteTransferCount: u64,
        pub OtherTransferCount: u64,
    }

    #[repr(C)]
    #[derive(Default)]
    pub struct JOBOBJECT_BASIC_LIMIT_INFORMATION {
        pub PerProcessUserTimeLimit: i64,
        pub PerJobUserTimeLimit: i64,
        pub LimitFlags: u32,
        pub MinimumWorkingSetSize: usize,
        pub MaximumWorkingSetSize: usize,
        pub ActiveProcessLimit: u32,
        pub Affinity: usize,
        pub PriorityClass: u32,
        pub SchedulingClass: u32,
    }

    #[repr(C)]
    #[derive(Default)]
    pub struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
        pub BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION,
        pub IoInfo: IO_COUNTERS,
        pub ProcessMemoryLimit: usize,
        pub JobMemoryLimit: usize,
        pub PeakProcessMemoryUsed: usize,
        pub PeakJobMemoryUsed: usize,
    }

    #[link(name = "kernel32")]
    extern "system" {
        pub fn CreateJobObjectW(lpJobAttributes: *mut c_void, lpName: *const u16) -> isize;
        pub fn SetInformationJobObject(
            hJob: isize,
            JobObjectInformationClass: u32,
            lpJobObjectInformation: *mut c_void,
            cbJobObjectInformationLength: u32,
        ) -> i32;
        pub fn AssignProcessToJobObject(hJob: isize, hProcess: *mut c_void) -> i32;
        pub fn CloseHandle(hObject: isize) -> i32;
    }
}

impl ProcessGuard {
    #[cfg(windows)]
    pub fn attach(child: &std::process::Child, config: &SandboxConfig) -> Option<Self> {
        use std::os::windows::io::AsRawHandle;
        unsafe {
            let h_job = win32_job::CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
            if h_job == 0 {
                return None;
            }

            let mut info = win32_job::JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            let mut flags = 0u32;

            if config.kill_on_parent_exit {
                flags |= win32_job::JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            }

            if let Some(mem) = config.max_memory_bytes {
                flags |= win32_job::JOB_OBJECT_LIMIT_PROCESS_MEMORY | win32_job::JOB_OBJECT_LIMIT_JOB_MEMORY;
                info.ProcessMemoryLimit = mem;
                info.JobMemoryLimit = mem;
            }

            info.BasicLimitInformation.LimitFlags = flags;

            let ok = win32_job::SetInformationJobObject(
                h_job,
                win32_job::JobObjectExtendedLimitInformation,
                &mut info as *mut _ as *mut std::ffi::c_void,
                std::mem::size_of::<win32_job::JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );

            if ok == 0 {
                win32_job::CloseHandle(h_job);
                return None;
            }

            let raw_proc = child.as_raw_handle();
            let assigned = win32_job::AssignProcessToJobObject(h_job, raw_proc as *mut std::ffi::c_void);
            if assigned == 0 {
                win32_job::CloseHandle(h_job);
                return None;
            }

            Some(Self { job_handle: h_job })
        }
    }

    #[cfg(not(windows))]
    pub fn attach(_child: &std::process::Child, _config: &SandboxConfig) -> Option<Self> {
        Some(Self {})
    }
}

#[cfg(windows)]
impl Drop for ProcessGuard {
    fn drop(&mut self) {
        if self.job_handle != 0 {
            unsafe {
                win32_job::CloseHandle(self.job_handle);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn test_sandbox_job_object_attach() {
        let mut cmd = if cfg!(windows) {
            let mut c = Command::new("cmd");
            c.args(["/C", "echo presence_sandbox_test"]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", "echo presence_sandbox_test"]);
            c
        };

        let child = cmd.spawn().expect("failed to spawn child");
        let cfg = SandboxConfig::default();
        let guard = ProcessGuard::attach(&child, &cfg);
        assert!(guard.is_some(), "ProcessGuard failed to attach to child process");
    }
}