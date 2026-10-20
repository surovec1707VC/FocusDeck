//! Scanning the desktop into a plan, moving files, the move journal and
//! undo. Files are never deleted or overwritten: name conflicts get a
//! « (2)» suffix, and a cross-drive move copies, checks the size, and only
//! then removes the original.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use super::rules::{self, FileInfo, Rule};
use crate::automation::system::wildcard;
use crate::db::new_id;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub root: String,
    pub exclude: Vec<String>,
    pub min_age_minutes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanItem {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub modified: String,
    pub category: String,
    pub rule_id: String,
    pub rule_name: String,
    pub target_dir: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Skipped {
    pub name: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    pub desktop: String,
    /// Files directly on the desktop (folders and shortcuts not counted).
    pub desktop_files: usize,
    pub items: Vec<PlanItem>,
    pub skipped: Vec<Skipped>,
    /// (target folder, number of files).
    pub targets: Vec<(String, usize)>,
}

/// "*.lnk, desktop.ini" → ["*.lnk", "desktop.ini"].
pub fn patterns(list: &str) -> Vec<String> {
    list.split([',', ';', '\n']).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

#[cfg(windows)]
fn hidden_or_system(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    meta.file_attributes() & (0x2 | 0x4) != 0
}

#[cfg(not(windows))]
fn hidden_or_system(_: &std::fs::Metadata) -> bool {
    false
}

/// Which rule takes each desktop file, and why the others stay.
pub fn plan(desktop: &Path, rules: &[Rule], opts: &ScanOptions) -> std::io::Result<Plan> {
    let now = Local::now();
    let mut items = Vec::new();
    let mut skipped = Vec::new();
    let mut count = 0;
    for entry in std::fs::read_dir(desktop)? {
        let entry = entry?;
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if !meta.is_file() || hidden_or_system(&meta) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.to_lowercase().ends_with(".lnk") || name.to_lowercase().ends_with(".url") {
            continue; // shortcuts are part of the desktop, not clutter
        }
        count += 1;
        if let Some(p) = opts.exclude.iter().find(|p| wildcard(p, &name)) {
            skipped.push(Skipped { name, reason: format!("исключение «{p}»") });
            continue;
        }
        let modified: DateTime<Local> = meta.modified().map(DateTime::from).unwrap_or(now);
        if (now - modified).num_minutes() < opts.min_age_minutes as i64 {
            skipped.push(Skipped { name, reason: "файл изменён только что".into() });
            continue;
        }
        let info = FileInfo { name: &name, size: meta.len(), modified };
        match rules.iter().filter(|r| r.enabled).find(|r| r.matcher.matches(&info, now)) {
            Some(r) => items.push(PlanItem {
                path: entry.path().display().to_string(),
                size: info.size,
                modified: modified.to_rfc3339(),
                category: rules::category(&name).into(),
                rule_id: r.id.clone(),
                rule_name: r.name.clone(),
                target_dir: rules::target_dir(&r.target, &opts.root, &info),
                name,
            }),
            None => skipped.push(Skipped { name, reason: "нет подходящего правила".into() }),
        }
    }
    items.sort_by(|a, b| a.target_dir.cmp(&b.target_dir).then(a.name.cmp(&b.name)));
    let mut targets: Vec<(String, usize)> = Vec::new();
    for i in &items {
        match targets.iter_mut().find(|(t, _)| *t == i.target_dir) {
            Some(t) => t.1 += 1,
            None => targets.push((i.target_dir.clone(), 1)),
        }
    }
    Ok(Plan { desktop: desktop.display().to_string(), desktop_files: count, items, skipped, targets })
}

/// `dir\name`, or `dir\stem (2).ext`, `(3)`… if taken. `suffix` is added before the number.
pub fn free_path(dir: &Path, name: &str, suffix: &str) -> PathBuf {
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
        _ => (name.to_string(), String::new()),
    };
    let first = dir.join(if suffix.is_empty() { name.to_string() } else { format!("{stem} {suffix}{ext}") });
    if !first.exists() {
        return first;
    }
    for n in 2.. {
        let label = if suffix.is_empty() { format!("{stem} ({n}){ext}") } else { format!("{stem} {suffix} ({n}){ext}") };
        let candidate = dir.join(label);
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

/// Rename, or copy + size check + remove original across drives. Never overwrites.
pub fn move_file(from: &Path, to: &Path) -> Result<(), String> {
    if to.exists() {
        return Err("целевой файл уже существует".into());
    }
    match std::fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_) => {
            let size = std::fs::metadata(from).map_err(|e| e.to_string())?.len();
            std::fs::copy(from, to).map_err(|e| format!("не удалось скопировать: {e}"))?;
            let copied = std::fs::metadata(to).map(|m| m.len()).unwrap_or(0);
            if copied != size {
                return Err("копия не совпадает по размеру — оригинал оставлен на месте".into());
            }
            std::fs::remove_file(from).map_err(|e| format!("копия создана, но оригинал не удалось убрать: {e}"))
        }
    }
}

// --- operations and journal ---

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Operation {
    pub id: String,
    pub source: String,
    /// done | partial | undone | partially_undone
    pub status: String,
    pub moved_count: i64,
    pub failed_count: i64,
    pub summary: String,
    pub created_at: String,
    pub undone_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Move {
    pub id: i64,
    pub operation_id: String,
    pub original_path: String,
    pub new_path: String,
    pub rule_name: String,
    pub size: i64,
    /// moved | failed | undone | undo_failed
    pub status: String,
    pub error: String,
    pub moved_at: String,
    pub undone_at: Option<String>,
    pub restored_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApplyReport {
    pub operation_id: String,
    pub moved: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

/// Moves the chosen plan items. Each item is re-checked: it must still be a
/// file directly on the desktop.
pub async fn apply(pool: &SqlitePool, desktop: &Path, items: &[PlanItem], source: &str) -> AppResult<ApplyReport> {
    if items.is_empty() {
        return Err(AppError::Invalid("не выбрано ни одного файла".into()));
    }
    let op = new_id();
    sqlx::query("INSERT INTO organizer_operations (id, source) VALUES (?, ?)").bind(&op).bind(source).execute(pool).await?;
    let mut report = ApplyReport { operation_id: op.clone(), moved: 0, failed: 0, errors: vec![] };
    for item in items {
        let from = PathBuf::from(&item.path);
        let target = PathBuf::from(&item.target_dir);
        let checked: Result<PathBuf, String> = (|| {
            if from.parent() != Some(desktop) || !from.is_file() {
                return Err("файла уже нет на рабочем столе".to_string());
            }
            if !target.is_absolute() {
                return Err("целевая папка должна быть полным путём".into());
            }
            std::fs::create_dir_all(&target).map_err(|e| format!("не удалось создать папку: {e}"))?;
            let name = from.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let to = free_path(&target, &name, "");
            move_file(&from, &to)?;
            Ok(to)
        })();
        let (status, new_path, error) = match &checked {
            Ok(to) => ("moved", to.display().to_string(), String::new()),
            Err(e) => ("failed", String::new(), e.clone()),
        };
        sqlx::query("INSERT INTO organizer_moves (operation_id, original_path, new_path, rule_name, size, status, error) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&op)
            .bind(&item.path)
            .bind(&new_path)
            .bind(&item.rule_name)
            .bind(item.size as i64)
            .bind(status)
            .bind(&error)
            .execute(pool)
            .await?;
        match checked {
            Ok(_) => report.moved += 1,
            Err(e) => {
                report.failed += 1;
                report.errors.push(format!("{}: {e}", item.name));
            }
        }
    }
    let status = if report.failed > 0 { "partial" } else { "done" };
    let summary = format!("перемещено {}, ошибок {}", report.moved, report.failed);
    sqlx::query("UPDATE organizer_operations SET status = ?, moved_count = ?, failed_count = ?, summary = ? WHERE id = ?")
        .bind(status)
        .bind(report.moved as i64)
        .bind(report.failed as i64)
        .bind(&summary)
        .bind(&op)
        .execute(pool)
        .await?;
    tracing::info!(operation = %op, source, "{summary}");
    Ok(report)
}

pub async fn operations(pool: &SqlitePool, limit: i64) -> AppResult<Vec<Operation>> {
    Ok(sqlx::query_as::<_, Operation>(
        "SELECT id, source, status, moved_count, failed_count, summary, created_at, undone_at FROM organizer_operations ORDER BY created_at DESC, rowid DESC LIMIT ?",
    )
    .bind(limit.clamp(1, 500))
    .fetch_all(pool)
    .await?)
}

pub async fn moves(pool: &SqlitePool, operation_id: &str) -> AppResult<Vec<Move>> {
    Ok(sqlx::query_as::<_, Move>(
        "SELECT id, operation_id, original_path, new_path, rule_name, size, status, error, moved_at, undone_at, restored_path
         FROM organizer_moves WHERE operation_id = ? ORDER BY id",
    )
    .bind(operation_id)
    .fetch_all(pool)
    .await?)
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct UndoReport {
    pub restored: usize,
    pub failed: usize,
    /// Restored under another name because the original name was taken.
    pub renamed: usize,
    pub errors: Vec<String>,
}

/// Moves files back to where they were. A name taken in the meantime gets
/// « (восстановлено)»; a file that is gone is reported, nothing is deleted.
pub async fn undo_moves(pool: &SqlitePool, ids: &[i64]) -> AppResult<UndoReport> {
    let mut report = UndoReport::default();
    let mut ops: Vec<String> = Vec::new();
    for id in ids {
        let Some(m) = sqlx::query_as::<_, Move>(
            "SELECT id, operation_id, original_path, new_path, rule_name, size, status, error, moved_at, undone_at, restored_path FROM organizer_moves WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?
        else {
            continue;
        };
        if m.status != "moved" && m.status != "undo_failed" {
            continue;
        }
        if !ops.contains(&m.operation_id) {
            ops.push(m.operation_id.clone());
        }
        let original = PathBuf::from(&m.original_path);
        let result: Result<PathBuf, String> = (|| {
            let from = PathBuf::from(&m.new_path);
            if !from.is_file() {
                return Err(format!("файла нет в {} (его переместили или удалили вручную)", m.new_path));
            }
            let dir = original.parent().ok_or("нет исходной папки")?;
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            let name = original.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let to = if original.exists() { free_path(dir, &name, "(восстановлено)") } else { original.clone() };
            move_file(&from, &to)?;
            Ok(to)
        })();
        match result {
            Ok(to) => {
                if to != original {
                    report.renamed += 1;
                }
                report.restored += 1;
                sqlx::query("UPDATE organizer_moves SET status = 'undone', undone_at = datetime('now'), restored_path = ?, error = '' WHERE id = ?")
                    .bind(to.display().to_string())
                    .bind(id)
                    .execute(pool)
                    .await?;
            }
            Err(e) => {
                report.failed += 1;
                report.errors.push(e.clone());
                sqlx::query("UPDATE organizer_moves SET status = 'undo_failed', error = ? WHERE id = ?").bind(&e).bind(id).execute(pool).await?;
            }
        }
    }
    for op in ops {
        let (left,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM organizer_moves WHERE operation_id = ? AND status IN ('moved', 'undo_failed')")
            .bind(&op)
            .fetch_one(pool)
            .await?;
        let status = if left == 0 { "undone" } else { "partially_undone" };
        sqlx::query("UPDATE organizer_operations SET status = ?, undone_at = datetime('now') WHERE id = ?").bind(status).bind(&op).execute(pool).await?;
    }
    tracing::info!(restored = report.restored, failed = report.failed, "organizer undo");
    Ok(report)
}

pub async fn undo_operation(pool: &SqlitePool, operation_id: &str) -> AppResult<UndoReport> {
    let ids: Vec<i64> = moves(pool, operation_id).await?.into_iter().filter(|m| m.status == "moved" || m.status == "undo_failed").map(|m| m.id).collect();
    if ids.is_empty() {
        return Err(AppError::Invalid("в этой операции нечего отменять".into()));
    }
    undo_moves(pool, &ids).await
}

/// The newest operation that still has moved files.
pub async fn last_undoable(pool: &SqlitePool) -> AppResult<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT o.id FROM organizer_operations o WHERE EXISTS
            (SELECT 1 FROM organizer_moves m WHERE m.operation_id = o.id AND m.status IN ('moved', 'undo_failed'))
         ORDER BY o.created_at DESC, o.rowid DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organizer::rules::{self, Match, RuleInput};

    struct Sandbox(PathBuf);
    impl Drop for Sandbox {
        fn drop(&mut self) {
            // Only the test's own temp folder.
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn sandbox(tag: &str) -> Sandbox {
        let dir = std::env::temp_dir().join(format!("fd-organizer-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Desktop")).unwrap();
        Sandbox(dir)
    }

    #[test]
    fn free_names() {
        let s = sandbox("names");
        let d = s.0.join("Desktop");
        std::fs::write(d.join("a.txt"), "1").unwrap();
        assert_eq!(free_path(&d, "a.txt", ""), d.join("a (2).txt"));
        assert_eq!(free_path(&d, "b.txt", ""), d.join("b.txt"));
        assert_eq!(free_path(&d, "a.txt", "(восстановлено)"), d.join("a (восстановлено).txt"));
        assert_eq!(patterns(" *.lnk, desktop.ini ;; "), ["*.lnk", "desktop.ini"]);
    }

    #[tokio::test]
    async fn plan_apply_and_undo() {
        let s = sandbox("flow");
        let desktop = s.0.join("Desktop");
        for f in ["Снимок экрана 1.png", "отчёт.docx", "setup.exe", "notes.xyz", "Ярлык.lnk", "keep.tmp"] {
            std::fs::write(desktop.join(f), "data").unwrap();
        }
        std::fs::create_dir(desktop.join("Папка")).unwrap();

        let pool = crate::db::test_pool().await;
        rules::seed(&pool).await.unwrap();
        // A user rule before the defaults: documents named «отчёт*» go elsewhere.
        let input = RuleInput { name: "Отчёты".into(), enabled: true, matcher: Match { name_pattern: "отчёт*".into(), ..Default::default() }, target: "{root}\\Отчёты".into() };
        rules::save(&pool, None, &input).await.unwrap();
        let rules = rules::list(&pool).await.unwrap();

        let root = s.0.join("Sorted").display().to_string();
        let opts = ScanOptions { root: root.clone(), exclude: vec!["*.tmp".into()], min_age_minutes: 0 };
        let p = plan(&desktop, &rules, &opts).unwrap();
        assert_eq!(p.desktop_files, 5, "folders and shortcuts don't count");
        assert_eq!(p.items.len(), 4);
        assert_eq!(p.skipped.len(), 1);
        let report_item = p.items.iter().find(|i| i.name == "отчёт.docx").unwrap();
        assert_eq!(report_item.rule_name, "Отчёты");
        assert!(p.items.iter().any(|i| i.name == "notes.xyz" && i.target_dir.ends_with("Прочее")));

        // Exclude setup.exe in the preview: only 3 are moved.
        let chosen: Vec<PlanItem> = p.items.iter().filter(|i| i.name != "setup.exe").cloned().collect();
        // A file with the same name already waits in the target.
        std::fs::create_dir_all(format!("{root}\\Отчёты")).unwrap();
        std::fs::write(format!("{root}\\Отчёты\\отчёт.docx"), "old").unwrap();
        let r = apply(&pool, &desktop, &chosen, "manual").await.unwrap();
        assert_eq!((r.moved, r.failed), (3, 0));
        assert!(desktop.join("setup.exe").exists());
        assert!(!desktop.join("отчёт.docx").exists());
        assert!(PathBuf::from(format!("{root}\\Отчёты\\отчёт (2).docx")).exists(), "no overwrite");
        assert_eq!(std::fs::read_to_string(format!("{root}\\Отчёты\\отчёт.docx")).unwrap(), "old");

        // Undo one file, then the rest of the operation.
        let ms = moves(&pool, &r.operation_id).await.unwrap();
        let one = ms.iter().find(|m| m.original_path.ends_with("notes.xyz")).unwrap();
        assert_eq!(undo_moves(&pool, &[one.id]).await.unwrap().restored, 1);
        assert!(desktop.join("notes.xyz").exists());
        assert_eq!(operations(&pool, 10).await.unwrap()[0].status, "partially_undone");

        // The original name got taken meanwhile → restored under another name.
        std::fs::write(desktop.join("отчёт.docx"), "new one").unwrap();
        assert_eq!(last_undoable(&pool).await.unwrap().as_deref(), Some(r.operation_id.as_str()));
        let u = undo_operation(&pool, &r.operation_id).await.unwrap();
        assert_eq!((u.restored, u.renamed), (2, 1));
        assert!(desktop.join("отчёт (восстановлено).docx").exists());
        assert_eq!(std::fs::read_to_string(desktop.join("отчёт.docx")).unwrap(), "new one");
        assert_eq!(operations(&pool, 10).await.unwrap()[0].status, "undone");
        assert!(last_undoable(&pool).await.unwrap().is_none());
        assert!(undo_operation(&pool, &r.operation_id).await.is_err());

        // A file vanished from the target: reported, nothing else touched.
        let r2 = apply(&pool, &desktop, &[p.items.iter().find(|i| i.name == "setup.exe").unwrap().clone()], "manual").await.unwrap();
        let m = &moves(&pool, &r2.operation_id).await.unwrap()[0];
        std::fs::remove_file(&m.new_path).unwrap();
        let u2 = undo_operation(&pool, &r2.operation_id).await.unwrap();
        assert_eq!(u2.failed, 1);
        assert_eq!(moves(&pool, &r2.operation_id).await.unwrap()[0].status, "undo_failed");
    }
}
