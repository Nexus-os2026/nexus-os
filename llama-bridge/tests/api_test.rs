use nexus_llama_bridge::*;

#[test]
fn test_default_generation_config() {
    let config = GenerationConfig::default();
    // Defaults are speed-optimized: greedy sampling, small context, quantized KV
    assert_eq!(config.temperature, 0.0); // Greedy
    assert!(config.max_tokens > 0);
    assert_eq!(config.top_p, 1.0); // Disabled
    assert_eq!(config.top_k, 0); // Disabled
    assert_eq!(config.repeat_penalty, 1.1); // Anti-repetition enabled
    assert_eq!(config.n_ctx, 2048);
    assert_eq!(config.n_batch, 2048);
    assert_eq!(config.n_ubatch, 512);
    assert!(config.flash_attn);
    assert!(config.n_threads.is_none());
    assert_eq!(config.type_k, Some(KvCacheType::Q8_0));
    assert_eq!(config.type_v, Some(KvCacheType::Q8_0));
}

#[test]
fn test_fast_generation_config() {
    let config = GenerationConfig::fast();
    assert_eq!(config.temperature, 0.0);
    assert_eq!(config.top_k, 0);
    assert_eq!(config.top_p, 1.0);
    assert_eq!(config.min_p, 0.0);
    assert_eq!(config.repeat_penalty, 1.1);
    assert_eq!(config.n_ctx, 2048);
    assert_eq!(config.type_k, Some(KvCacheType::Q8_0));
}

#[test]
fn test_balanced_generation_config() {
    let config = GenerationConfig::balanced();
    assert!(config.temperature > 0.0);
    assert!(config.top_p < 1.0);
    assert_eq!(config.n_ctx, 4096);
    assert_eq!(config.type_k, Some(KvCacheType::Q8_0));
}

#[test]
fn test_default_model_load_config() {
    let config = ModelLoadConfig::default();
    assert_eq!(config.n_gpu_layers, 0);
    assert!(config.use_mmap);
    assert!(!config.use_mlock);
    assert!(config.cpu_moe);
    assert!(config.n_threads.is_none());
    assert_eq!(config.numa_strategy, NumaStrategy::Disabled);
}

#[test]
fn test_model_load_config_custom() {
    let config = ModelLoadConfig {
        model_path: "/models/qwen3.5-moe-a3b.Q4_K_M.gguf".into(),
        n_gpu_layers: 0,
        use_mmap: true,
        use_mlock: false,
        cpu_moe: true,
        n_threads: Some(16),
        numa_strategy: NumaStrategy::Distribute,
    };
    assert_eq!(config.n_gpu_layers, 0);
    assert_eq!(config.n_threads, Some(16));
    assert_eq!(config.numa_strategy, NumaStrategy::Distribute);
}

#[test]
fn test_memory_usage_struct() {
    let usage = MemoryUsage {
        model_size_mb: 5500,
        context_size_mb: 2000,
        total_mb: 7500,
    };
    assert_eq!(usage.total_mb, 7500);
    assert_eq!(usage.model_size_mb, 5500);
}

#[test]
fn test_perf_stats() {
    let stats = PerfStats {
        tokens_generated: 100,
        prompt_tokens: 50,
        prompt_eval_time_ms: 1000.0,
        generation_time_ms: 10000.0,
        tokens_per_second: 10.0,
        prompt_tokens_per_second: 50.0,
        memory_used_mb: 8000,
    };
    assert!(stats.tokens_per_second > 0.0);
    assert_eq!(stats.tokens_generated, 100);
}

#[test]
fn test_model_metadata_moe_detection() {
    let meta = ModelMetadata {
        architecture: "qwen3.5".into(),
        total_params: 397_000_000_000,
        file_size_bytes: 209_000_000_000,
        context_length: 262144,
        vocab_size: 152064,
        quantization: "Q4_K_M".into(),
        is_moe: true,
        num_experts: Some(512),
        num_active_experts: Some(10),
        num_layers: 60,
        embedding_size: 4096,
    };
    assert!(meta.is_moe);
    assert_eq!(meta.num_experts, Some(512));
    assert_eq!(meta.num_active_experts, Some(10));
    assert_eq!(meta.context_length, 262144);
}

#[test]
fn test_model_metadata_dense() {
    let meta = ModelMetadata {
        architecture: "llama".into(),
        total_params: 8_000_000_000,
        file_size_bytes: 4_500_000_000,
        context_length: 8192,
        vocab_size: 32000,
        quantization: "Q4_0".into(),
        is_moe: false,
        num_experts: None,
        num_active_experts: None,
        num_layers: 32,
        embedding_size: 4096,
    };
    assert!(!meta.is_moe);
    assert_eq!(meta.num_experts, None);
}

#[test]
fn test_hardware_detection() {
    let hw = detect_hardware();
    assert!(hw.cpu_cores > 0);
    // RAM detection may return 0 on non-Linux, that's fine
}

#[test]
fn test_error_display_memory_budget() {
    let err = LlamaError::MemoryBudgetExceeded {
        needed_mb: 48000,
        available_mb: 32000,
    };
    let msg = format!("{err}");
    assert!(msg.contains("48000"));
    assert!(msg.contains("32000"));
}

#[test]
fn test_error_display_model_load() {
    let err = LlamaError::ModelLoadFailed {
        path: "/tmp/model.gguf".into(),
        reason: "file not found".into(),
    };
    let msg = format!("{err}");
    assert!(msg.contains("/tmp/model.gguf"));
    assert!(msg.contains("file not found"));
}

#[test]
fn test_error_display_decode() {
    let err = LlamaError::DecodeFailed(-1);
    let msg = format!("{err}");
    assert!(msg.contains("-1"));
}

#[test]
fn test_numa_strategy_serialization() {
    let strategy = NumaStrategy::Distribute;
    let json = serde_json::to_string(&strategy).expect("serialize");
    assert!(json.contains("Distribute"));

    let parsed: NumaStrategy = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed, NumaStrategy::Distribute);
}

#[test]
fn test_generation_config_serialization() {
    let config = GenerationConfig::default();
    let json = serde_json::to_string(&config).expect("serialize");
    let parsed: GenerationConfig = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed.temperature, config.temperature);
    assert_eq!(parsed.max_tokens, config.max_tokens);
}

#[test]
fn test_control_flow_eq() {
    assert_eq!(ControlFlow::Continue, ControlFlow::Continue);
    assert_eq!(ControlFlow::Stop, ControlFlow::Stop);
    assert_ne!(ControlFlow::Continue, ControlFlow::Stop);
}

#[test]
fn test_token_event_variants() {
    let tok = TokenEvent::Token {
        text: "hello".into(),
        token_id: 42,
    };
    match tok {
        TokenEvent::Token { text, token_id } => {
            assert_eq!(text, "hello");
            assert_eq!(token_id, 42);
        }
        _ => panic!("wrong variant"),
    }

    let stats = PerfStats {
        tokens_generated: 1,
        prompt_tokens: 1,
        prompt_eval_time_ms: 0.0,
        generation_time_ms: 0.0,
        tokens_per_second: 0.0,
        prompt_tokens_per_second: 0.0,
        memory_used_mb: 0,
    };
    let done = TokenEvent::Done {
        stats: stats.clone(),
    };
    match done {
        TokenEvent::Done { stats: s } => assert_eq!(s.tokens_generated, 1),
        _ => panic!("wrong variant"),
    }

    let err = TokenEvent::Error {
        message: "test error".into(),
    };
    match err {
        TokenEvent::Error { message } => assert_eq!(message, "test error"),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn test_backend_init_idempotent() {
    // Can call init multiple times safely
    nexus_llama_bridge::init();
    nexus_llama_bridge::init();
    // And cleanup
    nexus_llama_bridge::cleanup();
}

#[test]
fn test_model_load_fails_gracefully_with_stub() {
    nexus_llama_bridge::init();
    let result = LlamaModel::load(&ModelLoadConfig {
        model_path: "/nonexistent/model.gguf".into(),
        ..Default::default()
    });
    assert!(result.is_err());
    match result {
        Err(LlamaError::ModelLoadFailed { path, .. }) => {
            assert_eq!(path, "/nonexistent/model.gguf");
        }
        _ => panic!("expected ModelLoadFailed"),
    }
    nexus_llama_bridge::cleanup();
}

#[test]
fn test_hardware_info_serialization() {
    let hw = HardwareInfo {
        total_ram_mb: 65536,
        cpu_cores: 16,
        has_avx2: true,
        has_avx512: false,
        has_metal: false,
        has_cuda: false,
        ssd_detected: true,
    };
    let json = serde_json::to_string(&hw).expect("serialize");
    let parsed: HardwareInfo = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed.total_ram_mb, 65536);
    assert_eq!(parsed.cpu_cores, 16);
}

#[test]
fn test_chat_template_formats() {
    use nexus_llama_bridge::chat_template::ChatTemplateFormat;

    // DeepSeek
    let fmt = ChatTemplateFormat::from_architecture("deepseek2");
    assert_eq!(fmt, ChatTemplateFormat::DeepSeek);
    let result = fmt.apply("What is 2+2?");
    assert!(result.contains("<|User|>"));
    assert!(result.contains("<|Assistant|>"));

    // Qwen / ChatML
    let fmt = ChatTemplateFormat::from_architecture("qwen");
    assert_eq!(fmt, ChatTemplateFormat::ChatML);
    let result = fmt.apply("Hello");
    assert!(result.contains("<|im_start|>user"));
    assert!(result.contains("<|im_start|>assistant"));

    // Gemma
    let fmt = ChatTemplateFormat::from_architecture("gemma2");
    assert_eq!(fmt, ChatTemplateFormat::Gemma);
    let result = fmt.apply("Hi");
    assert!(result.contains("<start_of_turn>user"));
    assert!(result.contains("<start_of_turn>model"));

    // Llama / Mistral
    let fmt = ChatTemplateFormat::from_architecture("llama");
    assert_eq!(fmt, ChatTemplateFormat::Llama);
    let result = fmt.apply("Test");
    assert!(result.contains("[INST]"));

    // Unknown defaults to Llama
    let fmt = ChatTemplateFormat::from_architecture("totally_unknown");
    assert_eq!(fmt, ChatTemplateFormat::Llama);
}

#[test]
fn test_real_model_inference() {
    let model_path = match std::env::var("TEST_MODEL_PATH") {
        Ok(p) => p,
        Err(_) => {
            println!("TEST_MODEL_PATH not set, skipping real inference test");
            return;
        }
    };

    // Initialize backend
    nexus_llama_bridge::init();

    // Load model with CPU-only settings — all 16 cores
    let config = ModelLoadConfig {
        model_path: model_path.clone(),
        n_gpu_layers: 0,
        use_mmap: true,
        use_mlock: false,
        cpu_moe: true,
        n_threads: Some(16),
        numa_strategy: NumaStrategy::Disabled,
    };

    let model = LlamaModel::load(&config).expect("Failed to load model");

    println!("Model loaded: {:?}", model.metadata());

    // Create context with maximum performance settings
    let gen_config = GenerationConfig {
        max_tokens: 50,
        temperature: 0.7,
        n_ctx: 2048,
        n_batch: 512,
        n_ubatch: 512,
        flash_attn: true,
        n_threads: Some(16),
        type_k: Some(KvCacheType::Q8_0),
        type_v: Some(KvCacheType::Q8_0),
        ..Default::default()
    };

    let mut output = String::new();
    let mut ctx = LlamaContext::new(&model, &gen_config).expect("Failed to create context");

    let stats = ctx
        .generate_sync("Hello, I am Nexus OS. I can", &gen_config, |event| {
            match event {
                TokenEvent::Token { text, .. } => {
                    output.push_str(&text);
                    print!("{}", text);
                }
                TokenEvent::Done { .. } => {}
                TokenEvent::Error { message } => {
                    eprintln!("Error: {}", message);
                }
            }
            ControlFlow::Continue
        })
        .expect("Generation failed");

    println!(
        "\n\nGenerated {} tokens at {:.2} tok/s",
        stats.tokens_generated, stats.tokens_per_second
    );
    println!("Output: {}", output);

    assert!(!output.is_empty(), "Should generate text");
    assert!(stats.tokens_per_second > 0.0, "Should have positive tok/s");

    nexus_llama_bridge::cleanup();
}
