// <!-- # START OF FILE hainet-portal/src-tauri/src/video_handler.rs -->

use tauri::command;

#[command]
pub async fn play_video(path: String) -> Result<(), String> {
    // In a real application, you would use a library to play the video.
    // For this example, we'll just print the path to the console.
    println!("Playing video from: {}", path);
    Ok(())
}
