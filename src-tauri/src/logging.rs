//! Logging via `tracing`: human-readable output to stdout and a daily
//! rotating file in the app log directory. Level is controlled by the
//! `FOCUSDECK_LOG` environment variable (default `info`). Logs stay local.

use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Keeps the background log writer alive for the app's lifetime.
pub struct LogGuard(#[allow(dead_code)] WorkerGuard);

pub fn init(log_dir: &Path) -> anyhow::Result<LogGuard> {
    std::fs::create_dir_all(log_dir)?;
    let file = tracing_appender::rolling::daily(log_dir, "focusdeck.log");
    let (writer, guard) = tracing_appender::non_blocking(file);
    let filter = EnvFilter::try_from_env("FOCUSDECK_LOG").unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn"));

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(std::io::stdout))
        .with(fmt::layer().with_ansi(false).with_writer(writer))
        .try_init()?;
    Ok(LogGuard(guard))
}
