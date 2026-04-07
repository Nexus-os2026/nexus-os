//! DOM enumerator stub. Phase 1.1 returns an empty vector. Real DOM
//! scraping (via Ollama in Phase 1.2) lands later.

/// One enumerated UI element.
pub struct Element {
    pub id: String,
    pub fingerprint: String,
    pub kind: String,
    pub bounds: (i32, i32, i32, i32),
}

/// DOM enumerator. Holds no state in Phase 1.1.
pub struct Enumerator;

impl Enumerator {
    /// Enumerate every interactive element on a page. Phase 1.1 stub.
    pub fn enumerate_page(&self, _page: &str) -> crate::Result<Vec<Element>> {
        Ok(Vec::new())
    }
}
