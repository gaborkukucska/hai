//! # START OF FILE hainet-portal-tauri/src/video_handler.rs

use crate::VideoStreamingState;
use std::fs::File;
use std::net::TcpListener;
use std::path::Path;
use std::sync::Arc;
use std::thread;
use tauri::State;
use tiny_http::{Request, Response, Server};

fn find_open_port() -> Result<u16, String> {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr().map(|addr| addr.port()))
        .map_err(|e| e.to_string())
}

fn handle_request(request: Request, video_path: &Path) {
    let mut file = match File::open(video_path) {
        Ok(file) => file,
        Err(_) => {
            let response = Response::from_string("File not found").with_status_code(404);
            let _ = request.respond(response);
            return;
        }
    };
    let response = Response::from_file(file);
    let _ = request.respond(response);
}

#[tauri::command]
pub async fn stream_video(
    path: String,
    state: State<'_, VideoStreamingState>,
) -> Result<String, String> {
    let video_path = Path::new(&path);
    if !video_path.exists() {
        return Err("Video file not found.".into());
    }

    let port = find_open_port()?;
    let addr = format!("127.0.0.1:{}", port);
    let server = Arc::new(Server::http(&addr).map_err(|e| e.to_string())?);

    state.0.lock().unwrap().insert(port, server.clone());

    let video_path = video_path.to_path_buf();
    thread::spawn(move || {
        for request in server.incoming_requests() {
            handle_request(request, &video_path);
        }
    });

    Ok(format!("http://{}", addr))
}

#[tauri::command]
pub async fn stop_video_stream(
    port: u16,
    state: State<'_, VideoStreamingState>,
) -> Result<(), String> {
    if let Some(server) = state.0.lock().unwrap().remove(&port) {
        server.unblock();
    }
    Ok(())
}
