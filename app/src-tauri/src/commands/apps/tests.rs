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

// ── P0-002C5C: OAuth loopback callbacks ──────────────────────────────

const FLOW_STATE: &str = "4f9c2d7e-1b3a-4c5d-8e6f-7a8b9c0d1e2f";

fn callback(target: &str) -> String {
    format!("GET {target} HTTP/1.1\r\nHost: localhost:19823\r\n\r\n")
}

#[test]
fn p0_002c5c_oauth_callbacks_count_only_with_this_flows_state() {
    let ok = callback(&format!("/oauth/callback?code=abc&state={FLOW_STATE}"));
    assert_eq!(
        oauth_callback(&ok, FLOW_STATE),
        OAuthCallback::Code("abc".into())
    );
    // Parameter order does not matter, and the code is percent-decoded.
    let encoded = callback(&format!(
        "/oauth/callback?state={FLOW_STATE}&scope=x&code=4%2F0Ab-c"
    ));
    assert_eq!(
        oauth_callback(&encoded, FLOW_STATE),
        OAuthCallback::Code("4/0Ab-c".into())
    );
    let refused = callback(&format!(
        "/oauth/callback?error=access_denied&state={FLOW_STATE}"
    ));
    assert_eq!(oauth_callback(&refused, FLOW_STATE), OAuthCallback::Refused);

    let other = "5a0d3e8f-2c4b-4d6e-9f70-8b9c0d1e2f3a";
    for request in [
        // No state, a wrong state, or an empty state.
        callback("/oauth/callback?code=forged"),
        callback(&format!("/oauth/callback?code=forged&state={other}")),
        callback("/oauth/callback?code=forged&state="),
        // A repeated state or code is ambiguous.
        callback(&format!(
            "/oauth/callback?code=forged&state={other}&state={FLOW_STATE}"
        )),
        callback(&format!("/oauth/callback?code=a&code=b&state={FLOW_STATE}")),
        // No code, an empty code, or a code together with an error.
        callback(&format!("/oauth/callback?state={FLOW_STATE}")),
        callback(&format!("/oauth/callback?code=&state={FLOW_STATE}")),
        callback(&format!(
            "/oauth/callback?code=a&error=x&state={FLOW_STATE}"
        )),
        // A refusal for another flow.
        callback(&format!(
            "/oauth/callback?error=access_denied&state={other}"
        )),
        // The pre-C5C parser matched any `GET /?code=…`.
        callback(&format!("/?code=forged&state={FLOW_STATE}")),
        // Other paths and spellings of the path.
        callback(&format!("/oauth/callbackx?code=a&state={FLOW_STATE}")),
        callback(&format!("/oauth/./callback?code=a&state={FLOW_STATE}")),
        callback(&format!("/OAUTH/CALLBACK?code=a&state={FLOW_STATE}")),
        callback(&format!("/x/oauth/callback?code=a&state={FLOW_STATE}")),
        // Other methods and malformed request lines.
        format!("POST /oauth/callback?code=a&state={FLOW_STATE} HTTP/1.1\r\n\r\n"),
        format!("GET /oauth/callback?code=a&state={FLOW_STATE}\r\n\r\n"),
        format!("GET /oauth/callback?code=a&state={FLOW_STATE} HTTP/1.1 x\r\n\r\n"),
        format!("GET  /oauth/callback?code=a&state={FLOW_STATE} HTTP/1.1\r\n\r\n"),
        // The state may not arrive in a later header line.
        format!("GET /oauth/callback?code=a HTTP/1.1\r\nX: &state={FLOW_STATE}\r\n\r\n"),
        String::new(),
    ] {
        assert_eq!(
            oauth_callback(&request, FLOW_STATE),
            OAuthCallback::Unrelated,
            "{request:?}"
        );
    }
}

#[test]
fn p0_002c5c_oauth_client_ids_cannot_reshape_the_authorization_url() {
    for id in [
        "1234567890-abc123def.apps.googleusercontent.com",
        "3f1c2b4a-5d6e-4f70-8a9b-0c1d2e3f4a5b",
        "Iv1.8a61f9b3a7aba766",
        "1234567890.1234567890",
        "a_b",
    ] {
        assert_eq!(oauth_client_id(id.to_string()).as_deref(), Ok(id));
    }
    for id in [
        String::new(),
        "id&redirect_uri=https://attacker.example/".to_string(),
        "id\" & calc & \"".to_string(),
        "id%26x".to_string(),
        "id with space".to_string(),
        "id#frag".to_string(),
        "id\n".to_string(),
        "a".repeat(257),
    ] {
        assert!(oauth_client_id(id.clone()).is_err(), "{id:?}");
    }
}

/// Send one request to the loopback listener and return the status line.
fn send_callback(port: u16, request: &str) -> String {
    use std::io::{Read as _, Write as _};
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    // The listener answers each connection and closes it.
    let _ = stream.read_to_string(&mut response);
    response.lines().next().unwrap_or_default().to_string()
}

#[test]
fn p0_002c5c_a_forged_loopback_callback_neither_ends_the_flow_nor_supplies_a_code() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let client = std::thread::spawn(move || {
        let forged = [
            callback("/oauth/callback?code=forged"),
            callback("/oauth/callback?code=forged&state=5a0d3e8f-2c4b-4d6e-9f70-8b9c0d1e2f3a"),
            callback(&format!("/?code=forged&state={FLOW_STATE}")),
            "garbage\r\n\r\n".to_string(),
        ];
        let statuses: Vec<String> = forged.iter().map(|r| send_callback(port, r)).collect();
        let accepted = send_callback(
            port,
            &callback(&format!("/oauth/callback?code=real&state={FLOW_STATE}")),
        );
        (statuses, accepted)
    });
    let code = await_oauth_code(
        &listener,
        FLOW_STATE,
        std::time::Instant::now() + std::time::Duration::from_secs(60),
    );
    let (statuses, accepted) = client.join().unwrap();
    assert_eq!(code.as_deref(), Ok("real"));
    for status in statuses {
        assert_eq!(status, "HTTP/1.1 400 Bad Request");
    }
    assert_eq!(accepted, "HTTP/1.1 200 OK");
}

#[test]
fn p0_002c5c_a_refusal_for_this_flow_ends_it_without_a_code() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let client = std::thread::spawn(move || {
        send_callback(
            port,
            &callback(&format!(
                "/oauth/callback?error=access_denied&state={FLOW_STATE}"
            )),
        )
    });
    let outcome = await_oauth_code(
        &listener,
        FLOW_STATE,
        std::time::Instant::now() + std::time::Duration::from_secs(60),
    );
    assert_eq!(client.join().unwrap(), "HTTP/1.1 200 OK");
    assert_eq!(
        outcome,
        Err("The provider did not grant access.".to_string())
    );
}

#[test]
fn p0_002c5c_the_oauth_wait_ends_at_its_deadline() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    // A connection that never sends a request cannot hold the flow open.
    let silent = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    let started = std::time::Instant::now();
    let outcome = await_oauth_code(
        &listener,
        FLOW_STATE,
        started + std::time::Duration::from_millis(500),
    );
    let elapsed = started.elapsed();
    drop(silent);
    assert_eq!(
        outcome,
        Err("OAuth flow timed out or no auth code received".to_string())
    );
    assert!(
        elapsed >= std::time::Duration::from_millis(500),
        "{elapsed:?}"
    );
    assert!(elapsed < std::time::Duration::from_secs(30), "{elapsed:?}");
}

/// P0-002C5C: files written before C5B under raw, mixed-case names stay
/// listed, but no identifier selects, overwrites or deletes them through
/// another spelling, and nothing looks for a similar name.
#[test]
fn p0_002c5c_legacy_mixed_case_files_are_listed_but_never_selected_by_another_spelling() {
    let state = AppState::new_in_memory();
    let dir = std::env::temp_dir().join(format!("nexus-c5c-legacy-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let legacy_email = r#"{"id":"Msg-ABC","legacy":true}"#;
    let legacy_note = r#"{"id":"Note-1","legacy":true}"#;
    std::fs::write(dir.join("Msg-ABC.json"), legacy_email).unwrap();
    std::fs::write(dir.join("Note-1.json"), legacy_note).unwrap();

    let listed = json_documents(&dir).unwrap();
    assert_eq!(listed.len(), 2);

    // An email id whose stem differs from the legacy file only by case is
    // refused for save and delete alike.
    let stem = nexus_kernel::governed_path::storage_stem("msg-abc");
    assert_eq!(stem, "msg-abc");
    for action in ["email_save", "email_delete"] {
        assert_eq!(
            stored_file(&state, action, &dir, &stem),
            Err(format!("{action}: stored name differs by case"))
        );
    }
    // The original mixed-case id now has a digest stem: another file.
    let original = nexus_kernel::governed_path::storage_stem("Msg-ABC");
    assert!(original.starts_with("h-"));
    assert_eq!(
        stored_file(&state, "email_delete", &dir, &original),
        Ok(dir.join(format!("{original}.json")))
    );

    // An uppercase legacy note is not addressable: its own spelling fails
    // the grammar, and the lowercase spelling is refused, not folded.
    assert_eq!(
        identified_file(&state, "notes_get", &dir, "Note-1"),
        Err("notes_get: invalid identifier".to_string())
    );
    for action in ["notes_get", "notes_save", "notes_delete", "project_get"] {
        assert_eq!(
            identified_file(&state, action, &dir, "note-1"),
            Err(format!("{action}: stored name differs by case"))
        );
    }
    // Only an exact spelling is ever selected.
    std::fs::write(dir.join("note-1.json"), "{}").unwrap();
    assert_eq!(
        identified_file(&state, "notes_get", &dir, "note-1"),
        Ok(dir.join("note-1.json"))
    );
    // The legacy files are untouched.
    assert_eq!(
        std::fs::read_to_string(dir.join("Msg-ABC.json")).unwrap(),
        legacy_email
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("Note-1.json")).unwrap(),
        legacy_note
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// P0-002C5C: email recipients and subjects stay on one header line.
#[test]
fn p0_002c5c_email_header_values_cannot_add_headers() {
    for ok in ["a@example.com", "Weekly report", "Ünïcode ✓", ""] {
        assert!(email_header_ok(ok), "{ok:?}");
    }
    for hostile in [
        "a@example.com\r\nBcc: spy@example.com",
        "subject\nBcc: spy@example.com",
        "subject\rX: y",
        "nul\0byte",
    ] {
        assert!(!email_header_ok(hostile), "{hostile:?}");
    }
    let state = AppState::new_in_memory();
    assert_eq!(
        email_send_message(
            &state,
            "gmail".into(),
            "a@example.com\r\nBcc: spy@example.com".into(),
            "hello".into(),
            "body".into(),
        ),
        Err("email_send: invalid header".to_string())
    );
}
