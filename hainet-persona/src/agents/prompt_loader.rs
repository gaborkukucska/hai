//! # START OF FILE hainet-persona/src/agents/prompt_loader.rs
//! Centralized TrippleEffect prompt loader.
//!
//! Resolves `prompts.yaml` from multiple locations so that all agents
//! (Admin, PM, Worker) share one path-resolution strategy and one
//! YAML-parsing implementation.
//!
//! Resolution order:
//! 1. `/media/hai-drive/prompts/prompts.yaml`  — NFS shared drive (production)
//! 2. `/var/lib/hainet/.hainet/prompts.yaml`   — system install
//! 3. `_workspace/TrippleEffect/prompts.yaml`  — dev workspace (relative to workspace root)
//! 4. Hard-coded dev fallback path              — absolute dev machine path

use std::path::PathBuf;
use std::sync::OnceLock;

/// Cached prompts.yaml content so we only read the file once per process.
static PROMPTS_CACHE: OnceLock<Option<String>> = OnceLock::new();

/// Candidate paths for `prompts.yaml`, tried in order.
fn candidate_paths() -> Vec<PathBuf> {
    let mut paths = vec![
        PathBuf::from("/media/hai-drive/prompts/prompts.yaml"),      // Slave nodes (NFS mount)
        PathBuf::from("/media/fast/hai-drive/prompts/prompts.yaml"), // Master node (BigBOY local export)
        PathBuf::from("/var/lib/hainet/.hainet/prompts.yaml"),       // Local localhost deployment
    ];

    // Try to locate workspace root relative to executable
    if let Ok(exe) = std::env::current_exe() {
        // exe might be in target/debug or target/release
        let mut dir = exe.parent().unwrap_or(std::path::Path::new("/")).to_path_buf();
        for _ in 0..5 {
            let candidate = dir.join("_workspace/TrippleEffect/prompts.yaml");
            if candidate.exists() {
                paths.push(candidate);
                break;
            }
            if !dir.pop() { break; }
        }
    }

    // Hard-coded dev fallback
    paths.push(PathBuf::from("/home/tom/hai/_workspace/TrippleEffect/prompts.yaml"));
    
    paths
}

/// Load the prompts.yaml content, caching it for subsequent calls.
fn load_prompts_content() -> Option<&'static str> {
    PROMPTS_CACHE
        .get_or_init(|| {
            for path in candidate_paths() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    tracing::info!("Loaded TrippleEffect prompts from: {}", path.display());
                    return Some(content);
                }
            }
            tracing::warn!("Could not find prompts.yaml in any candidate location");
            None
        })
        .as_deref()
}

/// Extract a named prompt section from the cached prompts.yaml content.
///
/// The YAML uses block scalar indicators (`|`, `|-`, `|2`, `|2-`).
/// We scan for the key, then collect indented lines until the next
/// top-level key.
pub fn get_prompt(prompt_name: &str) -> Option<String> {
    let content = load_prompts_content()?;
    extract_prompt_from_content(content, prompt_name)
}

/// Core extraction logic, separated for testability.
fn extract_prompt_from_content(content: &str, prompt_name: &str) -> Option<String> {
    // Build all possible YAML key markers
    let markers: Vec<String> = vec![
        format!("{}: |", prompt_name),
        format!("{}: |-", prompt_name),
        format!("{}: |2", prompt_name),
        format!("{}: |2-", prompt_name),
    ];
    // Bare key (for single-line values)
    let bare_key = format!("{}:", prompt_name);

    let mut found = false;
    let mut prompt_content = String::new();

    for line in content.lines() {
        if !found {
            let matches = markers.iter().any(|m| line.starts_with(m)) || line.starts_with(&bare_key);
            if matches {
                found = true;
                continue;
            }
        } else {
            // Stop at next top-level key (non-indented, alphabetic start) or document separator
            if line.starts_with("--")
                || (line.chars().next().map_or(false, |c| c.is_alphabetic()) && !line.starts_with(' '))
            {
                break;
            }
            prompt_content.push_str(line);
            prompt_content.push('\n');
        }
    }

    if found {
        Some(prompt_content.trim().to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_block_scalar() {
        let yaml = r#"
some_other_key: value
pm_startup_prompt: |2

  --- Current State: STARTUP ---
  You are a PM.

  [YOUR GOAL]
  Do the thing.
pm_manage_prompt: |2

  --- Current State: MANAGE ---
"#;
        let result = extract_prompt_from_content(yaml, "pm_startup_prompt");
        assert!(result.is_some());
        let prompt = result.unwrap();
        assert!(prompt.contains("STARTUP"));
        assert!(prompt.contains("YOUR GOAL"));
        assert!(!prompt.contains("MANAGE"));
    }
}
