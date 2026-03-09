use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use nexus_kernel::audit::{AuditTrail, EventType};
use serde_json::json;
use uuid::Uuid;

fn bench_build_audit_chain_for_replay(c: &mut Criterion) {
    let mut group = c.benchmark_group("replay_build_chain");
    let agent_id = Uuid::new_v4();

    for count in [50, 500, 5000] {
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &n| {
            b.iter(|| {
                let mut trail = AuditTrail::new();
                for i in 0..n {
                    trail
                        .append_event(
                            agent_id,
                            EventType::ToolCall,
                            json!({"tool": "web.search", "seq": i}),
                        )
                        .expect("audit append");
                }
                black_box(trail.verify_integrity());
            });
        });
    }
    group.finish();
}

fn bench_serialize_audit_trail(c: &mut Criterion) {
    let mut group = c.benchmark_group("replay_serialize");
    let agent_id = Uuid::new_v4();

    for count in [50, 500] {
        let mut trail = AuditTrail::new();
        for i in 0..count {
            trail
                .append_event(
                    agent_id,
                    EventType::ToolCall,
                    json!({"tool": "web.search", "seq": i}),
                )
                .expect("audit append");
        }

        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            trail.events(),
            |b, events| {
                b.iter(|| black_box(serde_json::to_vec(events).unwrap()));
            },
        );
    }
    group.finish();
}

fn bench_deserialize_audit_events(c: &mut Criterion) {
    let mut group = c.benchmark_group("replay_deserialize");
    let agent_id = Uuid::new_v4();

    for count in [50, 500] {
        let mut trail = AuditTrail::new();
        for i in 0..count {
            trail
                .append_event(
                    agent_id,
                    EventType::ToolCall,
                    json!({"tool": "web.search", "seq": i}),
                )
                .expect("audit append");
        }
        let serialized = serde_json::to_vec(trail.events()).unwrap();

        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            &serialized,
            |b, data| {
                b.iter(|| {
                    black_box(
                        serde_json::from_slice::<Vec<nexus_kernel::audit::AuditEvent>>(data)
                            .unwrap(),
                    )
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_build_audit_chain_for_replay,
    bench_serialize_audit_trail,
    bench_deserialize_audit_events,
);
criterion_main!(benches);
