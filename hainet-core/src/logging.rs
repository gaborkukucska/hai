//! <!-- # START OF FILE hainet-core/src/logging.rs -->
//! Centralized logging for the HAI-Net framework.

use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
use std::path::PathBuf;
use toml::Value;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Finds the workspace root or falls back to a sensible default.
fn find_workspace_root() -> Result<PathBuf> {
    // First try to find workspace root from executable location
    let exe_path = env::current_exe()?;
    let mut current_dir = exe_path
        .parent()
        .expect("Executable must be in a directory.")
        .to_path_buf();

    // Try traversing up from executable path
    let original_dir = current_dir.clone();
    loop {
        let cargo_toml_path = current_dir.join("Cargo.toml");
        if cargo_toml_path.exists() {
            let toml_content =
                fs::read_to_string(&cargo_toml_path).context("Failed to read Cargo.toml")?;
            if let Ok(toml) = toml::from_str::<Value>(&toml_content) {
                // Check if this is a real workspace with members (not just an empty workspace table)
                if let Some(workspace) = toml.get("workspace") {
                    if workspace.get("members").is_some() {
                        return Ok(current_dir);
                    }
                }
            }
        }
        if !current_dir.pop() {
            break;
        }
    }

    // If we couldn't find a workspace (e.g., running from /usr/local/bin/),
    // fall back to current working directory if it looks like a workspace
    if let Ok(cwd) = env::current_dir() {
        let cargo_toml_path = cwd.join("Cargo.toml");
        if cargo_toml_path.exists() {
            if let Ok(toml_content) = fs::read_to_string(&cargo_toml_path) {
                if let Ok(toml) = toml::from_str::<Value>(&toml_content) {
                    if let Some(workspace) = toml.get("workspace") {
                        if workspace.get("members").is_some() {
                            return Ok(cwd);
                        }
                    }
                }
            }
        }
    }

    // Last resort: use /var/log/hainet/ for system-wide installations
    // This handles cases where binaries are installed in /usr/local/bin/ or similar
    let system_log_dir = PathBuf::from("/var/log/hainet");
    if original_dir.starts_with("/usr") || original_dir.starts_with("/opt") {
        return Ok(system_log_dir);
    }

    bail!("Could not find workspace root. Not in a workspace directory, and not a system installation.");
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

    let file_appender = tracing_appender::rolling::never(logs_dir, &log_file_name);
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

    let app_crate_name = app_name.replace('-', "_");
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(format!(
            "{app}={level},hainet={level},rmcp=info",
            app = app_crate_name,
            level = default_level
        ))
    });

    let stderr_layer = fmt::layer().with_writer(std::io::stderr);
    let file_layer = fmt::layer()
        .with_writer(file_writer)
        .with_ansi(false);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(stderr_layer)
        .with(file_layer)
        .init();

    tracing::info!(
        "📝 Logs for {} being written to: {}",
        app_name,
        log_file_path.display()
    );

    Ok(guard)
}
