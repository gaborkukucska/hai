//! <!-- # START OF FILE hainet-core/src/logging.rs -->
//! Centralized logging for the HAI-Net framework.

use anyhow::{Context, Result};
use std::path::PathBuf;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

/// Initializes the logging system for a HAI-Net application.
///
/// This sets up a tracing subscriber that logs to both stderr and a
/// timestamped file in a central `logs` directory at the project root.
///
/// The returned `WorkerGuard` must be held by the application in a manner
/// that ensures it is not dropped until all logging is complete. Typically,
/// this means binding it to a variable in the `main` function.
///
/// # Arguments
///
/// * `app_name` - The name of the application, used in the log file name.
/// * `default_level` - The default log level for this application (e.g., "debug", "info").
///
/// # Example
///
/// ```no_run
/// // In main.rs
/// fn main() -> anyhow::Result<()> {
///     let _guard = hainet_core::logging::initialize_logging("my-app", "debug")?;
///     // ... application code ...
///     Ok(())
/// }
/// ```
pub fn initialize_logging(
    app_name: &str,
    default_level: &str,
) -> Result<tracing_appender::non_blocking::WorkerGuard> {
    // Assume the binary is run from the workspace root.
    let logs_dir = PathBuf::from("logs");
    std::fs::create_dir_all(&logs_dir)
        .context("Failed to create central logs directory")?;

    let log_file_name = format!(
        "{}-{}.log",
        app_name,
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    );
    let log_file_path = logs_dir.join(&log_file_name);

    let file_appender = tracing_appender::rolling::never(&logs_dir, &log_file_name);
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("{app_name}={default_level},rmcp=info")));

    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(std::io::stderr)) // Log to stderr
        .with(fmt::layer().with_writer(file_writer).with_ansi(false)) // Log to file
        .with(env_filter)
        .init();

    tracing::info!(
        "📝 Logs for {} being written to: {}",
        app_name,
        log_file_path.display()
    );

    Ok(guard)
}
