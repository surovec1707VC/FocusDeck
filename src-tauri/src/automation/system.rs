//! Windows probes and effects used by automations: the foreground program,
//! running processes, monitor count, connectivity, screenshots, moving files
//! to the Recycle Bin and closing programs.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// "OBS64.EXE" / "obs64" → "obs64.exe".
pub fn normalize_process(name: &str) -> String {
    let n = name.trim().to_lowercase();
    if n.ends_with(".exe") { n } else { format!("{n}.exe") }
}

/// Expands `%VAR%` with the current environment (unknown variables are kept).
pub fn expand_env(path: &str) -> String {
    let mut out = String::new();
    let mut rest = path;
    while let Some(a) = rest.find('%') {
        let Some(b) = rest[a + 1..].find('%') else { break };
        out += &rest[..a];
        let name = &rest[a + 1..a + 1 + b];
        match std::env::var(name) {
            Ok(v) if !name.is_empty() => out += &v,
            _ => out += &rest[a..a + b + 2],
        }
        rest = &rest[a + b + 2..];
    }
    out + rest
}

/// `*.tmp`, `report-??.txt` — case-insensitive wildcard match.
pub fn wildcard(pattern: &str, name: &str) -> bool {
    let (p, n): (Vec<char>, Vec<char>) = (pattern.to_lowercase().chars().collect(), name.to_lowercase().chars().collect());
    let (mut pi, mut ni, mut star, mut mark) = (0, 0, None, 0);
    while ni < n.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == n[ni]) {
            pi += 1;
            ni += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ni;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ni = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

/// Files (not folders) in `folder` matching `pattern`, older than `days` (0 = any age).
pub fn matching_files(folder: &Path, pattern: &str, days: u32) -> std::io::Result<Vec<PathBuf>> {
    let cutoff = std::time::SystemTime::now() - std::time::Duration::from_secs(days as u64 * 86_400);
    let mut out = Vec::new();
    for entry in std::fs::read_dir(folder)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        if !meta.is_file() || !wildcard(pattern, &entry.file_name().to_string_lossy()) {
            continue;
        }
        if days > 0 && meta.modified().map(|m| m > cutoff).unwrap_or(true) {
            continue;
        }
        out.push(entry.path());
    }
    Ok(out)
}

#[cfg(windows)]
pub use imp::*;

#[cfg(windows)]
mod imp {
    use super::*;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, DeleteObject,
        GetDC, GetDIBits, ReleaseDC, SRCCOPY, SelectObject,
    };
    use windows_sys::Win32::Networking::WinInet::InternetGetConnectedState;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS};
    use windows_sys::Win32::UI::Shell::{FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, SHFILEOPSTRUCTW, SHFileOperationW};
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CMONITORS, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN};

    /// (exe, title) of the foreground window.
    pub fn foreground() -> (String, String) {
        crate::clipboard_hub::win::foreground()
    }

    /// Lower-case exe names of all running processes.
    pub fn processes() -> HashSet<String> {
        let mut out = HashSet::new();
        // SAFETY: snapshot handle is closed; entry has dwSize set.
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap.is_null() || snap as isize == -1 {
                return out;
            }
            let mut e: PROCESSENTRY32W = std::mem::zeroed();
            e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut ok = Process32FirstW(snap, &mut e) != 0;
            while ok {
                let len = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
                out.insert(String::from_utf16_lossy(&e.szExeFile[..len]).to_lowercase());
                ok = Process32NextW(snap, &mut e) != 0;
            }
            CloseHandle(snap);
        }
        out
    }

    pub fn monitors() -> i32 {
        // SAFETY: no pointers.
        unsafe { GetSystemMetrics(SM_CMONITORS) }
    }

    pub fn online() -> bool {
        let mut flags = 0u32;
        // SAFETY: valid out pointer.
        unsafe { InternetGetConnectedState(&mut flags, 0) != 0 }
    }

    pub fn uptime_secs() -> u64 {
        // SAFETY: no arguments.
        unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() / 1000 }
    }

    /// The whole virtual screen (all monitors) as a 24-bit BMP file.
    pub fn screenshot_bmp() -> Result<Vec<u8>, String> {
        // SAFETY: GDI objects are released in reverse order; buffers are sized from the bitmap.
        unsafe {
            let (x, y) = (GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_YVIRTUALSCREEN));
            let (w, h) = (GetSystemMetrics(SM_CXVIRTUALSCREEN), GetSystemMetrics(SM_CYVIRTUALSCREEN));
            if w <= 0 || h <= 0 {
                return Err("не удалось определить размер экрана".into());
            }
            let screen = GetDC(std::ptr::null_mut());
            let mem = CreateCompatibleDC(screen);
            let bmp = CreateCompatibleBitmap(screen, w, h);
            let old = SelectObject(mem, bmp);
            let copied = BitBlt(mem, 0, 0, w, h, screen, x, y, SRCCOPY) != 0;
            let stride = ((w * 3 + 3) & !3) as usize;
            let mut pixels = vec![0u8; stride * h as usize];
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: h,
                biPlanes: 1,
                biBitCount: 24,
                biCompression: BI_RGB,
                biSizeImage: pixels.len() as u32,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            };
            let lines = GetDIBits(mem, bmp, 0, h as u32, pixels.as_mut_ptr().cast(), &mut info, DIB_RGB_COLORS);
            SelectObject(mem, old);
            DeleteObject(bmp);
            DeleteDC(mem);
            ReleaseDC(std::ptr::null_mut(), screen);
            if !copied || lines == 0 {
                return Err("не удалось снять экран".into());
            }
            let mut file = Vec::with_capacity(54 + pixels.len());
            file.extend_from_slice(b"BM");
            file.extend_from_slice(&((54 + pixels.len()) as u32).to_le_bytes());
            file.extend_from_slice(&[0, 0, 0, 0]);
            file.extend_from_slice(&54u32.to_le_bytes());
            let header: [u8; 40] = std::mem::transmute(info.bmiHeader);
            file.extend_from_slice(&header);
            file.extend_from_slice(&pixels);
            Ok(file)
        }
    }

    /// Moves files to the Recycle Bin (can be restored from there).
    pub fn recycle(files: &[PathBuf]) -> Result<(), String> {
        if files.is_empty() {
            return Ok(());
        }
        // Double-NUL-terminated list of NUL-separated paths.
        let mut from: Vec<u16> = Vec::new();
        for f in files {
            from.extend(f.as_os_str().to_string_lossy().encode_utf16());
            from.push(0);
        }
        from.push(0);
        // SAFETY: `from` outlives the call; other fields are zeroed.
        let rc = unsafe {
            let mut op: SHFILEOPSTRUCTW = std::mem::zeroed();
            op.wFunc = FO_DELETE;
            op.pFrom = from.as_ptr();
            op.fFlags = (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_NOERRORUI | FOF_SILENT) as u16;
            SHFileOperationW(&mut op)
        };
        if rc == 0 { Ok(()) } else { Err(format!("Windows вернул код {rc}")) }
    }
}

#[cfg(not(windows))]
pub use stub::*;

#[cfg(not(windows))]
mod stub {
    use super::*;
    pub fn foreground() -> (String, String) {
        (String::new(), String::new())
    }
    pub fn processes() -> HashSet<String> {
        HashSet::new()
    }
    pub fn monitors() -> i32 {
        1
    }
    pub fn online() -> bool {
        true
    }
    pub fn uptime_secs() -> u64 {
        u64::MAX
    }
    pub fn screenshot_bmp() -> Result<Vec<u8>, String> {
        Err("только в Windows".into())
    }
    pub fn recycle(_: &[PathBuf]) -> Result<(), String> {
        Err("только в Windows".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(normalize_process(" OBS64 "), "obs64.exe");
        assert_eq!(normalize_process("Code.EXE"), "code.exe");
        assert!(wildcard("*.tmp", "Report.TMP"));
        assert!(wildcard("log-??.txt", "log-01.txt"));
        assert!(!wildcard("*.tmp", "a.tmp.txt"));
        assert!(wildcard("*", "anything"));
        unsafe { std::env::set_var("FD_TEST_DIR", r"C:\Data") };
        assert_eq!(expand_env(r"%FD_TEST_DIR%\x\%NOPE_FD%"), r"C:\Data\x\%NOPE_FD%");
    }

    #[cfg(windows)]
    #[test]
    fn probes() {
        assert!(!processes().is_empty());
        assert!(monitors() >= 1);
        assert!(uptime_secs() > 0);
    }

    #[test]
    fn file_matching() {
        let dir = std::env::temp_dir().join(format!("fd-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.tmp"), "x").unwrap();
        std::fs::write(dir.join("b.txt"), "x").unwrap();
        let found = matching_files(&dir, "*.tmp", 0).unwrap();
        assert_eq!(found.len(), 1);
        assert!(matching_files(&dir, "*.tmp", 30).unwrap().is_empty(), "too new");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
