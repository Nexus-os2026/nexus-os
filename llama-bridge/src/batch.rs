//! Batch building utilities for prompt and token processing.

use crate::ffi;

/// Allocate a new batch capable of holding `n_tokens` tokens.
pub fn create_batch(n_tokens: i32) -> ffi::LlamaBatch {
    unsafe { ffi::llama_batch_init(n_tokens, 0, 1) }
}

/// Free a previously allocated batch.
///
/// # Safety
/// The batch must have been created by [`create_batch`] and not already freed.
pub fn free_batch(batch: ffi::LlamaBatch) {
    unsafe { ffi::llama_batch_free(batch) };
}

/// Fill a batch with prompt tokens for initial processing.
///
/// Sets position IDs sequentially from 0, assigns sequence ID 0 to all tokens,
/// and marks only the last token for logit output.
///
/// # Safety
/// `batch` must be a valid batch allocated with capacity >= `tokens.len()`.
pub unsafe fn fill_batch_prompt(batch: &mut ffi::LlamaBatch, tokens: &[ffi::LlamaToken]) {
    batch.n_tokens = tokens.len() as i32;
    for (i, &tok) in tokens.iter().enumerate() {
        if !batch.token.is_null() {
            *batch.token.add(i) = tok;
        }
        if !batch.pos.is_null() {
            *batch.pos.add(i) = i as i32;
        }
        if !batch.n_seq_id.is_null() {
            *batch.n_seq_id.add(i) = 1;
        }
        if !batch.seq_id.is_null() {
            let seq_ptr = *batch.seq_id.add(i);
            if !seq_ptr.is_null() {
                *seq_ptr = 0;
            }
        }
        if !batch.logits.is_null() {
            // Only compute logits for the last token
            *batch.logits.add(i) = if i == tokens.len() - 1 { 1 } else { 0 };
        }
    }
}

/// Set up a batch for a single generated token.
///
/// # Safety
/// `batch` must be a valid batch with capacity >= 1.
pub unsafe fn fill_batch_single(batch: &mut ffi::LlamaBatch, token: ffi::LlamaToken, pos: i32) {
    batch.n_tokens = 1;
    if !batch.token.is_null() {
        *batch.token = token;
    }
    if !batch.pos.is_null() {
        *batch.pos = pos;
    }
    if !batch.n_seq_id.is_null() {
        *batch.n_seq_id = 1;
    }
    if !batch.seq_id.is_null() {
        let seq_ptr = *batch.seq_id;
        if !seq_ptr.is_null() {
            *seq_ptr = 0;
        }
    }
    if !batch.logits.is_null() {
        *batch.logits = 1;
    }
}
