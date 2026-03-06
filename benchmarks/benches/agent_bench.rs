use criterion::{black_box, criterion_group, criterion_main, Criterion};
use nexus_kernel::autonomy::AutonomyLevel;
use nexus_kernel::manifest::parse_manifest;

fn bench_manifest_parse_varied(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent_manifest_parse");

    let minimal = r#"
name = "min-agent"
version = "0.1.0"
capabilities = ["web.search"]
fuel_budget = 100
"#;

    let full = r#"
name = "full-agent"
version = "1.0.0"
capabilities = ["web.search", "llm.query", "fs.read", "fs.write", "social.post"]
fuel_budget = 500000
autonomy_level = 3
consent_policy_path = "/etc/nexus/consent.toml"
requester_id = "orchestrator.main"
schedule = "*/5 * * * *"
llm_model = "claude-sonnet-4-5"
fuel_period_id = "2026-03"
monthly_fuel_cap = 1000000
"#;

    group.bench_function("minimal", |b| {
        b.iter(|| black_box(parse_manifest(black_box(minimal))));
    });

    group.bench_function("full", |b| {
        b.iter(|| black_box(parse_manifest(black_box(full))));
    });

    group.finish();
}

fn bench_autonomy_level_lookup(c: &mut Criterion) {
    c.bench_function("autonomy_level_from_numeric", |b| {
        b.iter(|| {
            for level in 0..=5u8 {
                black_box(AutonomyLevel::from_numeric(level));
            }
        });
    });
}

criterion_group!(benches, bench_manifest_parse_varied, bench_autonomy_level_lookup,);
criterion_main!(benches);
