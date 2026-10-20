//! The clipboard widget uses the Clipboard Hub history (`crate::clipboard_hub`).

pub use crate::clipboard_hub::store::{clear, delete, list, set_pinned};

use crate::error::AppResult;

/// Puts an entry back on the clipboard with all its formats.
pub async fn copy_entry(pool: &sqlx::SqlitePool, id: i64) -> AppResult<()> {
    crate::clipboard_hub::write(crate::clipboard_hub::store::clip(pool, id).await?).await?;
    crate::clipboard_hub::store::touch(pool, id).await
}

/// Puts plain text on the clipboard.
pub async fn copy(text: String) -> AppResult<()> {
    crate::clipboard_hub::copy_text(&text).await
}
