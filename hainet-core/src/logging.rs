//! <!-- # START OF FILE hainet-core/src/logging.rs -->
//! Centralized logging for the HAI-Net framework.
//!
//! Supports two modes:
//! 1. **Development mode**: Logs to `<workspace_root>/logs/` when running from source.
//! 2. **System mode**: Logs to a configured directory (default `/var/log/hainet/`)
//!    when running as a deployed systemd service.

use anyhow::Result;
use std::env;
use std::fs;
use std::path::PathBuf;
use toml::Value;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Finds the workspace root by traversing up from the executable path
/// looking for a Cargo.toml with a [workspace] members section.
/// Returns None if not found (i.e. running from a system install).
fn find_workspace_root() -> Option<PathBuf> {
    // First try from executable location
    if let Ok(exe_path) = env::current_exe() {
        let mut current_dir = exe_path
            .parent()
            .expect("Executable must be in a directory.")
            .to_path_buf();

        loop {
            let cargo_toml_path = current_dir.join("Cargo.toml");
            if cargo_toml_path.exists() {
                if let Ok(toml_content) = fs::read_to_string(&cargo_toml_path) {
                    if let Ok(toml) = toml::from_str::<Value>(&toml_content) {
                        if let Some(workspace) = toml.get("workspace") {
                            if workspace.get("members").is_some() {
                                return Some(current_dir);
                            }
                        }
                    }
                }
            }
            if !current_dir.pop() {
                break;
            }
        }
    }

    // Then try from current working directory
    if let Ok(cwd) = env::current_dir() {
        let cargo_toml_path = cwd.join("Cargo.toml");
        if cargo_toml_path.exists() {
            if let Ok(toml_content) = fs::read_to_string(&cargo_toml_path) {
                if let Ok(toml) = toml::from_str::<Value>(&toml_content) {
                    if let Some(workspace) = toml.get("workspace") {
                        if workspace.get("members").is_some() {
                            return Some(cwd);
                        }
                    }
                }
            }
        }
    }

    None
}

/// Determines the log directory for a HAI-Net application.
///
/// Priority:
/// 1. Explicit `log_dir_override` (from loaded config)
/// 2. Workspace `logs/` directory (development mode)
/// 3. `/var/log/hainet/` (system installation)
/// 4. `/tmp/hainet-logs/` (last resort fallback)
fn resolve_log_dir(log_dir_override: Option<&str>) -> PathBuf {
    // Priority 1: Explicit override from config
    if let Some(override_dir) = log_dir_override {
        let dir = PathBuf::from(override_dir);
        if fs::create_dir_all(&dir).is_ok() {
            return dir;
        }
    }

    // Priority 2: Workspace logs directory (dev mode)
    if let Some(workspace_root) = find_workspace_root() {
        let logs_dir = workspace_root.join("logs");
        if fs::create_dir_all(&logs_dir).is_ok() {
            return logs_dir;
        }
    }

    // Priority 3: System log directory
    let system_dir = PathBuf::from("/var/log/hainet");
    if fs::create_dir_all(&system_dir).is_ok() {
        return system_dir;
    }

    // Priority 4: Last resort — temp directory
    let fallback = env::temp_dir().join("hainet-logs");
    let _ = fs::create_dir_all(&fallback);
    fallback
}

/// Initializes the logging system for a HAI-Net application.
///
/// This sets up a tracing subscriber that logs to both stderr and a
/// timestamped file in the resolved log directory.
///
/// The returned `WorkerGuard` must be held by the application to ensure
/// all log output is flushed. Bind it to a variable in `main`.
///
/// # Arguments
///
/// * `app_name` - The name of the application, used in the log file name.
/// * `default_level` - The default log level (e.g., "debug", "info").
///
/// # Example
///
/// ```no_run
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
    initialize_logging_with_dir(app_name, default_level, None)
}

/// Initializes logging with an optional explicit log directory override.
///
/// This is the primary entrypoint for deployed daemons that read their
/// log directory from `/etc/hainet/hainet.toml`.
pub fn initialize_logging_with_dir(
    app_name: &str,
    default_level: &str,
    log_dir_override: Option<&str>,
) -> Result<tracing_appender::non_blocking::WorkerGuard> {
    let logs_dir = resolve_log_dir(log_dir_override);

    let log_file_name = format!(
        "{}-{}.log",
        app_name,
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    );
    let log_file_path = logs_dir.join(&log_file_name);

    let file_appender = tracing_appender::rolling::never(&logs_dir, &log_file_name);
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
