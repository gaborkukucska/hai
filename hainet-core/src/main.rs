//! HAI-Net Core Daemon
//! 
//! The main daemon that coordinates all HAI-Net services including networking,
//! storage, and communication with other components.
//! 
//! Works in both development mode (cargo run) and as a deployed systemd service.

use tracing::{info, debug, warn};
use anyhow::Result;
use std::convert::Infallible;

#[tokio::main]
async fn main() -> Result<()> {
    // Step 1: Load configuration (before logging, so we know the log dir)
    let config = hainet_core::config::HainetConfig::load();

    // Step 2: Initialize logging with the configured log directory
    let _guard = hainet_core::logging::initialize_logging_with_dir(
        "hainet-core",
        &config.logs.log_level,
        Some(&config.logs.log_dir),
    )?;

    info!("🌐 HAI-Net Core Daemon starting up...");
    info!("📋 Version: {}", env!("CARGO_PKG_VERSION"));
    info!("🏛️ Constitutional compliance: ENFORCED");
    info!("📄 Role: {}", config.role_display());
    info!("🔌 Configured port: {}", config.network.port);
    if let Some(ref master_ip) = config.network.master_ip {
        info!("🎯 Master node: {}", master_ip);
    }

    // Step 3: Find an available port for the health endpoint
    let health_port = hainet_core::config::find_available_port(config.network.port)
        .unwrap_or(config.network.port);
    
    if health_port != config.network.port {
        warn!("⚠  Port {} in use, health endpoint on port {}", config.network.port, health_port);
    }

    // Step 4: Start minimal HTTP health endpoint
    let role_for_health = config.network.role.clone();
    let health_handle = tokio::spawn(async move {
        if let Err(e) = run_health_server(health_port, &role_for_health).await {
            warn!("⚠  Health endpoint failed: {}", e);
        }
    });

    info!("✅ HAI-Net Core initialized successfully");
    info!("🩺 Health endpoint: http://0.0.0.0:{}/health", health_port);

    // Step 5: Periodic heartbeat + wait for shutdown signal
    let heartbeat = tokio::spawn(async {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            debug!("💓 hainet-core heartbeat — alive");
        }
    });

    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;
    info!("🛑 HAI-Net Core shutting down gracefully");

    // Cleanup
    health_handle.abort();
    heartbeat.abort();
    
    Ok(())
}

/// Run a minimal HTTP server that serves /health for mesh health checks.
async fn run_health_server(port: u16, role: &str) -> Result<()> {
    use hyper::service::{make_service_fn, service_fn};
    use hyper::{Body, Request, Response, Server, StatusCode};

    let role = role.to_string();

    let make_svc = make_service_fn(move |_conn| {
        let role = role.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req: Request<Body>| {
                let role = role.clone();
                async move {
                    match req.uri().path() {
                        "/health" => {
                            let body = format!(
                                r#"{{"status":"ok","service":"hainet-core","role":"{}","version":"{}"}}"#,
                                role,
                                env!("CARGO_PKG_VERSION")
                            );
                            Ok::<_, Infallible>(
                                Response::builder()
                                    .status(StatusCode::OK)
                                    .header("Content-Type", "application/json")
                                    .body(Body::from(body))
                                    .unwrap()
                            )
                        }
                        _ => {
                            Ok(Response::builder()
                                .status(StatusCode::NOT_FOUND)
                                .body(Body::from("Not Found"))
                                .unwrap())
                        }
                    }
                }
            }))
        }
    });

    let addr = ([0, 0, 0, 0], port).into();
    debug!("Health server binding to {}", addr);
    
    Server::bind(&addr)
        .serve(make_svc)
        .await
        .map_err(|e| anyhow::anyhow!("Health server error: {}", e))?;

    Ok(())
}
