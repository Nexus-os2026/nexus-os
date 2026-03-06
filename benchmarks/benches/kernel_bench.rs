use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::fuel_hardening::{
    AgentFuelLedger, BudgetPeriodId, BurnAnomalyDetector, FuelToTokenModel,
};
use nexus_kernel::manifest::parse_manifest;
use nexus_kernel::redaction::RedactionEngine;
use serde_json::json;
use uuid::Uuid;

fn bench_audit_append(c: &mut Criterion) {
    let mut group = c.benchmark_group("audit_append");
    let agent_id = Uuid::new_v4();

    for count in [10, 100, 1000] {
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &n| {
            b.iter(|| {
                let mut trail = AuditTrail::new();
                for i in 0..n {
                    trail.append_event(
                        agent_id,
                        EventType::StateChange,
                        json!({"seq": i, "status": "ok"}),
                    );
                }
                black_box(&trail);
            });
        });
    }
    group.finish();
}

fn bench_audit_verify_integrity(c: &mut Criterion) {
    let mut group = c.benchmark_group("audit_verify_integrity");
    let agent_id = Uuid::new_v4();

    for count in [10, 100, 1000] {
        let mut trail = AuditTrail::new();
        for i in 0..count {
            trail.append_event(
                agent_id,
                EventType::StateChange,
                json!({"seq": i, "status": "ok"}),
            );
        }

        group.bench_with_input(BenchmarkId::from_parameter(count), &trail, |b, trail| {
            b.iter(|| black_box(trail.verify_integrity()));
        });
    }
    group.finish();
}

fn bench_fuel_record_spend(c: &mut Criterion) {
    let agent_id = Uuid::new_v4();

    c.bench_function("fuel_record_llm_spend", |b| {
        b.iter(|| {
            let mut audit = AuditTrail::new();
            let mut ledger = AgentFuelLedger::new(
                BudgetPeriodId::new("2026-03"),
                1_000_000,
                BurnAnomalyDetector::default(),
            );
            for _ in 0..100 {
                let _ = ledger.record_llm_spend(agent_id, "mock-1", 100, 50, 10, &mut audit);
            }
            black_box(&ledger);
        });
    });
}

fn bench_fuel_simulate_cost(c: &mut Criterion) {
    let model = FuelToTokenModel::with_defaults();

    c.bench_function("fuel_simulate_cost", |b| {
        b.iter(|| {
            black_box(model.simulate_cost("claude-sonnet-4-5", 4096, 1024));
        });
    });
}

fn bench_anomaly_detector(c: &mut Criterion) {
    c.bench_function("anomaly_detector_observe_x1000", |b| {
        b.iter(|| {
            let mut detector = BurnAnomalyDetector::new(100, 300, 50_000, 10);
            for i in 0..1000u64 {
                black_box(detector.observe(i % 200));
            }
        });
    });
}

fn bench_manifest_parse(c: &mut Criterion) {
    let toml = r#"
name = "bench-agent"
version = "1.0.0"
capabilities = ["web.search", "llm.query", "fs.read"]
fuel_budget = 10000
autonomy_level = 2
schedule = "*/10 * * * *"
llm_model = "claude-sonnet-4-5"
fuel_period_id = "2026-03"
monthly_fuel_cap = 500000
"#;

    c.bench_function("manifest_parse", |b| {
        b.iter(|| black_box(parse_manifest(black_box(toml))));
    });
}

fn bench_redaction_scan(c: &mut Criterion) {
    let mut group = c.benchmark_group("redaction_scan");

    let clean_text = "The quick brown fox jumps over the lazy dog. No PII here at all.";
    let pii_text = "Contact john@example.com or call +1 555-123-4567. \
                     Key: sk-abcdefghijklmnopqrstuvwxyz. \
                     Card: 4111111111111111.";

    group.bench_function("clean_text", |b| {
        b.iter(|| black_box(RedactionEngine::scan(black_box(clean_text))));
    });

    group.bench_function("pii_text", |b| {
        b.iter(|| black_box(RedactionEngine::scan(black_box(pii_text))));
    });

    group.finish();
}

fn bench_redaction_process_prompt(c: &mut Criterion) {
    let payload = "Please help user john@example.com with key sk-abcdefghijklmnopqrstuvwxyz \
                   reach out at +1 555-123-4567 regarding card 4111111111111111. \
                   Also check bearer eyJhbGciOiJIUzI1NiJ9 token validity.";

    c.bench_function("redaction_process_prompt", |b| {
        b.iter(|| {
            let mut engine = RedactionEngine::default();
            black_box(engine.process_prompt(
                "bench",
                "strict",
                vec!["ctx-bench".to_string()],
                payload,
            ));
        });
    });
}

fn bench_redaction_apply(c: &mut Criterion) {
    let text = "Email: user@test.com and key sk-0123456789abcdefghij end.";
    let findings = RedactionEngine::scan(text);

    c.bench_function("redaction_apply", |b| {
        b.iter(|| {
            black_box(RedactionEngine::apply(
                black_box(text),
                black_box(&findings),
            ))
        });
    });
}

criterion_group!(
    benches,
    bench_audit_append,
    bench_audit_verify_integrity,
    bench_fuel_record_spend,
    bench_fuel_simulate_cost,
    bench_anomaly_detector,
    bench_manifest_parse,
    bench_redaction_scan,
    bench_redaction_process_prompt,
    bench_redaction_apply,
);
criterion_main!(benches);
