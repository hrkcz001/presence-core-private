//! Windows environment sensory organ implemented in pure Rust via Win32 FFI.
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ForegroundInfo {
    pub title: String,
    pub pid: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WindowInfo {
    pub title: String,
    pub pid: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct PowerInfo {
    pub power_source: String,
    pub battery_percent: Option<u8>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Overview {
    pub foreground: ForegroundInfo,
    pub user_idle_seconds: f64,
    pub power: PowerInfo,
    pub open_windows: Vec<WindowInfo>,
    pub audio_devices: Vec<String>,
}

#[cfg(windows)]
mod win32 {
    #[repr(C)]
    pub struct LASTINPUTINFO {
        pub cb_size: u32,
        pub dw_time: u32,
    }

    #[repr(C)]
    pub struct SYSTEM_POWER_STATUS {
        pub ac_line_status: u8,
        pub battery_flag: u8,
        pub battery_life_percent: u8,
        pub system_status_flag: u8,
        pub battery_life_time: u32,
        pub battery_full_life_time: u32,
    }

    #[link(name = "user32")]
    extern "system" {
        pub fn OpenDesktopW(
            lpszDesktop: *const u16,
            dwFlags: u32,
            fInherit: i32,
            dwDesiredAccess: u32,
        ) -> isize;
        pub fn SetThreadDesktop(hDesktop: isize) -> i32;
        pub fn GetForegroundWindow() -> isize;
        pub fn GetWindowTextLengthW(hWnd: isize) -> i32;
        pub fn GetWindowTextW(hWnd: isize, lpString: *mut u16, nMaxCount: i32) -> i32;
        pub fn GetWindowThreadProcessId(hWnd: isize, lpdwProcessId: *mut u32) -> u32;
        pub fn IsWindowVisible(hWnd: isize) -> i32;
        pub fn EnumDesktopWindows(
            hDesktop: isize,
            lpfn: Option<unsafe extern "system" fn(isize, isize) -> i32>,
            lParam: isize,
        ) -> i32;
        pub fn GetLastInputInfo(plii: *mut LASTINPUTINFO) -> i32;
        pub fn GetTickCount() -> u32;
        pub fn GetSystemPowerStatus(lpSystemPowerStatus: *mut SYSTEM_POWER_STATUS) -> i32;
    }
}

#[cfg(windows)]
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn attach_default_desktop() -> isize {
    #[cfg(windows)]
    unsafe {
        let name = to_wide("Default");
        let h = win32::OpenDesktopW(name.as_ptr(), 0, 0, 0x01FF);
        if h != 0 {
            win32::SetThreadDesktop(h);
        }
        h
    }
    #[cfg(not(windows))]
    0
}

pub fn get_foreground() -> ForegroundInfo {
    #[cfg(windows)]
    unsafe {
        attach_default_desktop();
        let fg = win32::GetForegroundWindow();
        if fg == 0 {
            return ForegroundInfo::default();
        }
        let len = win32::GetWindowTextLengthW(fg);
        if len <= 0 {
            return ForegroundInfo::default();
        }
        let mut buf = vec![0u16; (len + 1) as usize];
        win32::GetWindowTextW(fg, buf.as_mut_ptr(), len + 1);
        let title = String::from_utf16_lossy(&buf[..len as usize]);
        let mut pid = 0u32;
        win32::GetWindowThreadProcessId(fg, &mut pid);
        ForegroundInfo { title, pid }
    }
    #[cfg(not(windows))]
    ForegroundInfo::default()
}

pub fn get_user_idle_seconds() -> f64 {
    #[cfg(windows)]
    unsafe {
        let mut lii = win32::LASTINPUTINFO {
            cb_size: std::mem::size_of::<win32::LASTINPUTINFO>() as u32,
            dw_time: 0,
        };
        if win32::GetLastInputInfo(&mut lii) != 0 {
            let tick = win32::GetTickCount();
            let diff = tick.wrapping_sub(lii.dw_time);
            return (diff as f64) / 1000.0;
        }
    }
    0.0
}

pub fn get_power_status() -> PowerInfo {
    #[cfg(windows)]
    unsafe {
        let mut sps = std::mem::zeroed::<win32::SYSTEM_POWER_STATUS>();
        if win32::GetSystemPowerStatus(&mut sps) != 0 {
            let power_source = if sps.ac_line_status == 1 {
                "ac_connected".to_string()
            } else {
                "battery".to_string()
            };
            let battery_percent = if sps.battery_life_percent <= 100 {
                Some(sps.battery_life_percent)
            } else {
                None
            };
            return PowerInfo {
                power_source,
                battery_percent,
            };
        }
    }
    PowerInfo {
        power_source: "unknown".into(),
        battery_percent: None,
    }
}

#[cfg(windows)]
struct EnumState {
    windows: Vec<WindowInfo>,
}

#[cfg(windows)]
unsafe extern "system" fn enum_cb(hwnd: isize, lparam: isize) -> i32 {
    let state = &mut *(lparam as *mut EnumState);
    if win32::IsWindowVisible(hwnd) != 0 {
        let len = win32::GetWindowTextLengthW(hwnd);
        if len > 0 {
            let mut buf = vec![0u16; (len + 1) as usize];
            win32::GetWindowTextW(hwnd, buf.as_mut_ptr(), len + 1);
            let title = String::from_utf16_lossy(&buf[..len as usize]).trim().to_string();
            if !title.is_empty()
                && title != "Program Manager"
                && title != "Windows Input Experience"
                && title != "Settings"
            {
                let mut pid = 0u32;
                win32::GetWindowThreadProcessId(hwnd, &mut pid);
                state.windows.push(WindowInfo { title, pid });
            }
        }
    }
    1
}

pub fn get_open_windows() -> Vec<WindowInfo> {
    #[cfg(windows)]
    unsafe {
        let h = attach_default_desktop();
        let mut state = EnumState { windows: Vec::new() };
        win32::EnumDesktopWindows(h, Some(enum_cb), &mut state as *mut _ as isize);
        state.windows
    }
    #[cfg(not(windows))]
    Vec::new()
}

pub fn get_audio_devices() -> Vec<String> {
    #[cfg(windows)]
    {
        use std::process::Command;
        use std::os::windows::process::CommandExt;
        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-Command", "Get-PnpDevice -Class AudioEndpoint -Status OK | Select-Object -ExpandProperty FriendlyName"]);
        cmd.creation_flags(0x08000000);
        if let Ok(out) = cmd.output() {
            if out.status.success() {
                return String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty())
                    .collect();
            }
        }
    }
    Vec::new()
}

pub fn get_overview() -> Overview {
    Overview {
        foreground: get_foreground(),
        user_idle_seconds: get_user_idle_seconds(),
        power: get_power_status(),
        open_windows: get_open_windows(),
        audio_devices: get_audio_devices(),
    }
}

pub fn query(action: &str) -> String {
    match action {
        "foreground" => serde_json::to_string_pretty(&get_foreground()).unwrap_or_default(),
        "windows" => serde_json::to_string_pretty(&get_open_windows()).unwrap_or_default(),
        "idle" => serde_json::json!({ "idle_seconds": get_user_idle_seconds() }).to_string(),
        "power" => serde_json::to_string_pretty(&get_power_status()).unwrap_or_default(),
        "audio" => serde_json::to_string_pretty(&get_audio_devices()).unwrap_or_default(),
        "overview" | _ => serde_json::to_string_pretty(&get_overview()).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_idle() {
        let idle = get_user_idle_seconds();
        assert!(idle >= 0.0);
    }

    #[test]
    fn test_power_status() {
        let p = get_power_status();
        assert!(!p.power_source.is_empty());
    }

    #[test]
    fn test_query_overview() {
        let s = query("overview");
        assert!(s.contains("foreground"));
        assert!(s.contains("user_idle_seconds"));
    }
}
