//! Benchmark suite for Nexus OS kernel and subsystems.
//!
//! Run: `cargo bench -p nexus-benchmarks`
//! HTML reports are generated in `target/criterion/`.

#[cfg(test)]
mod tests {
    #[test]
    fn kernel_types_reachable() {
        let audit = nexus_kernel::audit::AuditTrail::new();
        assert!(audit.events().is_empty());
    }

    #[test]
    fn supervisor_creates_with_zero_agents() {
        let supervisor = nexus_kernel::supervisor::Supervisor::new();
        let health = supervisor.health_check();
        assert!(health.is_empty());
    }

    #[test]
    fn redaction_engine_scans_and_applies_pii() {
        use nexus_kernel::redaction::RedactionEngine;
        let input = "My SSN is 123-45-6789 and email is test@example.com";
        let findings = RedactionEngine::scan(input);
        assert!(!findings.is_empty(), "should detect PII");
        let output = RedactionEngine::apply(input, &findings);
        assert!(!output.contains("123-45-6789"));
    }

    #[test]
    fn manifest_parse_toml() {
        let toml = r#"
name = "bench-agent"
version = "1.0.0"
description = "A benchmark test agent"
capabilities = ["web.search"]
fuel_budget = 1000
"#;
        let manifest = nexus_kernel::manifest::parse_manifest(toml);
        assert!(manifest.is_ok());
        assert_eq!(manifest.unwrap().name, "bench-agent");
    }

    #[test]
    fn supervisor_start_and_stop_agent() {
        let mut supervisor = nexus_kernel::supervisor::Supervisor::new();
        let manifest = nexus_kernel::manifest::parse_manifest(
            "name = \"fuel-test\"\nversion = \"1.0.0\"\ndescription = \"test\"\ncapabilities = [\"web.search\"]\nfuel_budget = 500\n",
        ).unwrap();
        let id = supervisor.start_agent(manifest).unwrap();
        assert_eq!(supervisor.health_check().len(), 1);
        supervisor.stop_agent(id).unwrap();
    }
}
