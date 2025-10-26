//! hainet-portal/src-tauri/tests/vision_integration_test.rs
use hainet_portal::vision_handler;
use tauri::test::{mock_builder, mock_context};

#[tokio::test]
async fn test_list_webcam_devices_command() {
    let (app, _webview) = mock_builder()
        .invoke_handler(tauri::generate_handler![vision_handler::list_webcam_devices])
        .build()
        .await;

    // This test will likely fail in a headless environment without webcam access.
    // In a real CI setup, you would mock the `kamkatch` crate.
    let res = app.invoke::<_, Vec<String>>("list_webcam_devices", ()).await;
    assert!(res.is_ok(), "list_webcam_devices should not error, even if empty");
}

#[tokio::test]
async fn test_webcam_lifecycle_commands() {
    let context = mock_context().await;
    let (app, _webview) = mock_builder()
        .invoke_handler(tauri::generate_handler![
            vision_handler::start_webcam,
            vision_handler::stop_webcam,
            vision_handler::capture_frame
        ])
        .setup(move |app| {
            app.manage(vision_handler::VisionState(Default::default()));
            Ok(())
        })
        .build()
        .await;

    // As with the device list test, these will fail without a real device.
    // The main purpose here is to ensure the commands are registered and callable.
    let config = hainet_core::multimodal::VisionConfig::default();
    let start_res = app.invoke::<_, ()>("start_webcam", config).await;
    // We expect this to fail gracefully if no camera is present.
    assert!(start_res.is_err(), "start_webcam should fail without a camera");

    let stop_res = app.invoke::<_, ()>("stop_webcam", ()).await;
    assert!(stop_res.is_ok(), "stop_webcam should always succeed");
}
