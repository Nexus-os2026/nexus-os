//! P0-FINAL-GATE-CLOSURE guards for items A (configuration key), E (vault key
//! source) and H (stored secrets).

// Source guards. The whole module is test-only; the inner `cfg(test)` keeps
// the workspace production-text scans of `tests.rs` from counting it.
#[cfg(test)]
mod guards {
    /// `source` with CRLF line endings normalized.
    fn normalized(source: &str) -> String {
        source.replace("\r\n", "\n")
    }

    /// `text` without whitespace.
    fn compact(text: &str) -> String {
        text.chars().filter(|c| !c.is_whitespace()).collect()
    }

    /// The part of a normalized source before its unit-test module.
    fn before_tests(source: &str) -> &str {
        source
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .expect("source")
    }

    /// The block of the first item whose text starts with `signature`, without
    /// its braces. String and character literals and line comments are
    /// skipped when matching braces.
    fn body(source: &str, signature: &str) -> String {
        let at = source
            .find(signature)
            .unwrap_or_else(|| panic!("{signature} not found"));
        let open = at + source[at..].find('{').expect("block");
        let bytes = source.as_bytes();
        let (mut depth, mut i) = (0usize, open);
        while i < bytes.len() {
            match bytes[i] {
                b'"' => {
                    i += 1;
                    while bytes[i] != b'"' {
                        if bytes[i] == b'\\' {
                            i += 1;
                        }
                        i += 1;
                    }
                }
                b'\'' if bytes.get(i + 2) == Some(&b'\'') => i += 2,
                b'\'' if bytes.get(i + 1) == Some(&b'\\') => {
                    i += 2;
                    while bytes[i] != b'\'' {
                        i += 1;
                    }
                }
                b'/' if bytes.get(i + 1) == Some(&b'/') => {
                    while bytes[i] != b'\n' {
                        i += 1;
                    }
                }
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return source[open + 1..i].to_string();
                    }
                }
                _ => {}
            }
            i += 1;
        }
        panic!("{signature}: unterminated block");
    }

    /// Whether each needle occurs in `text`, each after the previous one.
    fn in_order(text: &str, needles: &[&str]) -> bool {
        let mut from = 0;
        for needle in needles {
            match text[from..].find(needle) {
                Some(at) => from += at + needle.len(),
                None => return false,
            }
        }
        true
    }

    /// Final Gate item A: every configuration write goes through the checked
    /// writer. It reads the file already on disk and checks for a new or
    /// changed credential before anything is encrypted or replaced, and only
    /// an operator key made of non-blank material keys such a credential. A
    /// load writes only the first-run default of a missing file.
    #[test]
    fn p0_fg_a_configuration_writes_check_key_material_before_writing() {
        let config = normalized(include_str!("../../../../kernel/src/config.rs"));
        let production = before_tests(&config);
        // The ambient-only writer key is gone.
        assert!(!production.contains("fn config_user_key("));
        // The encrypting writer has one definition and one caller.
        assert_eq!(production.matches("write_envelope(").count(), 2);
        let checked = compact(&body(production, "pub fn save_config_checked_to_path("));
        assert!(
            in_order(
                &checked,
                &[
                    "read_stored_for_write(",
                    "introduces_credentials(",
                    "ConfigWriteRefusal::OperatorKeyRequired",
                    "write_envelope(",
                ],
            ),
            "{checked}"
        );
        assert!(compact(&body(production, "pub fn save_config_to_path("))
            .starts_with("save_config_checked_to_path("));
        assert!(compact(&body(production, "pub fn save_config_checked("))
            .contains("save_config_checked_to_path(&path,config,"));
        // A file already on disk that does not open refuses the write.
        let stored = compact(&body(production, "fn read_stored_for_write("));
        for needle in [
            "ErrorKind::NotFound=>returnOk(None)",
            "Err(_)=>returnErr(refused())",
            "ifraw.trim().is_empty(){returnErr(refused());}",
        ] {
            assert!(stored.contains(needle), "{needle}: {stored}");
        }
        // Only non-blank material is an operator key.
        assert!(body(production, "fn operator_key(").contains(".trim().is_empty()"));
        // A load writes only when the file is missing.
        let load = compact(&body(production, "pub fn load_config_from_path_with("));
        assert_eq!(load.matches("save_config_checked_to_path(").count(), 1);
        assert!(
            in_order(
                &load,
                &["ErrorKind::NotFound", "save_config_checked_to_path("]
            ),
            "{load}"
        );
    }

    /// Final Gate item A: the desktop's configuration writer takes key
    /// material only from the launch environment, and redaction uses the
    /// kernel's credential list.
    #[test]
    fn p0_fg_a_desktop_config_key_material_comes_from_the_launch_environment() {
        let chat = normalized(include_str!("../commands/chat_llm.rs"));
        let save = compact(&body(&chat, "fn save_keeping_stored_credentials("));
        assert!(
            save.contains("ConfigKeyMaterial::from_launch_environment()"),
            "{save}"
        );
        assert!(!save.contains("from_values"), "{save}");
        assert_eq!(
            compact(&body(&chat, "fn credential_fields(")),
            "nexus_kernel::config::credential_fields_mut(config)"
        );
        for (name, source) in [
            ("lib", include_str!("../lib.rs")),
            ("apps", include_str!("../commands/apps.rs")),
        ] {
            assert!(
                !source.contains("ConfigKeyMaterial::from_values("),
                "{name}"
            );
        }
    }

    /// The interface writer refuses a changed Ollama endpoint before anything
    /// is merged or written.
    #[test]
    fn p0_fg_interface_saves_keep_the_ollama_endpoint_backend_owned() {
        let chat = normalized(include_str!("../commands/chat_llm.rs"));
        let write = compact(&body(&chat, "fn write_keeping_stored_credentials("));
        assert!(
            in_order(
                &write,
                &[
                    "load_config_from_path_with(path,keys)",
                    "ifconfig.llm.ollama_url!=stored.llm.ollama_url{returnErr(InterfaceWriteError::Refused(OLLAMA_ENDPOINT_BACKEND_OWNED));}",
                    "keep_stored_credentials(",
                    "save_config_checked_to_path(",
                ],
            ),
            "{write}"
        );
    }
}
