fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/ffi.rs");
    println!("cargo:rerun-if-changed=src/llama_stub.c");
    println!("cargo:rerun-if-changed=src/llama_helpers.c");

    // When real llama.cpp is available, compile it.
    // For now, build a stub C file that provides the same API surface
    // so tests and compilation work without the 50MB llama.cpp source.
    //
    // To use real llama.cpp:
    // 1. Clone ggml-org/llama.cpp into llama-bridge/llama-cpp/
    // 2. Set environment variable NEXUS_LLAMA_CPP_PATH to point to it
    // 3. The build script will build via cmake and link it

    let llama_path = std::env::var("NEXUS_LLAMA_CPP_PATH").ok();

    if let Some(path) = llama_path {
        compile_real_llama(&path);
    } else {
        compile_stub();
    }
}

fn compile_stub() {
    // Compile the stub (provides no-op implementations of all llama.cpp functions)
    // and the helpers (heap-allocated param wrappers) together in one library.
    cc::Build::new()
        .file("src/llama_stub.c")
        .file("src/llama_helpers.c")
        .compile("llama_stub");
}

fn compile_real_llama(llama_path: &str) {
    // Build llama.cpp via cmake for maximum compatibility.
    let build_dir = format!("{}/build-nexus", llama_path);
    std::fs::create_dir_all(&build_dir).expect("failed to create build dir");

    let cmake_status = std::process::Command::new("cmake")
        .current_dir(&build_dir)
        .args([
            "..",
            "-DCMAKE_BUILD_TYPE=Release",
            "-DBUILD_SHARED_LIBS=OFF",
            "-DLLAMA_BUILD_TESTS=OFF",
            "-DLLAMA_BUILD_EXAMPLES=OFF",
            "-DLLAMA_BUILD_SERVER=OFF",
            "-DGGML_CUDA=OFF", // CPU only for now
        ])
        .status()
        .expect("cmake not found — install cmake to build real llama.cpp");

    assert!(cmake_status.success(), "cmake configuration failed");

    let nproc = std::thread::available_parallelism()
        .map(|p| p.get().to_string())
        .unwrap_or_else(|_| "4".to_string());

    let build_status = std::process::Command::new("cmake")
        .current_dir(&build_dir)
        .args(["--build", ".", "--config", "Release", "-j", &nproc])
        .status()
        .expect("cmake build failed");

    assert!(build_status.success(), "cmake build of llama.cpp failed");

    // Compile our helper shims against the real llama.h headers
    cc::Build::new()
        .file("src/llama_helpers.c")
        .include(format!("{}/include", llama_path))
        .include(format!("{}/ggml/include", llama_path))
        .define("NEXUS_LLAMA_REAL", None)
        .opt_level(2)
        .compile("nexus_helpers");

    // Link the static libraries produced by cmake.
    // The exact lib names depend on the llama.cpp version; search common paths.
    for search_dir in &[
        format!("{}/src", build_dir),
        format!("{}/ggml/src", build_dir),
        build_dir.clone(),
    ] {
        println!("cargo:rustc-link-search=native={}", search_dir);
    }

    println!("cargo:rustc-link-lib=static=llama");
    println!("cargo:rustc-link-lib=static=ggml");
    println!("cargo:rustc-link-lib=static=ggml-base");
    println!("cargo:rustc-link-lib=static=ggml-cpu");

    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-lib=framework=Accelerate");
        println!("cargo:rustc-link-lib=framework=Metal");
        println!("cargo:rustc-link-lib=framework=MetalKit");
        println!("cargo:rustc-link-lib=framework=Foundation");
    }

    #[cfg(target_os = "linux")]
    {
        println!("cargo:rustc-link-lib=stdc++");
        println!("cargo:rustc-link-lib=m");
        println!("cargo:rustc-link-lib=pthread");
        println!("cargo:rustc-link-lib=gomp");
    }

    #[cfg(target_os = "windows")]
    {
        println!("cargo:rustc-link-lib=user32");
        println!("cargo:rustc-link-lib=kernel32");
    }
}
