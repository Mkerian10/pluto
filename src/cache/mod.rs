//! Test cache management for incremental test execution.
//!
//! This module provides functionality to cache test dependency hashes
//! so that tests can be skipped if their dependencies haven't changed.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

/// Cache entry for a single source file's test hashes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCacheEntry {
    /// Hash of the source file content (for validation)
    pub source_hash: String,
    /// Map of test display_name -> dependency hash
    pub test_hashes: HashMap<String, String>,
    /// Timestamp of when this cache was created
    pub timestamp: u64,
}

impl TestCacheEntry {
    pub fn new(source_hash: String, test_hashes: HashMap<String, String>) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            source_hash,
            test_hashes,
            timestamp,
        }
    }
}

/// Get the cache directory path for test hashes
fn get_cache_dir() -> PathBuf {
    PathBuf::from(".pluto-cache/test-hashes")
}

/// Compute a simple hash of file content for cache validation
fn hash_file_content(content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// Load test cache for a given source file
pub fn load_cache(source_path: &Path, source_content: &str) -> Option<TestCacheEntry> {
    let cache_dir = get_cache_dir();

    // Use the source file's absolute path as the cache key
    let cache_key = source_path
        .canonicalize()
        .ok()?
        .to_string_lossy()
        .replace(['/', '\\'], "_");

    let cache_file = cache_dir.join(format!("{}.json", cache_key));

    if !cache_file.exists() {
        return None;
    }

    // Read and deserialize cache
    let cache_json = fs::read_to_string(&cache_file).ok()?;
    let cache: TestCacheEntry = serde_json::from_str(&cache_json).ok()?;

    // Validate that source file hasn't changed
    let current_hash = hash_file_content(source_content);
    if cache.source_hash != current_hash {
        // Source changed, cache is invalid
        return None;
    }

    Some(cache)
}

/// Save test cache for a given source file
pub fn save_cache(
    source_path: &Path,
    source_content: &str,
    test_hashes: HashMap<String, String>,
) -> Result<(), std::io::Error> {
    let cache_dir = get_cache_dir();

    // Create cache directory if it doesn't exist
    fs::create_dir_all(&cache_dir)?;

    // Use the source file's absolute path as the cache key
    let cache_key = source_path
        .canonicalize()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
        .to_string_lossy()
        .replace(['/', '\\'], "_");

    let cache_file = cache_dir.join(format!("{}.json", cache_key));

    // Create cache entry
    let source_hash = hash_file_content(source_content);
    let entry = TestCacheEntry::new(source_hash, test_hashes);

    // Serialize and write
    let cache_json = serde_json::to_string_pretty(&entry)?;
    fs::write(&cache_file, cache_json)?;

    Ok(())
}

/// Clear all test caches
pub fn clear_all_caches() -> Result<(), std::io::Error> {
    let cache_dir = get_cache_dir();
    if cache_dir.exists() {
        fs::remove_dir_all(&cache_dir)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_file_content_stable() {
        let content = "fn main() { }";
        let hash1 = hash_file_content(content);
        let hash2 = hash_file_content(content);
        assert_eq!(hash1, hash2, "Hash should be stable for same content");
    }

    #[test]
    fn test_hash_file_content_different() {
        let content1 = "fn main() { }";
        let content2 = "fn main() { return 1 }";
        let hash1 = hash_file_content(content1);
        let hash2 = hash_file_content(content2);
        assert_ne!(hash1, hash2, "Hash should differ for different content");
    }

    #[test]
    fn test_cache_entry_creation() {
        let mut test_hashes = HashMap::new();
        test_hashes.insert("test1".to_string(), "abc123".to_string());

        let entry = TestCacheEntry::new("source_hash".to_string(), test_hashes.clone());

        assert_eq!(entry.source_hash, "source_hash");
        assert_eq!(entry.test_hashes, test_hashes);
        assert!(entry.timestamp > 0);
    }
}
