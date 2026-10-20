//! Clipboard history in SQLite.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use super::win::{Clip, html_text};
use crate::error::{AppError, AppResult};

pub const KINDS: &[&str] = &["text", "link", "files", "image", "html", "rtf"];

/// A history entry without the heavy payload (image, HTML, RTF).
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ClipItem {
    pub id: i64,
    /// text | link | files | image | html | rtf
    pub kind: String,
    /// Readable text: the text itself, a file list or an image summary.
    pub content: String,
    pub pinned: bool,
    pub source_app: String,
    pub size: i64,
    pub image_width: Option<i64>,
    pub image_height: Option<i64>,
    pub files_json: Option<String>,
    pub has_html: bool,
    pub has_rtf: bool,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ClipFilter {
    pub search: Option<String>,
    pub kind: Option<String>,
    #[serde(default)]
    pub pinned_only: bool,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ClipPage {
    pub items: Vec<ClipItem>,
    /// Matching entries in total (for paging).
    pub total: i64,
}

/// FNV-1a: a stable hash to spot repeated copies.
pub fn hash(parts: &[&[u8]]) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for part in parts {
        for b in part.iter().chain(&[0xff]) {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    }
    format!("{h:016x}")
}

pub fn is_link(text: &str) -> bool {
    let t = text.trim();
    (t.starts_with("http://") || t.starts_with("https://")) && !t.contains(char::is_whitespace) && t.len() > 8
}

/// What gets stored for a clip: kind, readable content, size, hash.
pub fn describe(clip: &Clip) -> (&'static str, String, i64, String) {
    let text = clip.text.clone().unwrap_or_default();
    let (kind, content) = if let Some(files) = &clip.files {
        ("files", files.join("\n"))
    } else if let Some((_, w, h)) = &clip.image {
        ("image", if text.is_empty() { format!("Изображение {w}×{h}") } else { text.clone() })
    } else if clip.html.is_some() && !is_link(&text) {
        let t = if text.is_empty() { html_text(clip.html.as_deref().unwrap_or("")) } else { text.clone() };
        ("html", t)
    } else if clip.rtf.is_some() && !is_link(&text) {
        ("rtf", text.clone())
    } else if is_link(&text) {
        ("link", text.trim().to_string())
    } else {
        ("text", text.clone())
    };
    let image = clip.image.as_ref().map(|i| i.0.as_slice()).unwrap_or(&[]);
    let files_len = clip.files.as_ref().map_or(0, |f| f.iter().map(String::len).sum());
    let size = [text.len(), clip.html.as_ref().map_or(0, String::len), clip.rtf.as_ref().map_or(0, String::len), image.len(), files_len]
        .iter()
        .sum::<usize>() as i64;
    let files = clip.files.as_ref().map(|f| f.join("\n")).unwrap_or_default();
    // Text + files + image identify a copy; formatting differences don't make a new entry.
    let h = hash(&[kind.as_bytes(), text.as_bytes(), files.as_bytes(), image]);
    (kind, content, size, h)
}

/// Saves a clip as the newest entry. A repeated unpinned copy moves to the
/// top; if a pinned copy exists, nothing is added. Old unpinned entries above
/// `max_items` are removed.
pub async fn add(pool: &SqlitePool, clip: &Clip, source_app: &str, max_items: i64) -> AppResult<bool> {
    let (kind, content, size, h) = describe(clip);
    if content.trim().is_empty() {
        return Ok(false);
    }
    let (pinned,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM clipboard_history WHERE hash = ? AND pinned = 1").bind(&h).fetch_one(pool).await?;
    if pinned > 0 {
        return Ok(false);
    }
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM clipboard_history WHERE hash = ? AND pinned = 0").bind(&h).execute(&mut *tx).await?;
    sqlx::query(
        "INSERT INTO clipboard_history (kind, content, text, html, rtf, image, image_width, image_height, files_json, source_app, hash, size)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(kind)
    .bind(&content)
    .bind(&clip.text)
    .bind(&clip.html)
    .bind(&clip.rtf)
    .bind(clip.image.as_ref().map(|i| i.0.clone()))
    .bind(clip.image.as_ref().map(|i| i.1))
    .bind(clip.image.as_ref().map(|i| i.2))
    .bind(clip.files.as_ref().map(|f| serde_json::to_string(f).unwrap_or_default()))
    .bind(source_app)
    .bind(&h)
    .bind(size)
    .execute(&mut *tx)
    .await?;
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

pub async fn list(pool: &SqlitePool, limit: i64) -> AppResult<Vec<ClipItem>> {
    Ok(query(pool, &ClipFilter { limit: Some(limit), ..Default::default() }).await?.items)
}

/// Filtered page: pinned first, newest first.
pub async fn query(pool: &SqlitePool, f: &ClipFilter) -> AppResult<ClipPage> {
    if f.kind.as_ref().is_some_and(|k| !KINDS.contains(&k.as_str())) {
        return Err(AppError::Invalid("неизвестный тип записи".into()));
    }
    let search = f.search.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(|s| format!("%{}%", s.replace('!', "!!").replace('%', "!%").replace('_', "!_")));
    let items = sqlx::query_as::<_, ClipItem>(
        "SELECT id, kind, content, pinned, source_app, size, image_width, image_height, files_json,
            html IS NOT NULL AS has_html, rtf IS NOT NULL AS has_rtf, created_at, last_used_at
         FROM clipboard_history
         WHERE (?1 IS NULL OR kind = ?1) AND (?2 = 0 OR pinned = 1)
           AND (?3 IS NULL OR content LIKE ?3 ESCAPE '!' OR source_app LIKE ?3 ESCAPE '!')
         ORDER BY pinned DESC, id DESC LIMIT ?4 OFFSET ?5",
    )
    .bind(&f.kind)
    .bind(f.pinned_only)
    .bind(&search)
    .bind(f.limit.unwrap_or(100).clamp(1, 1000))
    .bind(f.offset.unwrap_or(0).max(0))
    .fetch_all(pool)
    .await?;
    let (total,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM clipboard_history
         WHERE (?1 IS NULL OR kind = ?1) AND (?2 = 0 OR pinned = 1)
           AND (?3 IS NULL OR content LIKE ?3 ESCAPE '!' OR source_app LIKE ?3 ESCAPE '!')",
    )
        .bind(&f.kind)
        .bind(f.pinned_only)
        .bind(&search)
        .fetch_one(pool)
        .await?;
    Ok(ClipPage { items, total })
}

#[derive(sqlx::FromRow)]
struct FullRow {
    text: Option<String>,
    content: String,
    kind: String,
    html: Option<String>,
    rtf: Option<String>,
    image: Option<Vec<u8>>,
    image_width: Option<i64>,
    image_height: Option<i64>,
    files_json: Option<String>,
}

/// Everything needed to put the entry back on the clipboard.
pub async fn clip(pool: &SqlitePool, id: i64) -> AppResult<Clip> {
    let r = sqlx::query_as::<_, FullRow>(
        "SELECT text, content, kind, html, rtf, image, image_width, image_height, files_json FROM clipboard_history WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("запись буфера обмена".into()))?;
    let text = r.text.or_else(|| matches!(r.kind.as_str(), "text" | "link").then_some(r.content));
    Ok(Clip {
        text,
        html: r.html,
        rtf: r.rtf,
        image: r.image.map(|b| (b, r.image_width.unwrap_or(0), r.image_height.unwrap_or(0))),
        files: r.files_json.and_then(|j| serde_json::from_str(&j).ok()),
    })
}

pub async fn image(pool: &SqlitePool, id: i64) -> AppResult<Vec<u8>> {
    let row: Option<(Option<Vec<u8>>,)> = sqlx::query_as("SELECT image FROM clipboard_history WHERE id = ?").bind(id).fetch_optional(pool).await?;
    row.and_then(|r| r.0).ok_or_else(|| AppError::NotFound("изображение".into()))
}

pub async fn touch(pool: &SqlitePool, id: i64) -> AppResult<()> {
    sqlx::query("UPDATE clipboard_history SET last_used_at = datetime('now') WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

pub async fn set_pinned(pool: &SqlitePool, id: i64, pinned: bool) -> AppResult<()> {
    sqlx::query("UPDATE clipboard_history SET pinned = ? WHERE id = ?").bind(pinned).bind(id).execute(pool).await?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: i64) -> AppResult<()> {
    sqlx::query("DELETE FROM clipboard_history WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

/// Removes unpinned entries (or everything with `include_pinned`).
pub async fn clear_all(pool: &SqlitePool, include_pinned: bool) -> AppResult<u64> {
    Ok(sqlx::query("DELETE FROM clipboard_history WHERE pinned = 0 OR ?").bind(include_pinned).execute(pool).await?.rows_affected())
}

pub async fn clear(pool: &SqlitePool) -> AppResult<()> {
    clear_all(pool, false).await.map(|_| ())
}

/// Unpinned entries older than `days` (0 = keep forever).
pub async fn purge_older_than(pool: &SqlitePool, days: i64) -> AppResult<u64> {
    if days <= 0 {
        return Ok(0);
    }
    Ok(sqlx::query("DELETE FROM clipboard_history WHERE pinned = 0 AND created_at < datetime('now', ?)")
        .bind(format!("-{days} days"))
        .execute(pool)
        .await?
        .rows_affected())
}

#[derive(Debug, Serialize)]
pub struct ClipStats {
    pub total: i64,
    pub pinned: i64,
    pub bytes: i64,
    pub by_kind: Vec<(String, i64)>,
}

pub async fn stats(pool: &SqlitePool) -> AppResult<ClipStats> {
    let (total, pinned, bytes): (i64, Option<i64>, Option<i64>) =
        sqlx::query_as("SELECT COUNT(*), SUM(pinned), SUM(size) FROM clipboard_history").fetch_one(pool).await?;
    let by_kind = sqlx::query_as("SELECT kind, COUNT(*) FROM clipboard_history GROUP BY kind ORDER BY kind").fetch_all(pool).await?;
    Ok(ClipStats { total, pinned: pinned.unwrap_or(0), bytes: bytes.unwrap_or(0), by_kind })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(t: &str) -> Clip {
        Clip { text: Some(t.into()), ..Default::default() }
    }

    #[test]
    fn kinds() {
        assert_eq!(describe(&text("https://example.com/a")).0, "link");
        assert_eq!(describe(&text("см. https://example.com")).0, "text");
        let html = Clip { text: Some("Привет".into()), html: Some("<b>Привет</b>".into()), ..Default::default() };
        assert_eq!(describe(&html).0, "html");
        let files = Clip { files: Some(vec![r"C:\a.txt".into(), r"C:\b".into()]), ..Default::default() };
        assert_eq!(describe(&files), ("files", "C:\\a.txt\nC:\\b".into(), 12, describe(&files).3));
        let img = Clip { image: Some((vec![1, 2, 3], 4, 5)), ..Default::default() };
        assert_eq!(describe(&img).1, "Изображение 4×5");
    }

    #[tokio::test]
    async fn history_filters_and_round_trip() {
        let pool = crate::db::test_pool().await;
        for t in ["первый", "https://example.com", "второй", "первый"] {
            add(&pool, &text(t), "notepad.exe", 10).await.unwrap();
        }
        let img = Clip { image: Some((vec![9; 100], 10, 10)), ..Default::default() };
        add(&pool, &img, "mspaint.exe", 10).await.unwrap();

        let all = query(&pool, &ClipFilter::default()).await.unwrap();
        assert_eq!(all.total, 4, "repeated copy moved to the top");
        assert_eq!(all.items[1].content, "первый");
        let links = query(&pool, &ClipFilter { kind: Some("link".into()), ..Default::default() }).await.unwrap();
        assert_eq!(links.total, 1);
        let found = query(&pool, &ClipFilter { search: Some("паинт".into()), ..Default::default() }).await.unwrap();
        assert_eq!(found.total, 0);
        let by_app = query(&pool, &ClipFilter { search: Some("mspaint".into()), ..Default::default() }).await.unwrap();
        assert_eq!(by_app.items[0].kind, "image");
        assert_eq!(clip(&pool, by_app.items[0].id).await.unwrap(), img);
        assert!(query(&pool, &ClipFilter { kind: Some("video".into()), ..Default::default() }).await.is_err());

        set_pinned(&pool, all.items[1].id, true).await.unwrap();
        assert!(!add(&pool, &text("первый"), "x", 10).await.unwrap());
        assert_eq!(clear_all(&pool, false).await.unwrap(), 3);
        assert_eq!(stats(&pool).await.unwrap().pinned, 1);
        assert_eq!(purge_older_than(&pool, 0).await.unwrap(), 0);
    }
}
