use super::*;

#[test]
fn test_normalize_path_sanitization() {
    let base_path = std::env::temp_dir();
    let storage_path = base_path.join("test_storage");
    let server = FilesServer::new(storage_path, base_path.clone()).unwrap();

    let project_name = "SnakeyDo - The Ultimate Snake Game!";
    let path = "src/main.rs";
    
    let normalized = server.normalize_path(path, Some(project_name)).unwrap();
    let path_str = normalized.to_str().unwrap();
    
    // Check that "!" is gone and replaced by underscore
    assert!(!path_str.contains('!'));
    // "SnakeyDo - The Ultimate Snake Game!" -> "SnakeyDo_-_The_Ultimate_Snake_Game_"
    // Spaces become _, - stays -, ! becomes _
    assert!(path_str.contains("SnakeyDo_-_The_Ultimate_Snake_Game_"));
}

#[test]
fn test_normalize_path_traversal() {
    let base_path = std::env::temp_dir();
    let storage_path = base_path.join("test_storage");
    let server = FilesServer::new(storage_path, base_path.clone()).unwrap();

    let project_name = "TestProject";
    let path = "../secret.txt";
    
    let result = server.normalize_path(path, Some(project_name));
    assert!(result.is_err());
}

#[tokio::test]
async fn test_directory_create_on_existing_file() {
    let base_path = std::env::temp_dir().join("hainet_test_dir_create");
    let storage_path = base_path.join("storage");
    let _ = std::fs::remove_dir_all(&base_path); // Clean up
    std::fs::create_dir_all(&base_path).unwrap();
    
    let server = FilesServer::new(storage_path, base_path.clone()).unwrap();
    let project_name = "TestProject";
    
    // 1. Create a file
    let file_path = "src/main.rs";
    let content = "fn main() {}";
    server.handle_file_write(file_path.to_string(), content.to_string(), Some(project_name.to_string())).await.unwrap();
    
    // 2. Try to create a directory with the same name
    let result = server.handle_directory_create(file_path.to_string(), Some(project_name.to_string())).await;
    
    // 3. Assert failure
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Path already exists as a file"));
    
    // Clean up
    let _ = std::fs::remove_dir_all(&base_path);
}
