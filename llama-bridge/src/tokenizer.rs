//! Tokenization utilities wrapping llama.cpp's vocab functions.

use std::ffi::CString;

use crate::error::LlamaError;
use crate::ffi;

/// Tokenize a text string into token IDs.
///
/// Returns a `Vec` of token IDs. `add_special` controls whether BOS/EOS tokens
/// are prepended/appended.
pub(crate) fn tokenize(
    vocab: *const ffi::LlamaVocab,
    text: &str,
    add_special: bool,
) -> Result<Vec<ffi::LlamaToken>, LlamaError> {
    if vocab.is_null() {
        return Err(LlamaError::TokenizationFailed("vocab is null".into()));
    }

    let c_text = CString::new(text)
        .map_err(|_| LlamaError::TokenizationFailed("text contains null byte".into()))?;
    let text_len = text.len() as i32;

    // First call to get required buffer size (negative = needed capacity)
    let n_tokens = unsafe {
        ffi::llama_tokenize(
            vocab,
            c_text.as_ptr(),
            text_len,
            std::ptr::null_mut(),
            0,
            add_special,
            false,
        )
    };

    // llama_tokenize returns negative count when buffer too small
    let capacity = if n_tokens < 0 {
        (-n_tokens) as usize
    } else {
        // Stub returns -1 which we handle, but just in case:
        return Err(LlamaError::TokenizationFailed(
            "unexpected return from llama_tokenize".into(),
        ));
    };

    let mut tokens = vec![0i32; capacity];
    let actual = unsafe {
        ffi::llama_tokenize(
            vocab,
            c_text.as_ptr(),
            text_len,
            tokens.as_mut_ptr(),
            tokens.len() as i32,
            add_special,
            false,
        )
    };

    if actual < 0 {
        return Err(LlamaError::TokenizationFailed(format!(
            "tokenization failed with code {actual}"
        )));
    }

    tokens.truncate(actual as usize);
    Ok(tokens)
}

/// Convert a single token ID back to its text representation.
pub(crate) fn token_to_text(vocab: *const ffi::LlamaVocab, token: ffi::LlamaToken) -> String {
    if vocab.is_null() {
        return String::new();
    }

    let mut buf = [0i8; 128];
    let len = unsafe {
        ffi::llama_token_to_piece(
            vocab,
            token,
            buf.as_mut_ptr() as *mut libc::c_char,
            buf.len() as i32,
            0,
            false,
        )
    };

    if len <= 0 {
        return String::new();
    }

    let bytes: Vec<u8> = buf[..len as usize].iter().map(|&b| b as u8).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Get the end-of-sequence token ID.
pub(crate) fn eos_token(vocab: *const ffi::LlamaVocab) -> ffi::LlamaToken {
    if vocab.is_null() {
        return 2; // common default
    }
    unsafe { ffi::llama_token_eos(vocab) }
}

/// Get the beginning-of-sequence token ID.
#[allow(dead_code)]
pub(crate) fn bos_token(vocab: *const ffi::LlamaVocab) -> ffi::LlamaToken {
    if vocab.is_null() {
        return 1; // common default
    }
    unsafe { ffi::llama_token_bos(vocab) }
}
