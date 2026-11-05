//! Immutable baseline evidence, distinct from the current product verdict.
#![deny(unsafe_code)]

#[test]
fn executed_baseline_contains_observations_not_planned_matrix_keys() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/session/executed-baseline.json");
    let output = std::process::Command::new("python3")
        .args([
            "-c",
            r#"
import json,sys
with open(sys.argv[1]) as source:
    evidence=json.load(source)
cases=evidence['scheduled_cases']
assert len(cases)==583
assert len({case['case'] for case in cases})==len(cases)
assert all(case['observed'] and type(case['conforms']) is bool for case in cases)
assert sum(case['conforms'] for case in cases)==287
assert len(evidence['verified_native_deaths'])==22
assert len(evidence['workspace_failed_targets'])==12
assert evidence['counts']['executed_schedules']==len(cases)
print('583 executed observations verified; planned matrix and model counts separate')
"#,
        ])
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
