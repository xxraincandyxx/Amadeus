//! # Test Assertions
//!
//! Custom assertion utilities for testing.

use std::time::Duration;

/// Assert that a condition becomes true within a timeout.
pub async fn assert_eventually<F>(condition: F, timeout: Duration, message: &str)
where
    F: Fn() -> bool,
{
    let start = std::time::Instant::now();
    loop {
        if condition() {
            return;
        }
        if start.elapsed() > timeout {
            panic!(
                "Assertion failed: condition did not become true within {:?} - {}",
                timeout, message
            );
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// Assert that a file was created.
pub async fn assert_file_created(path: &std::path::Path) {
    assert!(
        tokio::fs::try_exists(path).await.unwrap_or(false),
        "File was not created: {:?}",
        path
    );
}

/// Assert that a file was NOT created.
pub async fn assert_file_not_created(path: &std::path::Path) {
    assert!(
        !tokio::fs::try_exists(path).await.unwrap_or(true),
        "File should not exist: {:?}",
        path
    );
}

/// Assert that a file contains specific content.
pub async fn assert_file_contains(path: &std::path::Path, expected: &str) {
    let content = tokio::fs::read_to_string(path)
        .await
        .unwrap_or_else(|_| panic!("Failed to read file: {:?}", path));

    assert!(
        content.contains(expected),
        "File {:?} does not contain expected content.\nExpected: {}\nActual:\n{}",
        path, expected, content
    );
}

/// Assert that a JSON value contains specific keys.
pub fn assert_json_contains_keys(value: &serde_json::Value, keys: &[&str]) {
    if let serde_json::Value::Object(map) = value {
        for key in keys {
            assert!(
                map.contains_key(*key),
                "JSON object does not contain key '{}'\nActual: {}",
                key, value
            );
        }
    } else {
        panic!("Value is not an object: {}", value);
    }
}

/// Assert that two JSON values are deeply equal.
pub fn assert_json_eq(actual: &serde_json::Value, expected: &serde_json::Value) {
    assert_eq!(
        actual, expected,
        "JSON values are not equal.\nExpected: {}\nActual: {}",
        expected, actual
    );
}

/// Assert that a string matches a regex pattern.
pub fn assert_matches_regex(text: &str, pattern: &str) {
    let re = regex::Regex::new(pattern).expect("Invalid regex pattern");
    assert!(
        re.is_match(text),
        "Text does not match pattern '{}'\nActual: {}",
        pattern, text
    );
}
