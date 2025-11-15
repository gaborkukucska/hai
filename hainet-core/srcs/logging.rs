//! <!-- # START OF FILE hainet-core/src/logging.rs -->
//! Centralized logging for the HAI-Net framework.

use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use toml::Value;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

/// Finds the workspace root from the current directory by looking for a Cargo.toml with a [workspace] table.
fn find_workspace_root() -> Result<PathBuf> {
    let mut current_dir = env::current_dir()?;
    loop {
        let cargo_toml_path = current_dir.join("Cargo.toml");
        if cargo_toml_path.exists() {
            let toml_content = fs::read_to_string(&cargo_toml_path)?;
            let toml: Value = toml::from_str(&toml_content)?;
            if toml.get("workspace").is_some() {
                return Ok(current_dir);
            }
        }
        if !current_dir.pop() {
            bail!("Could not find workspace root");
        }
    }
}

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
    let workspace_root =
        find_workspace_root().context("Failed to find workspace root for logging")?;
    let logs_dir = workspace_root.join("logs");
    std::fs::create_dir_all(&logs_dir).context("Failed to create central logs directory")?;

    let log_file_name = format!(
        "{}-{}.log",
        app_name,
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    );
    let log_file_path = logs_dir.join(&log_file_name);

    let file_appender = tracing_appender::rolling::never(&logs_dir, &log_file_name);
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(format!(
            "hainet={0},app={0},rmcp=info",
            default_level
        ))
    });

    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(std::io::stderr)) // Log to stderr
        .with(
            fmt::layer()
                .with_writer(file_writer)
                .with_ansi(false),
        ) // Log to file
        .with(env_filter)
        .init();

    tracing::info!(
        "📝 Logs for {} being written to: {}",
        app_name,
        log_file_path.display()
    );

    Ok(guard)
}
