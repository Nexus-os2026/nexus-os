//! M3-Cq kernel oracles (m3cq-spec-v6 §6.4, rows in file K): the floor is
//! the transport's whole-identity record, refusals precede any I/O, and N1
//! pairs and conversations need both the route and the record (CQ-R4).
//!
//! Every runtime is the shared fake on a fixture listener of this process.
//! Observations are taken after the synchronous call has returned: every
//! request it made was accepted and logged before its reply.

use super::super::fixture::FixtureRecord;
use super::*;

/// The §3.1 identity v2 goldens of [`spec`]'s pin, one per purpose.
pub(super) const PB_V2: &str = "c5b838e3070d2ffcfb20f15e4420cca44c4f17456ff08a2232c0cd2a91c5b978";
pub(super) const N1_V2: &str = "5fad3aa48de2db319cb39dc9659aa54eeb75807c3ec5c88f2174adc010df9c9c";
pub(super) const JUDGE_V2: &str =
    "b46c2ff5a58505f7f8a01cbb24a50818683dc1ad51a2ec55733aec506a008584";

/// JUDGE_ONLY: only the Judge standard-fake record.
pub(super) fn judge_only() -> Vec<FixtureRecord> {
    vec![fixture_record(Purpose::Judge, JUDGE_V2, 132)]
}

/// The counters a refusal must leave unchanged (v6 §6.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Snapshot {
    pub(super) accepts: usize,
    pub(super) connection_events: usize,
    pub(super) metadata: [usize; 4],
    pub(super) chats: usize,
    pub(super) charge_calls: usize,
    pub(super) charge_entries: usize,
    pub(super) settlements: usize,
}

pub(super) fn snapshot(fake: &Fake, ctx: &Ctx) -> Snapshot {
    let events = ctx.meter.events();
    Snapshot {
        accepts: fake.connections(),
        connection_events: fake.connection_events(),
        metadata: ["/api/version", "/api/status", "/api/tags", "/api/show"]
            .map(|path| fake.count(path)),
        chats: fake.count("/api/chat"),
        charge_calls: ctx.meter.calls.load(Ordering::SeqCst),
        charge_entries: events
            .iter()
            .filter(|e| matches!(e, Metered::Charge(_)))
            .count(),
        settlements: events
            .iter()
            .filter(|e| matches!(e, Metered::Settle(_)))
            .count(),
    }
}

/// Z: no accept, accept event, metadata, chat, charge call, charge or
/// settlement.
#[track_caller]
pub(super) fn assert_z(before: Snapshot, after: Snapshot) {
    assert_eq!(after.accepts, before.accepts, "cq.no_accepts");
    assert_eq!(
        after.connection_events, before.connection_events,
        "cq.no_connection_events"
    );
    assert_eq!(after.metadata, before.metadata, "cq.no_metadata");
    assert_eq!(after.chats, before.chats, "cq.no_chat");
    assert_eq!(
        after.charge_calls, before.charge_calls,
        "cq.no_charge_calls"
    );
    assert_eq!(
        after.charge_entries, before.charge_entries,
        "cq.no_charge_entries"
    );
    assert_eq!(after.settlements, before.settlements, "cq.no_settlements");
}

/// H: one healthy call: four accepts, metadata [1,1,1,0], one chat, one
/// charge and one settlement.
#[track_caller]
pub(super) fn assert_h(before: Snapshot, after: Snapshot) {
    assert_eq!(after.accepts - before.accepts, 4, "cq.h.accepts");
    assert_eq!(
        after.connection_events - before.connection_events,
        4,
        "cq.h.connection_events"
    );
    let metadata: Vec<usize> = (0..4)
        .map(|i| after.metadata[i] - before.metadata[i])
        .collect();
    assert_eq!(metadata, [1, 1, 1, 0], "cq.h.metadata");
    assert_eq!(after.chats - before.chats, 1, "cq.h.chat");
    assert_eq!(
        after.charge_calls - before.charge_calls,
        1,
        "cq.h.charge_call"
    );
    assert_eq!(
        after.charge_entries - before.charge_entries,
        1,
        "cq.h.charge"
    );
    assert_eq!(after.settlements - before.settlements, 1, "cq.h.settlement");
}

pub(super) fn spec_for(purpose: Purpose) -> ModelPinSpec {
    ModelPinSpec { purpose, ..spec() }
}

pub(super) fn pair(purpose: Purpose) -> ModelRequest {
    ModelRequest::new(
        purpose,
        "system".into(),
        "user".into(),
        grammar::worker_schema(),
    )
    .unwrap()
}

fn conversation_request() -> ModelRequest {
    let conversation = Conversation::new(
        grammar::WORKER_SYSTEM_INSTRUCTION.to_string(),
        "Owner task:\nfix it\n".to_string(),
    )
    .unwrap();
    ModelRequest::conversation(&conversation, grammar::worker_schema()).unwrap()
}

/// A typed registration of `spec` on `fake`, its events then forgotten.
pub(super) fn registered(fake: &Fake, ctx: &Arc<Ctx>, spec: ModelPinSpec) -> Arc<VerifiedOllama> {
    let transport = Arc::new(
        done(&verify_in_thread(fake, ctx, spec))
            .result
            .expect("registration"),
    );
    fake.drain();
    transport
}

/// A call with N1's route shut (the production state).
pub(super) fn closed(
    transport: &Arc<VerifiedOllama>,
    ctx: &Arc<Ctx>,
    request: ModelRequest,
) -> Outcome<ModelReply> {
    done(&call_with(transport, ctx, request, ctx.watch.clone(), None))
}

/// A call with the private seam open on its calling thread.
pub(super) fn open(
    transport: &Arc<VerifiedOllama>,
    ctx: &Arc<Ctx>,
    request: ModelRequest,
) -> Outcome<ModelReply> {
    done(&call_with_open_n1(
        transport,
        ctx,
        request,
        ctx.watch.clone(),
        None,
    ))
}

/// `call` and its refusal with Z.
#[track_caller]
fn refused(
    fake: &Fake,
    ctx: &Arc<Ctx>,
    call: impl FnOnce() -> Outcome<ModelReply>,
    expected: ModelError,
    label: &str,
) {
    let before = snapshot(fake, ctx);
    let outcome = call();
    assert_z(before, snapshot(fake, ctx));
    assert_eq!(outcome.code(), Err(expected), "{label}");
}

/// `call` and H.
#[track_caller]
fn healthy_call(
    fake: &Fake,
    ctx: &Arc<Ctx>,
    call: impl FnOnce() -> Outcome<ModelReply>,
    label: &str,
) {
    let before = snapshot(fake, ctx);
    let outcome = call();
    assert_eq!(outcome.code(), Ok(()), "{label}");
    assert_h(before, snapshot(fake, ctx));
}

fn with_prompt(fake: &Fake, count: u64) {
    let mut value = chat_value();
    value["prompt_eval_count"] = json!(count);
    fake.set_route(chat(value));
}

// ── O-1 ─────────────────────────────────────────────────────────────────────

#[test]
fn cq_verified_identity_selects_floor() {
    for (purpose, floor) in [
        (Purpose::N1Worker, 256u64),
        (Purpose::PageBuilder, 300),
        (Purpose::Judge, 100),
    ] {
        let fake = Fake::healthy();
        let ctx = Ctx::new();
        let transport = registered(&fake, &ctx, spec_for(purpose));
        let call = |n: u64| {
            with_prompt(&fake, n);
            let before = snapshot(&fake, &ctx);
            let outcome = if purpose == Purpose::N1Worker {
                open(&transport, &ctx, pair(purpose))
            } else {
                closed(&transport, &ctx, pair(purpose))
            };
            let after = snapshot(&fake, &ctx);
            assert_eq!(
                after.charge_entries - before.charge_entries,
                1,
                "{purpose:?} {n}: one charge"
            );
            assert_eq!(
                after.settlements - before.settlements,
                1,
                "{purpose:?} {n}: settled"
            );
            outcome.code()
        };
        for n in [0, floor - 1] {
            assert_eq!(
                call(n),
                Err(ModelError::Truncated),
                "cq.o1 {purpose:?}: prompt {n} is below F"
            );
        }
        assert_eq!(
            call(floor),
            Ok(()),
            "cq.o1 {purpose:?}: prompt F is accepted"
        );
    }
}

// ── O-2, O-3 ────────────────────────────────────────────────────────────────

#[test]
fn cq_unqualified_transport_refused_before_any_io() {
    let empty = Fake::start_with(&[], route(|_| None));
    let ctx = Ctx::new();
    let transport = registered(&empty, &ctx, spec_for(Purpose::PageBuilder));
    refused(
        &empty,
        &ctx,
        || closed(&transport, &ctx, pair(Purpose::PageBuilder)),
        ModelError::PurposeNotQualified,
        "cq.empty_profile_is_not_qualified",
    );

    let fake = Fake::start_with(&judge_only(), route(|_| None));
    let ctx = Ctx::new();
    let page = registered(&fake, &ctx, spec_for(Purpose::PageBuilder));
    let judge = registered(&fake, &ctx, spec_for(Purpose::Judge));
    refused(
        &fake,
        &ctx,
        || closed(&page, &ctx, pair(Purpose::Judge)),
        ModelError::PurposeNotQualified,
        "cq.transport_qualification_precedes_mismatch",
    );
    healthy_call(
        &fake,
        &ctx,
        || closed(&judge, &ctx, pair(Purpose::Judge)),
        "Judge H",
    );
}

#[test]
fn cq_wrong_purpose_refused_before_any_io() {
    let fake = Fake::healthy();
    let ctx = Ctx::new();
    let page = registered(&fake, &ctx, spec_for(Purpose::PageBuilder));
    refused(
        &fake,
        &ctx,
        || closed(&page, &ctx, pair(Purpose::Judge)),
        ModelError::PurposeMismatch,
        "cq.purpose_mismatch",
    );
    healthy_call(
        &fake,
        &ctx,
        || closed(&page, &ctx, pair(Purpose::PageBuilder)),
        "PageBuilder H",
    );
}

// ── O-4 ─────────────────────────────────────────────────────────────────────

#[test]
fn cq_refusal_precedence() {
    let empty = Fake::start_with(&[], route(|_| None));
    let ctx = Ctx::new();
    let unqualified = registered(&empty, &ctx, spec_for(Purpose::PageBuilder));
    // Every conversation is refused first while the route is shut.
    refused(
        &empty,
        &ctx,
        || closed(&unqualified, &ctx, conversation_request()),
        ModelError::ConfigMismatch,
        "cq.o4 conversation over qualification",
    );
    let fake = Fake::healthy();
    let page = registered(&fake, &ctx, spec_for(Purpose::PageBuilder));
    let worker = registered(&fake, &ctx, spec_for(Purpose::N1Worker));
    refused(
        &fake,
        &ctx,
        || closed(&page, &ctx, conversation_request()),
        ModelError::ConfigMismatch,
        "cq.o4 conversation over mismatch",
    );
    // The size bound, for a qualified, matching transport.
    refused(
        &fake,
        &ctx,
        || {
            closed(
                &page,
                &ctx,
                ModelRequest::cq_oversize_pair(Purpose::PageBuilder),
            )
        },
        ModelError::RequestTooLarge,
        "cq.o4 qualified+same: request bound",
    );
    // Qualification, then purpose, before the request bound.
    refused(
        &empty,
        &ctx,
        || {
            closed(
                &unqualified,
                &ctx,
                ModelRequest::cq_oversize_pair(Purpose::Judge),
            )
        },
        ModelError::PurposeNotQualified,
        "cq.o4 unqualified+mismatch",
    );
    refused(
        &fake,
        &ctx,
        || closed(&page, &ctx, ModelRequest::cq_oversize_pair(Purpose::Judge)),
        ModelError::PurposeMismatch,
        "cq.o4 qualified+mismatch",
    );
    // A closed N1 route wins over both, for N1 in the request, the identity or both.
    refused(
        &fake,
        &ctx,
        || {
            closed(
                &page,
                &ctx,
                ModelRequest::cq_oversize_pair(Purpose::N1Worker),
            )
        },
        ModelError::ConfigMismatch,
        "cq.o4 N1 request only",
    );
    refused(
        &fake,
        &ctx,
        || {
            closed(
                &worker,
                &ctx,
                ModelRequest::cq_oversize_pair(Purpose::PageBuilder),
            )
        },
        ModelError::ConfigMismatch,
        "cq.o4 N1 identity only",
    );
    refused(
        &fake,
        &ctx,
        || {
            closed(
                &worker,
                &ctx,
                ModelRequest::cq_oversize_pair(Purpose::N1Worker),
            )
        },
        ModelError::ConfigMismatch,
        "cq.o4 N1 both",
    );
    // With the seam open, qualification and mismatch are reached.
    let empty_worker = registered(&empty, &ctx, spec_for(Purpose::N1Worker));
    refused(
        &empty,
        &ctx,
        || open(&empty_worker, &ctx, pair(Purpose::N1Worker)),
        ModelError::PurposeNotQualified,
        "cq.o4 open: not qualified",
    );
    refused(
        &fake,
        &ctx,
        || open(&worker, &ctx, pair(Purpose::PageBuilder)),
        ModelError::PurposeMismatch,
        "cq.o4 open: mismatch",
    );
    healthy_call(
        &fake,
        &ctx,
        || closed(&page, &ctx, pair(Purpose::PageBuilder)),
        "PageBuilder H",
    );
}

// ── O-5 ─────────────────────────────────────────────────────────────────────

/// A runtime that honestly serves `spec`'s name, digest and version.
fn serving(spec: &ModelPinSpec) -> Fake {
    let (name, digest, version) = (
        spec.name.clone(),
        spec.digest.clone(),
        spec.runtime_version.clone(),
    );
    Fake::start(route(move |path| {
        match path {
        "/api/version" => Some(Reply::Json(200, json!({ "version": version }).to_string())),
        "/api/tags" => Some(Reply::Json(
            200,
            json!({"models": [{"name": name, "model": name, "digest": digest, "size": 1, "details": {}}]})
                .to_string(),
        )),
        _ => None,
    }
    }))
}

/// One honest change to a pin, and its name.
type Change = (&'static str, fn(&mut ModelPinSpec));

#[test]
fn cq_qualification_binds_measured_identity() {
    let changes: [Change; 5] = [
        ("name", |s| s.name = "qwen3.6:36b".to_string()),
        ("manifest digest", |s| s.digest = OTHER_DIGEST.to_string()),
        ("runtime version", |s| {
            s.runtime_version = "0.40.14".to_string()
        }),
        ("seed", |s| s.config.seed = 8),
        ("context", |s| s.config.num_ctx = 16_384),
    ];
    for (what, change) in changes {
        let mut other = spec_for(Purpose::PageBuilder);
        change(&mut other);
        let fake = serving(&other);
        let ctx = Ctx::new();
        let transport = registered(&fake, &ctx, other);
        refused(
            &fake,
            &ctx,
            || closed(&transport, &ctx, pair(Purpose::PageBuilder)),
            ModelError::PurposeNotQualified,
            &format!("cq.o5 another {what} has no record"),
        );
    }
    let fake = Fake::healthy();
    let ctx = Ctx::new();
    let original = registered(&fake, &ctx, spec_for(Purpose::PageBuilder));
    healthy_call(
        &fake,
        &ctx,
        || closed(&original, &ctx, pair(Purpose::PageBuilder)),
        "original pin H",
    );
}

// ── O-6 ─────────────────────────────────────────────────────────────────────

#[test]
fn cq_production_path_ignores_fixtures() {
    let fake = Fake::healthy();
    let ctx = Ctx::new();
    // The ordinary string path to the same live STANDARD listener, with an
    // identity no production record can hold (v6 §6.7).
    let endpoint = LoopbackEndpoint::parse(&format!("http://{}", fake.addr)).unwrap();
    let c = ctx.clone();
    let ordinary = Arc::new(
        done(&in_thread(
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
        .expect("ordinary registration"),
    );
    fake.drain();
    refused(
        &fake,
        &ctx,
        || closed(&ordinary, &ctx, pair(Purpose::Judge)),
        ModelError::PurposeNotQualified,
        "cq.o6 the ordinary path reads only production records",
    );
    assert_eq!(
        ordinary.provenance(),
        Provenance::Frozen(None),
        "cq.o6 ordinary provenance"
    );
    let typed = registered(&fake, &ctx, spec_for(Purpose::Judge));
    healthy_call(
        &fake,
        &ctx,
        || closed(&typed, &ctx, pair(Purpose::Judge)),
        "typed Judge H",
    );
    assert_eq!(
        typed.provenance(),
        Provenance::Fixture,
        "cq.o6 typed provenance"
    );
}

// ── O-9, O-10 ───────────────────────────────────────────────────────────────

/// Identity v2 bytes from literal fields and explicit big-endian lengths
/// (never canonical(), code() or digest()): (encoded length, sha256 hex).
fn v2(name: &str, digest: &str, canonical: &str, code: &str) -> (usize, String) {
    let mut bytes = b"nexus.inference.identity.v2".to_vec();
    for field in ["ollama", name, digest, "0.40.13", canonical, code] {
        bytes.extend((field.len() as u32).to_be_bytes());
        bytes.extend(field.as_bytes());
    }
    (bytes.len(), sha256_hex(&bytes))
}

#[test]
fn cq_identity_v2_goldens() {
    const CANONICAL: &str = "num_ctx=8192;temperature_milli=0;seed=7;think=off;truncate=false";
    const SESSION_DIGEST: &str = "abababababababababababababababababababababababababababababababab";
    let standard = [
        ("page_builder", 219, PB_V2, Purpose::PageBuilder),
        ("n1_worker", 216, N1_V2, Purpose::N1Worker),
        ("judge", 212, JUDGE_V2, Purpose::Judge),
    ];
    for (code, length, golden, _) in standard {
        assert_eq!(
            v2(NAME, DIGEST, CANONICAL, code),
            (length, golden.to_string()),
            "{code}"
        );
    }
    for (code, golden) in [
        (
            "page_builder",
            "50c6effe8d484adca29c968f04823cf2d5ecb89fbfd35bbd364c1f906ec9c4fe",
        ),
        (
            "n1_worker",
            "abfb8e84f2c96337875649b78cb3d8158cc2d891be318258704a751716dac8aa",
        ),
        (
            "judge",
            "aa47f87aaf1fe62bdd1bffd9abef51f82cdd8b557be16b061c63c165912ea2c6",
        ),
    ] {
        assert_eq!(
            v2("qwen3:8b", SESSION_DIGEST, CANONICAL, code).1,
            golden,
            "session {code}"
        );
    }
    let context = CANONICAL.replace("num_ctx=8192", "num_ctx=16384");
    assert_eq!(
        v2(NAME, DIGEST, &context, "n1_worker"),
        (
            217,
            "116f4ff9bbfdc908bddcb2f3f61f356d1e44084b32296fe13b4d1e0b66244bf6".to_string()
        )
    );
    const V1: &str = "4d7211f035868b327c0e10f2bc23155a7d492d7beecc5b505edfa373db82f9a4";
    for (code, _, golden, purpose) in standard {
        assert_ne!(golden, V1, "{code}: v1 is retired");
        let identity = ModelIdentity::from_spec(&spec_for(purpose)).unwrap();
        assert_eq!(
            hex::encode(identity.digest()),
            golden,
            "cq.identity_v2 {code}"
        );
    }
}

#[test]
fn cq_wire_bytes_unchanged() {
    let fake = Fake::healthy();
    let ctx = Ctx::new();
    let mut bodies = Vec::new();
    for purpose in [Purpose::PageBuilder, Purpose::N1Worker, Purpose::Judge] {
        let transport = registered(&fake, &ctx, spec_for(purpose));
        let outcome = if purpose == Purpose::N1Worker {
            open(&transport, &ctx, pair(purpose))
        } else {
            closed(&transport, &ctx, pair(purpose))
        };
        assert_eq!(outcome.code(), Ok(()), "{purpose:?}");
        bodies.push(fake.last_body("/api/chat"));
    }
    assert!(
        bodies.iter().all(|body| body == &bodies[0]),
        "cq.wire: one body for every purpose"
    );
    assert!(
        !bodies[0].contains("purpose") && !bodies[0].contains("page_builder"),
        "cq.wire: no purpose"
    );
    assert!(bodies[0].starts_with(
        r#"{"model":"qwen3.6:35b-a3b","stream":false,"think":false,"truncate":false,"format":"#
    ));
    assert!(bodies[0].contains(
        r#""messages":[{"role":"system","content":"system"},{"role":"user","content":"user"}]"#
    ));
}

// ── N-1 ─────────────────────────────────────────────────────────────────────

#[test]
fn cq_n1_pair_and_conversation_require_route_and_record() {
    let fake = Fake::healthy();
    let ctx = Ctx::new();
    let worker = registered(&fake, &ctx, spec_for(Purpose::N1Worker));
    let page = registered(&fake, &ctx, spec_for(Purpose::PageBuilder));
    let judge = registered(&fake, &ctx, spec_for(Purpose::Judge));
    refused(
        &fake,
        &ctx,
        || closed(&worker, &ctx, conversation_request()),
        ModelError::ConfigMismatch,
        "cq.n1 closed conversation",
    );
    refused(
        &fake,
        &ctx,
        || closed(&worker, &ctx, pair(Purpose::N1Worker)),
        ModelError::ConfigMismatch,
        "cq.n1 closed N1 pair",
    );
    refused(
        &fake,
        &ctx,
        || closed(&page, &ctx, pair(Purpose::N1Worker)),
        ModelError::ConfigMismatch,
        "cq.n1 closed: N1 request only",
    );
    refused(
        &fake,
        &ctx,
        || closed(&worker, &ctx, pair(Purpose::PageBuilder)),
        ModelError::ConfigMismatch,
        "cq.n1 closed: N1 transport only",
    );
    healthy_call(
        &fake,
        &ctx,
        || open(&worker, &ctx, pair(Purpose::N1Worker)),
        "open N1 pair H",
    );
    assert_eq!(
        open(&worker, &ctx, conversation_request()).code(),
        Ok(()),
        "open N1 conversation"
    );
    refused(
        &fake,
        &ctx,
        || open(&worker, &ctx, pair(Purpose::PageBuilder)),
        ModelError::PurposeMismatch,
        "cq.n1 open: qualified, wrong purpose",
    );
    let empty = Fake::start_with(&[], route(|_| None));
    let unqualified = registered(&empty, &ctx, spec_for(Purpose::N1Worker));
    refused(
        &empty,
        &ctx,
        || open(&unqualified, &ctx, pair(Purpose::N1Worker)),
        ModelError::PurposeNotQualified,
        "cq.n1 open: EMPTY",
    );
    healthy_call(
        &fake,
        &ctx,
        || closed(&page, &ctx, pair(Purpose::PageBuilder)),
        "PageBuilder H without seam",
    );
    healthy_call(
        &fake,
        &ctx,
        || closed(&judge, &ctx, pair(Purpose::Judge)),
        "Judge H without seam",
    );
}
