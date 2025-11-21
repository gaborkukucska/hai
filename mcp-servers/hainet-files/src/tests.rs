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
