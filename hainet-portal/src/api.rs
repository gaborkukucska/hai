use axum::{
    routing::{get, post},
    Json, Router, extract::State,
    http::{StatusCode, header},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::fs;
use crate::auth::{get_hainet_dir, hash_password, verify_password, encrypt_seed, generate_jwt};

#[derive(Clone)]
pub struct AppState {
    pub jwt_secret: String,
    pub qr_sessions: std::sync::Arc<tokio::sync::Mutex<std::collections::HashMap<String, bool>>>,
}

#[derive(Serialize)]
pub struct StatusResponse {
    pub status: String, // "setup_required" or "secure"
}

#[derive(Deserialize)]
pub struct SetupRequest {
    pub seed_phrase: String,
    pub app_passphrase: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub app_passphrase: String,
}

pub fn api_routes(state: AppState) -> Router {
    Router::new()
        .route("/status", get(auth_status))
        .route("/setup", post(auth_setup))
        .route("/login", post(auth_login))
        .route("/generate-seed", get(generate_seed_route))
        .route("/verify", get(auth_verify))
        
        .route("/qr/init", post(qr_login_init))
        .route("/qr/verify", post(qr_login_verify))
        .route("/qr/status/:session_id", get(qr_login_status))
        .with_state(state)
}

async fn auth_verify(headers: axum::http::HeaderMap, State(state): State<AppState>) -> impl IntoResponse {
    let cookie_header = headers.get(header::COOKIE).and_then(|v| v.to_str().ok());
    
    if let Some(cookie) = cookie_header {
        if cookie.contains("hainet_token=") {
            let token_str = cookie.split("hainet_token=").nth(1).unwrap_or("").split(';').next().unwrap_or("");
            use jsonwebtoken::{decode, DecodingKey, Validation};
            let mut validation = Validation::default();
            validation.validate_exp = true;
            
            if decode::<crate::auth::Claims>(
                token_str,
                &DecodingKey::from_secret(state.jwt_secret.as_bytes()),
                &validation
            ).is_ok() {
                return (StatusCode::OK, Json(serde_json::json!({"authenticated": true}))).into_response();
            }
        }
    }
    
    (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"authenticated": false}))).into_response()
}

async fn generate_seed_route() -> impl IntoResponse {
    use bip39::{Mnemonic, Language};
    let mnemonic = match Mnemonic::generate_in(Language::English, 24) {
        Ok(m) => m,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to generate seed").into_response(),
    };
    Json(serde_json::json!({ "seed_phrase": mnemonic.to_string() })).into_response()
}

async fn auth_status() -> impl IntoResponse {
    let hainet_dir = get_hainet_dir();
    let seed_file = hainet_dir.join(".mesh_seed");
    let pass_file = hainet_dir.join(".mesh_pass"); // Storing password hash here instead of toml for simplicity

    if seed_file.exists() && pass_file.exists() {
        Json(StatusResponse { status: "secure".to_string() })
    } else {
        Json(StatusResponse { status: "setup_required".to_string() })
    }
}

async fn auth_setup(Json(payload): Json<SetupRequest>) -> impl IntoResponse {
    let hainet_dir = get_hainet_dir();
    std::fs::create_dir_all(&hainet_dir).unwrap_or_default();

    let seed_file = hainet_dir.join(".mesh_seed");
    let pass_file = hainet_dir.join(".mesh_pass");

    if seed_file.exists() || pass_file.exists() {
        return (StatusCode::BAD_REQUEST, "Already configured").into_response();
    }

    // 1. Hash the App Passphrase
    let hashed_pass = match hash_password(&payload.app_passphrase) {
        Ok(h) => h,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to hash password").into_response(),
    };

    // 2. Encrypt the Seed Phrase
    let encrypted_seed = match encrypt_seed(&payload.seed_phrase, &payload.app_passphrase) {
        Ok(enc) => enc,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to encrypt seed").into_response(),
    };

    // 3. Save to disk
    if fs::write(&pass_file, hashed_pass).is_err() || fs::write(&seed_file, encrypted_seed).is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to save configuration").into_response();
    }

    (StatusCode::OK, "Setup complete").into_response()
}

async fn auth_login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>
) -> impl IntoResponse {
    let hainet_dir = get_hainet_dir();
    let pass_file = hainet_dir.join(".mesh_pass");

    if !pass_file.exists() {
        return (StatusCode::BAD_REQUEST, "Setup required").into_response();
    }

    let hashed_pass = match fs::read_to_string(&pass_file) {
        Ok(h) => h,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read hash").into_response(),
    };

    if !verify_password(&payload.app_passphrase, &hashed_pass) {
        return (StatusCode::UNAUTHORIZED, "Invalid passphrase").into_response();
    }

    let token = match generate_jwt(&state.jwt_secret) {
        Ok(t) => t,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to generate token").into_response(),
    };

    // Set HttpOnly cookie
    let cookie = format!("hainet_token={}; Path=/; HttpOnly; SameSite=Strict; Max-Age=86400", token);
    
    ([(header::SET_COOKIE, cookie)], Json(serde_json::json!({"status": "success"}))).into_response()
}

#[derive(serde::Deserialize)]
pub struct QrVerifyRequest {
    pub session_id: String,
    pub public_key: String,
    pub signature: String,
}

async fn qr_login_init(axum::extract::State(state): axum::extract::State<AppState>) -> impl IntoResponse {
    let session_id = uuid::Uuid::new_v4().to_string();
    state.qr_sessions.lock().await.insert(session_id.clone(), false);
    axum::Json(serde_json::json!({ "session_id": session_id })).into_response()
}

async fn qr_login_verify(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::Json(payload): axum::Json<QrVerifyRequest>
) -> impl IntoResponse {
    match crate::auth::verify_qr_signature(&payload.session_id, &payload.public_key, &payload.signature) {
        Ok(true) => {
            let mut sessions = state.qr_sessions.lock().await;
            if sessions.contains_key(&payload.session_id) {
                sessions.insert(payload.session_id.clone(), true);
                return (StatusCode::OK, axum::Json(serde_json::json!({"status": "verified"}))).into_response();
            }
            (StatusCode::NOT_FOUND, "Session not found").into_response()
        },
        _ => (StatusCode::UNAUTHORIZED, "Invalid signature").into_response()
    }
}

async fn qr_login_status(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(session_id): axum::extract::Path<String>
) -> impl IntoResponse {
    let is_verified = {
        let sessions = state.qr_sessions.lock().await;
        sessions.get(&session_id).copied().unwrap_or(false)
    };

    if is_verified {
        let token = crate::auth::generate_jwt(&state.jwt_secret).unwrap_or_default();
        let cookie = format!("hainet_token={}; Path=/; HttpOnly; SameSite=Strict; Max-Age=86400", token);
        state.qr_sessions.lock().await.remove(&session_id);
        ([(axum::http::header::SET_COOKIE, cookie)], axum::Json(serde_json::json!({"status": "authenticated"}))).into_response()
    } else {
        (StatusCode::OK, axum::Json(serde_json::json!({"status": "pending"}))).into_response()
    }
}
