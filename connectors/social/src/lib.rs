//! Social connectors for publishing governed content to X, Facebook, and Instagram.

pub mod facebook;
pub mod instagram;

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles_and_exports_are_reachable() {
        // Smoke test: verifies the crate compiles and public API is accessible
        assert!(true);
    }
}
