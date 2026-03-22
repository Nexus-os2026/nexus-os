// STUB: Replace with real llama.cpp — see build.rs
// Every function returns an error value or null so the crate compiles
// and type-level tests pass without downloading llama.cpp source.

#include <stddef.h>
#include <stdint.h>
#include <stdbool.h>

// Opaque types
typedef struct llama_model llama_model;
typedef struct llama_context llama_context;
typedef struct llama_vocab llama_vocab;
typedef struct llama_sampler llama_sampler;
typedef struct llama_memory_i llama_memory_i;
typedef llama_memory_i *llama_memory_t;
typedef int32_t llama_token;

// --- Model params ---

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

// --- Context params ---

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

// --- Batch ---

typedef struct {
    int32_t n_tokens;
    llama_token *token;
    float *embd;
    int32_t *pos;
    int32_t *n_seq_id;
    int32_t **seq_id;
    int8_t *logits;
} llama_batch;

// --- Sampler chain params ---

typedef struct {
    bool no_perf;
} llama_sampler_chain_params;

// --- Perf data ---

typedef struct {
    double t_start_ms;
    double t_load_ms;
    double t_p_eval_ms;
    double t_eval_ms;
    int32_t n_p_eval;
    int32_t n_eval;
    int32_t n_reused;
} llama_perf_context_data;


// ===== Backend =====

void llama_backend_init(void) {}
void llama_backend_free(void) {}


// ===== Model =====

llama_model_params llama_model_default_params(void) {
    llama_model_params p;
    p.n_gpu_layers = 0;
    p.split_mode = 0;
    p.main_gpu = 0;
    p.tensor_split = 0.0f;
    p.progress_callback = NULL;
    p.progress_callback_user_data = NULL;
    p.kv_overrides = NULL;
    p.vocab_only = false;
    p.use_mmap = true;
    p.use_mlock = false;
    p.check_tensors = false;
    return p;
}

llama_model *llama_model_load_from_file(const char *path, llama_model_params params) {
    (void)path;
    (void)params;
    return NULL;  // stub: always fails
}

void llama_model_free(llama_model *model) {
    (void)model;
}

uint64_t llama_model_n_params(const llama_model *model) {
    (void)model;
    return 0;
}

uint64_t llama_model_size(const llama_model *model) {
    (void)model;
    return 0;
}

int32_t llama_model_n_ctx_train(const llama_model *model) {
    (void)model;
    return 0;
}

int32_t llama_model_meta_val_str(const llama_model *model, const char *key,
                                  char *buf, size_t buf_size) {
    (void)model;
    (void)key;
    (void)buf;
    (void)buf_size;
    return -1;  // stub: key not found
}

const llama_vocab *llama_model_get_vocab(const llama_model *model) {
    (void)model;
    return NULL;
}


// ===== Vocab =====

int32_t llama_vocab_n_tokens(const llama_vocab *vocab) {
    (void)vocab;
    return 0;
}


// ===== Context =====

llama_context_params llama_context_default_params(void) {
    llama_context_params p;
    p.n_ctx = 2048;
    p.n_batch = 512;
    p.n_ubatch = 512;
    p.n_seq_max = 1;
    p.n_threads = 4;
    p.n_threads_batch = 4;
    p.rope_scaling_type = -1;
    p.pooling_type = 0;
    p.attention_type = 0;
    p.flash_attn_type = 0;
    p.rope_freq_base = 0.0f;
    p.rope_freq_scale = 0.0f;
    p.yarn_ext_factor = -1.0f;
    p.yarn_attn_factor = 1.0f;
    p.yarn_beta_fast = 32.0f;
    p.yarn_beta_slow = 1.0f;
    p.yarn_orig_ctx = 0;
    p.defrag_thold = -1.0f;
    p.cb_eval = NULL;
    p.cb_eval_user_data = NULL;
    p.type_k = 1;
    p.type_v = 1;
    p.abort_callback = NULL;
    p.abort_callback_data = NULL;
    p.embeddings = false;
    p.offload_kqv = true;
    p.no_perf = true;
    p.op_offload = true;
    p.swa_full = true;
    p.kv_unified = true;
    p.samplers = NULL;
    p.n_samplers = 0;
    return p;
}

llama_context *llama_init_from_model(llama_model *model, llama_context_params params) {
    (void)model;
    (void)params;
    return NULL;  // stub: always fails
}

void llama_free(llama_context *ctx) {
    (void)ctx;
}

uint32_t llama_n_ctx(const llama_context *ctx) {
    (void)ctx;
    return 0;
}


// ===== Tokenization =====

int32_t llama_tokenize(const llama_vocab *vocab, const char *text, int32_t text_len,
                        llama_token *tokens, int32_t n_tokens_max,
                        bool add_special, bool parse_special) {
    (void)vocab;
    (void)text;
    (void)text_len;
    (void)tokens;
    (void)n_tokens_max;
    (void)add_special;
    (void)parse_special;
    return -1;  // stub: tokenization failed
}

int32_t llama_token_to_piece(const llama_vocab *vocab, llama_token token,
                              char *buf, int32_t length,
                              int32_t lstrip, bool special) {
    (void)vocab;
    (void)token;
    (void)buf;
    (void)length;
    (void)lstrip;
    (void)special;
    return 0;  // stub: empty piece
}

llama_token llama_token_eos(const llama_vocab *vocab) {
    (void)vocab;
    return 2;
}

llama_token llama_token_bos(const llama_vocab *vocab) {
    (void)vocab;
    return 1;
}


// ===== Batch =====

llama_batch llama_batch_init(int32_t n_tokens, int32_t embd, int32_t n_seq_max) {
    (void)n_tokens;
    (void)embd;
    (void)n_seq_max;
    llama_batch b;
    b.n_tokens = 0;
    b.token = NULL;
    b.embd = NULL;
    b.pos = NULL;
    b.n_seq_id = NULL;
    b.seq_id = NULL;
    b.logits = NULL;
    return b;
}

void llama_batch_free(llama_batch batch) {
    (void)batch;
}

int32_t llama_decode(llama_context *ctx, llama_batch batch) {
    (void)ctx;
    (void)batch;
    return -1;  // stub: decode failed
}


// ===== Memory (KV cache) =====

llama_memory_t llama_get_memory(const llama_context *ctx) {
    (void)ctx;
    return NULL;
}

void llama_memory_clear(llama_memory_t mem, bool data) {
    (void)mem;
    (void)data;
}


// ===== Sampling =====

llama_sampler_chain_params llama_sampler_chain_default_params(void) {
    llama_sampler_chain_params p;
    p.no_perf = true;
    return p;
}

llama_sampler *llama_sampler_chain_init(llama_sampler_chain_params params) {
    (void)params;
    return NULL;
}

void llama_sampler_chain_add(llama_sampler *chain, llama_sampler *smpl) {
    (void)chain;
    (void)smpl;
}

void llama_sampler_free(llama_sampler *smpl) {
    (void)smpl;
}

void llama_sampler_reset(llama_sampler *smpl) {
    (void)smpl;
}

llama_sampler *llama_sampler_init_temp(float temp) {
    (void)temp;
    return NULL;
}

llama_sampler *llama_sampler_init_top_p(float p, size_t min_keep) {
    (void)p;
    (void)min_keep;
    return NULL;
}

llama_sampler *llama_sampler_init_top_k(int32_t k) {
    (void)k;
    return NULL;
}

llama_sampler *llama_sampler_init_min_p(float p, size_t min_keep) {
    (void)p;
    (void)min_keep;
    return NULL;
}

llama_sampler *llama_sampler_init_penalties(int32_t last_n, float repeat,
                                             float freq, float present) {
    (void)last_n;
    (void)repeat;
    (void)freq;
    (void)present;
    return NULL;
}

llama_sampler *llama_sampler_init_dist(uint32_t seed) {
    (void)seed;
    return NULL;
}

llama_sampler *llama_sampler_init_greedy(void) {
    return NULL;
}

llama_token llama_sampler_sample(llama_sampler *smpl, llama_context *ctx, int32_t idx) {
    (void)smpl;
    (void)ctx;
    (void)idx;
    return -1;  // stub: invalid token
}


// ===== Chat template =====

typedef struct {
    const char *role;
    const char *content;
} llama_chat_message;

const char *llama_model_chat_template(const llama_model *model, const char *name) {
    (void)model;
    (void)name;
    return NULL;  // stub: no template
}

int32_t llama_chat_apply_template(const char *tmpl,
                                   const llama_chat_message *chat,
                                   size_t n_msg,
                                   bool add_ass,
                                   char *buf,
                                   int32_t length) {
    (void)tmpl;
    (void)chat;
    (void)n_msg;
    (void)add_ass;
    (void)buf;
    (void)length;
    return -1;  // stub: template not available
}


// ===== Performance =====

llama_perf_context_data llama_perf_context(const llama_context *ctx) {
    (void)ctx;
    llama_perf_context_data d;
    d.t_start_ms = 0.0;
    d.t_load_ms = 0.0;
    d.t_p_eval_ms = 0.0;
    d.t_eval_ms = 0.0;
    d.n_p_eval = 0;
    d.n_eval = 0;
    d.n_reused = 0;
    return d;
}

void llama_perf_context_reset(llama_context *ctx) {
    (void)ctx;
}
