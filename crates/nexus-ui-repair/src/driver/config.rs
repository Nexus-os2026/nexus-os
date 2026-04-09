//! Phase 1.5 Group B — driver enumeration source selector.
//!
//! The scout driver can now be pointed either at a pre-recorded
//! fixture file (the original Phase 1.4 path) or at a live running
//! Nexus OS instance via AT-SPI. This module defines the selector;
//! the field is embedded in [`crate::driver::DriverConfig`].

use std::path::PathBuf;

use crate::specialists::live_enumerator::LiveTargetConfig;

/// Selects whether the driver uses pre-recorded fixture data or a
/// live AT-SPI walk.
#[derive(Debug, Clone)]
pub enum EnumerationSource {
    /// Run against a fixture file (pre-recorded element list). All
    /// existing Phase 1.4 tests use this mode. No running Nexus OS
    /// instance is required.
    Fixture(PathBuf),
    /// Run against a live Nexus OS instance via the AT-SPI
    /// accessibility tree. Requires Nexus OS to be running and
    /// AT-SPI to be accessible.
    Live(LiveTargetConfig),
}

impl EnumerationSource {
    /// Safe default used by existing Phase 1.4 / Group A construction
    /// sites that have not been migrated to a real target yet.
    pub fn default_fixture() -> Self {
        EnumerationSource::Fixture(PathBuf::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::DriverConfig;

    #[test]
    fn test_enumeration_source_fixture_variant() {
        let src = EnumerationSource::Fixture(PathBuf::from("/tmp/fixture.json"));
        match src {
            EnumerationSource::Fixture(p) => {
                assert_eq!(p, PathBuf::from("/tmp/fixture.json"));
            }
            EnumerationSource::Live(_) => panic!("expected Fixture variant"),
        }
    }

    #[test]
    fn test_enumeration_source_live_variant() {
        let src = EnumerationSource::Live(LiveTargetConfig {
            app_name: "nexus-os".to_string(),
            page_route: "/chat".to_string(),
            tab: None,
        });
        match src {
            EnumerationSource::Live(cfg) => {
                assert_eq!(cfg.app_name, "nexus-os");
            }
            EnumerationSource::Fixture(_) => panic!("expected Live variant"),
        }
    }

    #[test]
    fn test_driver_config_default_target() {
        let mut cfg = DriverConfig::default_at_home();
        cfg.target = EnumerationSource::Fixture(PathBuf::from("/tmp/x.json"));
        assert!(matches!(cfg.target, EnumerationSource::Fixture(_)));
    }
}
