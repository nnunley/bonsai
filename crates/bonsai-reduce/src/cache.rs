use std::collections::HashMap;
use xxhash_rust::xxh3::xxh3_64;

/// Caches interestingness test results to avoid redundant test executions.
/// Uses xxhash-64 of source bytes as the key.
///
/// Uses 64-bit hashes. Collisions are extremely rare (~1/2^64 per lookup)
/// but can cause incorrect cache hits. The reducer re-verifies its final
/// output to catch any corruption from collisions.
pub struct TestCache {
    results: HashMap<u64, bool>,
    hits: u64,
    misses: u64,
}

impl TestCache {
    pub fn new() -> Self {
        Self {
            results: HashMap::new(),
            hits: 0,
            misses: 0,
        }
    }

    /// Look up a cached result for the given source bytes.
    /// Returns Some(result) on cache hit, None on miss.
    pub fn get(&mut self, source: &[u8]) -> Option<bool> {
        let hash = xxh3_64(source);
        match self.results.get(&hash) {
            Some(&result) => {
                self.hits += 1;
                Some(result)
            }
            None => {
                self.misses += 1;
                None
            }
        }
    }

    /// Store a test result for the given source bytes.
    pub fn put(&mut self, source: &[u8], result: bool) {
        let hash = xxh3_64(source);
        self.results.insert(hash, result);
    }

    /// Number of cache hits.
    pub fn hits(&self) -> u64 {
        self.hits
    }

    /// Number of cache misses.
    pub fn misses(&self) -> u64 {
        self.misses
    }

    /// Total lookups (hits + misses).
    pub fn lookups(&self) -> u64 {
        self.hits + self.misses
    }

    /// Cache hit rate as a fraction [0.0, 1.0].
    pub fn hit_rate(&self) -> f64 {
        if self.lookups() == 0 {
            0.0
        } else {
            self.hits as f64 / self.lookups() as f64
        }
    }

    /// Number of entries in the cache.
    pub fn len(&self) -> usize {
        self.results.len()
    }

    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }
}

impl Default for TestCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_miss_then_hit() {
        let mut cache = TestCache::new();

        // First lookup: miss
        assert_eq!(cache.get(b"hello world"), None);
        assert_eq!(cache.misses(), 1);
        assert_eq!(cache.hits(), 0);

        // Store result
        cache.put(b"hello world", true);

        // Second lookup: hit
        assert_eq!(cache.get(b"hello world"), Some(true));
        assert_eq!(cache.hits(), 1);
        assert_eq!(cache.misses(), 1);
    }

    #[test]
    fn test_cache_different_content() {
        let mut cache = TestCache::new();
        cache.put(b"content A", true);
        cache.put(b"content B", false);

        assert_eq!(cache.get(b"content A"), Some(true));
        assert_eq!(cache.get(b"content B"), Some(false));
        assert_eq!(cache.get(b"content C"), None);
    }

    #[test]
    fn test_cache_hit_rate() {
        let mut cache = TestCache::new();
        cache.put(b"data", true);

        cache.get(b"data");  // hit
        cache.get(b"data");  // hit
        cache.get(b"other"); // miss

        assert_eq!(cache.hits(), 2);
        assert_eq!(cache.misses(), 1);
        assert!((cache.hit_rate() - 2.0/3.0).abs() < 0.01);
    }

    #[test]
    fn test_cache_empty() {
        let cache = TestCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.hit_rate(), 0.0);
    }

    #[test]
    fn test_cache_overwrite() {
        let mut cache = TestCache::new();
        cache.put(b"data", true);
        cache.put(b"data", false);
        assert_eq!(cache.get(b"data"), Some(false));
        assert_eq!(cache.len(), 1); // same hash, overwritten
    }
}
