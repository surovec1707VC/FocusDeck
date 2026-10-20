//! Windows clipboard access: read and write text, HTML, RTF, images (DIB ↔
//! BMP) and file lists (CF_HDROP); the program that owns the clipboard and
//! the title of the foreground window (to skip private browser windows).

/// One clipboard snapshot. Every format that was present is kept.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Clip {
    pub text: Option<String>,
    /// CF_HTML as copied (header + fragment).
    pub html: Option<String>,
    pub rtf: Option<String>,
    /// Complete BMP file and its size.
    pub image: Option<(Vec<u8>, i64, i64)>,
    pub files: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy)]
pub struct ReadOptions {
    pub images: bool,
    pub max_image_bytes: usize,
    pub rich: bool,
    pub skip_hidden: bool,
}

/// DIB (BITMAPINFOHEADER + palette + pixels) → BMP file. Returns (bmp, width, height).
pub fn dib_to_bmp(dib: &[u8]) -> Option<(Vec<u8>, i64, i64)> {
    if dib.len() < 40 {
        return None;
    }
    let u32_at = |i: usize| u32::from_le_bytes(dib[i..i + 4].try_into().unwrap());
    let header = u32_at(0) as usize;
    let width = i32::from_le_bytes(dib[4..8].try_into().unwrap()) as i64;
    let height = (i32::from_le_bytes(dib[8..12].try_into().unwrap()) as i64).abs();
    let bits = u16::from_le_bytes(dib[14..16].try_into().unwrap()) as u32;
    let compression = u32_at(16);
    let used = u32_at(32) as usize;
    let palette = if used > 0 { used } else if bits <= 8 { 1 << bits } else { 0 } * 4;
    let masks = if compression == 3 && header == 40 { 12 } else { 0 };
    let offset = 14 + header + palette + masks;
    if header < 40 || offset > dib.len() + 14 || width <= 0 || height == 0 {
        return None;
    }
    let mut bmp = Vec::with_capacity(dib.len() + 14);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&((dib.len() + 14) as u32).to_le_bytes());
    bmp.extend_from_slice(&[0, 0, 0, 0]);
    bmp.extend_from_slice(&(offset as u32).to_le_bytes());
    bmp.extend_from_slice(dib);
    Some((bmp, width, height))
}

/// Visible text of a CF_HTML fragment (tags dropped, entities decoded).
pub fn html_text(cf_html: &str) -> String {
    let start = cf_html.find("<!--StartFragment-->").map(|i| i + 20).unwrap_or(0);
    let end = cf_html.find("<!--EndFragment-->").unwrap_or(cf_html.len());
    let fragment = &cf_html[start.min(end)..end];
    let mut out = String::new();
    let mut in_tag = false;
    for c in fragment.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    let out = out.replace("&nbsp;", " ").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&amp;", "&");
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Titles of private/incognito browser windows (Edge, Chrome, Firefox, Yandex, Opera, Brave).
pub fn is_private_title(title: &str) -> bool {
    let t = title.to_lowercase();
    ["inprivate", "private browsing", "incognito", "инкогнито", "приватный просмотр", "приватное окно", "private window", "приватный режим"]
        .iter()
        .any(|p| t.contains(p))
}

#[cfg(windows)]
pub use imp::*;

#[cfg(windows)]
mod imp {
    use std::thread::sleep;
    use std::time::Duration;

    use super::{Clip, ReadOptions, dib_to_bmp};
    use windows_sys::Win32::Foundation::{CloseHandle, GlobalFree};
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardOwner, GetClipboardSequenceNumber, IsClipboardFormatAvailable,
        OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock};
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW};
    use windows_sys::Win32::UI::Shell::DragQueryFileW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId};

    const CF_UNICODETEXT: u32 = 13;
    const CF_DIB: u32 = 8;
    const CF_HDROP: u32 = 15;

    pub fn sequence() -> u32 {
        // SAFETY: no arguments.
        unsafe { GetClipboardSequenceNumber() }
    }

    fn format(name: &str) -> u32 {
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        // SAFETY: null-terminated UTF-16 string.
        unsafe { RegisterClipboardFormatW(wide.as_ptr()) }
    }

    fn available(f: u32) -> bool {
        // SAFETY: plain format id check.
        f != 0 && unsafe { IsClipboardFormatAvailable(f) } != 0
    }

    /// Another program may hold the clipboard for a moment: retry briefly.
    fn open() -> bool {
        for _ in 0..10 {
            // SAFETY: a null owner window is allowed.
            if unsafe { OpenClipboard(std::ptr::null_mut()) } != 0 {
                return true;
            }
            sleep(Duration::from_millis(20));
        }
        false
    }

    /// Bytes of a clipboard format; the clipboard must be open.
    unsafe fn bytes(f: u32, max: usize) -> Option<Vec<u8>> {
        unsafe {
            let h = GetClipboardData(f);
            if h.is_null() {
                return None;
            }
            let size = GlobalSize(h);
            if size == 0 || size > max {
                return None;
            }
            let p = GlobalLock(h) as *const u8;
            if p.is_null() {
                return None;
            }
            let v = std::slice::from_raw_parts(p, size).to_vec();
            GlobalUnlock(h);
            Some(v)
        }
    }

    /// UTF-8 (as the formats require); some programs write the ANSI code page instead.
    fn decode_8bit(b: &[u8]) -> String {
        if let Ok(s) = std::str::from_utf8(b) {
            return s.to_string();
        }
        use windows_sys::Win32::Globalization::{CP_ACP, MultiByteToWideChar};
        // SAFETY: sizes come from the input; the buffer is sized by the first call.
        unsafe {
            let n = MultiByteToWideChar(CP_ACP, 0, b.as_ptr(), b.len() as i32, std::ptr::null_mut(), 0);
            if n > 0 {
                let mut wide = vec![0u16; n as usize];
                MultiByteToWideChar(CP_ACP, 0, b.as_ptr(), b.len() as i32, wide.as_mut_ptr(), n);
                return String::from_utf16_lossy(&wide);
            }
        }
        String::from_utf8_lossy(b).into_owned()
    }

    /// Text up to the first NUL.
    fn until_nul(b: &[u8]) -> &[u8] {
        &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
    }

    /// Formats set by password managers and Windows for content that must not be recorded.
    fn is_hidden() -> bool {
        if available(format("ExcludeClipboardContentFromMonitorProcessing")) || available(format("Clipboard Viewer Ignore")) {
            return true;
        }
        let f = format("CanIncludeInClipboardHistory");
        if !available(f) || !open() {
            return false;
        }
        // SAFETY: the clipboard is open.
        let zero = unsafe { bytes(f, 16) }.is_some_and(|b| b.len() >= 4 && b[..4] == [0, 0, 0, 0]);
        // SAFETY: opened above.
        unsafe { CloseClipboard() };
        zero
    }

    pub fn read(opts: ReadOptions) -> Option<Clip> {
        if opts.skip_hidden && is_hidden() {
            return None;
        }
        let (html_f, rtf_f) = (format("HTML Format"), format("Rich Text Format"));
        if !open() {
            return None;
        }
        let mut clip = Clip::default();
        // SAFETY: the clipboard is open for the whole block; data is copied out.
        unsafe {
            if available(CF_UNICODETEXT) {
                if let Some(b) = bytes(CF_UNICODETEXT, 64 << 20) {
                    let units: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|&u| u != 0).collect();
                    clip.text = Some(String::from_utf16_lossy(&units));
                }
            }
            if opts.rich && available(html_f) {
                clip.html = bytes(html_f, 16 << 20).map(|b| decode_8bit(until_nul(&b)));
            }
            if opts.rich && available(rtf_f) {
                clip.rtf = bytes(rtf_f, 16 << 20).map(|b| decode_8bit(until_nul(&b)));
            }
            if available(CF_HDROP) {
                let h = GetClipboardData(CF_HDROP);
                if !h.is_null() {
                    let count = DragQueryFileW(h, u32::MAX, std::ptr::null_mut(), 0);
                    let files: Vec<String> = (0..count.min(1000))
                        .filter_map(|i| {
                            let len = DragQueryFileW(h, i, std::ptr::null_mut(), 0);
                            let mut buf = vec![0u16; len as usize + 1];
                            (DragQueryFileW(h, i, buf.as_mut_ptr(), buf.len() as u32) > 0).then(|| String::from_utf16_lossy(&buf[..len as usize]))
                        })
                        .collect();
                    if !files.is_empty() {
                        clip.files = Some(files);
                    }
                }
            }
            if opts.images && clip.files.is_none() && available(CF_DIB) {
                clip.image = bytes(CF_DIB, opts.max_image_bytes).and_then(|d| dib_to_bmp(&d));
            }
            CloseClipboard();
        }
        (clip != Clip::default()).then_some(clip)
    }

    /// Puts a global memory block on the open clipboard.
    unsafe fn put(f: u32, data: &[u8]) -> bool {
        unsafe {
            let h = GlobalAlloc(GMEM_MOVEABLE, data.len().max(1));
            let p = GlobalLock(h) as *mut u8;
            if p.is_null() {
                return false;
            }
            std::ptr::copy_nonoverlapping(data.as_ptr(), p, data.len());
            GlobalUnlock(h);
            if SetClipboardData(f, h).is_null() {
                GlobalFree(h);
                return false;
            }
            true
        }
    }

    pub fn write(clip: &Clip) -> Result<(), String> {
        let (html_f, rtf_f) = (format("HTML Format"), format("Rich Text Format"));
        if !open() {
            return Err("буфер занят другой программой".into());
        }
        // SAFETY: the clipboard is open; ownership of each block passes to Windows.
        let ok = unsafe {
            EmptyClipboard();
            let mut ok = true;
            if let Some(t) = &clip.text {
                let wide: Vec<u8> = t.encode_utf16().chain(Some(0)).flat_map(u16::to_le_bytes).collect();
                ok &= put(CF_UNICODETEXT, &wide);
            }
            if let Some(h) = &clip.html {
                ok &= put(html_f, &[h.as_bytes(), &[0]].concat());
            }
            if let Some(r) = &clip.rtf {
                ok &= put(rtf_f, &[r.as_bytes(), &[0]].concat());
            }
            if let Some((bmp, _, _)) = &clip.image {
                ok &= bmp.len() > 14 && put(CF_DIB, &bmp[14..]);
            }
            if let Some(files) = &clip.files {
                // DROPFILES { pFiles = 20, pt = 0,0, fNC = 0, fWide = 1 } + paths + double NUL.
                let mut data: Vec<u8> = Vec::new();
                for v in [20u32, 0, 0, 0, 1] {
                    data.extend_from_slice(&v.to_le_bytes());
                }
                for f in files {
                    data.extend(f.encode_utf16().chain(Some(0)).flat_map(u16::to_le_bytes));
                }
                data.extend_from_slice(&[0, 0]);
                ok &= put(CF_HDROP, &data);
            }
            CloseClipboard();
            ok
        };
        if ok { Ok(()) } else { Err("Windows не принял часть данных".into()) }
    }

    fn exe_of_window(hwnd: *mut core::ffi::c_void) -> String {
        if hwnd.is_null() {
            return String::new();
        }
        let mut pid = 0u32;
        // SAFETY: valid out pointer; handles are closed.
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid == 0 {
                return String::new();
            }
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process.is_null() {
                return String::new();
            }
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(process, 0, buf.as_mut_ptr(), &mut len) != 0;
            CloseHandle(process);
            if !ok {
                return String::new();
            }
            let path = String::from_utf16_lossy(&buf[..len as usize]);
            path.rsplit('\\').next().unwrap_or("").to_string()
        }
    }

    /// (exe of the clipboard owner or the foreground window, foreground window title).
    pub fn source() -> (String, String) {
        // SAFETY: plain window queries.
        unsafe {
            let fg = GetForegroundWindow();
            let mut title = [0u16; 512];
            let n = if fg.is_null() { 0 } else { GetWindowTextW(fg, title.as_mut_ptr(), title.len() as i32) };
            let title = String::from_utf16_lossy(&title[..n.max(0) as usize]);
            let mut exe = exe_of_window(GetClipboardOwner());
            if exe.is_empty() {
                exe = exe_of_window(fg);
            }
            (exe, title)
        }
    }
}

#[cfg(not(windows))]
pub use stub::*;

#[cfg(not(windows))]
mod stub {
    use super::{Clip, ReadOptions};
    pub fn sequence() -> u32 {
        0
    }
    pub fn read(_: ReadOptions) -> Option<Clip> {
        None
    }
    pub fn write(_: &Clip) -> Result<(), String> {
        Err("поддерживается только в Windows".into())
    }
    pub fn source() -> (String, String) {
        (String::new(), String::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dib_conversion() {
        // 2×1, 24 bit: header 40 bytes + 8 bytes of pixels (rows padded to 4).
        let mut dib = vec![0u8; 48];
        dib[0..4].copy_from_slice(&40u32.to_le_bytes());
        dib[4..8].copy_from_slice(&2i32.to_le_bytes());
        dib[8..12].copy_from_slice(&(-1i32).to_le_bytes());
        dib[14..16].copy_from_slice(&24u16.to_le_bytes());
        let (bmp, w, h) = dib_to_bmp(&dib).unwrap();
        assert_eq!((w, h), (2, 1));
        assert_eq!(&bmp[0..2], b"BM");
        assert_eq!(u32::from_le_bytes(bmp[10..14].try_into().unwrap()), 54);
        assert!(dib_to_bmp(&[0; 10]).is_none());
    }

    #[test]
    fn html_and_private_windows() {
        let cf = "Version:0.9\r\nStartHTML:0\r\n<html><body><!--StartFragment--><b>Привет</b>&nbsp;мир &amp; <i>всё</i><!--EndFragment--></body></html>";
        assert_eq!(html_text(cf), "Привет мир & всё");
        assert!(is_private_title("Новая вкладка — Личный — Microsoft​ Edge InPrivate"));
        assert!(is_private_title("Новая вкладка - Google Chrome (Инкогнито)"));
        assert!(!is_private_title("Документ — Word"));
    }
}
