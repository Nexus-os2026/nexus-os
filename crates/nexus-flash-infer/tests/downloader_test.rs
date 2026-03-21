#[cfg(feature = "download")]
mod download_tests {
    use nexus_flash_infer::downloader::{extract_quant_from_filename, format_bytes, ModelStorage};

    #[test]
    fn format_bytes_display() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(2048), "2.0 KB");
        assert_eq!(format_bytes(10_485_760), "10.0 MB");
        assert_eq!(format_bytes(1_073_741_824), "1.0 GB");
        assert_eq!(format_bytes(225_485_783_040), "210.0 GB");
    }

    #[test]
    fn extract_quant_standard() {
        assert_eq!(
            extract_quant_from_filename("Llama-3.3-70B-Q4_K_M.gguf"),
            "Q4_K_M"
        );
        assert_eq!(extract_quant_from_filename("model-Q8_0.gguf"), "Q8_0");
        assert_eq!(
            extract_quant_from_filename("phi-4-mini-IQ4_XS.gguf"),
            "IQ4_XS"
        );
    }

    #[test]
    fn extract_quant_with_ud_prefix() {
        assert_eq!(
            extract_quant_from_filename("Qwen3.5-397B-A17B-UD-Q4_K_XL.gguf"),
            "UD-Q4_K_XL"
        );
    }

    #[test]
    fn extract_quant_with_shard_suffix() {
        assert_eq!(
            extract_quant_from_filename("Qwen3.5-397B-A17B-UD-Q4_K_XL-00001-of-00005.gguf"),
            "UD-Q4_K_XL"
        );
    }

    #[test]
    fn extract_quant_unknown() {
        assert_eq!(extract_quant_from_filename("random-model.gguf"), "Unknown");
    }

    #[test]
    fn storage_create_and_list() {
        let tmp = std::env::temp_dir().join("nexus-flash-dl-test-1");
        let _ = std::fs::remove_dir_all(&tmp);

        let storage = ModelStorage::with_dir(tmp.clone()).unwrap();
        assert!(storage.list_models().unwrap().is_empty());

        // Create fake .gguf files
        std::fs::write(storage.model_path("test-Q4_K_M.gguf"), b"fake-gguf").unwrap();
        std::fs::write(storage.model_path("another-Q8_0.gguf"), b"data").unwrap();
        // Non-gguf should be ignored
        std::fs::write(tmp.join("notes.txt"), b"ignore").unwrap();

        let models = storage.list_models().unwrap();
        assert_eq!(models.len(), 2);

        // Check quant extraction
        let names: Vec<&str> = models.iter().map(|m| m.quant_type.as_str()).collect();
        assert!(names.contains(&"Q4_K_M"));
        assert!(names.contains(&"Q8_0"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn storage_delete_model() {
        let tmp = std::env::temp_dir().join("nexus-flash-dl-test-2");
        let _ = std::fs::remove_dir_all(&tmp);

        let storage = ModelStorage::with_dir(tmp.clone()).unwrap();
        std::fs::write(storage.model_path("to-delete-Q4_K_M.gguf"), b"data").unwrap();
        // Also create a .part file
        std::fs::write(tmp.join("to-delete-Q4_K_M.gguf.part"), b"partial").unwrap();

        assert_eq!(storage.list_models().unwrap().len(), 1);

        storage.delete_model("to-delete-Q4_K_M.gguf").unwrap();
        assert!(storage.list_models().unwrap().is_empty());
        // .part file should also be cleaned up
        assert!(!tmp.join("to-delete-Q4_K_M.gguf.part").exists());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn storage_delete_nonexistent_ok() {
        let tmp = std::env::temp_dir().join("nexus-flash-dl-test-3");
        let _ = std::fs::remove_dir_all(&tmp);

        let storage = ModelStorage::with_dir(tmp.clone()).unwrap();
        assert!(storage.delete_model("nonexistent.gguf").is_ok());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn storage_disk_space() {
        let tmp = std::env::temp_dir().join("nexus-flash-dl-test-4");
        let _ = std::fs::remove_dir_all(&tmp);

        let storage = ModelStorage::with_dir(tmp.clone()).unwrap();
        let space = storage.available_disk_space().unwrap();
        // Should be positive on any real filesystem
        assert!(space > 0, "expected positive disk space, got {space}");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn storage_total_models_size() {
        let tmp = std::env::temp_dir().join("nexus-flash-dl-test-5");
        let _ = std::fs::remove_dir_all(&tmp);

        let storage = ModelStorage::with_dir(tmp.clone()).unwrap();
        assert_eq!(storage.total_models_size().unwrap(), 0);

        std::fs::write(storage.model_path("a-Q4_K_M.gguf"), b"12345").unwrap();
        std::fs::write(storage.model_path("b-Q8_0.gguf"), b"123456789").unwrap();

        let total = storage.total_models_size().unwrap();
        assert_eq!(total, 14); // 5 + 9 bytes

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
