// ===== Identity tests =====
mod identity {
    use nexus_code::governance::SessionIdentity;

    #[test]
    fn test_identity_creation() {
        let identity = SessionIdentity::new().unwrap();
        assert!(!identity.session_id().is_empty());
        assert_ne!(identity.public_key_bytes(), [0u8; 32]);
    }

    #[test]
    fn test_sign_and_verify() {
        let identity = SessionIdentity::new().unwrap();
        let data = b"hello world";
        let sig = identity.sign(data);
        assert!(identity.verify(data, &sig));
    }

    #[test]
    fn test_verify_wrong_data() {
        let identity = SessionIdentity::new().unwrap();
        let sig = identity.sign(b"hello");
        assert!(!identity.verify(b"tampered", &sig));
    }

    #[test]
    fn test_verify_wrong_signature() {
        let id1 = SessionIdentity::new().unwrap();
        let id2 = SessionIdentity::new().unwrap();
        let sig = id1.sign(b"data");
        // id2 should fail to verify id1's signature
        assert!(!id2.verify(b"data", &sig));
    }

    #[test]
    fn test_unique_session_ids() {
        let id1 = SessionIdentity::new().unwrap();
        let id2 = SessionIdentity::new().unwrap();
        assert_ne!(id1.session_id(), id2.session_id());
    }
}

// ===== Audit trail tests =====
mod audit {
    use std::sync::Arc;

    use nexus_code::governance::{AuditAction, AuditTrail, SessionIdentity};

    #[test]
    fn test_audit_trail_creation() {
        let identity = Arc::new(SessionIdentity::new().unwrap());
        let trail = AuditTrail::new(identity);
        assert_eq!(trail.len(), 0);
        assert!(trail.is_empty());
    }

    #[test]
    fn test_record_single_entry() {
        let identity = Arc::new(SessionIdentity::new().unwrap());
        let mut trail = AuditTrail::new(identity);
        let entry = trail.record(AuditAction::SessionStarted {
            public_key: "test".to_string(),
        });
        assert!(!entry.entry_hash.is_empty());
        assert_eq!(entry.entry_hash.len(), 64); // SHA-256 = 64 hex chars
        assert!(!entry.signature.is_empty());
        assert_eq!(entry.sequence, 0);
        assert_eq!(trail.len(), 1);
    }

    #[test]
    fn test_hash_chain_integrity() {
        let identity = Arc::new(SessionIdentity::new().unwrap());
        let mut trail = AuditTrail::new(identity);
        for i in 0..5 {
            trail.record(AuditAction::FuelConsumed {
                amount: i * 100,
                remaining: 50000 - i * 100,
            });
        }
        assert_eq!(trail.len(), 5);
        assert!(trail.verify_chain().is_ok());
    }

    #[test]
    fn test_hash_chain_tamper_detection() {
        let identity = Arc::new(SessionIdentity::new().unwrap());
        let mut trail = AuditTrail::new(identity);
        trail.record(AuditAction::SessionStarted {
            public_key: "key1".to_string(),
        });
        trail.record(AuditAction::FuelConsumed {
            amount: 100,
            remaining: 49900,
        });
        trail.record(AuditAction::SessionEnded {
            reason: "test".to_string(),
        });

        // Verify chain is valid before tampering
        assert!(trail.verify_chain().is_ok());

        // Tamper with entry[1]'s action
        trail.entries_mut()[1].action = AuditAction::Error {
            message: "TAMPERED".to_string(),
        };

        // Now verify should fail
        let result = trail.verify_chain();
        assert!(result.is_err());
    }

    #[test]
    fn test_previous_hash_tamper_detection() {
        let identity = Arc::new(SessionIdentity::new().unwrap());
        let mut trail = AuditTrail::new(identity);
        trail.record(AuditAction::SessionStarted {
            public_key: "key".to_string(),
        });
        trail.record(AuditAction::FuelConsumed {
            amount: 50,
            remaining: 49950,
        });
        trail.record(AuditAction::SessionEnded {
            reason: "done".to_string(),
        });

        assert!(trail.verify_chain().is_ok());

        // Tamper with entry[2]'s previous_hash
        trail.entries_mut()[2].previous_hash = "ff".repeat(32);

        let result = trail.verify_chain();
        assert!(result.is_err());
    }

    #[test]
    fn test_signature_verification() {
        let identity = Arc::new(SessionIdentity::new().unwrap());
        let mut trail = AuditTrail::new(identity.clone());
        trail.record(AuditAction::SessionStarted {
            public_key: "test_key".to_string(),
        });

        // verify_chain checks all signatures
        assert!(trail.verify_chain().is_ok());

        // Manually verify the signature
        let entry = &trail.entries()[0];
        let hash_bytes = hex::decode(&entry.entry_hash).unwrap();
        let sig_bytes = hex::decode(&entry.signature).unwrap();
        let sig = ed25519_dalek::Signature::from_bytes(sig_bytes.as_slice().try_into().unwrap());
        assert!(identity.verify(&hash_bytes, &sig));
    }

    #[test]
    fn test_first_entry_previous_hash() {
        let identity = Arc::new(SessionIdentity::new().unwrap());
        let mut trail = AuditTrail::new(identity);
        trail.record(AuditAction::SessionStarted {
            public_key: "key".to_string(),
        });
        let first = &trail.entries()[0];
        assert_eq!(
            first.previous_hash,
            "0000000000000000000000000000000000000000000000000000000000000000"
        );
    }

    #[test]
    fn test_sequential_numbering() {
        let identity = Arc::new(SessionIdentity::new().unwrap());
        let mut trail = AuditTrail::new(identity);
        for _ in 0..3 {
            trail.record(AuditAction::Error {
                message: "test".to_string(),
            });
        }
        for (i, entry) in trail.entries().iter().enumerate() {
            assert_eq!(entry.sequence, i as u64);
        }
    }

    #[test]
    fn test_chain_links_correctly() {
        let identity = Arc::new(SessionIdentity::new().unwrap());
        let mut trail = AuditTrail::new(identity);
        trail.record(AuditAction::SessionStarted {
            public_key: "a".to_string(),
        });
        trail.record(AuditAction::SessionEnded {
            reason: "done".to_string(),
        });

        let entries = trail.entries();
        assert_eq!(entries[1].previous_hash, entries[0].entry_hash);
    }

    #[test]
    fn test_audit_hash_determinism() {
        let identity = Arc::new(SessionIdentity::new().unwrap());
        let mut trail = AuditTrail::new(identity);

        trail.record(AuditAction::FuelConsumed {
            amount: 100,
            remaining: 900,
        });
        trail.record(AuditAction::FuelConsumed {
            amount: 100,
            remaining: 900,
        });

        let entries = trail.entries();
        assert_eq!(entries.len(), 2);

        // Same action, but different hashes (because previous_hash differs)
        assert_ne!(entries[0].entry_hash, entries[1].entry_hash);

        // Second entry's previous_hash is first entry's entry_hash
        assert_eq!(entries[1].previous_hash, entries[0].entry_hash);

        // Chain verifies
        assert!(trail.verify_chain().is_ok());
    }

    #[test]
    fn test_audit_tamper_detection_action_mutation() {
        let identity = Arc::new(SessionIdentity::new().unwrap());
        let mut trail = AuditTrail::new(identity);

        trail.record(AuditAction::SessionStarted {
            public_key: "abc123".to_string(),
        });
        trail.record(AuditAction::FuelConsumed {
            amount: 50,
            remaining: 950,
        });
        trail.record(AuditAction::FuelConsumed {
            amount: 100,
            remaining: 850,
        });

        assert!(trail.verify_chain().is_ok());

        // Tamper with the middle entry's action
        trail.entries_mut()[1].action = AuditAction::FuelConsumed {
            amount: 9999,
            remaining: 0,
        };

        // Chain MUST now fail verification
        assert!(trail.verify_chain().is_err());
    }

    #[test]
    fn test_audit_signature_manual_verification() {
        let identity = Arc::new(SessionIdentity::new().unwrap());
        let mut trail = AuditTrail::new(identity.clone());

        trail.record(AuditAction::SessionStarted {
            public_key: "test".to_string(),
        });

        let entry = &trail.entries()[0];

        // Manually verify: decode entry_hash to bytes, decode signature, verify with public key
        let hash_bytes = hex::decode(&entry.entry_hash).unwrap();
        let sig_bytes = hex::decode(&entry.signature).unwrap();
        let signature =
            ed25519_dalek::Signature::from_bytes(sig_bytes.as_slice().try_into().unwrap());

        // This MUST succeed
        assert!(identity.verify(&hash_bytes, &signature));

        // Verify with wrong data MUST fail
        assert!(!identity.verify(b"wrong data", &signature));
    }
}

// ===== Capability tests =====
mod capability {
    use nexus_code::governance::{Capability, CapabilityManager, CapabilityScope};

    #[test]
    fn test_default_capabilities() {
        let mgr = CapabilityManager::with_defaults();
        let granted = mgr.granted();
        let caps: Vec<_> = granted.iter().map(|g| g.capability).collect();
        assert!(caps.contains(&Capability::FileRead));
        assert!(caps.contains(&Capability::GitRead));
        assert!(caps.contains(&Capability::EnvRead));
        assert!(caps.contains(&Capability::LlmCall));
        assert_eq!(granted.len(), 4);
    }

    #[test]
    fn test_capability_grant_and_check() {
        let mut mgr = CapabilityManager::with_defaults();
        mgr.grant(Capability::FileWrite, CapabilityScope::Full);
        assert!(mgr.check(Capability::FileWrite, "/any/path").is_ok());
    }

    #[test]
    fn test_capability_denied() {
        let mut mgr = CapabilityManager::with_defaults();
        let result = mgr.check(Capability::FileWrite, "/some/path");
        assert!(result.is_err());
    }

    #[test]
    fn test_path_scoped_capability() {
        let mut mgr = CapabilityManager::with_defaults();
        mgr.grant(
            Capability::FileWrite,
            CapabilityScope::PathScoped(vec!["/home/user/project/**".to_string()]),
        );
        assert!(mgr
            .check(Capability::FileWrite, "/home/user/project/src/main.rs")
            .is_ok());
        assert!(mgr.check(Capability::FileWrite, "/etc/passwd").is_err());
    }

    #[test]
    fn test_command_scoped_capability() {
        let mut mgr = CapabilityManager::with_defaults();
        mgr.grant(
            Capability::ShellExecute,
            CapabilityScope::CommandScoped(vec!["cargo".to_string(), "git".to_string()]),
        );
        assert!(mgr.check(Capability::ShellExecute, "cargo test").is_ok());
        assert!(mgr.check(Capability::ShellExecute, "git status").is_ok());
        assert!(mgr.check(Capability::ShellExecute, "rm -rf /").is_err());
    }

    #[test]
    fn test_revoke_capability() {
        let mut mgr = CapabilityManager::with_defaults();
        mgr.grant(Capability::FileWrite, CapabilityScope::Full);
        assert!(mgr.check(Capability::FileWrite, "test").is_ok());
        mgr.revoke(Capability::FileWrite);
        assert!(mgr.check(Capability::FileWrite, "test").is_err());
    }

    #[test]
    fn test_denial_log() {
        let mut mgr = CapabilityManager::with_defaults();
        let _ = mgr.check(Capability::FileDelete, "/important/file");
        let _ = mgr.check(Capability::ShellExecute, "rm -rf /");
        assert_eq!(mgr.denial_log().len(), 2);
        assert_eq!(mgr.denial_log()[0].0, Capability::FileDelete);
        assert_eq!(mgr.denial_log()[1].0, Capability::ShellExecute);
    }

    #[test]
    fn test_capability_for_tool() {
        assert_eq!(
            Capability::for_tool("file_read"),
            Some(Capability::FileRead)
        );
        assert_eq!(Capability::for_tool("bash"), Some(Capability::ShellExecute));
        assert_eq!(Capability::for_tool("unknown"), None);
    }
}

// ===== Consent tests (2-phase model) =====
mod consent {
    use nexus_code::governance::{ConsentGate, ConsentOutcome, ConsentTier, SessionIdentity};

    #[test]
    fn test_tier1_auto_approve() {
        let mut gate = ConsentGate::new();
        let identity = SessionIdentity::new().unwrap();
        let outcome = gate.prepare("file_read", "reading test.rs", &identity);
        assert!(matches!(outcome, ConsentOutcome::AutoApproved(_)));
    }

    #[test]
    fn test_tier2_requires_consent() {
        let mut gate = ConsentGate::new();
        let identity = SessionIdentity::new().unwrap();
        let outcome = gate.prepare("file_write", "writing test.rs", &identity);
        assert!(matches!(outcome, ConsentOutcome::Required(_)));
        if let ConsentOutcome::Required(req) = outcome {
            assert_eq!(req.tier, ConsentTier::Tier2);
        }
    }

    #[test]
    fn test_tier3_destructive() {
        let mut gate = ConsentGate::new();
        let identity = SessionIdentity::new().unwrap();
        let outcome = gate.prepare("bash", "rm -rf /tmp/test", &identity);
        assert!(matches!(outcome, ConsentOutcome::Required(_)));
        if let ConsentOutcome::Required(req) = outcome {
            assert_eq!(req.tier, ConsentTier::Tier3);
        }
    }

    #[test]
    fn test_finalize_granted() {
        let mut gate = ConsentGate::new();
        let identity = SessionIdentity::new().unwrap();
        let outcome = gate.prepare("file_write", "writing", &identity);
        if let ConsentOutcome::Required(req) = outcome {
            let decision = gate.finalize(&req.id, true, &identity);
            assert!(decision.granted);
        } else {
            panic!("Expected Required");
        }
    }

    #[test]
    fn test_finalize_denied() {
        let mut gate = ConsentGate::new();
        let identity = SessionIdentity::new().unwrap();
        let outcome = gate.prepare("file_write", "writing", &identity);
        if let ConsentOutcome::Required(req) = outcome {
            let decision = gate.finalize(&req.id, false, &identity);
            assert!(!decision.granted);
        } else {
            panic!("Expected Required");
        }
    }

    #[test]
    fn test_decision_signature_non_empty() {
        let mut gate = ConsentGate::new();
        let identity = SessionIdentity::new().unwrap();

        // Auto-approved decision
        if let ConsentOutcome::AutoApproved(decision) = gate.prepare("file_read", "r", &identity) {
            assert!(!decision.signature.is_empty());
            assert_eq!(decision.signature.len(), 128);
        }

        // Finalized decision
        if let ConsentOutcome::Required(req) = gate.prepare("file_write", "w", &identity) {
            let decision = gate.finalize(&req.id, true, &identity);
            assert!(!decision.signature.is_empty());
            assert_eq!(decision.signature.len(), 128);
        }
    }

    #[test]
    fn test_decisions_recorded() {
        let mut gate = ConsentGate::new();
        let identity = SessionIdentity::new().unwrap();

        // Auto-approved
        gate.prepare("file_read", "r", &identity);
        assert_eq!(gate.decisions().len(), 1);

        // Required + finalized
        if let ConsentOutcome::Required(req) = gate.prepare("file_write", "w", &identity) {
            gate.finalize(&req.id, true, &identity);
        }
        assert_eq!(gate.decisions().len(), 2);
    }

    #[test]
    fn test_add_auto_approve() {
        let mut gate = ConsentGate::new();
        let identity = SessionIdentity::new().unwrap();

        assert!(!gate.is_auto_approved("file_write"));
        gate.add_auto_approve("file_write");
        assert!(gate.is_auto_approved("file_write"));

        let outcome = gate.prepare("file_write", "writing", &identity);
        assert!(matches!(outcome, ConsentOutcome::AutoApproved(_)));
    }
}

// ===== Fuel tests =====
mod fuel {
    use nexus_code::governance::{FuelCost, FuelMeter};

    #[test]
    fn test_fuel_creation() {
        let meter = FuelMeter::new(50_000);
        assert_eq!(meter.remaining(), 50_000);
        assert!(!meter.is_exhausted());
        assert_eq!(meter.usage_percentage(), 0.0);
    }

    #[test]
    fn test_reserve_and_consume() {
        let mut meter = FuelMeter::new(50_000);
        meter.reserve(1000).unwrap();
        assert_eq!(meter.remaining(), 49_000);

        meter.consume(
            "anthropic",
            FuelCost {
                input_tokens: 500,
                output_tokens: 300,
                fuel_units: 800,
                estimated_usd: 0.0024,
            },
        );
        // consumed=800, reserved reduced by 800 (1000-800=200 still reserved)
        // remaining = 50000 - 800 - 200 = 49000
        assert_eq!(meter.remaining(), 49_000);
        meter.release_reservation(200);
        assert_eq!(meter.remaining(), 49_200);
    }

    #[test]
    fn test_fuel_exhausted() {
        let mut meter = FuelMeter::new(100);
        assert!(meter.reserve(101).is_err());
    }

    #[test]
    fn test_release_reservation() {
        let mut meter = FuelMeter::new(1000);
        meter.reserve(500).unwrap();
        assert_eq!(meter.remaining(), 500);
        meter.release_reservation(500);
        assert_eq!(meter.remaining(), 1000);
    }

    #[test]
    fn test_cost_history() {
        let mut meter = FuelMeter::new(50_000);
        meter.consume(
            "openai",
            FuelCost {
                input_tokens: 100,
                output_tokens: 200,
                fuel_units: 300,
                estimated_usd: 0.001,
            },
        );
        meter.consume(
            "anthropic",
            FuelCost {
                input_tokens: 50,
                output_tokens: 100,
                fuel_units: 150,
                estimated_usd: 0.0005,
            },
        );
        assert_eq!(meter.cost_history().len(), 2);
    }

    #[test]
    fn test_usage_percentage() {
        let mut meter = FuelMeter::new(50_000);
        meter.consume(
            "test",
            FuelCost {
                input_tokens: 12500,
                output_tokens: 12500,
                fuel_units: 25000,
                estimated_usd: 0.075,
            },
        );
        let pct = meter.usage_percentage();
        assert!((pct - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_saturating_sub() {
        let mut meter = FuelMeter::new(100);
        meter.release_reservation(500); // more than reserved (0), shouldn't underflow
        assert_eq!(meter.remaining(), 100);
    }
}

// ===== GovernanceKernel tests =====
mod kernel {
    use nexus_code::governance::{AuthorizationResult, GovernanceKernel};

    #[test]
    fn test_governance_kernel_creation() {
        let kernel = GovernanceKernel::new(50_000).unwrap();
        assert_eq!(kernel.audit.len(), 1); // SessionStarted
        assert!(!kernel.identity.session_id().is_empty());
        assert_eq!(kernel.fuel.remaining(), 50_000);
    }

    #[test]
    fn test_authorize_tool_auto_approved() {
        let mut kernel = GovernanceKernel::new(50_000).unwrap();
        let result = kernel.authorize_tool("file_read", "/some/path", 100);
        assert!(result.is_ok());
        match result.unwrap() {
            AuthorizationResult::Authorized(decision) => {
                assert!(decision.granted);
            }
            AuthorizationResult::ConsentNeeded(_) => {
                panic!("file_read should be auto-approved");
            }
        }
    }

    #[test]
    fn test_authorize_tool_consent_needed() {
        let mut kernel = GovernanceKernel::new(50_000).unwrap();
        // Grant FileWrite first so capability check passes
        kernel.capabilities.grant(
            nexus_code::governance::Capability::FileWrite,
            nexus_code::governance::CapabilityScope::Full,
        );
        let result = kernel.authorize_tool("file_write", "/path", 100);
        assert!(result.is_ok());
        match result.unwrap() {
            AuthorizationResult::ConsentNeeded(req) => {
                assert_eq!(req.action, "file_write");
                assert_eq!(req.tier, nexus_code::governance::ConsentTier::Tier2);
            }
            AuthorizationResult::Authorized(_) => {
                panic!("file_write should require consent");
            }
        }
    }

    #[test]
    fn test_authorize_tool_denied_capability() {
        let mut kernel = GovernanceKernel::new(50_000).unwrap();
        let result = kernel.authorize_tool("file_delete", "/important", 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_authorize_tool_fuel_exhaustion() {
        let mut kernel = GovernanceKernel::new(100).unwrap();
        let result = kernel.authorize_tool("file_read", "/path", 200);
        assert!(result.is_err());
    }

    #[test]
    fn test_finalize_authorization_granted() {
        let mut kernel = GovernanceKernel::new(50_000).unwrap();
        kernel.capabilities.grant(
            nexus_code::governance::Capability::FileWrite,
            nexus_code::governance::CapabilityScope::Full,
        );
        let result = kernel.authorize_tool("file_write", "/path", 100).unwrap();
        if let AuthorizationResult::ConsentNeeded(req) = result {
            let decision = kernel.finalize_authorization(&req, true, 100);
            assert!(decision.is_ok());
            assert!(decision.unwrap().granted);
        }
    }

    #[test]
    fn test_finalize_authorization_denied() {
        let mut kernel = GovernanceKernel::new(50_000).unwrap();
        kernel.capabilities.grant(
            nexus_code::governance::Capability::FileWrite,
            nexus_code::governance::CapabilityScope::Full,
        );
        let result = kernel.authorize_tool("file_write", "/path", 100).unwrap();
        if let AuthorizationResult::ConsentNeeded(req) = result {
            let decision = kernel.finalize_authorization(&req, false, 100);
            assert!(decision.is_err()); // ConsentDenied
        }
    }

    #[test]
    fn test_end_session() {
        let mut kernel = GovernanceKernel::new(50_000).unwrap();
        let len_before = kernel.audit.len();
        kernel.end_session("test exit");
        assert_eq!(kernel.audit.len(), len_before + 1);
    }

    #[test]
    fn test_consent_two_phase_flow_tier2() {
        let mut kernel = GovernanceKernel::new(50_000).unwrap();

        kernel.capabilities.grant(
            nexus_code::governance::Capability::FileWrite,
            nexus_code::governance::CapabilityScope::Full,
        );

        let result = kernel
            .authorize_tool("file_write", "/tmp/test.rs", 100)
            .unwrap();

        match result {
            AuthorizationResult::ConsentNeeded(request) => {
                assert_eq!(request.action, "file_write");
                assert!(matches!(
                    request.tier,
                    nexus_code::governance::ConsentTier::Tier2
                ));

                let decision = kernel.finalize_authorization(&request, true, 100).unwrap();
                assert!(decision.granted);
                assert!(!decision.signature.is_empty());
            }
            AuthorizationResult::Authorized(_) => {
                panic!("file_write should require consent, not be auto-approved");
            }
        }

        assert!(kernel.audit.len() >= 3);
        assert!(kernel.audit.verify_chain().is_ok());
    }

    #[test]
    fn test_consent_two_phase_flow_denied() {
        let mut kernel = GovernanceKernel::new(50_000).unwrap();

        kernel.capabilities.grant(
            nexus_code::governance::Capability::FileWrite,
            nexus_code::governance::CapabilityScope::Full,
        );

        let result = kernel
            .authorize_tool("file_write", "/tmp/test.rs", 500)
            .unwrap();

        match result {
            AuthorizationResult::ConsentNeeded(request) => {
                let err = kernel.finalize_authorization(&request, false, 500);
                assert!(err.is_err());

                // Fuel reservation should be released
                assert_eq!(kernel.fuel.remaining(), 50_000);
            }
            _ => panic!("Expected ConsentNeeded"),
        }

        assert!(kernel.audit.verify_chain().is_ok());
    }

    #[test]
    fn test_consent_tier1_auto_approved() {
        let mut kernel = GovernanceKernel::new(50_000).unwrap();

        let result = kernel
            .authorize_tool("file_read", "/tmp/test.rs", 0)
            .unwrap();

        match result {
            AuthorizationResult::Authorized(decision) => {
                assert!(decision.granted);
            }
            AuthorizationResult::ConsentNeeded(_) => {
                panic!("file_read should be auto-approved (Tier1)");
            }
        }
    }
}
