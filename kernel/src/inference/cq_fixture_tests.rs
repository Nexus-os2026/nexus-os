//! M3-Cq fixture oracles (m3cq-spec-v6 §6.4, rows in file F): fixture
//! authority, its expiry and lease, provenance, the accept counter, record
//! keys, the G1 source gate, and ordinary verification of a cloned fixture
//! endpoint.

use super::super::fixture::{FixtureListener, FixtureRecord};
use super::cq::{assert_h, assert_z, closed, pair, registered, snapshot, spec_for};
use super::*;

fn record(purpose: Purpose, byte: u8, minimum: u32) -> FixtureRecord {
    FixtureRecord {
        purpose,
        identity_sha256: [byte; 32],
        floor: freeze_floor(minimum).unwrap(),
    }
}

/// Wait (bounded, no fixed sleep) until the fake has accepted `want`.
#[track_caller]
fn accepted(fake: &Fake, want: usize) {
    let deadline = std::time::Instant::now() + PATIENCE;
    while fake.connections() < want {
        assert!(
            std::time::Instant::now() < deadline,
            "accepts never reached {want}"
        );
        std::thread::yield_now();
    }
}

// ── F-1 ─────────────────────────────────────────────────────────────────────

#[test]
fn cq_expired_fixture_refuses_while_peer_serves() {
    // Expired before verification: the clone still serves, nothing connects.
    let mut fake = Fake::healthy();
    let ctx = Ctx::new();
    let endpoint = fake.endpoint();
    fake.expire();
    let before = snapshot(&fake, &ctx);
    let c = ctx.clone();
    let result = done(&in_thread(
        move || VerifiedOllama::verify_fixture(endpoint, &spec(), &c.cancel, &c.deadline, &c.watch),
        None,
    ))
    .result;
    assert_z(before, snapshot(&fake, &ctx));
    assert_eq!(
        result.err(),
        Some(ModelError::Unavailable),
        "cq.f1 expired before verify"
    );
    drop(TcpStream::connect(fake.addr).unwrap());
    accepted(&fake, before.accepts + 1);

    // Expired after a verification made while the owner was alive.
    let mut fake = Fake::healthy();
    let ctx = Ctx::new();
    let transport = registered(&fake, &ctx, spec_for(Purpose::PageBuilder));
    let live = Fake::healthy();
    let control = registered(&live, &ctx, spec_for(Purpose::PageBuilder));
    let before = snapshot(&live, &ctx);
    assert_eq!(
        closed(&control, &ctx, pair(Purpose::PageBuilder)).code(),
        Ok(()),
        "live-owner H"
    );
    assert_h(before, snapshot(&live, &ctx));
    fake.expire();
    let before = snapshot(&fake, &ctx);
    let outcome = closed(&transport, &ctx, pair(Purpose::PageBuilder));
    assert_z(before, snapshot(&fake, &ctx));
    assert_eq!(
        outcome.code(),
        Err(ModelError::Unavailable),
        "cq.f1 expired after verify"
    );
    drop(TcpStream::connect(fake.addr).unwrap());
    accepted(&fake, before.accepts + 1);
}

// ── F-2 ─────────────────────────────────────────────────────────────────────

#[test]
fn cq_exchange_lease_outlives_owner() {
    let mut fake = Fake::healthy();
    let ctx = Ctx::new();
    let transport = registered(&fake, &ctx, spec_for(Purpose::PageBuilder));
    let observer = fake.endpoint();
    let (paused_tx, paused) = mpsc::channel::<()>();
    let (resume, resume_rx) = mpsc::channel::<()>();
    let resume_rx = Arc::new(Mutex::new(resume_rx));
    let (t, c) = (transport.clone(), ctx.clone());
    let rx = in_thread(
        move || {
            let fired = AtomicBool::new(false);
            seams::set_after_lease(Some(Box::new(move || {
                if !fired.swap(true, Ordering::SeqCst) {
                    let _ = paused_tx.send(());
                    let _ = resume_rx.lock().unwrap().recv_timeout(PATIENCE);
                }
            })));
            let result = t.call(&pair(Purpose::PageBuilder), &c.call_ctx(&c.watch));
            seams::set_after_lease(None);
            result
        },
        None,
    );
    paused
        .recv_timeout(PATIENCE)
        .expect("the lease hook was reached");
    // Scope guard: the paused exchange is always released.
    struct Release(mpsc::Sender<()>);
    impl Drop for Release {
        fn drop(&mut self) {
            let _ = self.0.send(());
        }
    }
    let release = Release(resume);
    fake.expire();
    let alive = observer.authority_alive_for_test();
    drop(release);
    let outcome = done(&rx);
    assert!(
        alive,
        "cq.f2 the admitted exchange holds the authority after the owner drops"
    );
    assert_eq!(
        outcome.code(),
        Ok(()),
        "cq.f2 the admitted exchange completes"
    );
    assert_eq!(outcome.live, 0, "cq.f2 every exchange thread joined");
    assert!(
        !observer.authority_alive_for_test(),
        "cq.f2 released after the joined exchange"
    );
    let before = snapshot(&fake, &ctx);
    assert_eq!(
        closed(&transport, &ctx, pair(Purpose::PageBuilder)).code(),
        Err(ModelError::Unavailable),
        "cq.f2 a later exchange after the drop is refused"
    );
    assert_z(before, snapshot(&fake, &ctx));
}

// ── F-3 ─────────────────────────────────────────────────────────────────────

#[test]
fn cq_fixture_provenance_is_marked() {
    let mut fake = Fake::healthy();
    let ctx = Ctx::new();
    let typed = registered(&fake, &ctx, spec_for(Purpose::PageBuilder));
    assert_eq!(
        typed.provenance(),
        Provenance::Fixture,
        "cq.f3 typed instance"
    );
    // An ordinary instance of an identity no production record can hold.
    let endpoint = LoopbackEndpoint::parse(&format!("http://{}", fake.addr)).unwrap();
    let c = ctx.clone();
    let ordinary = done(&in_thread(
        move || {
            VerifiedOllama::verify(
                endpoint,
                &spec_for(Purpose::Judge),
                &c.cancel,
                &c.deadline,
                &c.watch,
            )
        },
        None,
    ))
    .result
    .unwrap();
    assert_eq!(
        ordinary.provenance(),
        Provenance::Frozen(None),
        "cq.f3 ordinary instance"
    );
    fake.expire();
    assert_eq!(
        typed.provenance(),
        Provenance::Fixture,
        "cq.f3 typed instance after expiry"
    );
}

// ── F-4 ─────────────────────────────────────────────────────────────────────

#[test]
fn cq_accept_counter_counts_connect_without_request() {
    let fake = Fake::healthy();
    let ctx = Ctx::new();
    let transport = registered(&fake, &ctx, spec_for(Purpose::PageBuilder));
    let before = snapshot(&fake, &ctx);
    // C: a raw connect without any HTTP request.
    drop(TcpStream::connect(fake.addr).unwrap());
    accepted(&fake, before.accepts + 1);
    let after = snapshot(&fake, &ctx);
    assert_eq!(after.accepts, before.accepts + 1, "cq.c accepts +1");
    assert_eq!(
        after.connection_events,
        before.connection_events + 1,
        "cq.c accept events +1"
    );
    assert_eq!(
        (
            after.metadata,
            after.chats,
            after.charge_calls,
            after.charge_entries,
            after.settlements
        ),
        (
            before.metadata,
            before.chats,
            before.charge_calls,
            before.charge_entries,
            before.settlements
        ),
        "cq.c nothing else"
    );
    // H counts positive metadata, chat, charge and settlement.
    assert_eq!(
        closed(&transport, &ctx, pair(Purpose::PageBuilder)).code(),
        Ok(())
    );
    assert_h(after, snapshot(&fake, &ctx));
}

// ── F-5 ─────────────────────────────────────────────────────────────────────

#[test]
fn cq_fixture_records_reject_duplicate_keys() {
    let duplicate = [
        record(Purpose::Judge, 3, 132),
        record(Purpose::Judge, 3, 132),
    ];
    let other_floor = [
        record(Purpose::Judge, 3, 132),
        record(Purpose::Judge, 3, 200),
    ];
    for records in [&duplicate[..], &other_floor[..]] {
        match FixtureListener::bind(records) {
            Err(error) => assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput),
            Ok(listener) => {
                drop(listener);
                panic!("cq.duplicate_key_rejected: a duplicate key bound a listener");
            }
        }
    }
    let distinct = [
        record(Purpose::Judge, 3, 132),
        record(Purpose::PageBuilder, 3, 332),
        record(Purpose::Judge, 4, 132),
    ];
    assert!(FixtureListener::bind(&distinct).is_ok());
    assert!(FixtureListener::bind(&[]).is_ok());
}

// ── R-3 ─────────────────────────────────────────────────────────────────────

#[test]
fn cq_ordinary_verify_rejects_cloned_fixture_endpoint() {
    let fake = Fake::healthy();
    let ctx = Ctx::new();
    let typed = registered(&fake, &ctx, spec_for(Purpose::PageBuilder));
    let cloned = typed.endpoint().clone();
    let before = snapshot(&fake, &ctx);
    let c = ctx.clone();
    let result = done(&in_thread(
        move || {
            VerifiedOllama::verify(
                cloned,
                &spec_for(Purpose::PageBuilder),
                &c.cancel,
                &c.deadline,
                &c.watch,
            )
        },
        None,
    ))
    .result;
    assert_z(before, snapshot(&fake, &ctx));
    assert_eq!(
        result.err(),
        Some(ModelError::ConfigMismatch),
        "cq.r3 ordinary verify refuses a fixture clone"
    );
    let again = registered(&fake, &ctx, spec_for(Purpose::PageBuilder));
    assert_eq!(
        again.provenance(),
        Provenance::Fixture,
        "verify_fixture stays reachable"
    );
    let parsed = LoopbackEndpoint::parse(&format!("http://{}", fake.addr)).unwrap();
    assert_ne!(
        &parsed,
        typed.endpoint(),
        "a parse of the same address stays ordinary"
    );
}

// ── G-1: the G1 source gate (Cq1a-N1: the Step 0 semantics) ────────────────

fn source(rel: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// Comments removed and string-literal contents emptied (quotes kept).
fn code_only(raw: &str) -> String {
    let b = raw.as_bytes();
    let mut out = String::with_capacity(raw.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if b[i] == b'"' {
            out.push('"');
            i += 1;
            while i < b.len() && b[i] != b'"' {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            out.push('"');
            i += 1;
        } else {
            out.push(b[i] as char);
            i += 1;
        }
    }
    out
}

/// Byte ranges of each item, field, variant, statement or arm that follows
/// an exact `#[cfg(test)]` (to its `;`, `,` or closing brace at depth 0).
fn cfg_test_ranges(code: &str) -> Vec<(usize, usize)> {
    let needle = "#[cfg(test)]";
    let b = code.as_bytes();
    let mut ranges = Vec::new();
    let mut from = 0;
    while let Some(at) = code[from..].find(needle) {
        let start = from + at;
        let mut i = start + needle.len();
        let mut depth = 0i32;
        while i < b.len() {
            match b[i] {
                b'{' | b'(' | b'[' => depth += 1,
                b'}' | b')' | b']' => {
                    depth -= 1;
                    if depth == 0 && b[i] == b'}' {
                        let next = code[i + 1..].trim_start();
                        if !next.starts_with(',') && !next.starts_with('=') {
                            i += 1;
                            break;
                        }
                    }
                    if depth < 0 {
                        break;
                    }
                }
                b';' | b',' if depth == 0 => {
                    i += 1;
                    break;
                }
                _ => {}
            }
            i += 1;
        }
        ranges.push((start, i));
        from = start + needle.len();
    }
    ranges
}

/// Part 1: every top-level item of fixture.rs carries exactly
/// `#[cfg(test)]` and no other cfg, and no `cfg(any`/feature appears; the
/// fixture source variant, its arms and every fixture-only helper in
/// endpoint.rs, ollama.rs and exchange.rs lie inside an exact `#[cfg(test)]`.
fn part1_findings() -> Vec<String> {
    let mut found = Vec::new();
    let code = code_only(&source("src/inference/fixture.rs"));
    let mut attrs: Vec<String> = Vec::new();
    let mut depth = 0i32;
    for line in code.lines() {
        let t = line.trim();
        if depth == 0 && !t.is_empty() && !t.starts_with("//!") {
            if t.starts_with("#[") {
                attrs.push(t.to_owned());
            } else if t.starts_with("#!") {
                found.push(format!("part1 inner attribute: {t}"));
            } else {
                let exact = attrs.iter().filter(|a| *a == "#[cfg(test)]").count();
                let other = attrs
                    .iter()
                    .filter(|a| a.starts_with("#[cfg") && *a != "#[cfg(test)]")
                    .count();
                if exact != 1 || other != 0 {
                    found.push(format!("part1 item without exact #[cfg(test)]: {t}"));
                }
                attrs.clear();
            }
        }
        for c in line.bytes() {
            match c {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
        }
    }
    if code.contains("cfg(any") || code.contains("feature") {
        found.push("part1 cfg(any) or feature in fixture.rs".into());
    }
    for rel in [
        "src/inference/endpoint.rs",
        "src/inference/ollama.rs",
        "src/inference/exchange.rs",
    ] {
        let code = code_only(&source(rel));
        let ranges = cfg_test_ranges(&code);
        for token in ["Fixture", "fixture", "WeakAuthority"] {
            let mut from = 0;
            while let Some(at) = code[from..].find(token) {
                let pos = from + at;
                // `Provenance::Fixture` is the one ungated, public mention (§5.1).
                let in_provenance = code[..pos].ends_with("Provenance::")
                    || code[..pos]
                        .rfind("pub enum Provenance")
                        .is_some_and(|p| !code[p..pos].contains('}'));
                if !in_provenance && !ranges.iter().any(|&(a, z)| a <= pos && pos < z) {
                    let line = code[..pos].lines().count();
                    found.push(format!("part1 ungated {token} at {rel}:{line}"));
                }
                from = pos + token.len();
            }
        }
    }
    found
}

/// Part 2: the `mod fixture;` declaration itself carries `#[cfg(test)]`.
fn part2_findings() -> Vec<String> {
    let code = code_only(&source("src/inference/mod.rs"));
    let lines: Vec<&str> = code.lines().map(str::trim).collect();
    let mut found = Vec::new();
    let mut seen = 0;
    for (index, line) in lines.iter().enumerate() {
        let words: Vec<&str> = line.split_whitespace().collect();
        if words.windows(2).any(|w| w == ["mod", "fixture;"]) {
            seen += 1;
            let mut j = index;
            let mut exact = false;
            while j > 0 && lines[j - 1].starts_with("#[") {
                exact |= lines[j - 1] == "#[cfg(test)]";
                j -= 1;
            }
            if !exact {
                found.push(format!("part2 mod fixture; without #[cfg(test)]: {line}"));
            }
        }
    }
    if seen != 1 {
        found.push(format!(
            "part2 expected exactly one mod fixture; declaration, saw {seen}"
        ));
    }
    found
}

/// Part 3: no `#[path]` attribute or include-family macro anywhere under
/// kernel/src names fixture.rs (this file excluded by name).
fn part3_findings() -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
    let target = concat!("fixture", ".rs");
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|e| e != "rs")
                || path.file_name().is_some_and(|n| n == "cq_fixture_tests.rs")
            {
                continue;
            }
            for line in std::fs::read_to_string(&path).unwrap().lines() {
                let t = line.trim_start();
                if t.contains(target)
                    && (t.starts_with("#[path")
                        || t.contains("include!")
                        || t.contains("include_str!"))
                {
                    found.push(format!("part3 {}: {t}", path.display()));
                }
            }
        }
    }
    found
}

/// Part 4: kernel/build.rs prints no `cargo:rustc-cfg`.
fn part4_findings() -> Vec<String> {
    let raw = source("build.rs");
    ["cargo:rustc-cfg", "cargo::rustc-cfg"]
        .iter()
        .filter(|needle| raw.contains(*needle))
        .map(|needle| format!("part4 build.rs prints {needle}"))
        .collect()
}

#[test]
fn cq_fixture_gate_is_test_only() {
    let p1 = part1_findings();
    assert!(p1.is_empty(), "cq.g1.part1_exact_cfg_test: {p1:?}");
    let p2 = part2_findings();
    assert!(p2.is_empty(), "cq.g1.part2_mod_declaration: {p2:?}");
    let p3 = part3_findings();
    assert!(p3.is_empty(), "cq.g1.part3_no_path_include: {p3:?}");
    let p4 = part4_findings();
    assert!(p4.is_empty(), "cq.g1.part4_build_rs_no_cfg: {p4:?}");
}
