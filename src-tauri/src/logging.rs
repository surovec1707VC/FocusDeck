//! Logging via `tracing`: human-readable output to stdout and, while the
//! «Журнал приложения» module is on, a daily rotating file in the app log
//! directory. The level comes from the module settings (or the
//! `FOCUSDECK_LOG` environment variable). Logs stay local.

use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// Keeps the background log writer alive for the app's lifetime.
pub struct LogGuard(#[allow(dead_code)] Option<WorkerGuard>);

pub fn init(log_dir: &Path, level: &str, to_file: bool) -> anyhow::Result<LogGuard> {
    let filter = EnvFilter::try_from_env("FOCUSDECK_LOG")
        .unwrap_or_else(|_| EnvFilter::new(format!("{level},sqlx=warn")));
    let stdout = fmt::layer().with_writer(std::io::stdout);

    if !to_file {
        tracing_subscriber::registry().with(filter).with(stdout).try_init()?;
        return Ok(LogGuard(None));
    }
    std::fs::create_dir_all(log_dir)?;
    let file = tracing_appender::rolling::daily(log_dir, "focusdeck.log");
    let (writer, guard) = tracing_appender::non_blocking(file);
    tracing_subscriber::registry()
        .with(filter)
        .with(stdout)
        .with(fmt::layer().with_ansi(false).with_writer(writer))
        .try_init()?;
    Ok(LogGuard(Some(guard)))
}
