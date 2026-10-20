//! System information for the «Система» widget: CPU load, memory, fixed
//! disks, battery and uptime, read through the Windows API. Sampling sleeps
//! briefly, so callers run it on a blocking thread.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Disk {
    pub mount: String,
    pub total: u64,
    pub free: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Battery {
    pub percent: Option<u8>,
    pub on_ac: bool,
    pub charging: bool,
    /// Remaining time on battery, when Windows knows it.
    pub seconds_left: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemInfo {
    pub cpu_percent: Option<f32>,
    pub cores: usize,
    pub ram_total: u64,
    pub ram_used: u64,
    pub disks: Vec<Disk>,
    /// `None` on computers without a battery.
    pub battery: Option<Battery>,
    pub uptime_secs: u64,
}

/// Last snapshot: several widgets refreshing at once share one reading.
static CACHE: Mutex<Option<(Instant, SystemInfo)>> = Mutex::new(None);
const CACHE_TTL: Duration = Duration::from_millis(900);

pub fn snapshot() -> SystemInfo {
    if let Some((at, info)) = CACHE.lock().unwrap().as_ref() {
        if at.elapsed() < CACHE_TTL {
            return info.clone();
        }
    }
    let info = imp::read();
    *CACHE.lock().unwrap() = Some((Instant::now(), info.clone()));
    info
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives};
    use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    use windows_sys::Win32::System::SystemInformation::{GetTickCount64, GlobalMemoryStatusEx, MEMORYSTATUSEX};
    use windows_sys::Win32::System::Threading::GetSystemTimes;

    const DRIVE_FIXED: u32 = 3;

    /// (idle, kernel + user) in 100 ns units; kernel time includes idle time.
    fn cpu_times() -> Option<(u64, u64)> {
        let mut idle = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        let mut kernel = idle;
        let mut user = idle;
        // SAFETY: valid out pointers to stack values.
        let ok = unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } != 0;
        let v = |f: FILETIME| ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64;
        ok.then(|| (v(idle), v(kernel) + v(user)))
    }

    static LAST_CPU: Mutex<Option<(Instant, u64, u64)>> = Mutex::new(None);

    /// Load since the previous call, or over a short sample on the first call.
    fn cpu_percent() -> Option<f32> {
        let now = cpu_times()?;
        let prev = LAST_CPU.lock().unwrap().take();
        let (idle0, total0) = match prev {
            Some((at, i, t)) if at.elapsed() >= Duration::from_millis(200) && at.elapsed() < Duration::from_secs(120) => (i, t),
            _ => {
                std::thread::sleep(Duration::from_millis(250));
                let later = cpu_times()?;
                *LAST_CPU.lock().unwrap() = Some((Instant::now(), later.0, later.1));
                return load(now, later);
            }
        };
        *LAST_CPU.lock().unwrap() = Some((Instant::now(), now.0, now.1));
        load((idle0, total0), now)
    }

    fn load(a: (u64, u64), b: (u64, u64)) -> Option<f32> {
        let total = b.1.checked_sub(a.1)?;
        let idle = b.0.checked_sub(a.0)?;
        (total > 0).then(|| ((total.saturating_sub(idle)) as f32 / total as f32 * 100.0).clamp(0.0, 100.0))
    }

    fn memory() -> (u64, u64) {
        let mut m: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
        m.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        // SAFETY: dwLength is set as the API requires.
        if unsafe { GlobalMemoryStatusEx(&mut m) } == 0 {
            return (0, 0);
        }
        (m.ullTotalPhys, m.ullTotalPhys.saturating_sub(m.ullAvailPhys))
    }

    fn disks() -> Vec<Disk> {
        // SAFETY: no arguments.
        let mask = unsafe { GetLogicalDrives() };
        (0..26u8)
            .filter(|i| mask & (1 << i) != 0)
            .filter_map(|i| {
                let mount = format!("{}:\\", (b'A' + i) as char);
                let wide: Vec<u16> = mount.encode_utf16().chain(Some(0)).collect();
                // SAFETY: null-terminated UTF-16 path and valid out pointers.
                unsafe {
                    if GetDriveTypeW(wide.as_ptr()) != DRIVE_FIXED {
                        return None;
                    }
                    let (mut avail, mut total, mut free) = (0u64, 0u64, 0u64);
                    if GetDiskFreeSpaceExW(wide.as_ptr(), &mut avail, &mut total, &mut free) == 0 {
                        return None;
                    }
                    Some(Disk { mount: mount.trim_end_matches('\\').to_string(), total, free: avail })
                }
            })
            .collect()
    }

    fn battery() -> Option<Battery> {
        let mut s: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
        // SAFETY: valid out pointer.
        if unsafe { GetSystemPowerStatus(&mut s) } == 0 {
            return None;
        }
        // 128 = no system battery, 255 = unknown.
        if s.BatteryFlag & 128 != 0 || s.BatteryFlag == 255 {
            return None;
        }
        Some(Battery {
            percent: (s.BatteryLifePercent <= 100).then_some(s.BatteryLifePercent),
            on_ac: s.ACLineStatus == 1,
            charging: s.BatteryFlag & 8 != 0,
            seconds_left: (s.BatteryLifeTime != u32::MAX).then_some(s.BatteryLifeTime),
        })
    }

    pub fn read() -> SystemInfo {
        let (ram_total, ram_used) = memory();
        SystemInfo {
            cpu_percent: cpu_percent(),
            cores: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
            ram_total,
            ram_used,
            disks: disks(),
            battery: battery(),
            // SAFETY: no arguments.
            uptime_secs: unsafe { GetTickCount64() } / 1000,
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub fn read() -> SystemInfo {
        SystemInfo {
            cpu_percent: None,
            cores: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
            ram_total: 0,
            ram_used: 0,
            disks: Vec::new(),
            battery: None,
            uptime_secs: 0,
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn reads_something() {
        let info = super::snapshot();
        assert!(info.ram_total > 0);
        assert!(info.ram_used <= info.ram_total);
        assert!(!info.disks.is_empty());
        assert!(info.cpu_percent.is_some_and(|c| (0.0..=100.0).contains(&c)));
    }
}
