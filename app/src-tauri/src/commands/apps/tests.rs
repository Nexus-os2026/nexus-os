//! P0-002C5B: caller identifiers never become paths, and providers and
//! platforms come from allowlists checked before any token file is named.

use super::*;

const HOSTILE_IDS: &[&str] = &[
    "../../escape",
    "../notes",
    "..",
    ".",
    "",
    "/etc/passwd",
    "C:\\Windows\\win.ini",
    "\\\\server\\share\\x",
    "a/b",
    "a\\b",
    "id:stream",
    "CON",
    "nul.json",
    "trailing.",
    ".hidden",
];

#[test]
fn store_identifiers_are_refused_unless_they_match_the_grammar() {
    let state = AppState::new_in_memory();
    let dir = Path::new("/nexus/notes");
    for id in HOSTILE_IDS {
        let error = identified_file(&state, "notes_get", dir, id).unwrap_err();
        assert_eq!(error, "notes_get: invalid identifier", "{id:?}");
    }
    assert!(identified_file(
        &state,
        "notes_get",
        dir,
        &"a".repeat(MAX_STORE_ID_BYTES + 1)
    )
    .is_err());
    for id in ["n-1712345678901", "default", "em-1"] {
        assert_eq!(
            identified_file(&state, "project_get", dir, id).unwrap(),
            dir.join(format!("{id}.json"))
        );
    }
}

#[test]
fn refusals_record_only_the_operation_and_a_reason_class() {
    let state = AppState::new_in_memory();
    let marker = "../../marker-that-must-not-be-logged";
    assert!(identified_file(&state, "notes_save", Path::new("/n"), marker).is_err());
    assert!(email_provider(&state, "email_send", marker).is_err());
    assert!(messaging_platform(&state, "messaging_send", marker).is_err());
    let audit = state.audit.lock().unwrap_or_else(|p| p.into_inner());
    let logged = serde_json::to_string(
        &audit
            .events()
            .iter()
            .map(|e| &e.payload)
            .collect::<Vec<_>>(),
    )
    .unwrap();
    assert!(!logged.contains("marker-that-must-not-be-logged"));
    assert!(logged.contains("invalid_identifier"));
    assert!(logged.contains("unsupported_provider"));
    assert!(logged.contains("unsupported_platform"));
}

#[test]
fn email_message_ids_map_to_a_storage_stem_never_a_path() {
    for id in HOSTILE_IDS {
        let stem = nexus_kernel::governed_path::storage_stem(id);
        assert!(stem.starts_with("h-"), "{id:?} -> {stem}");
        assert_eq!(
            nexus_kernel::governed_path::validate_component(&format!("{stem}.json")),
            Ok(())
        );
    }
    // Existing local and Gmail ids keep their stored names.
    for id in ["em-1712345678901", "18c2f5a1b2c3d4e5"] {
        assert_eq!(nexus_kernel::governed_path::storage_stem(id), id);
    }
}

#[test]
fn providers_and_platforms_come_from_allowlists() {
    let state = AppState::new_in_memory();
    assert_eq!(email_provider(&state, "t", "gmail"), Ok("gmail"));
    assert_eq!(email_provider(&state, "t", "outlook"), Ok("outlook"));
    for provider in ["", "../gmail", "gmail/../../x", "yahoo", "GMAIL"] {
        assert!(
            email_provider(&state, "t", provider).is_err(),
            "{provider:?}"
        );
    }
    for platform in ["telegram", "discord", "slack"] {
        assert_eq!(messaging_platform(&state, "t", platform), Ok(platform));
    }
    for platform in ["", "whatsapp", "../slack", "slack.json", "matrix"] {
        assert!(
            messaging_platform(&state, "t", platform).is_err(),
            "{platform:?}"
        );
    }
}

#[test]
fn url_path_values_are_checked_before_use() {
    let state = AppState::new_in_memory();
    assert!(discord_snowflake(&state, "t", "123456789012345678").is_ok());
    for channel in [
        "",
        "../../users/@me",
        "1/messages?x",
        "12a",
        &"1".repeat(21),
    ] {
        assert!(
            discord_snowflake(&state, "t", channel).is_err(),
            "{channel:?}"
        );
    }
    assert!(telegram_token_ok("123456:ABC-def_ghi"));
    for token in [
        "",
        "123456",
        "abc:def",
        "123:abc/def",
        "123:abc?x=1",
        "123:abc#x",
        "123:a b",
        "123:../../x",
    ] {
        assert!(!telegram_token_ok(token), "{token:?}");
    }
}

#[test]
fn p0_002c5b_api_client_requests_never_become_curl_syntax() {
    let state = AppState::new_in_memory();
    let request = |method: &str, url: &str, headers: &str| {
        api_client_request(
            &state,
            method.into(),
            url.into(),
            headers.into(),
            "@/etc/passwd".into(),
        )
    };
    for url in [
        "file:///etc/passwd",
        "-K/etc/passwd",
        "--config=/etc/passwd",
        "@/etc/passwd",
        "gopher://example.com/",
        "https://trusted.example@marker-host.example/",
    ] {
        assert_eq!(
            request("POST", url, "[]").unwrap_err(),
            "api_client_request: invalid url",
            "{url:?}"
        );
    }
    for method in ["-K", "TRACE", "GET /x HTTP/1.1", ""] {
        assert_eq!(
            request(method, "https://example.invalid/", "[]").unwrap_err(),
            "api_client_request: unsupported method",
            "{method:?}"
        );
    }
    for headers in [
        r#"[["@/etc/passwd","x"]]"#,
        r#"[["X-A","v\r\nX-Injected: 1"]]"#,
    ] {
        assert_eq!(
            request("GET", "https://example.invalid/", headers).unwrap_err(),
            "api_client_request: invalid header",
            "{headers:?}"
        );
    }
    let audit = state.audit.lock().unwrap_or_else(|p| p.into_inner());
    let logged = serde_json::to_string(
        &audit
            .events()
            .iter()
            .map(|e| &e.payload)
            .collect::<Vec<_>>(),
    )
    .unwrap();
    assert!(!logged.contains("marker-host"));
    assert!(!logged.contains("/etc/passwd"));
    for reason in ["invalid_url", "unsupported_method", "invalid_header"] {
        assert!(logged.contains(reason), "{reason}");
    }
}

#[test]
fn p0_002c5b_ollama_deletes_need_an_http_base_url() {
    for base in ["file:///etc", "-K/etc/passwd", "https://a@example.com"] {
        assert!(
            crate::commands::chat_llm::delete_ollama_model("llama3".into(), Some(base.into()))
                .unwrap_err()
                .contains("URL must be an http or https URL"),
            "{base:?}"
        );
    }
}

#[test]
fn p0_002c5b_store_ids_have_one_spelling_and_email_stems_never_alias() {
    let state = AppState::new_in_memory();
    let dir = Path::new("/nexus/notes");
    // Note and project ids are lowercase storage identifiers: a second
    // spelling is refused rather than folded onto the same file.
    for id in ["N-1712345678901", "Default", "DEFAULT", "n-1712345678901X"] {
        assert_eq!(
            identified_file(&state, "notes_get", dir, id).unwrap_err(),
            "notes_get: invalid identifier",
            "{id:?}"
        );
    }
    for id in ["n-1712345678901", "default"] {
        assert!(
            identified_file(&state, "project_get", dir, id).is_ok(),
            "{id:?}"
        );
    }
    // Email ids that differ only by case keep distinct stems, even folded,
    // and no spelling of a generated stem selects it.
    let lower = nexus_kernel::governed_path::storage_stem("messagea");
    let upper = nexus_kernel::governed_path::storage_stem("MessageA");
    assert_eq!(lower, "messagea");
    assert!(!lower.eq_ignore_ascii_case(&upper));
    for spelling in [
        upper.clone(),
        upper.to_uppercase(),
        format!("H{}", &upper[1..]),
    ] {
        let stem = nexus_kernel::governed_path::storage_stem(&spelling);
        assert!(!stem.eq_ignore_ascii_case(&upper), "{spelling:?}");
    }
}
