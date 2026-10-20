//! Clipboard history: a background task of the «История буфера обмена»
//! module saves copied text to the local database while the module is on and
//! the `clipboard.read` permission is granted. Text that password managers
//! mark as hidden is skipped.

use std::time::Duration;

use serde::Serialize;
use serde_json::Value;
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager};

use crate::error::{AppError, AppResult};
use crate::modules::{catalog::CLIPBOARD, registry};
use crate::permissions::{self, PERM_CLIPBOARD_READ};
use crate::state::AppState;

pub const CHANGED_EVENT: &str = "focusdeck://clipboard-changed";
const POLL: Duration = Duration::from_millis(1000);

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ClipItem {
    pub id: i64,
    pub content: String,
    pub pinned: bool,
    pub created_at: String,
}

pub async fn list(pool: &SqlitePool, limit: i64) -> AppResult<Vec<ClipItem>> {
    Ok(sqlx::query_as::<_, ClipItem>(
        "SELECT id, content, pinned, created_at FROM clipboard_history ORDER BY pinned DESC, id DESC LIMIT ?",
    )
    .bind(limit.clamp(1, 1000))
    .fetch_all(pool)
    .await?)
}

/// Saves text as the newest entry. An equal unpinned entry moves to the top;
/// old unpinned entries above `max_items` are dropped.
pub async fn add(pool: &SqlitePool, text: &str, max_items: i64) -> AppResult<bool> {
    let (pinned,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM clipboard_history WHERE content = ? AND pinned = 1")
        .bind(text)
        .fetch_one(pool)
        .await?;
    if pinned > 0 {
        return Ok(false);
    }
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM clipboard_history WHERE content = ? AND pinned = 0").bind(text).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO clipboard_history (content) VALUES (?)").bind(text).execute(&mut *tx).await?;
    sqlx::query(
        "DELETE FROM clipboard_history WHERE pinned = 0 AND id NOT IN
            (SELECT id FROM clipboard_history WHERE pinned = 0 ORDER BY id DESC LIMIT ?)",
    )
    .bind(max_items)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(true)
}

pub async fn content(pool: &SqlitePool, id: i64) -> AppResult<String> {
    let row: Option<(String,)> = sqlx::query_as("SELECT content FROM clipboard_history WHERE id = ?").bind(id).fetch_optional(pool).await?;
    row.map(|r| r.0).ok_or_else(|| AppError::NotFound("запись буфера обмена".into()))
}

pub async fn set_pinned(pool: &SqlitePool, id: i64, pinned: bool) -> AppResult<()> {
    sqlx::query("UPDATE clipboard_history SET pinned = ? WHERE id = ?").bind(pinned).bind(id).execute(pool).await?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: i64) -> AppResult<()> {
    sqlx::query("DELETE FROM clipboard_history WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

/// Removes everything except pinned entries.
pub async fn clear(pool: &SqlitePool) -> AppResult<()> {
    sqlx::query("DELETE FROM clipboard_history WHERE pinned = 0").execute(pool).await?;
    Ok(())
}

/// Puts text on the Windows clipboard (the «copy again» button).
pub async fn copy(text: String) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || imp::write_text(&text))
        .await
        .map_err(anyhow::Error::from)?
        .map_err(|e| AppError::Other(anyhow::anyhow!("не удалось записать в буфер обмена: {e}")))
}

/// Background task: started and stopped with the module (see `background.rs`).
pub async fn poll_loop(app: AppHandle) {
    // Only text copied while FocusDeck runs is saved, not what was there before.
    let mut last_seq = imp::sequence();
    loop {
        tokio::time::sleep(POLL).await;
        let seq = imp::sequence();
        if seq == last_seq {
            continue;
        }
        last_seq = seq;
        if let Err(e) = capture(&app).await {
            tracing::warn!("clipboard history: {e}");
        }
    }
}

async fn capture(app: &AppHandle) -> AppResult<()> {
    let db = &app.state::<AppState>().db;
    if !permissions::is_granted(db, PERM_CLIPBOARD_READ).await? {
        return Ok(());
    }
    let settings = registry::settings(db, CLIPBOARD).await?;
    let skip_sensitive = settings.get("skip_sensitive").and_then(Value::as_bool).unwrap_or(true);
    let max_len = settings.get("max_length").and_then(Value::as_u64).unwrap_or(20_000) as usize;
    let max_items = settings.get("max_items").and_then(Value::as_i64).unwrap_or(200);

    let text = tauri::async_runtime::spawn_blocking(move || imp::read_text(skip_sensitive))
        .await
        .map_err(anyhow::Error::from)?;
    let Some(text) = text else { return Ok(()) };
    if text.trim().is_empty() || text.chars().count() > max_len {
        return Ok(());
    }
    if add(db, &text, max_items).await? {
        let _ = app.emit(CHANGED_EVENT, ());
    }
    Ok(())
}

#[cfg(windows)]
mod imp {
    use std::thread::sleep;
    use std::time::Duration;

    use windows_sys::Win32::Foundation::GlobalFree;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber, IsClipboardFormatAvailable,
        OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock};

    const CF_UNICODETEXT: u32 = 13;

    pub fn sequence() -> u32 {
        // SAFETY: no arguments.
        unsafe { GetClipboardSequenceNumber() }
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

    fn format(name: &str) -> u32 {
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        // SAFETY: null-terminated UTF-16 string.
        unsafe { RegisterClipboardFormatW(wide.as_ptr()) }
    }

    /// Formats set by password managers and Windows for content that must not be recorded.
    fn is_hidden() -> bool {
        ["ExcludeClipboardContentFromMonitorProcessing", "CanIncludeInClipboardHistory", "Clipboard Viewer Ignore"]
            .iter()
            .enumerate()
            .any(|(i, name)| {
                let f = format(name);
                // SAFETY: plain format id check.
                let present = f != 0 && unsafe { IsClipboardFormatAvailable(f) } != 0;
                if i == 1 {
                    // "CanIncludeInClipboardHistory" = 0 means "don't keep in history".
                    present && can_include_is_zero(f)
                } else {
                    present
                }
            })
    }

    fn can_include_is_zero(f: u32) -> bool {
        if !open() {
            return false;
        }
        // SAFETY: the clipboard is open; the handle is only read while locked.
        let zero = unsafe {
            let h = GetClipboardData(f);
            let p = GlobalLock(h) as *const u32;
            let zero = !p.is_null() && GlobalSize(h) >= 4 && *p == 0;
            if !p.is_null() {
                GlobalUnlock(h);
            }
            zero
        };
        // SAFETY: opened above.
        unsafe { CloseClipboard() };
        zero
    }

    pub fn read_text(skip_hidden: bool) -> Option<String> {
        // SAFETY: plain format id check.
        if unsafe { IsClipboardFormatAvailable(CF_UNICODETEXT) } == 0 {
            return None;
        }
        if skip_hidden && is_hidden() {
            return None;
        }
        if !open() {
            return None;
        }
        // SAFETY: the clipboard is open; the text is copied out while the handle is locked.
        let text = unsafe {
            let h = GetClipboardData(CF_UNICODETEXT);
            let p = GlobalLock(h) as *const u16;
            if p.is_null() {
                None
            } else {
                let max = GlobalSize(h) / 2;
                let len = (0..max).take_while(|&i| *p.add(i) != 0).count();
                let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, len));
                GlobalUnlock(h);
                Some(s)
            }
        };
        // SAFETY: opened above.
        unsafe { CloseClipboard() };
        text
    }

    pub fn write_text(text: &str) -> Result<(), String> {
        let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        if !open() {
            return Err("буфер занят другой программой".into());
        }
        // SAFETY: the clipboard is open; on success the system owns the memory,
        // on failure it is freed here.
        let result = unsafe {
            EmptyClipboard();
            let h = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2);
            let p = GlobalLock(h) as *mut u16;
            if p.is_null() {
                Err("нет памяти".to_string())
            } else {
                std::ptr::copy_nonoverlapping(wide.as_ptr(), p, wide.len());
                GlobalUnlock(h);
                if SetClipboardData(CF_UNICODETEXT, h).is_null() {
                    GlobalFree(h);
                    Err("Windows не принял данные".to_string())
                } else {
                    Ok(())
                }
            }
        };
        // SAFETY: opened above.
        unsafe { CloseClipboard() };
        result
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn sequence() -> u32 {
        0
    }
    pub fn read_text(_skip_hidden: bool) -> Option<String> {
        None
    }
    pub fn write_text(_text: &str) -> Result<(), String> {
        Err("поддерживается только в Windows".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn history_dedupes_and_prunes() {
        let pool = crate::db::test_pool().await;
        for t in ["a", "b", "c", "a"] {
            add(&pool, t, 10).await.unwrap();
        }
        let texts = |v: Vec<ClipItem>| v.into_iter().map(|c| c.content).collect::<Vec<_>>();
        assert_eq!(texts(list(&pool, 10).await.unwrap()), ["a", "c", "b"]);

        let b = list(&pool, 10).await.unwrap().into_iter().find(|c| c.content == "b").unwrap();
        set_pinned(&pool, b.id, true).await.unwrap();
        assert!(!add(&pool, "b", 10).await.unwrap(), "pinned text isn't duplicated");
        for t in ["d", "e"] {
            add(&pool, t, 2).await.unwrap();
        }
        // Two newest unpinned + the pinned one.
        assert_eq!(texts(list(&pool, 10).await.unwrap()), ["b", "e", "d"]);
        clear(&pool).await.unwrap();
        assert_eq!(texts(list(&pool, 10).await.unwrap()), ["b"]);
    }
}
