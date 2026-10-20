//! Window management and keyboard shortcuts through the Win32 API.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct WindowInfo {
    /// HWND as a number (stable while the window exists).
    pub hwnd: i64,
    pub title: String,
    pub process: String,
    pub minimized: bool,
    pub maximized: bool,
    pub active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShowState {
    Normal,
    Minimized,
    Maximized,
}

#[cfg(windows)]
pub use imp::*;

#[cfg(windows)]
mod imp {
    use super::*;
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, SendInput};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GW_OWNER, GWL_EXSTYLE, GetForegroundWindow, GetWindow, GetWindowLongW, GetWindowTextLengthW, GetWindowTextW, IsIconic, IsWindow,
        IsWindowVisible, IsZoomed, PostMessageW, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE, SetForegroundWindow, ShowWindow, WM_CLOSE, WS_EX_TOOLWINDOW,
    };

    fn hwnd(h: i64) -> HWND {
        h as isize as HWND
    }

    fn title(h: HWND) -> String {
        // SAFETY: buffer sized from GetWindowTextLengthW.
        unsafe {
            let len = GetWindowTextLengthW(h);
            if len <= 0 {
                return String::new();
            }
            let mut buf = vec![0u16; len as usize + 1];
            let n = GetWindowTextW(h, buf.as_mut_ptr(), buf.len() as i32);
            String::from_utf16_lossy(&buf[..n.max(0) as usize])
        }
    }

    /// Windows a user sees in Alt+Tab: visible, titled, not tool windows, not cloaked.
    fn is_app_window(h: HWND) -> bool {
        // SAFETY: plain window queries.
        unsafe {
            if IsWindowVisible(h) == 0 || !GetWindow(h, GW_OWNER).is_null() {
                return false;
            }
            if GetWindowLongW(h, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW != 0 {
                return false;
            }
            let mut cloaked: u32 = 0;
            let hr = DwmGetWindowAttribute(h, DWMWA_CLOAKED as u32, (&mut cloaked as *mut u32).cast(), 4);
            if hr == 0 && cloaked != 0 {
                return false;
            }
            GetWindowTextLengthW(h) > 0
        }
    }

    unsafe extern "system" fn collect(h: HWND, data: LPARAM) -> i32 {
        // SAFETY: `data` is the Vec passed by `windows()` for the duration of EnumWindows.
        let list = unsafe { &mut *(data as *mut Vec<HWND>) };
        if is_app_window(h) {
            list.push(h);
        }
        1
    }

    fn info(h: HWND, fg: HWND) -> WindowInfo {
        // SAFETY: plain window queries.
        unsafe {
            WindowInfo {
                hwnd: h as isize as i64,
                title: title(h),
                process: crate::clipboard_hub::win::exe_of_window(h),
                minimized: IsIconic(h) != 0,
                maximized: IsZoomed(h) != 0,
                active: h == fg,
            }
        }
    }

    pub fn windows() -> Vec<WindowInfo> {
        let mut list: Vec<HWND> = Vec::new();
        // SAFETY: the callback only writes into `list`, which outlives the call.
        unsafe {
            EnumWindows(Some(collect), &mut list as *mut Vec<HWND> as LPARAM);
            let fg = GetForegroundWindow();
            list.into_iter().map(|h| info(h, fg)).collect()
        }
    }

    pub fn active() -> Option<WindowInfo> {
        // SAFETY: plain window queries.
        unsafe {
            let fg = GetForegroundWindow();
            (!fg.is_null()).then(|| info(fg, fg))
        }
    }

    pub fn exists(h: i64) -> bool {
        // SAFETY: IsWindow accepts any value.
        unsafe { IsWindow(hwnd(h)) != 0 }
    }

    pub fn state(h: i64) -> ShowState {
        // SAFETY: plain window queries.
        unsafe {
            if IsIconic(hwnd(h)) != 0 {
                ShowState::Minimized
            } else if IsZoomed(hwnd(h)) != 0 {
                ShowState::Maximized
            } else {
                ShowState::Normal
            }
        }
    }

    pub fn show(h: i64, s: ShowState) {
        let cmd = match s {
            ShowState::Minimized => SW_MINIMIZE,
            ShowState::Maximized => SW_MAXIMIZE,
            ShowState::Normal => SW_RESTORE,
        };
        // SAFETY: ShowWindow on a window handle checked by the caller.
        unsafe { ShowWindow(hwnd(h), cmd) };
    }

    pub fn focus(h: i64) -> bool {
        // SAFETY: plain window calls.
        unsafe {
            if IsIconic(hwnd(h)) != 0 {
                ShowWindow(hwnd(h), SW_RESTORE);
            }
            SetForegroundWindow(hwnd(h)) != 0
        }
    }

    /// Asks the window to close (like clicking ✕); the program may ask to save.
    pub fn close(h: i64) -> bool {
        // SAFETY: posting WM_CLOSE to an existing window.
        unsafe { PostMessageW(hwnd(h), WM_CLOSE, 0, 0) != 0 }
    }

    const VK_LWIN: u16 = 0x5B;
    const VK_CONTROL: u16 = 0x11;
    pub const VK_LEFT: u16 = 0x25;
    pub const VK_RIGHT: u16 = 0x27;
    pub const VK_D: u16 = 0x44;
    pub const VK_F4: u16 = 0x73;

    /// Presses Win+Ctrl+`key` (virtual desktop shortcuts).
    pub fn win_ctrl(key: u16) {
        let ext = |vk: u16| if matches!(vk, VK_LEFT | VK_RIGHT | VK_LWIN) { KEYEVENTF_EXTENDEDKEY } else { 0 };
        let k = |vk: u16, up: bool| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: ext(vk) | if up { KEYEVENTF_KEYUP } else { 0 }, time: 0, dwExtraInfo: 0 } },
        };
        let inputs = [k(VK_LWIN, false), k(VK_CONTROL, false), k(key, false), k(key, true), k(VK_CONTROL, true), k(VK_LWIN, true)];
        // SAFETY: a valid array of keyboard inputs.
        unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
    }
}

#[cfg(not(windows))]
pub use stub::*;

#[cfg(not(windows))]
mod stub {
    use super::*;
    pub const VK_LEFT: u16 = 0;
    pub const VK_RIGHT: u16 = 0;
    pub const VK_D: u16 = 0;
    pub const VK_F4: u16 = 0;
    pub fn windows() -> Vec<WindowInfo> {
        vec![]
    }
    pub fn active() -> Option<WindowInfo> {
        None
    }
    pub fn exists(_: i64) -> bool {
        false
    }
    pub fn state(_: i64) -> ShowState {
        ShowState::Normal
    }
    pub fn show(_: i64, _: ShowState) {}
    pub fn focus(_: i64) -> bool {
        false
    }
    pub fn close(_: i64) -> bool {
        false
    }
    pub fn win_ctrl(_: u16) {}
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn lists_windows() {
        // Read-only: there may be no visible windows on a CI agent.
        for w in super::windows() {
            assert!(w.hwnd != 0 && !w.title.is_empty());
        }
        assert!(!super::exists(0));
    }
}
