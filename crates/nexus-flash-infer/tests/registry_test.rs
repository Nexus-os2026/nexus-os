use std::path::Path;

use nexus_flash_infer::backend::ModelFormat;
use nexus_flash_infer::llama_backend::LlamaBackend;
use nexus_flash_infer::registry::{detect_format, BackendRegistry};
use nexus_flash_infer::types::{HardwareInfo, RamType, SsdType};

fn test_hw() -> HardwareInfo {
    HardwareInfo {
        total_ram_mb: 32768,
        cpu_cores: 16,
        has_avx2: true,
        has_avx512: false,
        has_metal: false,
        has_cuda: false,
        ssd_type: SsdType::NVMe,
        ssd_read_speed_mb_s: 3500,
        numa_nodes: 1,
        ram_type: RamType::DDR5,
        mem_bandwidth_gbps: 11.0,
    }
}

#[test]
fn test_detect_format_gguf() {
    let format = detect_format(Path::new("/models/qwen.gguf")).unwrap();
    assert_eq!(format, ModelFormat::GGUF);
}

#[test]
fn test_detect_format_safetensors() {
    let format = detect_format(Path::new("/models/model.safetensors")).unwrap();
    assert_eq!(format, ModelFormat::SafeTensors);
}

#[test]
fn test_detect_format_unknown() {
    let result = detect_format(Path::new("/models/model.bin"));
    assert!(result.is_err());
}

#[test]
fn test_registry_empty() {
    let registry = BackendRegistry::new();
    assert!(registry.list_backends().is_empty());
    assert!(registry.supported_formats().is_empty());
}

#[test]
fn test_registry_register_llama() {
    let mut registry = BackendRegistry::new();
    registry.register(Box::new(LlamaBackend::new(test_hw())));

    assert_eq!(registry.list_backends(), vec!["llama.cpp"]);
    assert!(registry.supported_formats().contains(&ModelFormat::GGUF));
}

#[test]
fn test_registry_select_gguf_backend() {
    let mut registry = BackendRegistry::new();
    registry.register(Box::new(LlamaBackend::new(test_hw())));

    let backend = registry
        .select_backend(Path::new("/models/test.gguf"))
        .unwrap();
    assert_eq!(backend.name(), "llama.cpp");
}

#[test]
fn test_registry_no_backend_for_onnx() {
    let mut registry = BackendRegistry::new();
    registry.register(Box::new(LlamaBackend::new(test_hw())));

    let result = registry.select_for_format(&ModelFormat::ONNX);
    assert!(result.is_err());
}

#[test]
fn test_registry_select_backend_for_model_path() {
    let mut registry = BackendRegistry::new();
    registry.register(Box::new(LlamaBackend::new(test_hw())));

    let backend = registry
        .select_backend(Path::new("/home/user/models/Qwen3.5-397B-A17B-Q4_K_M.gguf"))
        .unwrap();
    assert_eq!(backend.name(), "llama.cpp");
}
