//! Chat template application for proper prompt formatting.
//!
//! Different model families expect prompts wrapped in specific chat templates.
//! Sending a raw prompt without the template causes the model to treat it as
//! a continuation rather than an instruction, leading to garbage repetition.
//!
//! This module first tries llama.cpp's built-in `llama_chat_apply_template()`
//! which reads the template from GGUF metadata. If that fails (older GGUFs or
//! stub mode), it falls back to architecture-based template detection.

use std::ffi::{CStr, CString};

use tracing::debug;

use crate::ffi;

/// Known chat template formats, keyed by model architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatTemplateFormat {
    /// `<|begin▁of▁sentence|><|User|>{prompt}<|Assistant|>`
    DeepSeek,
    /// `<|im_start|>user\n{prompt}<|im_end|>\n<|im_start|>assistant\n`
    ChatML,
    /// `<start_of_turn>user\n{prompt}<end_of_turn>\n<start_of_turn>model\n`
    Gemma,
    /// `<s>[INST] {prompt} [/INST]`
    Llama,
    /// `<|begin_of_text|><|start_header_id|>user<|end_header_id|>\n\n{prompt}<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\n`
    Llama3,
    /// `<|user|>\n{prompt}<|end|>\n<|assistant|>\n`
    Phi3,
}

impl ChatTemplateFormat {
    /// Detect the template format from the model architecture string.
    pub fn from_architecture(arch: &str) -> Self {
        let arch_lower = arch.to_lowercase();
        if arch_lower.contains("deepseek") {
            Self::DeepSeek
        } else if arch_lower.contains("qwen") || arch_lower.contains("chatml") {
            Self::ChatML
        } else if arch_lower.contains("gemma") {
            Self::Gemma
        } else if arch_lower.contains("llama") || arch_lower.contains("mistral") {
            // Llama 3+ uses a different format than Llama 2
            // We default to Llama 2 style; llama.cpp's built-in template
            // will handle Llama 3 correctly when available.
            Self::Llama
        } else if arch_lower.contains("phi") {
            Self::Phi3
        } else {
            // Default to Llama/Mistral instruct format (most common)
            Self::Llama
        }
    }

    /// Wrap a user prompt in the chat template.
    pub fn apply(&self, prompt: &str) -> String {
        match self {
            Self::DeepSeek => {
                // DeepSeek uses a special Unicode separator (U+2581 = ▁)
                format!("<|begin\u{2581}of\u{2581}sentence|><|User|>{prompt}<|Assistant|>")
            }
            Self::ChatML => {
                format!("<|im_start|>user\n{prompt}<|im_end|>\n<|im_start|>assistant\n")
            }
            Self::Gemma => {
                format!("<start_of_turn>user\n{prompt}<end_of_turn>\n<start_of_turn>model\n")
            }
            Self::Llama => {
                format!("<s>[INST] {prompt} [/INST]")
            }
            Self::Llama3 => {
                format!(
                    "<|begin_of_text|><|start_header_id|>user<|end_header_id|>\n\n{prompt}<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\n"
                )
            }
            Self::Phi3 => {
                format!("<|user|>\n{prompt}<|end|>\n<|assistant|>\n")
            }
        }
    }
}

/// Apply the model's chat template to a user prompt.
///
/// Strategy:
/// 1. Try `llama_chat_apply_template()` with the model's built-in template
/// 2. Fall back to architecture-based template detection
pub fn apply_chat_template(
    model_ptr: *const ffi::LlamaModel,
    architecture: &str,
    prompt: &str,
) -> String {
    // First, try llama.cpp's built-in chat template support
    if let Some(formatted) = try_builtin_template(model_ptr, prompt) {
        debug!(
            len = formatted.len(),
            formatted = %formatted,
            "applied built-in chat template"
        );
        return formatted;
    }

    // Fall back to architecture-based detection
    let format = ChatTemplateFormat::from_architecture(architecture);
    let formatted = format.apply(prompt);
    debug!(
        ?format,
        len = formatted.len(),
        formatted = %formatted,
        "applied fallback chat template"
    );
    formatted
}

/// Try to use llama.cpp's built-in `llama_chat_apply_template()`.
/// Returns `None` if the function isn't available or the model has no template.
fn try_builtin_template(model_ptr: *const ffi::LlamaModel, prompt: &str) -> Option<String> {
    if model_ptr.is_null() {
        return None;
    }

    // Get the model's built-in template string
    let tmpl_ptr = unsafe { ffi::llama_model_chat_template(model_ptr, std::ptr::null()) };
    if tmpl_ptr.is_null() {
        return None;
    }
    let tmpl = unsafe { CStr::from_ptr(tmpl_ptr) };

    // Build a single user message
    let role = CString::new("user").ok()?;
    let content = CString::new(prompt).ok()?;
    let msg = ffi::LlamaChatMessage {
        role: role.as_ptr(),
        content: content.as_ptr(),
    };

    // First call: get required buffer size
    let needed = unsafe {
        ffi::llama_chat_apply_template(
            tmpl.as_ptr(),
            &msg,
            1,
            true, // add_ass: end with assistant turn start
            std::ptr::null_mut(),
            0,
        )
    };

    if needed <= 0 {
        return None;
    }

    // Second call: fill buffer
    let mut buf = vec![0u8; (needed + 1) as usize];
    let written = unsafe {
        ffi::llama_chat_apply_template(
            tmpl.as_ptr(),
            &msg,
            1,
            true,
            buf.as_mut_ptr() as *mut libc::c_char,
            buf.len() as i32,
        )
    };

    if written <= 0 {
        return None;
    }

    let result = String::from_utf8_lossy(&buf[..written as usize]).into_owned();
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deepseek_template() {
        let fmt = ChatTemplateFormat::from_architecture("deepseek2");
        assert_eq!(fmt, ChatTemplateFormat::DeepSeek);
        let result = fmt.apply("What is 2+2?");
        assert!(result.contains("<|User|>What is 2+2?<|Assistant|>"));
    }

    #[test]
    fn test_qwen_template() {
        let fmt = ChatTemplateFormat::from_architecture("qwen");
        assert_eq!(fmt, ChatTemplateFormat::ChatML);
        let result = fmt.apply("Hello");
        assert!(result.contains("<|im_start|>user\nHello<|im_end|>"));
        assert!(result.contains("<|im_start|>assistant\n"));
    }

    #[test]
    fn test_qwen35moe_template() {
        let fmt = ChatTemplateFormat::from_architecture("qwen35moe");
        assert_eq!(fmt, ChatTemplateFormat::ChatML);
    }

    #[test]
    fn test_gemma_template() {
        let fmt = ChatTemplateFormat::from_architecture("gemma2");
        assert_eq!(fmt, ChatTemplateFormat::Gemma);
        let result = fmt.apply("Hi");
        assert!(result.contains("<start_of_turn>user\nHi<end_of_turn>"));
        assert!(result.contains("<start_of_turn>model\n"));
    }

    #[test]
    fn test_llama_template() {
        let fmt = ChatTemplateFormat::from_architecture("llama");
        assert_eq!(fmt, ChatTemplateFormat::Llama);
        let result = fmt.apply("Test");
        assert_eq!(result, "<s>[INST] Test [/INST]");
    }

    #[test]
    fn test_mistral_template() {
        let fmt = ChatTemplateFormat::from_architecture("mistral");
        assert_eq!(fmt, ChatTemplateFormat::Llama);
    }

    #[test]
    fn test_phi3_template() {
        let fmt = ChatTemplateFormat::from_architecture("phi");
        assert_eq!(fmt, ChatTemplateFormat::Phi3);
        let result = fmt.apply("Hello");
        assert!(result.contains("<|user|>\nHello<|end|>"));
        assert!(result.contains("<|assistant|>\n"));
    }

    #[test]
    fn test_unknown_architecture_defaults_to_llama() {
        let fmt = ChatTemplateFormat::from_architecture("unknown_arch_xyz");
        assert_eq!(fmt, ChatTemplateFormat::Llama);
    }

    #[test]
    fn test_builtin_template_null_model() {
        // Should return None for null model pointer
        let result = try_builtin_template(std::ptr::null(), "Hello");
        assert!(result.is_none());
    }
}
