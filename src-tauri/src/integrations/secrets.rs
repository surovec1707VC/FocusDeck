//! Integration secrets (tokens, API keys, passwords) live in Windows
//! Credential Manager as generic credentials `FocusDeck/integration/<id>`,
//! never in SQLite, settings or logs. The UI can save or remove a secret
//! and see whether one exists, but never reads it back.

use crate::error::{AppError, AppResult};

pub const MAX_LEN: usize = 4096;

pub fn target(id: &str) -> String {
    format!("FocusDeck/integration/{id}")
}

pub fn check(secret: &str) -> AppResult<&str> {
    let s = secret.trim();
    if s.is_empty() || s.len() > MAX_LEN {
        return Err(AppError::Invalid(format!("секрет: от 1 до {MAX_LEN} символов")));
    }
    if s.chars().any(char::is_control) {
        return Err(AppError::Invalid("секрет содержит управляющие символы".into()));
    }
    Ok(s)
}

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::Foundation::{ERROR_NOT_FOUND, GetLastError};
    use windows_sys::Win32::Security::Credentials::{
        CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW, CredFree, CredReadW, CredWriteW,
    };

    use crate::error::{AppError, AppResult};

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn os_error(what: &str, code: u32) -> AppError {
        AppError::Other(anyhow::anyhow!("Credential Manager: {what} (код {code})"))
    }

    pub fn write(target: &str, secret: &str) -> AppResult<()> {
        let mut name = wide(target);
        let mut user = wide("FocusDeck");
        let mut blob = secret.as_bytes().to_vec();
        // SAFETY: every pointer refers to a live local buffer for the duration of the call.
        let mut cred: CREDENTIALW = unsafe { std::mem::zeroed() };
        cred.Type = CRED_TYPE_GENERIC;
        cred.TargetName = name.as_mut_ptr();
        cred.UserName = user.as_mut_ptr();
        cred.CredentialBlobSize = blob.len() as u32;
        cred.CredentialBlob = blob.as_mut_ptr();
        cred.Persist = CRED_PERSIST_LOCAL_MACHINE;
        let ok = unsafe { CredWriteW(&cred, 0) };
        blob.fill(0);
        if ok == 0 { Err(os_error("не удалось сохранить секрет", unsafe { GetLastError() })) } else { Ok(()) }
    }

    pub fn read(target: &str) -> AppResult<Option<String>> {
        let name = wide(target);
        let mut out: *mut CREDENTIALW = std::ptr::null_mut();
        // SAFETY: `out` is freed with CredFree; the blob is copied before that.
        unsafe {
            if CredReadW(name.as_ptr(), CRED_TYPE_GENERIC, 0, &mut out) == 0 {
                let code = GetLastError();
                return if code == ERROR_NOT_FOUND { Ok(None) } else { Err(os_error("не удалось прочитать секрет", code)) };
            }
            let c = &*out;
            let bytes = std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize).to_vec();
            CredFree(out as *const _);
            Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
        }
    }

    pub fn exists(target: &str) -> bool {
        matches!(read(target), Ok(Some(_)))
    }

    pub fn delete(target: &str) -> AppResult<()> {
        let name = wide(target);
        // SAFETY: plain call with a NUL-terminated string.
        unsafe {
            if CredDeleteW(name.as_ptr(), CRED_TYPE_GENERIC, 0) == 0 {
                let code = GetLastError();
                if code != ERROR_NOT_FOUND {
                    return Err(os_error("не удалось удалить секрет", code));
                }
            }
        }
        Ok(())
    }
}

#[cfg(not(windows))]
mod imp {
    use crate::error::{AppError, AppResult};

    fn unsupported() -> AppError {
        AppError::Invalid("хранилище секретов есть только в Windows".into())
    }
    pub fn write(_: &str, _: &str) -> AppResult<()> {
        Err(unsupported())
    }
    pub fn read(_: &str) -> AppResult<Option<String>> {
        Ok(None)
    }
    pub fn exists(_: &str) -> bool {
        false
    }
    pub fn delete(_: &str) -> AppResult<()> {
        Ok(())
    }
}

pub fn save(id: &str, secret: &str) -> AppResult<()> {
    imp::write(&target(id), check(secret)?)
}

/// For connector code only; never sent to the UI.
#[allow(dead_code)]
pub fn read(id: &str) -> AppResult<Option<String>> {
    imp::read(&target(id))
}

pub fn exists(id: &str) -> bool {
    imp::exists(&target(id))
}

pub fn remove(id: &str) -> AppResult<()> {
    imp::delete(&target(id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_rules() {
        assert_eq!(target("github"), "FocusDeck/integration/github");
        assert_eq!(check("  ghp_abc  ").unwrap(), "ghp_abc");
        assert!(check("   ").is_err());
        assert!(check("a\nb").is_err());
        assert!(check(&"x".repeat(MAX_LEN + 1)).is_err());
    }
}
