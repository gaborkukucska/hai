//! HAI-Net Portal Main Binary
//! 
//! Secure Web Interface serving the React UI.

mod api;
mod assets;
mod auth;

use axum::{
    Router,
    response::IntoResponse,
    http::{StatusCode, Uri, header},
};
use tracing::info;
use anyhow::Result;
use std::net::SocketAddr;

use assets::Assets;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env().add_directive("hainet_portal=info".parse()?))
        .init();

    info!("🖥️  HAI-Net Portal starting up...");
    info!("📋 Version: {}", env!("CARGO_PKG_VERSION"));
    
    // Generate a random JWT secret for the session (regenerated on restart)
    let jwt_secret = uuid::Uuid::new_v4().to_string();
    let state = api::AppState { jwt_secret };

    // Build the Axum router
    let app = Router::new()
        .nest("/api/auth", api::api_routes(state.clone()))
        // Serve embedded static files
        .fallback(static_handler);

    // Run the server
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("✅ Server running on http://{}", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}

async fn static_handler(uri: Uri) -> impl IntoResponse {
    let mut path = uri.path().trim_start_matches('/').to_string();

    if path.is_empty() {
        path = "index.html".to_string();
    }

    if path.starts_with("api/") {
        return (
            StatusCode::NOT_FOUND, 
            axum::Json(serde_json::json!({"error": "not_found"}))
        ).into_response();
    }

    match Assets::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            (
                [(header::CONTENT_TYPE, mime.as_ref())],
                content.data.into_owned(),
            ).into_response()
        }
        None => {
            // Support React Router: return index.html for unknown routes
            if let Some(index) = Assets::get("index.html") {
                (
                    [(header::CONTENT_TYPE, "text/html")],
                    index.data.into_owned(),
                ).into_response()
            } else {
                (StatusCode::NOT_FOUND, "404 Not Found").into_response()
            }
        }
    }
}

