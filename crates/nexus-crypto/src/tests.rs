#[cfg(test)]
mod crypto_tests {
    use crate::*;

    #[test]
    fn generate_ed25519_identity() {
        let id = CryptoIdentity::generate(SignatureAlgorithm::Ed25519).unwrap();
        assert_eq!(id.algorithm(), SignatureAlgorithm::Ed25519);
        assert_eq!(id.verifying_key().len(), 32);
        assert_eq!(id.signing_key_bytes().len(), 32);
    }

    #[test]
    fn sign_and_verify_happy_path() {
        let id = CryptoIdentity::generate(SignatureAlgorithm::Ed25519).unwrap();
        let msg = b"hello nexus-os";
        let sig = id.sign(msg).unwrap();
        assert_eq!(sig.len(), 64);

        let valid =
            CryptoIdentity::verify(SignatureAlgorithm::Ed25519, id.verifying_key(), msg, &sig)
                .unwrap();
        assert!(valid);
    }

    #[test]
    fn verify_wrong_key_fails() {
        let id1 = CryptoIdentity::generate(SignatureAlgorithm::Ed25519).unwrap();
        let id2 = CryptoIdentity::generate(SignatureAlgorithm::Ed25519).unwrap();
        let msg = b"test message";
        let sig = id1.sign(msg).unwrap();

        let valid =
            CryptoIdentity::verify(SignatureAlgorithm::Ed25519, id2.verifying_key(), msg, &sig)
                .unwrap();
        assert!(!valid);
    }

    #[test]
    fn verify_tampered_message_fails() {
        let id = CryptoIdentity::generate(SignatureAlgorithm::Ed25519).unwrap();
        let sig = id.sign(b"original message").unwrap();

        let valid = CryptoIdentity::verify(
            SignatureAlgorithm::Ed25519,
            id.verifying_key(),
            b"tampered message",
            &sig,
        )
        .unwrap();
        assert!(!valid);
    }

    #[test]
    fn verify_tampered_signature_fails() {
        let id = CryptoIdentity::generate(SignatureAlgorithm::Ed25519).unwrap();
        let msg = b"test";
        let mut sig = id.sign(msg).unwrap();
        sig[0] ^= 0xff; // flip bits

        let valid =
            CryptoIdentity::verify(SignatureAlgorithm::Ed25519, id.verifying_key(), msg, &sig)
                .unwrap();
        assert!(!valid);
    }

    #[test]
    fn roundtrip_to_bytes_from_bytes() {
        let id = CryptoIdentity::generate(SignatureAlgorithm::Ed25519).unwrap();
        let msg = b"roundtrip test";
        let sig = id.sign(msg).unwrap();

        // Serialize signing key
        let sk_bytes = id.signing_key_bytes().to_vec();
        let vk_bytes = id.verifying_key().to_vec();

        // Reconstruct
        let restored = CryptoIdentity::from_bytes(SignatureAlgorithm::Ed25519, &sk_bytes).unwrap();
        assert_eq!(restored.verifying_key(), vk_bytes.as_slice());

        // Verify with restored identity
        let valid = CryptoIdentity::verify(
            SignatureAlgorithm::Ed25519,
            restored.verifying_key(),
            msg,
            &sig,
        )
        .unwrap();
        assert!(valid);
    }

    #[test]
    fn to_bytes_includes_algorithm() {
        let id = CryptoIdentity::generate(SignatureAlgorithm::Ed25519).unwrap();
        let bytes = id.to_bytes();
        // Algorithm byte + 32-byte signing key + 32-byte verifying key
        assert_eq!(bytes.len(), 1 + 32 + 32);
        assert_eq!(bytes[0], 0x01); // Ed25519 = 0x01
    }

    #[test]
    fn crypto_config_defaults() {
        let config = CryptoConfig::default();
        assert_eq!(
            config.default_signature_algorithm,
            SignatureAlgorithm::Ed25519
        );
        assert_eq!(
            config.default_key_exchange_algorithm,
            KeyExchangeAlgorithm::X25519
        );
        assert!(!config.require_hybrid_signatures);
    }

    #[test]
    fn crypto_config_serialization() {
        let config = CryptoConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let restored: CryptoConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(
            restored.default_signature_algorithm,
            config.default_signature_algorithm
        );
    }

    #[test]
    fn crypto_error_display() {
        let err = CryptoError::VerificationFailed;
        assert_eq!(err.to_string(), "signature verification failed");

        let err = CryptoError::InvalidKeyLength {
            expected: 32,
            actual: 16,
        };
        assert!(err.to_string().contains("32"));
        assert!(err.to_string().contains("16"));
    }

    #[test]
    fn generate_multiple_unique_identities() {
        let id1 = CryptoIdentity::generate(SignatureAlgorithm::Ed25519).unwrap();
        let id2 = CryptoIdentity::generate(SignatureAlgorithm::Ed25519).unwrap();
        assert_ne!(id1.verifying_key(), id2.verifying_key());
        assert_ne!(id1.signing_key_bytes(), id2.signing_key_bytes());
    }

    #[test]
    fn sign_empty_message() {
        let id = CryptoIdentity::generate(SignatureAlgorithm::Ed25519).unwrap();
        let sig = id.sign(b"").unwrap();
        assert_eq!(sig.len(), 64);

        let valid =
            CryptoIdentity::verify(SignatureAlgorithm::Ed25519, id.verifying_key(), b"", &sig)
                .unwrap();
        assert!(valid);
    }

    #[test]
    fn sign_large_message() {
        let id = CryptoIdentity::generate(SignatureAlgorithm::Ed25519).unwrap();
        let msg = vec![0x42u8; 1_000_000]; // 1 MB
        let sig = id.sign(&msg).unwrap();

        let valid =
            CryptoIdentity::verify(SignatureAlgorithm::Ed25519, id.verifying_key(), &msg, &sig)
                .unwrap();
        assert!(valid);
    }

    #[test]
    fn from_bytes_wrong_length_fails() {
        let result = CryptoIdentity::from_bytes(SignatureAlgorithm::Ed25519, &[0u8; 16]);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CryptoError::InvalidKeyLength {
                expected: 32,
                actual: 16
            }
        ));
    }

    #[test]
    fn verify_wrong_signature_length_fails() {
        let id = CryptoIdentity::generate(SignatureAlgorithm::Ed25519).unwrap();
        let result = CryptoIdentity::verify(
            SignatureAlgorithm::Ed25519,
            id.verifying_key(),
            b"msg",
            &[0u8; 32], // wrong length
        );
        assert!(result.is_err());
    }

    #[test]
    fn verify_wrong_key_length_fails() {
        let result = CryptoIdentity::verify(
            SignatureAlgorithm::Ed25519,
            &[0u8; 16], // wrong length
            b"msg",
            &[0u8; 64],
        );
        assert!(result.is_err());
    }

    #[test]
    fn hybrid_signature_serialization() {
        let hybrid = HybridSignature {
            classical: Some(vec![1, 2, 3]),
            post_quantum: None,
            classical_algorithm: Some(SignatureAlgorithm::Ed25519),
            post_quantum_algorithm: None,
        };
        let json = serde_json::to_string(&hybrid).unwrap();
        let restored: HybridSignature = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.classical, Some(vec![1, 2, 3]));
        assert!(restored.post_quantum.is_none());
    }

    #[test]
    fn algorithm_display() {
        assert_eq!(format!("{}", SignatureAlgorithm::Ed25519), "Ed25519");
        assert_eq!(format!("{}", KeyExchangeAlgorithm::X25519), "X25519");
    }

    #[test]
    fn algorithm_serialization() {
        let algo = SignatureAlgorithm::Ed25519;
        let json = serde_json::to_string(&algo).unwrap();
        assert_eq!(json, "\"Ed25519\"");
        let restored: SignatureAlgorithm = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, algo);
    }

    #[test]
    fn from_raw_keys_preserves_data() {
        let sk = vec![7u8; 32];
        let vk = vec![8u8; 32];
        let id = CryptoIdentity::from_raw_keys(SignatureAlgorithm::Ed25519, sk.clone(), vk.clone());
        assert_eq!(id.signing_key_bytes(), sk.as_slice());
        assert_eq!(id.verifying_key(), vk.as_slice());
    }

    // ── X25519 key exchange tests ──

    #[test]
    fn x25519_generate_and_dh() {
        let alice = KeyExchange::generate(KeyExchangeAlgorithm::X25519).unwrap();
        let bob = KeyExchange::generate(KeyExchangeAlgorithm::X25519).unwrap();

        assert_eq!(alice.public_key().len(), 32);
        assert_eq!(bob.public_key().len(), 32);
        assert_ne!(alice.public_key(), bob.public_key());

        let secret_a = alice.diffie_hellman(bob.public_key()).unwrap();
        let secret_b = bob.diffie_hellman(alice.public_key()).unwrap();
        assert_eq!(secret_a, secret_b);
        assert_eq!(secret_a.len(), 32);
    }

    #[test]
    fn x25519_from_secret_bytes_roundtrip() {
        let original = KeyExchange::generate(KeyExchangeAlgorithm::X25519).unwrap();
        let bob = KeyExchange::generate(KeyExchangeAlgorithm::X25519).unwrap();

        // Restore from secret key bytes
        let _restored = KeyExchange::from_secret_bytes(
            KeyExchangeAlgorithm::X25519,
            original.public_key(), // we need secret bytes, not public — get from internal
        );
        // from_secret_bytes expects the secret key, not public key — test that path too
        // We can't easily extract secret bytes through the public API, so test with a known key
        let known_secret = [42u8; 32];
        let kp1 =
            KeyExchange::from_secret_bytes(KeyExchangeAlgorithm::X25519, &known_secret).unwrap();
        let kp2 =
            KeyExchange::from_secret_bytes(KeyExchangeAlgorithm::X25519, &known_secret).unwrap();
        assert_eq!(kp1.public_key(), kp2.public_key());

        let shared1 = kp1.diffie_hellman(bob.public_key()).unwrap();
        let shared2 = kp2.diffie_hellman(bob.public_key()).unwrap();
        assert_eq!(shared1, shared2);
    }

    #[test]
    fn x25519_bad_key_length() {
        let alice = KeyExchange::generate(KeyExchangeAlgorithm::X25519).unwrap();
        let bad_key = vec![1u8; 16]; // too short
        let result = alice.diffie_hellman(&bad_key);
        assert!(result.is_err());
    }

    #[test]
    fn x25519_different_peers_different_secrets() {
        let alice = KeyExchange::generate(KeyExchangeAlgorithm::X25519).unwrap();
        let bob = KeyExchange::generate(KeyExchangeAlgorithm::X25519).unwrap();
        let carol = KeyExchange::generate(KeyExchangeAlgorithm::X25519).unwrap();

        let secret_ab = alice.diffie_hellman(bob.public_key()).unwrap();
        let secret_ac = alice.diffie_hellman(carol.public_key()).unwrap();
        assert_ne!(secret_ab, secret_ac);
    }
}
