//! WASM module compilation cache.
//!
//! Keyed by SHA-256 content hash so identical bytecode is compiled only once
//! per `Engine` lifetime. Thread-safe via `Arc<Mutex<>>`.

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use wasmtime::{Engine, Module};

/// SHA-256 hex digest of WASM bytecode, used as cache key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContentHash(pub String);

impl ContentHash {
    /// Compute the SHA-256 content hash of the given bytes.
    pub fn of(bytes: &[u8]) -> Self {
        let digest = Sha256::digest(bytes);
        let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        Self(hex)
    }
}

/// Thread-safe cache of compiled `wasmtime::Module` instances keyed by content hash.
#[derive(Debug)]
pub struct ModuleCache {
    inner: Arc<Mutex<HashMap<ContentHash, Module>>>,
    hits: Arc<AtomicU64>,
    misses: Arc<AtomicU64>,
}

impl Clone for ModuleCache {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            hits: Arc::clone(&self.hits),
            misses: Arc::clone(&self.misses),
        }
    }
}

impl Default for ModuleCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleCache {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            hits: Arc::new(AtomicU64::new(0)),
            misses: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Get a cached module or compile, store, and return it.
    ///
    /// Returns `Ok((module, true))` on cache hit, `Ok((module, false))` on miss.
    pub fn get_or_compile(
        &self,
        engine: &Engine,
        wasm_bytes: &[u8],
    ) -> Result<(Module, bool), wasmtime::Error> {
        let hash = ContentHash::of(wasm_bytes);
        let mut map = self.inner.lock().unwrap_or_else(|poisoned| {
            eprintln!("Lock was poisoned, recovering inner data");
            poisoned.into_inner()
        });

        if let Some(module) = map.get(&hash) {
            self.hits.fetch_add(1, Ordering::Relaxed);
            return Ok((module.clone(), true));
        }

        let module = Module::new(engine, wasm_bytes)?;
        map.insert(hash, module.clone());
        self.misses.fetch_add(1, Ordering::Relaxed);
        Ok((module, false))
    }

    /// Cache hit rate as a value between 0.0 and 1.0.
    ///
    /// Returns -1.0 if no lookups have occurred yet.
    pub fn hit_rate(&self) -> f64 {
        let h = self.hits.load(Ordering::Relaxed);
        let m = self.misses.load(Ordering::Relaxed);
        let total = h + m;
        if total == 0 {
            return -1.0;
        }
        h as f64 / total as f64
    }

    /// Number of cached modules.
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap_or_else(|poisoned| {
            eprintln!("Lock was poisoned, recovering inner data");
            poisoned.into_inner()
        }).len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Check if a module with the given hash is cached.
    pub fn contains(&self, hash: &ContentHash) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| {
                eprintln!("Lock was poisoned, recovering inner data");
                poisoned.into_inner()
            })
            .contains_key(hash)
    }

    /// Clear all cached modules.
    pub fn clear(&self) {
        self.inner.lock().unwrap_or_else(|poisoned| {
            eprintln!("Lock was poisoned, recovering inner data");
            poisoned.into_inner()
        }).clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_engine() -> Engine {
        let mut config = wasmtime::Config::new();
        config.consume_fuel(true);
        Engine::new(&config).unwrap()
    }

    #[test]
    fn cache_miss_compiles_and_stores() {
        let engine = test_engine();
        let cache = ModuleCache::new();
        let wasm = wat::parse_str("(module)").unwrap();

        assert!(cache.is_empty());

        let (_, hit) = cache.get_or_compile(&engine, &wasm).unwrap();
        assert!(!hit, "first compilation should be a cache miss");
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn cache_hit_avoids_recompilation() {
        let engine = test_engine();
        let cache = ModuleCache::new();
        let wasm = wat::parse_str("(module)").unwrap();

        let (_, hit1) = cache.get_or_compile(&engine, &wasm).unwrap();
        assert!(!hit1);

        let (_, hit2) = cache.get_or_compile(&engine, &wasm).unwrap();
        assert!(hit2, "second lookup should be a cache hit");
        assert_eq!(cache.len(), 1, "cache should still have exactly 1 entry");
    }

    #[test]
    fn different_modules_get_separate_entries() {
        let engine = test_engine();
        let cache = ModuleCache::new();
        let wasm_a = wat::parse_str("(module)").unwrap();
        let wasm_b = wat::parse_str("(module (memory 1))").unwrap();

        cache.get_or_compile(&engine, &wasm_a).unwrap();
        cache.get_or_compile(&engine, &wasm_b).unwrap();
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn content_hash_deterministic() {
        let data = b"hello wasm";
        let h1 = ContentHash::of(data);
        let h2 = ContentHash::of(data);
        assert_eq!(h1, h2);
    }

    #[test]
    fn clear_empties_cache() {
        let engine = test_engine();
        let cache = ModuleCache::new();
        let wasm = wat::parse_str("(module)").unwrap();

        cache.get_or_compile(&engine, &wasm).unwrap();
        assert_eq!(cache.len(), 1);

        cache.clear();
        assert!(cache.is_empty());
    }
}
