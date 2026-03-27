//! Raw C bindings to llama.cpp's public API.
//!
//! Parameter structs (`llama_model_params`, `llama_context_params`) are
//! **opaque** — Rust never sees their layout. We heap-allocate them via
//! `nexus_model_params_create()` / `nexus_ctx_params_create()`, mutate
//! individual fields through thin C helpers, and pass **pointers** to
//! `nexus_model_load_from_file()` / `nexus_init_from_model()` which
//! dereference on the C side. This avoids by-value ABI mismatches when
//! the real llama.cpp struct layout differs from the stub.

use libc::c_char;

/// Token identifier (same as llama_token / llama_pos / llama_seq_id in llama.h — all i32).
pub type LlamaToken = i32;

// Opaque C types — only ever accessed via pointer.
#[repr(C)]
pub struct LlamaModel {
    _opaque: [u8; 0],
}
#[repr(C)]
pub struct LlamaContextRaw {
    _opaque: [u8; 0],
}
#[repr(C)]
pub struct LlamaVocab {
    _opaque: [u8; 0],
}
#[repr(C)]
pub struct LlamaSampler {
    _opaque: [u8; 0],
}
#[repr(C)]
pub struct LlamaMemory {
    _opaque: [u8; 0],
}

/// Opaque model params — heap-allocated by `nexus_model_params_create()`.
#[repr(C)]
pub struct LlamaModelParams {
    _opaque: [u8; 0],
}

/// Opaque context params — heap-allocated by `nexus_ctx_params_create()`.
#[repr(C)]
pub struct LlamaContextParams {
    _opaque: [u8; 0],
}

/// Token batch for prompt/generation processing (matches `llama_batch` in llama.h).
/// All pointer fields use i32 because llama_token, llama_pos, llama_seq_id are all i32.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct LlamaBatch {
    pub n_tokens: i32,
    pub token: *mut LlamaToken,
    pub embd: *mut f32,
    pub pos: *mut i32, // llama_pos = i32
    pub n_seq_id: *mut i32,
    pub seq_id: *mut *mut i32, // llama_seq_id = i32
    pub logits: *mut i8,
}

/// Chat message for template application (mirrors `llama_chat_message`).
#[repr(C)]
pub struct LlamaChatMessage {
    pub role: *const c_char,
    pub content: *const c_char,
}

/// Sampler chain configuration (mirrors `llama_sampler_chain_params`).
#[repr(C)]
#[derive(Debug, Clone)]
pub struct LlamaSamplerChainParams {
    pub no_perf: bool,
}

/// Performance counters from a context (mirrors `llama_perf_context_data`).
#[repr(C)]
#[derive(Debug, Clone)]
pub struct LlamaPerfContextData {
    pub t_start_ms: f64,
    pub t_load_ms: f64,
    pub t_p_eval_ms: f64,
    pub t_eval_ms: f64,
    pub n_p_eval: i32,
    pub n_eval: i32,
    pub n_reused: i32,
}

extern "C" {
    // ── Backend lifecycle ──────────────────────────────────────────
    pub fn llama_backend_init();
    pub fn llama_backend_free();

    // ── Model ──────────────────────────────────────────────────────
    pub fn llama_model_free(model: *mut LlamaModel);
    pub fn llama_model_n_params(model: *const LlamaModel) -> u64;
    pub fn llama_model_size(model: *const LlamaModel) -> u64;
    pub fn llama_model_n_ctx_train(model: *const LlamaModel) -> i32;
    pub fn llama_model_meta_val_str(
        model: *const LlamaModel,
        key: *const c_char,
        buf: *mut c_char,
        buf_size: usize,
    ) -> i32;
    pub fn llama_model_get_vocab(model: *const LlamaModel) -> *const LlamaVocab;

    // ── Vocab ──────────────────────────────────────────────────────
    pub fn llama_vocab_n_tokens(vocab: *const LlamaVocab) -> i32;

    // ── Context ────────────────────────────────────────────────────
    pub fn llama_free(ctx: *mut LlamaContextRaw);
    pub fn llama_n_ctx(ctx: *const LlamaContextRaw) -> u32;

    // ── Memory (KV cache) ───────────────────────────────────────
    pub fn llama_get_memory(ctx: *const LlamaContextRaw) -> *mut LlamaMemory;
    pub fn llama_memory_clear(mem: *mut LlamaMemory, data: bool);

    // ── Tokenization ───────────────────────────────────────────────
    pub fn llama_tokenize(
        vocab: *const LlamaVocab,
        text: *const c_char,
        text_len: i32,
        tokens: *mut LlamaToken,
        n_tokens_max: i32,
        add_special: bool,
        parse_special: bool,
    ) -> i32;
    pub fn llama_token_to_piece(
        vocab: *const LlamaVocab,
        token: LlamaToken,
        buf: *mut c_char,
        length: i32,
        lstrip: i32,
        special: bool,
    ) -> i32;
    pub fn llama_token_eos(vocab: *const LlamaVocab) -> LlamaToken;
    pub fn llama_token_bos(vocab: *const LlamaVocab) -> LlamaToken;
    pub fn llama_vocab_is_eog(vocab: *const LlamaVocab, token: LlamaToken) -> bool;

    // ── Batch ──────────────────────────────────────────────────────
    pub fn llama_batch_init(n_tokens: i32, embd: i32, n_seq_max: i32) -> LlamaBatch;
    pub fn llama_batch_free(batch: LlamaBatch);
    pub fn llama_decode(ctx: *mut LlamaContextRaw, batch: LlamaBatch) -> i32;

    // ── Sampling ───────────────────────────────────────────────────
    pub fn llama_sampler_chain_init(params: LlamaSamplerChainParams) -> *mut LlamaSampler;
    pub fn llama_sampler_chain_add(chain: *mut LlamaSampler, smpl: *mut LlamaSampler);
    pub fn llama_sampler_free(smpl: *mut LlamaSampler);
    pub fn llama_sampler_chain_default_params() -> LlamaSamplerChainParams;
    pub fn llama_sampler_init_temp(temp: f32) -> *mut LlamaSampler;
    pub fn llama_sampler_init_top_p(p: f32, min_keep: usize) -> *mut LlamaSampler;
    pub fn llama_sampler_init_top_k(k: i32) -> *mut LlamaSampler;
    pub fn llama_sampler_init_min_p(p: f32, min_keep: usize) -> *mut LlamaSampler;
    pub fn llama_sampler_init_penalties(
        penalty_last_n: i32,
        penalty_repeat: f32,
        penalty_freq: f32,
        penalty_present: f32,
    ) -> *mut LlamaSampler;
    pub fn llama_sampler_init_dist(seed: u32) -> *mut LlamaSampler;
    pub fn llama_sampler_init_greedy() -> *mut LlamaSampler;
    pub fn llama_sampler_reset(smpl: *mut LlamaSampler);
    pub fn llama_sampler_sample(
        smpl: *mut LlamaSampler,
        ctx: *mut LlamaContextRaw,
        idx: i32,
    ) -> LlamaToken;

    // ── Chat template ──────────────────────────────────────────────
    /// Get the model's built-in chat template. Returns null if unavailable.
    /// If `name` is null, returns the default template.
    pub fn llama_model_chat_template(
        model: *const LlamaModel,
        name: *const c_char,
    ) -> *const c_char;

    /// Apply a chat template to format messages. Returns the number of bytes
    /// written (or needed if buf is too small). `tmpl` can be null to use the
    /// model's built-in template.
    pub fn llama_chat_apply_template(
        tmpl: *const c_char,
        chat: *const LlamaChatMessage,
        n_msg: usize,
        add_ass: bool,
        buf: *mut c_char,
        length: i32,
    ) -> i32;

    // ── Performance ────────────────────────────────────────────────
    pub fn llama_perf_context(ctx: *const LlamaContextRaw) -> LlamaPerfContextData;
    pub fn llama_perf_context_reset(ctx: *mut LlamaContextRaw);

    // ── Helpers (defined in llama_helpers.c) ────────────────────────
    // Heap-allocated param management — avoids by-value ABI mismatches.
    pub fn nexus_model_params_create() -> *mut LlamaModelParams;
    pub fn nexus_model_params_free(params: *mut LlamaModelParams);
    pub fn nexus_ctx_params_create() -> *mut LlamaContextParams;
    pub fn nexus_ctx_params_free(params: *mut LlamaContextParams);

    // Model/context creation via pointer (dereferences on C side).
    pub fn nexus_model_load_from_file(
        path: *const c_char,
        params: *const LlamaModelParams,
    ) -> *mut LlamaModel;
    pub fn nexus_init_from_model(
        model: *mut LlamaModel,
        params: *const LlamaContextParams,
    ) -> *mut LlamaContextRaw;

    // Field setters — mutate individual fields on the opaque param structs.
    pub fn nexus_model_params_set_n_gpu_layers(params: *mut LlamaModelParams, n: i32);
    pub fn nexus_model_params_set_use_mmap(params: *mut LlamaModelParams, v: bool);
    pub fn nexus_model_params_set_use_mlock(params: *mut LlamaModelParams, v: bool);

    pub fn nexus_ctx_params_set_n_ctx(params: *mut LlamaContextParams, n: u32);
    pub fn nexus_ctx_params_set_n_batch(params: *mut LlamaContextParams, n: u32);
    pub fn nexus_ctx_params_set_n_threads(params: *mut LlamaContextParams, n: i32);
    pub fn nexus_ctx_params_set_n_threads_batch(params: *mut LlamaContextParams, n: i32);
    pub fn nexus_ctx_params_set_flash_attn(params: *mut LlamaContextParams, v: bool);
    pub fn nexus_ctx_params_set_no_perf(params: *mut LlamaContextParams, v: bool);
    pub fn nexus_ctx_params_set_n_ubatch(params: *mut LlamaContextParams, n: u32);
    pub fn nexus_ctx_params_set_type_k(params: *mut LlamaContextParams, t: i32);
    pub fn nexus_ctx_params_set_type_v(params: *mut LlamaContextParams, t: i32);

    /// Size of real `llama_model_params` and `llama_context_params` structs.
    pub fn nexus_sizeof_model_params() -> usize;
    pub fn nexus_sizeof_context_params() -> usize;
}

// ── System memory management ──────────────────────────────────────
extern "C" {
    /// Force glibc to return freed memory to the OS.
    /// `pad` bytes are retained; pass 0 to release everything possible.
    pub fn malloc_trim(pad: usize) -> i32;
}
