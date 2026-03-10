use super::bundle::{BundleError, EvidenceBundle};
use std::fs;
use std::path::Path;

const MAGIC_HEADER: &str = "NEXUS-EVIDENCE-V1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceFile {
    pub magic: String,
    pub bundle: EvidenceBundle,
}

impl EvidenceFile {
    pub fn from_bundle(bundle: EvidenceBundle) -> Self {
        Self {
            magic: MAGIC_HEADER.to_string(),
            bundle,
        }
    }

    pub fn serialize(&self) -> Result<Vec<u8>, BundleError> {
        let wrapper = SerializedEvidence {
            magic: &self.magic,
            bundle: &self.bundle,
        };
        serde_json::to_vec_pretty(&wrapper)
            .map_err(|e| BundleError::FormatError(format!("serialization failed: {e}")))
    }

    pub fn deserialize(data: &[u8]) -> Result<Self, BundleError> {
        let wrapper: DeserializedEvidence = serde_json::from_slice(data)
            .map_err(|e| BundleError::FormatError(format!("deserialization failed: {e}")))?;

        if wrapper.magic != MAGIC_HEADER {
            return Err(BundleError::FormatError(format!(
                "invalid magic header: expected '{}', got '{}'",
                MAGIC_HEADER, wrapper.magic
            )));
        }

        Ok(Self {
            magic: wrapper.magic,
            bundle: wrapper.bundle,
        })
    }

    pub fn write_to_file(&self, path: &Path) -> Result<(), BundleError> {
        let data = self.serialize()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| BundleError::FormatError(format!("cannot create directory: {e}")))?;
        }
        fs::write(path, data)
            .map_err(|e| BundleError::FormatError(format!("cannot write file: {e}")))
    }

    pub fn read_from_file(path: &Path) -> Result<Self, BundleError> {
        let data = fs::read(path)
            .map_err(|e| BundleError::FormatError(format!("cannot read file: {e}")))?;
        Self::deserialize(&data)
    }
}

#[derive(serde::Serialize)]
struct SerializedEvidence<'a> {
    magic: &'a str,
    bundle: &'a EvidenceBundle,
}

#[derive(serde::Deserialize)]
struct DeserializedEvidence {
    magic: String,
    bundle: EvidenceBundle,
}

#[cfg(test)]
mod tests {
    use super::{EvidenceFile, MAGIC_HEADER};
    use crate::audit::{AuditTrail, EventType};
    use crate::autonomy::AutonomyLevel;
    use crate::manifest::AgentManifest;
    use crate::replay::bundle::{BundleError, EvidenceBundle, PolicySnapshot};
    use serde_json::json;
    use uuid::Uuid;

    fn test_manifest() -> AgentManifest {
        AgentManifest {
            name: "fmt-agent".to_string(),
            version: "1.0.0".to_string(),
            capabilities: vec!["web.search".to_string(), "fs.read".to_string()],
            fuel_budget: 10_000,
            autonomy_level: Some(3),
            consent_policy_path: None,
            requester_id: None,
            schedule: None,
            llm_model: None,
            fuel_period_id: None,
            monthly_fuel_cap: None,
            allowed_endpoints: None,
            domain_tags: vec![],
        }
    }

    fn make_bundle() -> EvidenceBundle {
        let agent_id = Uuid::new_v4();
        let manifest = test_manifest();
        let mut trail = AuditTrail::new();
        for i in 0..3 {
            trail
                .append_event(
                    agent_id,
                    EventType::ToolCall,
                    json!({"tool": "fs.read", "seq": i}),
                )
                .expect("audit: fail-closed");
        }
        let policy = PolicySnapshot {
            autonomy_level: AutonomyLevel::L3,
            consent_tiers: std::collections::BTreeMap::new(),
            capabilities: manifest.capabilities.clone(),
            fuel_budget: manifest.fuel_budget,
        };
        EvidenceBundle::export(
            agent_id,
            Uuid::new_v4(),
            &manifest,
            policy,
            &trail,
            vec![json!({"input": "test"})],
            vec![],
            100,
            10_000,
            AutonomyLevel::L3,
            vec![],
        )
        .unwrap()
    }

    #[test]
    fn round_trip_serialize_deserialize() {
        let bundle = make_bundle();
        let file = EvidenceFile::from_bundle(bundle.clone());

        let serialized = file.serialize().expect("serialize should succeed");
        let deserialized =
            EvidenceFile::deserialize(&serialized).expect("deserialize should succeed");

        assert_eq!(deserialized.magic, MAGIC_HEADER);
        assert_eq!(deserialized.bundle.bundle_id, bundle.bundle_id);
        assert_eq!(
            deserialized.bundle.audit_events.len(),
            bundle.audit_events.len()
        );
        assert_eq!(deserialized.bundle.bundle_digest, bundle.bundle_digest);
        assert_eq!(deserialized.bundle.run_id, bundle.run_id);
        assert_eq!(deserialized.bundle.manifest_hash, bundle.manifest_hash);
    }

    #[test]
    fn invalid_magic_header_rejected() {
        let bad_data = br#"{"magic":"WRONG-HEADER","bundle":{}}"#;
        let result = EvidenceFile::deserialize(bad_data);
        assert!(matches!(result, Err(BundleError::FormatError(_))));
    }

    #[test]
    fn file_round_trip() {
        let bundle = make_bundle();
        let file = EvidenceFile::from_bundle(bundle.clone());
        let path = std::env::temp_dir().join(format!(
            "nexus-evidence-test-{}.nexus-evidence",
            Uuid::new_v4()
        ));

        file.write_to_file(&path).expect("write should succeed");
        let loaded = EvidenceFile::read_from_file(&path).expect("read should succeed");

        assert_eq!(loaded.bundle.bundle_id, bundle.bundle_id);
        assert_eq!(loaded.bundle.bundle_digest, bundle.bundle_digest);
        assert_eq!(loaded.bundle.fuel_consumed, bundle.fuel_consumed);
        assert_eq!(loaded.bundle.fuel_budget, bundle.fuel_budget);

        let _ = std::fs::remove_file(&path);
    }
}
