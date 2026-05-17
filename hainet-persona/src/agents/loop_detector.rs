//! # START OF FILE hainet-persona/src/agents/loop_detector.rs
//! Autoregressive Loop Detector — Ported from TrippleEffect's core.py
//!
//! Detects when an LLM gets stuck repeating the same pattern.
//! Algorithm: check if the end of the text contains N repetitions of
//! a pattern of length >= min_pattern_length.
//!
//! From TE: min 20 chars, 4+ repetitions, hard 32K char output limit.

use tracing::warn;

/// Minimum pattern length to consider (chars)
const MIN_PATTERN_LENGTH: usize = 20;

/// Minimum repetitions to classify as a loop
const MIN_REPETITIONS: usize = 4;

/// Maximum pattern length to check (prevents O(n²) on large texts)
const MAX_PATTERN_LENGTH: usize = 1000;

/// Hard character limit on LLM output
pub const MAX_OUTPUT_CHARS: usize = 32_768;

/// Detect if the end of the text contains a repeating autoregressive loop.
///
/// Direct port from TrippleEffect's `detect_autoregressive_loop()` function.
///
/// Algorithm:
/// 1. For each candidate pattern length (20..1000 chars):
/// 2. Extract the last `pattern_length` chars as the candidate pattern
/// 3. Check if the preceding `pattern_length * (min_repetitions - 1)` chars
///    each match the pattern exactly
/// 4. If all match → loop detected
///
/// Returns the pattern length if detected, None otherwise.
pub fn detect_autoregressive_loop(text: &str) -> Option<usize> {
    let text_len = text.len();

    // Need at least min_pattern_length * min_repetitions chars
    if text_len < MIN_PATTERN_LENGTH * MIN_REPETITIONS {
        return None;
    }

    let max_pattern = std::cmp::min(MAX_PATTERN_LENGTH, text_len / MIN_REPETITIONS);

    for pattern_length in MIN_PATTERN_LENGTH..=max_pattern {
        let pattern = &text[text_len - pattern_length..];

        let mut is_loop = true;
        for i in 1..MIN_REPETITIONS {
            let start = text_len - (pattern_length * (i + 1));
            let end = text_len - (pattern_length * i);
            let segment = &text[start..end];

            if segment != pattern {
                is_loop = false;
                break;
            }
        }

        if is_loop {
            warn!(
                pattern_length,
                repetitions = MIN_REPETITIONS,
                "Autoregressive loop detected — LLM is repeating itself"
            );
            return Some(pattern_length);
        }
    }

    None
}

/// Check if text exceeds the hard character limit
pub fn check_output_limit(text: &str) -> bool {
    text.len() > MAX_OUTPUT_CHARS
}

/// Strip `<think>` tags from LLM output
/// (From TE: ROBUST_THINK_TAG_PATTERN)
pub fn strip_think_tags(text: &str) -> String {
    // Simple implementation — strip everything between <think> and </think>
    let mut result = String::new();
    let mut in_think = false;
    let mut chars = text.chars().peekable();
    let mut buffer = String::new();

    while let Some(c) = chars.next() {
        buffer.push(c);

        if !in_think && buffer.ends_with("<think>") {
            // Remove the <think> tag from result
            let tag_start = result.len() - 6; // "<think" already in result minus ">"
            result.truncate(result.len().saturating_sub(6));
            buffer.clear();
            in_think = true;
            continue;
        }

        if in_think && buffer.ends_with("</think>") {
            buffer.clear();
            in_think = false;
            continue;
        }

        if !in_think {
            result.push(c);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_loop_in_normal_text() {
        let text = "This is a normal response with varied content. \
                     The agent is doing useful work and producing different output each time.";
        assert!(detect_autoregressive_loop(text).is_none());
    }

    #[test]
    fn test_detects_simple_loop() {
        // Create a repeating pattern of exactly 25 chars, repeated 5 times
        let pattern = "This is a repeating text.";  // 25 chars
        let text = pattern.repeat(5);
        let result = detect_autoregressive_loop(&text);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), 25);
    }

    #[test]
    fn test_short_text_no_detection() {
        let text = "Short text";
        assert!(detect_autoregressive_loop(text).is_none());
    }

    #[test]
    fn test_output_limit_check() {
        let short = "a".repeat(100);
        assert!(!check_output_limit(&short));

        let long = "a".repeat(MAX_OUTPUT_CHARS + 1);
        assert!(check_output_limit(&long));
    }

    #[test]
    fn test_barely_under_repetition_count() {
        // Only 3 repetitions (min is 4) — should NOT detect
        let pattern = "This is a repeating text.";
        let text = pattern.repeat(3);
        assert!(detect_autoregressive_loop(&text).is_none());
    }

    #[test]
    fn test_loop_at_end_of_normal_text() {
        // Normal text followed by a loop
        let normal = "This is some normal, varied text. ";
        let pattern = "I am stuck in a loop now.";  // 25 chars
        let text = format!("{}{}", normal, pattern.repeat(5));
        let result = detect_autoregressive_loop(&text);
        assert!(result.is_some());
    }
}
