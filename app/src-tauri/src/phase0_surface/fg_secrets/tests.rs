//! Source guards for Final Gate items A, E and H. This file ends in
//! `tests.rs`, so the Phase Zero production-source scans skip it.

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
    let config = normalized(include_str!("../../../../../kernel/src/config.rs"));
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
    let chat = normalized(include_str!("../../commands/chat_llm.rs"));
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
        ("lib", include_str!("../../lib.rs")),
        ("apps", include_str!("../../commands/apps.rs")),
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
    let chat = normalized(include_str!("../../commands/chat_llm.rs"));
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

/// Final Gate items A and E: the vault key file is opened once without
/// following a final symlink, and every check and the read use that opened
/// file; blank environment key material and a mismatched `key_env` are
/// refused; startup verifies the key against the stored secrets before a
/// facade exists.
#[test]
fn p0_fg_e_vault_key_sources_are_validated_on_what_is_read() {
    let crypto = normalized(include_str!("../../../../../kernel/src/crypto.rs"));
    let production = before_tests(&crypto);
    let from_file = compact(&body(production, "pub fn from_file("));
    assert!(from_file.contains("key_file::open(path)"), "{from_file}");
    assert!(!from_file.contains("fs::read"), "{from_file}");
    let open = compact(&body(production, "pub(super) fn open("));
    for flag in ["O_NOFOLLOW", "O_NONBLOCK", "O_NOCTTY"] {
        assert!(open.contains(flag), "{flag}: {open}");
    }
    let read = compact(&body(production, "pub(super) fn read("));
    assert!(
        in_order(
            &read,
            &[
                "file.metadata()",
                "check_opened_key_file(",
                "take(MAX_KEY_FILE_BYTES+1)",
                "file.metadata()",
                "unchanged(&before,&after)",
            ],
        ),
        "{read}"
    );
    let env_value = compact(&body(production, "fn from_env_value("));
    assert!(
        env_value.starts_with("ifraw.trim().is_empty(){returnErr("),
        "{env_value}"
    );
    let from_config = compact(&body(production, "pub fn from_config("));
    assert!(
        in_order(
            &from_config,
            &[
                "ifconfig.key_env!=DEFAULT_KEY_ENV{returnErr(",
                "Self::from_env()"
            ],
        ),
        "{from_config}"
    );
    let startup = normalized(include_str!("../../../../../kernel/src/startup/mod.rs"));
    let run = compact(&body(before_tests(&startup), "pub fn run_migrations("));
    assert!(
        in_order(
            &run,
            &[
                "EncryptionKey::from_config(",
                "verify_vault_key(&sqlite)?;",
                "SecretsFacade::new(",
                "migrate_config_to_vault(",
                "install(",
            ],
        ),
        "{run}"
    );
}

/// The Final Gate items A and H closure reason is bounded and echoes no
/// input (the registry guard lists every other variant).
#[test]
fn p0_fg_secret_storage_closure_reason_is_bounded() {
    use crate::phase0_surface::{closed, Closure};
    let reason = Closure::SecretStorage.reason();
    assert!(reason.contains("Phase Zero"), "{reason}");
    assert!(reason.len() <= 160, "{reason}");
    assert!(!reason.contains('/') && !reason.contains('\\'), "{reason}");
    assert_eq!(
        closed("surface", Closure::SecretStorage),
        format!("surface: {reason}")
    );
}

/// Final Gate items A and H: the deploy and Supabase credential commands,
/// whose only effect was storing a credential, are closed; the legacy
/// store refuses new credentials and keeps its explicit legacy read path.
#[test]
fn p0_fg_a_deploy_credentials_are_never_newly_stored() {
    let lib = normalized(include_str!("../../lib.rs"));
    for command in [
        "builder_deploy_store_credentials",
        "builder_backend_connect",
    ] {
        let signature = format!("pub(crate) fn {command}() -> Result<(), String>");
        assert_eq!(
            lib.matches(&format!("fn {command}(")).count(),
            1,
            "{command}"
        );
        assert_eq!(
            compact(&body(&lib, &signature)),
            format!(
                "Err(crate::phase0_surface::closed(\"{command}\",crate::phase0_surface::Closure::SecretStorage,))"
            ),
            "{command}"
        );
    }
    let store = normalized(include_str!(
        "../../../../../agents/web-builder/src/deploy/credentials.rs"
    ));
    let production = before_tests(&store);
    assert_eq!(
        compact(&body(production, "fn store_to_path(")),
        "Err(DeployError::Credential(STORAGE_REFUSED.into()))"
    );
    // The legacy key is used only by the legacy read path.
    assert_eq!(production.matches("machine_key()").count(), 2);
    assert!(compact(&body(production, "fn load_from_path("))
        .contains("load_from_path_with_key(path,provider,&machine_key())"));
    assert!(compact(&body(production, "fn delete_from_path("))
        .starts_with("letmutstore=load_store_for_rewrite(path)?;"));
}

/// Final Gate item H: the email and integration sign-in commands are
/// closed with `SecretStorage` before any side effect. No production code
/// writes an OAuth token file, binds a sign-in port or opens a browser;
/// the loopback helpers are compiled for their tests only, and the legacy
/// email token readers remain.
#[test]
fn p0_fg_h_sign_in_flows_persist_no_token() {
    let lib = normalized(include_str!("../../lib.rs"));
    for command in ["email_start_oauth", "integration_start_oauth"] {
        let signature = format!("pub(crate) fn {command}() -> Result<String, String>");
        assert_eq!(
            lib.matches(&format!("fn {command}(")).count(),
            1,
            "{command}"
        );
        assert_eq!(
            compact(&body(&lib, &signature)),
            format!(
                "Err(crate::phase0_surface::closed(\"{command}\",crate::phase0_surface::Closure::SecretStorage,))"
            ),
            "{command}"
        );
    }
    let apps = normalized(include_str!("../../commands/apps.rs"));
    for gone in [
        "fn email_start_oauth(",
        "fn integration_start_oauth(",
        "fn read_oauth_setting(",
        "oauth_settings.json",
        "TcpListener::bind(",
        "open::that(",
        "_oauth.json",
        "\"refresh_token\"",
    ] {
        assert!(!apps.contains(gone), "{gone}");
    }
    for helper in [
        "#[cfg(test)]\nfn oauth_client_id(",
        "#[cfg(test)]\nfn oauth_callback(",
        "#[cfg(test)]\nfn read_oauth_request(",
        "#[cfg(test)]\nfn await_oauth_code(",
        "#[cfg(test)]\n#[derive(Debug, PartialEq, Eq)]\nenum OAuthCallback",
    ] {
        assert!(apps.contains(helper), "{helper}");
    }
    for reader in [
        "pub(crate) fn email_oauth_status(",
        "pub(crate) fn get_email_access_token(",
        "pub(crate) fn email_disconnect(",
    ] {
        assert!(apps.contains(reader), "{reader}");
    }
}

/// Final Gate item H: messaging tokens live in the configuration. Connect
/// accepts only the stored token and writes no token or socket-URL file;
/// the reader prefers the configuration and still reads a legacy file.
#[test]
fn p0_fg_h_messaging_tokens_are_never_copied_to_plaintext_files() {
    let apps = normalized(include_str!("../../commands/apps.rs"));
    // The command reads the configuration's token and the platforms' real
    // endpoints only through its one call to the injected seam.
    assert_eq!(
        compact(&body(&apps, "pub(crate) fn messaging_connect_platform(")),
        "messaging_connect_with(state,platform,token_value,stored_messaging_token,&MESSAGING_ENDPOINTS,)"
    );
    let connect = compact(&body(&apps, "fn messaging_connect_with("));
    assert!(
        in_order(
            &connect,
            &[
                "messaging_platform(state,\"messaging_connect\",&platform)?;",
                "iftoken_value!=STORED_SECRET{returnErr(deny(state,\"messaging_connect\",\"token_must_be_saved_first\"",
                "stored_token(known)?",
                "check_messaging_connectivity(known,&token_value,endpoints)",
            ],
        ),
        "{connect}"
    );
    for gone in [
        "fs::write(",
        "create_dir_all(",
        "messaging_tokens",
        "slack_ws_url",
    ] {
        assert!(!connect.contains(gone), "{gone}");
    }
    assert!(!apps.contains("slack_ws_url"));
    assert!(!apps.contains("apps.connections.open"));
    let read = compact(&body(&apps, "pub(crate) fn read_messaging_token("));
    assert!(
        in_order(
            &read,
            &[
                "stored_messaging_token(platform)",
                ".join(\"messaging_tokens\")"
            ]
        ),
        "{read}"
    );
    assert!(!read.contains("fs::write("), "{read}");
    // A connectivity error never carries the request URL, which holds the
    // Telegram token. The request is bounded in time (one client, built with
    // the endpoints' timeout) and every body is read through the size cap.
    let check = body(&apps, "async fn check_messaging_connectivity(");
    assert!(!check.contains("{e}"), "{check}");
    assert_eq!(check.matches("e.without_url()").count(), 4, "{check}");
    let compact_check = compact(&check);
    assert!(
        compact_check
            .starts_with("letclient=reqwest::Client::builder().timeout(endpoints.timeout).build()"),
        "{check}"
    );
    assert_eq!(check.matches("reqwest::Client").count(), 1, "{check}");
    assert_eq!(
        compact_check.matches("capped_body(resp,max,").count(),
        3,
        "{check}"
    );
    for unbounded in [".text()", ".bytes()", ".json("] {
        assert!(!check.contains(unbounded), "{unbounded}");
    }
    let capped = body(&apps, "async fn capped_body(");
    assert!(!capped.contains("{e}"), "{capped}");
    assert_eq!(capped.matches("e.without_url()").count(), 1, "{capped}");
}

/// Final Gate item H: API Client collections are checked for authentication
/// secrets before the file is resolved or written.
#[test]
fn p0_fg_h_api_client_collections_are_checked_before_writing() {
    let apps = normalized(include_str!("../../commands/apps.rs"));
    assert_eq!(
        compact(&body(&apps, "pub(crate) fn api_client_save_collections(")),
        "save_api_collections_to(data_json,api_collections_path)"
    );
    let save = compact(&body(&apps, "fn save_api_collections_to("));
    assert!(
        in_order(
            &save,
            &[
                "refuse_api_client_secrets(&data_json)?;",
                "letpath=path()?;",
                "std::fs::write(",
            ],
        ),
        "{save}"
    );
    assert!(
        save.starts_with("refuse_api_client_secrets(&data_json)?;"),
        "{save}"
    );
    let fields = compact(&apps);
    assert!(
        fields.contains(
            "constAPI_CLIENT_SECRET_FIELDS:&[&str]=&[\"authToken\",\"authPass\",\"authKeyValue\"];"
        ),
        "secret field list"
    );
}
