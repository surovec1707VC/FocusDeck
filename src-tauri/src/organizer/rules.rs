//! Sorting rules: which files a rule takes (category, extension, name,
//! date, size — all set conditions must match) and the target folder
//! template. Rules are checked top to bottom; the first match wins.

use chrono::{DateTime, Datelike, Local};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::automation::system::wildcard;
use crate::db::new_id;
use crate::error::{AppError, AppResult};

pub const CATEGORIES: &[&str] = &["screenshots", "images", "documents", "archives", "installers", "video", "music", "code", "other"];

const IMAGES: &[&str] = &["jpg", "jpeg", "png", "gif", "bmp", "webp", "heic", "heif", "tif", "tiff", "svg", "ico", "avif", "raw", "cr2", "nef", "psd"];
const DOCUMENTS: &[&str] = &[
    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp", "rtf", "txt", "md", "csv", "epub", "djvu", "fb2", "xps", "pages", "numbers", "key",
];
const ARCHIVES: &[&str] = &["zip", "rar", "7z", "tar", "gz", "bz2", "xz", "tgz", "zst", "cab", "iso", "img"];
const INSTALLERS: &[&str] = &["exe", "msi", "msix", "msixbundle", "appx", "appxbundle", "appinstaller"];
const VIDEO: &[&str] = &["mp4", "mkv", "avi", "mov", "wmv", "webm", "flv", "m4v", "mpg", "mpeg", "3gp", "ts"];
const MUSIC: &[&str] = &["mp3", "wav", "flac", "aac", "ogg", "m4a", "wma", "opus", "aiff", "mid", "midi"];
const CODE: &[&str] = &[
    "js", "mjs", "cjs", "jsx", "ts", "tsx", "py", "ipynb", "rs", "go", "java", "kt", "c", "h", "cpp", "hpp", "cc", "cs", "php", "rb", "swift", "html", "htm",
    "css", "scss", "json", "xml", "yaml", "yml", "toml", "ini", "sql", "sh", "ps1", "bat", "cmd", "vue", "svelte", "sln", "csproj", "lua", "dart", "r",
];
/// Default file names of Windows, ShareX, Lightshot, macOS-style tools, Steam…
const SCREENSHOT_NAMES: &[&str] = &["screenshot*", "screen shot*", "снимок экрана*", "скриншот*", "screenshot_*", "capture*", "lightshot*", "sharex*"];

/// Built-in category of a file by name and extension.
pub fn category(name: &str) -> &'static str {
    let ext = extension(name);
    let lower = name.to_lowercase();
    if IMAGES.contains(&ext.as_str()) && SCREENSHOT_NAMES.iter().any(|p| wildcard(p, &lower)) {
        return "screenshots";
    }
    for (cat, list) in [("images", IMAGES), ("documents", DOCUMENTS), ("archives", ARCHIVES), ("installers", INSTALLERS), ("video", VIDEO), ("music", MUSIC), ("code", CODE)] {
        if list.contains(&ext.as_str()) {
            return cat;
        }
    }
    "other"
}

pub fn extension(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => ext.to_lowercase(),
        _ => String::new(),
    }
}

/// What a rule takes. Empty / missing fields don't restrict.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Match {
    /// Built-in categories (any of).
    #[serde(default)]
    pub categories: Vec<String>,
    /// Extensions without the dot (any of).
    #[serde(default)]
    pub extensions: Vec<String>,
    /// Wildcard on the file name, e.g. `Счёт*` or `*отчёт*.pdf`.
    #[serde(default)]
    pub name_pattern: String,
    /// Only files last modified more than N days ago.
    #[serde(default)]
    pub older_than_days: Option<u32>,
    /// Only files modified within the last N days.
    #[serde(default)]
    pub newer_than_days: Option<u32>,
    #[serde(default)]
    pub min_size_kb: Option<u64>,
    #[serde(default)]
    pub max_size_kb: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub position: i64,
    #[serde(rename = "match")]
    pub matcher: Match,
    /// Folder template: `{root}`, `{category}`, `{ext}`, `{year}`, `{month}`, `{day}`.
    pub target: String,
    /// Key of a default rule.
    pub builtin: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuleInput {
    pub name: String,
    pub enabled: bool,
    #[serde(rename = "match")]
    pub matcher: Match,
    pub target: String,
}

/// A file as seen by the rules.
pub struct FileInfo<'a> {
    pub name: &'a str,
    pub size: u64,
    pub modified: DateTime<Local>,
}

impl Match {
    pub fn matches(&self, f: &FileInfo, now: DateTime<Local>) -> bool {
        let ext = extension(f.name);
        let age_days = (now - f.modified).num_days();
        (self.categories.is_empty() || self.categories.iter().any(|c| c == category(f.name)))
            && (self.extensions.is_empty() || self.extensions.iter().any(|e| e.trim_start_matches('.').eq_ignore_ascii_case(&ext)))
            && (self.name_pattern.trim().is_empty() || wildcard(self.name_pattern.trim(), f.name))
            && self.older_than_days.is_none_or(|d| age_days >= d as i64)
            && self.newer_than_days.is_none_or(|d| age_days < d as i64)
            && self.min_size_kb.is_none_or(|k| f.size >= k * 1024)
            && self.max_size_kb.is_none_or(|k| f.size <= k * 1024)
    }

    pub fn is_empty(&self) -> bool {
        *self == Match::default()
    }
}

/// Fills the folder template. `{root}` is the module's target root.
pub fn target_dir(template: &str, root: &str, f: &FileInfo) -> String {
    let t = template.trim();
    let filled = t
        .replace("{root}", root)
        .replace("{category}", category_title(category(f.name)))
        .replace("{ext}", &extension(f.name))
        .replace("{year}", &f.modified.year().to_string())
        .replace("{month}", &format!("{:02}", f.modified.month()))
        .replace("{day}", &format!("{:02}", f.modified.day()));
    let expanded = crate::automation::system::expand_env(&filled);
    // A relative target goes under the root.
    if std::path::Path::new(&expanded).is_absolute() { expanded } else { format!("{}\\{}", crate::automation::system::expand_env(root).trim_end_matches('\\'), expanded) }
}

pub fn category_title(c: &str) -> &'static str {
    match c {
        "screenshots" => "Скриншоты",
        "images" => "Изображения",
        "documents" => "Документы",
        "archives" => "Архивы",
        "installers" => "Установщики",
        "video" => "Видео",
        "music" => "Музыка",
        "code" => "Код и проекты",
        _ => "Прочее",
    }
}

pub fn validate(r: &RuleInput) -> AppResult<()> {
    let n = r.name.trim().chars().count();
    if n == 0 || n > 60 {
        return Err(AppError::Invalid("название правила — от 1 до 60 символов".into()));
    }
    let t = r.target.trim();
    if t.is_empty() || t.len() > 500 {
        return Err(AppError::Invalid("укажите целевую папку".into()));
    }
    if t.contains("..") || t.contains(['*', '?', '"', '<', '>', '|']) {
        return Err(AppError::Invalid("в целевой папке не должно быть «..» и символов * ? \" < > |".into()));
    }
    if let Some(c) = r.matcher.categories.iter().find(|c| !CATEGORIES.contains(&c.as_str())) {
        return Err(AppError::Invalid(format!("неизвестная категория {c}")));
    }
    if r.matcher.extensions.len() > 50 || r.matcher.name_pattern.len() > 200 {
        return Err(AppError::Invalid("слишком длинное условие".into()));
    }
    if r.matcher.is_empty() {
        return Err(AppError::Invalid("задайте хотя бы одно условие (категорию, расширение, имя, дату или размер)".into()));
    }
    Ok(())
}

// --- defaults ---

fn cat(c: &str) -> Match {
    Match { categories: vec![c.into()], ..Default::default() }
}

/// (key, name, match, target). Screenshots are grouped by month.
pub fn defaults() -> Vec<(&'static str, &'static str, Match, &'static str)> {
    vec![
        ("screenshots", "Скриншоты", cat("screenshots"), "{root}\\Скриншоты\\{year}-{month}"),
        ("images", "Изображения", cat("images"), "{root}\\Изображения"),
        ("documents", "Документы", cat("documents"), "{root}\\Документы"),
        ("archives", "Архивы", cat("archives"), "{root}\\Архивы"),
        ("installers", "Установщики", cat("installers"), "{root}\\Установщики"),
        ("video", "Видео", cat("video"), "{root}\\Видео"),
        ("music", "Музыка", cat("music"), "{root}\\Музыка"),
        ("code", "Код и проекты", cat("code"), "{root}\\Код"),
        ("other", "Прочие файлы", cat("other"), "{root}\\Прочее"),
    ]
}

// --- storage ---

#[derive(sqlx::FromRow)]
struct Row {
    id: String,
    name: String,
    enabled: bool,
    position: i64,
    match_json: String,
    target: String,
    builtin: Option<String>,
}

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<Rule>> {
    let rows = sqlx::query_as::<_, Row>("SELECT id, name, enabled, position, match_json, target, builtin FROM organizer_rules ORDER BY position, created_at")
        .fetch_all(pool)
        .await?;
    rows.into_iter()
        .map(|r| {
            Ok(Rule { matcher: serde_json::from_str(&r.match_json)?, id: r.id, name: r.name, enabled: r.enabled, position: r.position, target: r.target, builtin: r.builtin })
        })
        .collect()
}

/// Creates the default rules if there are none (first start or after «reset»).
pub async fn seed(pool: &SqlitePool) -> AppResult<()> {
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM organizer_rules").fetch_one(pool).await?;
    if n > 0 {
        return Ok(());
    }
    for (pos, (key, name, m, target)) in defaults().into_iter().enumerate() {
        sqlx::query("INSERT INTO organizer_rules (id, name, enabled, position, match_json, target, builtin) VALUES (?, ?, 1, ?, ?, ?, ?)")
            .bind(new_id())
            .bind(name)
            .bind(pos as i64)
            .bind(serde_json::to_string(&m)?)
            .bind(target)
            .bind(key)
            .execute(pool)
            .await?;
    }
    Ok(())
}

pub async fn reset(pool: &SqlitePool) -> AppResult<()> {
    sqlx::query("DELETE FROM organizer_rules").execute(pool).await?;
    seed(pool).await
}

pub async fn save(pool: &SqlitePool, id: Option<&str>, r: &RuleInput) -> AppResult<()> {
    validate(r)?;
    let m = serde_json::to_string(&r.matcher)?;
    match id {
        Some(id) => {
            sqlx::query("UPDATE organizer_rules SET name = ?, enabled = ?, match_json = ?, target = ?, updated_at = datetime('now') WHERE id = ?")
                .bind(r.name.trim())
                .bind(r.enabled)
                .bind(m)
                .bind(r.target.trim())
                .bind(id)
                .execute(pool)
                .await?;
        }
        None => {
            // New user rules go first: specific rules should win over the broad defaults.
            sqlx::query("UPDATE organizer_rules SET position = position + 1").execute(pool).await?;
            sqlx::query("INSERT INTO organizer_rules (id, name, enabled, position, match_json, target) VALUES (?, ?, ?, 0, ?, ?)")
                .bind(new_id())
                .bind(r.name.trim())
                .bind(r.enabled)
                .bind(m)
                .bind(r.target.trim())
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM organizer_rules WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

pub async fn reorder(pool: &SqlitePool, ids: &[String]) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    for (i, id) in ids.iter().enumerate() {
        sqlx::query("UPDATE organizer_rules SET position = ? WHERE id = ?").bind(i as i64).bind(id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn file(name: &str, days_old: i64, size: u64) -> (String, u64, DateTime<Local>) {
        (name.into(), size, Local::now() - chrono::Duration::days(days_old))
    }

    #[test]
    fn categories() {
        assert_eq!(category("Снимок экрана 2026-10-20 120000.png"), "screenshots");
        assert_eq!(category("Screenshot_20261020.JPG"), "screenshots");
        assert_eq!(category("photo.jpg"), "images");
        assert_eq!(category("Отчёт.docx"), "documents");
        assert_eq!(category("setup.EXE"), "installers");
        assert_eq!(category("backup.tar.gz"), "archives");
        assert_eq!(category("main.rs"), "code");
        assert_eq!(category("song.flac"), "music");
        assert_eq!(category("clip.mkv"), "video");
        assert_eq!(category("Makefile"), "other");
        assert_eq!(category(".gitignore"), "other");
    }

    #[test]
    fn matching_and_targets() {
        let now = Local::now();
        let (n, s, m) = file("Счёт №12.pdf", 40, 300 * 1024);
        let f = FileInfo { name: &n, size: s, modified: m };
        let invoice = Match { name_pattern: "Счёт*".into(), extensions: vec![".PDF".into()], older_than_days: Some(30), ..Default::default() };
        assert!(invoice.matches(&f, now));
        assert!(!Match { newer_than_days: Some(30), ..invoice.clone() }.matches(&f, now));
        assert!(!Match { max_size_kb: Some(100), ..invoice.clone() }.matches(&f, now));
        assert!(cat("documents").matches(&f, now));

        let shot = FileInfo { name: "Screenshot 1.png", size: 1, modified: Local.with_ymd_and_hms(2026, 3, 5, 10, 0, 0).unwrap() };
        assert_eq!(target_dir("{root}\\Скриншоты\\{year}-{month}", "D:\\Sorted", &shot), "D:\\Sorted\\Скриншоты\\2026-03");
        assert_eq!(target_dir("{category}\\{ext}", "D:\\Sorted\\", &shot), "D:\\Sorted\\Скриншоты\\png");
    }

    #[test]
    fn rule_validation_and_defaults() {
        let ok = RuleInput { name: "Счета".into(), enabled: true, matcher: Match { name_pattern: "Счёт*".into(), ..Default::default() }, target: "{root}\\Счета".into() };
        assert!(validate(&ok).is_ok());
        assert!(validate(&RuleInput { target: "..\\Windows".into(), ..ok.clone() }).is_err());
        assert!(validate(&RuleInput { matcher: Match::default(), ..ok.clone() }).is_err(), "a rule must restrict something");
        assert!(validate(&RuleInput { matcher: Match { categories: vec!["memes".into()], ..Default::default() }, ..ok }).is_err());
        assert_eq!(defaults().len(), CATEGORIES.len());
    }

    #[tokio::test]
    async fn storage_seed_and_order() {
        let pool = crate::db::test_pool().await;
        seed(&pool).await.unwrap();
        seed(&pool).await.unwrap();
        assert_eq!(list(&pool).await.unwrap().len(), 9);
        let input = RuleInput { name: "Счета".into(), enabled: true, matcher: Match { name_pattern: "Счёт*".into(), ..Default::default() }, target: "{root}\\Счета".into() };
        save(&pool, None, &input).await.unwrap();
        let all = list(&pool).await.unwrap();
        assert_eq!(all[0].name, "Счета", "user rules go first");
        reset(&pool).await.unwrap();
        assert_eq!(list(&pool).await.unwrap().len(), 9);
    }
}
