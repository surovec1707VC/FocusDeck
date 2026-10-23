//! File-system guards shared by modules that touch user files.
//!
//! - `inside`: a relative path from the UI must stay inside a base folder
//!   (no `..`, no absolute paths, no escaping through links).
//! - `check_user_folder`: folders where FocusDeck may move or recycle files
//!   must not be system locations (drive roots, Windows, Program Files,
//!   ProgramData, the profile root itself, FocusDeck's own data).

use std::path::{Component, Path, PathBuf};

use crate::error::{AppError, AppResult};

/// Normalises `.`/`..` lexically and lowercases for comparison (Windows paths are case-insensitive).
fn key(p: &Path) -> String {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    let s = out.to_string_lossy().replace('/', "\\").to_lowercase();
    s.trim_end_matches('\\').to_string()
}

/// `base.join(rel)` if `rel` is a plain relative path that stays inside `base`
/// (checked lexically and, when the file exists, after resolving links).
pub fn inside(base: &Path, rel: &str) -> AppResult<PathBuf> {
    let p = Path::new(rel);
    let bad = rel.is_empty()
        || rel.contains('\0')
        || p.is_absolute()
        || p.components().any(|c| matches!(c, Component::ParentDir | Component::Prefix(_) | Component::RootDir));
    if bad {
        return Err(AppError::Invalid(format!("недопустимый путь «{rel}»")));
    }
    let full = base.join(p);
    if let (Ok(real), Ok(real_base)) = (full.canonicalize(), base.canonicalize()) {
        if !real.starts_with(&real_base) {
            return Err(AppError::Invalid(format!("путь «{rel}» ведёт за пределы папки")));
        }
    }
    Ok(full)
}

fn env_dir(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty()).map(|v| key(Path::new(&v)))
}

/// Folders FocusDeck never moves or recycles files in (or out of).
fn protected() -> Vec<(String, bool)> {
    // (path, also every subfolder)
    let mut list = Vec::new();
    for var in ["WINDIR", "SystemRoot", "ProgramFiles", "ProgramFiles(x86)", "ProgramW6432", "ProgramData"] {
        if let Some(d) = env_dir(var) {
            list.push((d, true));
        }
    }
    if let Some(appdata) = env_dir("APPDATA") {
        list.push((format!("{appdata}\\com.focusdeck.app"), true));
    }
    if let Some(profile) = env_dir("USERPROFILE") {
        list.push((profile, false));
    }
    list
}

pub fn check_user_folder(path: &Path) -> AppResult<()> {
    if !path.is_absolute() {
        return Err(AppError::Invalid("нужен полный путь к папке".into()));
    }
    let k = key(path);
    // A drive root: "c:" or a UNC share root.
    if k.len() <= 3 || k.matches('\\').count() == 0 {
        return Err(AppError::PermissionDenied(format!("корень диска {} — защищённая папка", path.display())));
    }
    for (p, subtree) in protected() {
        if k == p || (subtree && k.starts_with(&format!("{p}\\"))) {
            return Err(AppError::PermissionDenied(format!("{} — системная или защищённая папка", path.display())));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths_stay_inside() {
        let base = std::env::temp_dir();
        assert!(inside(&base, "a/b.txt").is_ok());
        for bad in ["", "..", "../x", "a/../../x", "C:\\Windows\\win.ini", "\\\\server\\share\\x", "/etc/passwd", "a\0b"] {
            assert!(inside(&base, bad).is_err(), "{bad:?} must be rejected");
        }
    }

    #[test]
    fn system_folders_are_protected() {
        let win = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
        for bad in [win.clone(), format!("{win}\\System32"), "C:\\".into(), "C:".into(), format!("{win}\\..\\Windows\\Temp")] {
            assert!(check_user_folder(Path::new(&bad)).is_err(), "{bad} must be protected");
        }
        if let Ok(pf) = std::env::var("ProgramFiles") {
            assert!(check_user_folder(&Path::new(&pf).join("App")).is_err());
        }
        if let Ok(profile) = std::env::var("USERPROFILE") {
            assert!(check_user_folder(Path::new(&profile)).is_err());
            assert!(check_user_folder(&Path::new(&profile).join("Downloads")).is_ok());
        }
        assert!(check_user_folder(Path::new("relative\\dir")).is_err());
    }
}
