// Thin C helper functions that set individual fields on llama.cpp param structs.
// This avoids Rust needing to know the exact struct layout — we only need
// the default-params functions and these field setters.
//
// When compiled with the stub, the param struct typedefs come from llama_stub.c.
// When compiled with real llama.cpp, they come from llama.h.

#include <stddef.h>
#include <stdint.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>

// Forward-declare the param struct types. The actual layout is defined
// by either llama_stub.c or the real llama.h — we include the right one
// via build.rs include paths.
#ifdef NEXUS_LLAMA_REAL
#include "llama.h"

// Wrap the enum-typed field into a bool-style setter.
// flash_attn_type: -1=auto, 0=disabled, 1=enabled
void nexus_ctx_params_set_flash_attn(struct llama_context_params *p, bool v) {
    p->flash_attn_type = v ? 1 : 0;  // LLAMA_FLASH_ATTN_TYPE_ENABLED / DISABLED
}

#else
// Stub mode: use the stub's typedef

typedef struct {
    int32_t n_gpu_layers;
    int32_t split_mode;
    int32_t main_gpu;
    float tensor_split;
    void *progress_callback;
    void *progress_callback_user_data;
    void *kv_overrides;
    bool vocab_only;
    bool use_mmap;
    bool use_mlock;
    bool check_tensors;
} llama_model_params;

typedef struct {
    uint32_t n_ctx;
    uint32_t n_batch;
    uint32_t n_ubatch;
    uint32_t n_seq_max;
    int32_t n_threads;
    int32_t n_threads_batch;
    int32_t rope_scaling_type;
    int32_t pooling_type;
    int32_t attention_type;
    int32_t flash_attn_type;
    float rope_freq_base;
    float rope_freq_scale;
    float yarn_ext_factor;
    float yarn_attn_factor;
    float yarn_beta_fast;
    float yarn_beta_slow;
    uint32_t yarn_orig_ctx;
    float defrag_thold;
    void *cb_eval;
    void *cb_eval_user_data;
    int32_t type_k;
    int32_t type_v;
    void *abort_callback;
    void *abort_callback_data;
    bool embeddings;
    bool offload_kqv;
    bool no_perf;
    bool op_offload;
    bool swa_full;
    bool kv_unified;
    void *samplers;
    size_t n_samplers;
} llama_context_params;

void nexus_ctx_params_set_flash_attn(llama_context_params *p, bool v) {
    p->flash_attn_type = v ? 1 : 0;
}

// Stub forward declarations (implemented in llama_stub.c)
llama_model_params llama_model_default_params(void);
llama_context_params llama_context_default_params(void);

typedef struct llama_model llama_model;
typedef struct llama_context llama_context;

llama_model *llama_model_load_from_file(const char *path, llama_model_params params);
llama_context *llama_init_from_model(llama_model *model, llama_context_params params);

#endif

// ── Heap-allocated param helpers ─────────────────────────────────────
// These allocate default params on the heap so Rust never passes the
// param structs by value — avoiding ABI/size mismatches.

#ifdef NEXUS_LLAMA_REAL
#define MODEL_PARAMS_T struct llama_model_params
#define CTX_PARAMS_T   struct llama_context_params
#else
#define MODEL_PARAMS_T llama_model_params
#define CTX_PARAMS_T   llama_context_params
#endif

MODEL_PARAMS_T *nexus_model_params_create(void) {
    MODEL_PARAMS_T *p = (MODEL_PARAMS_T *)malloc(sizeof(MODEL_PARAMS_T));
    if (p) {
        MODEL_PARAMS_T defaults = llama_model_default_params();
        memcpy(p, &defaults, sizeof(MODEL_PARAMS_T));
    }
    return p;
}

void nexus_model_params_free(MODEL_PARAMS_T *p) {
    free(p);
}

CTX_PARAMS_T *nexus_ctx_params_create(void) {
    CTX_PARAMS_T *p = (CTX_PARAMS_T *)malloc(sizeof(CTX_PARAMS_T));
    if (p) {
        CTX_PARAMS_T defaults = llama_context_default_params();
        memcpy(p, &defaults, sizeof(CTX_PARAMS_T));
    }
    return p;
}

void nexus_ctx_params_free(CTX_PARAMS_T *p) {
    free(p);
}

// Wrapper: load model via pointer to params (avoids by-value ABI mismatch)
#ifdef NEXUS_LLAMA_REAL
struct llama_model *nexus_model_load_from_file(const char *path, struct llama_model_params *params) {
    return llama_model_load_from_file(path, *params);
}
struct llama_context *nexus_init_from_model(struct llama_model *model, struct llama_context_params *params) {
    return llama_init_from_model(model, *params);
}
#else
llama_model *nexus_model_load_from_file(const char *path, llama_model_params *params) {
    return llama_model_load_from_file(path, *params);
}
llama_context *nexus_init_from_model(llama_model *model, llama_context_params *params) {
    return llama_init_from_model(model, *params);
}
#endif

// ── Model params setters ──────────────────────────────────────────

void nexus_model_params_set_n_gpu_layers(MODEL_PARAMS_T *p, int32_t n) {
    p->n_gpu_layers = n;
}

void nexus_model_params_set_use_mmap(MODEL_PARAMS_T *p, bool v) {
    p->use_mmap = v;
}

void nexus_model_params_set_use_mlock(MODEL_PARAMS_T *p, bool v) {
    p->use_mlock = v;
}

// ── Context params setters ────────────────────────────────────────

void nexus_ctx_params_set_n_ctx(CTX_PARAMS_T *p, uint32_t n) {
    p->n_ctx = n;
}

void nexus_ctx_params_set_n_batch(CTX_PARAMS_T *p, uint32_t n) {
    p->n_batch = n;
}

void nexus_ctx_params_set_n_threads(CTX_PARAMS_T *p, int32_t n) {
    p->n_threads = n;
}

void nexus_ctx_params_set_n_threads_batch(CTX_PARAMS_T *p, int32_t n) {
    p->n_threads_batch = n;
}

void nexus_ctx_params_set_no_perf(CTX_PARAMS_T *p, bool v) {
    p->no_perf = v;
}

void nexus_ctx_params_set_n_ubatch(CTX_PARAMS_T *p, uint32_t n) {
    p->n_ubatch = n;
}

void nexus_ctx_params_set_type_k(CTX_PARAMS_T *p, int32_t t) {
    p->type_k = t;
}

void nexus_ctx_params_set_type_v(CTX_PARAMS_T *p, int32_t t) {
    p->type_v = t;
}

// ── Sizeof queries ────────────────────────────────────────────────

size_t nexus_sizeof_model_params(void) {
    return sizeof(MODEL_PARAMS_T);
}

size_t nexus_sizeof_context_params(void) {
    return sizeof(CTX_PARAMS_T);
}
