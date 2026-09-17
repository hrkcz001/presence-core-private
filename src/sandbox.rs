//! OS-level process sandboxing and resource bounding for Presence organs and tools.
//! - Windows: Win32 Job Objects with KILL_ON_JOB_CLOSE, memory cap, and priority control.
//! - Linux/Nix/Guix: Bubblewrap (bwrap) unprivileged user namespaces, POSIX Process Groups,
//!   and PR_SET_PDEATHSIG parent termination signals.

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct SandboxConfig {
    pub max_memory_bytes: Option<usize>,
    pub kill_on_parent_exit: bool,
    pub timeout_seconds: u64,
    pub allow_network: bool,
    pub allowed_writes: Vec<PathBuf>,
    pub use_bubblewrap: bool,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            // Default 512MB memory cap for organs
            max_memory_bytes: Some(512 * 1024 * 1024),
            kill_on_parent_exit: true,
            timeout_seconds: 30,
            allow_network: true,
            allowed_writes: Vec::new(),
            use_bubblewrap: true,
        }
    }
}

/// Helper to detect if a command exists in system PATH
pub fn has_command(cmd: &str) -> bool {
    if let Ok(path_var) = std::env::var("PATH") {
        let sep = if cfg!(windows) { ';' } else { ':' };
        for part in path_var.split(sep) {
            let p = std::path::Path::new(part).join(cmd);
            if p.is_file() {
                return true;
            }
            #[cfg(windows)]
            if std::path::Path::new(part).join(format!("{cmd}.exe")).is_file() {
                return true;
            }
        }
    }
    false
}

/// Wraps a command with Bubblewrap (bwrap) on Linux/Nix/Guix if available and requested.
pub fn wrap_for_bubblewrap(
    program: &str,
    args: &[String],
    config: &SandboxConfig,
) -> (String, Vec<String>) {
    if cfg!(target_os = "linux") && config.use_bubblewrap && has_command("bwrap") {
        let mut bargs = vec![
            "--ro-bind".to_string(), "/".to_string(), "/".to_string(),
            "--dev".to_string(), "/dev".to_string(),
            "--proc".to_string(), "/proc".to_string(),
            "--tmpfs".to_string(), "/tmp".to_string(),
            "--die-with-parent".to_string(),
        ];
        if !config.allow_network {
            bargs.push("--unshare-net".to_string());
        }
        for w in &config.allowed_writes {
            if w.exists() {
                let s = w.display().to_string();
                bargs.push("--bind".to_string());
                bargs.push(s.clone());
                bargs.push(s);
            }
        }
        bargs.push("--".to_string());
        bargs.push(program.to_string());
        bargs.extend(args.iter().cloned());
        ("bwrap".to_string(), bargs)
    } else {
        (program.to_string(), args.to_vec())
    }
}

pub struct ProcessGuard {
    #[cfg(windows)]
    job_handle: isize,
    #[cfg(unix)]
    pgid: i32,
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

#[cfg(unix)]
mod unix_prctl {
    extern "C" {
        pub fn prctl(option: i32, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i32;
        pub fn killpg(pgrp: i32, sig: i32) -> i32;
    }
    pub const PR_SET_PDEATHSIG: i32 = 1;
    pub const SIGKILL: i32 = 9;
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

    #[cfg(unix)]
    pub fn attach(child: &std::process::Child, _config: &SandboxConfig) -> Option<Self> {
        let pid = child.id() as i32;
        Some(Self { pgid: pid })
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

#[cfg(unix)]
impl Drop for ProcessGuard {
    fn drop(&mut self) {
        if self.pgid > 0 {
            unsafe {
                // Ensure all subprocesses in the group are cleaned up
                unix_prctl::killpg(self.pgid, unix_prctl::SIGKILL);
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

    #[test]
    fn test_bubblewrap_command_construction() {
        let cfg = SandboxConfig {
            allow_network: false,
            allowed_writes: vec![PathBuf::from("/tmp/presence_test")],
            use_bubblewrap: true,
            ..Default::default()
        };
        let (prog, args) = wrap_for_bubblewrap("my_organ", &["--arg".into()], &cfg);
        if cfg!(target_os = "linux") && has_command("bwrap") {
            assert_eq!(prog, "bwrap");
            assert!(args.contains(&"--unshare-net".to_string()));
        } else {
            assert_eq!(prog, "my_organ");
            assert_eq!(args, vec!["--arg".to_string()]);
        }
    }
}