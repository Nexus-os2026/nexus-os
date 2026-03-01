use nexus_cli::create_agent_from_manifest_str;

#[test]
fn test_cli_create_agent() {
    let manifest = r#"
name = "my-social-poster"
version = "0.1.0"
capabilities = ["web.search", "llm.query", "fs.read"]
fuel_budget = 10000
schedule = "*/10 * * * *"
llm_model = "claude-sonnet-4-5"
"#;

    let output = create_agent_from_manifest_str(manifest)
        .expect("valid manifest should produce create confirmation");
    assert_eq!(
        output,
        "Agent 'my-social-poster' created successfully (fuel: 10000)"
    );
}
