//! Vercel Deployments API implementation.
//!
//! Flow:
//! 1. Create deployment: POST /v13/deployments with files array
//! 2. Poll GET /v13/deployments/{id} until state is "READY"
//!
//! API base: https://api.vercel.com
//! Auth: Bearer token in Authorization header
//!
//! For static file deployment, each file is sent as base64-encoded data.

use super::{
    now_iso8601, Credentials, DeployError, DeployFile, DeployGovernance, DeployResult, SiteInfo,
};

const API_BASE: &str = "https://api.vercel.com";

fn auth_header(creds: &Credentials) -> String {
    format!("Bearer {}", creds.token)
}

/// Create a deployment on Vercel (also serves as "create project" for new sites).
///
/// Vercel creates the project automatically on first deployment.
pub async fn deploy(
    project_name: &str,
    files: &[DeployFile],
    creds: &Credentials,
    client: &reqwest::Client,
    governance: &DeployGovernance,
) -> Result<DeployResult, DeployError> {
    governance.check()?;
    let start = std::time::Instant::now();

    // Build files array with base64-encoded content
    let files_payload: Vec<serde_json::Value> = files
        .iter()
        .map(|f| {
            use base64::Engine;
            let encoded = base64::engine::general_purpose::STANDARD.encode(&f.content);
            serde_json::json!({
                "file": f.path,
                "data": encoded,
                "encoding": "base64",
            })
        })
        .collect();

    let body = serde_json::json!({
        "name": project_name,
        "files": files_payload,
        "projectSettings": {
            "framework": null,
            "buildCommand": "",
            "outputDirectory": "",
        },
        "target": "production",
    });

    let url = format!("{API_BASE}/v13/deployments");
    let resp = client
        .post(&url)
        .header("Authorization", auth_header(creds))
        .json(&body)
        .send()
        .await
        .map_err(|e| DeployError::Network(e.to_string()))?;

    let status = resp.status().as_u16();
    if status == 401 || status == 403 {
        return Err(DeployError::InvalidToken);
    }
    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        if body.contains("already used") || body.contains("DEPLOYMENT_NAME_CONFLICT") {
            return Err(DeployError::SiteNameTaken(project_name.to_string()));
        }
        return Err(DeployError::ProviderApi {
            status,
            message: body,
        });
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| DeployError::Network(e.to_string()))?;

    let deploy_id = json["id"].as_str().unwrap_or("").to_string();
    let deploy_url = json["url"].as_str().unwrap_or("").to_string();

    // Poll until READY (max 60 seconds)
    let mut live_url = if deploy_url.starts_with("http") {
        deploy_url.clone()
    } else {
        format!("https://{deploy_url}")
    };

    for _ in 0..60 {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        let poll_url = format!("{API_BASE}/v13/deployments/{deploy_id}");
        let resp = client
            .get(&poll_url)
            .header("Authorization", auth_header(creds))
            .send()
            .await
            .map_err(|e| DeployError::Network(e.to_string()))?;

        if resp.status().is_success() {
            let json: serde_json::Value = resp
                .json()
                .await
                .map_err(|e| DeployError::Network(e.to_string()))?;
            let ready_state = json["readyState"].as_str().unwrap_or("");
            if ready_state == "READY" {
                if let Some(url) = json["url"].as_str() {
                    live_url = if url.starts_with("http") {
                        url.to_string()
                    } else {
                        format!("https://{url}")
                    };
                }
                break;
            }
            if ready_state == "ERROR" {
                return Err(DeployError::ProviderApi {
                    status: 500,
                    message: json["errorMessage"]
                        .as_str()
                        .unwrap_or("deployment failed")
                        .to_string(),
                });
            }
        }
    }

    let build_hash = super::compute_build_hash(files);
    Ok(DeployResult {
        deploy_id,
        url: live_url,
        provider: "vercel".into(),
        site_id: project_name.to_string(),
        timestamp: now_iso8601(),
        build_hash,
        duration_ms: start.elapsed().as_millis() as u64,
    })
}

/// List Vercel projects for the authenticated user.
pub async fn list_sites(
    creds: &Credentials,
    client: &reqwest::Client,
) -> Result<Vec<SiteInfo>, DeployError> {
    let url = format!("{API_BASE}/v9/projects");
    let resp = client
        .get(&url)
        .header("Authorization", auth_header(creds))
        .send()
        .await
        .map_err(|e| DeployError::Network(e.to_string()))?;

    let status = resp.status().as_u16();
    if status == 401 || status == 403 {
        return Err(DeployError::InvalidToken);
    }
    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(DeployError::ProviderApi {
            status,
            message: body,
        });
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| DeployError::Network(e.to_string()))?;

    let projects = json["projects"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|p| {
                    let name = p["name"].as_str().unwrap_or("").to_string();
                    SiteInfo {
                        id: p["id"].as_str().unwrap_or("").to_string(),
                        name: name.clone(),
                        url: format!("https://{name}.vercel.app"),
                        provider: "vercel".into(),
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(projects)
}

/// Rollback not natively supported by Vercel API in a single call.
/// Instead, we redeploy a previous deployment's files.
pub async fn rollback(
    project_name: &str,
    deploy_id: &str,
    creds: &Credentials,
    client: &reqwest::Client,
    governance: &DeployGovernance,
) -> Result<DeployResult, DeployError> {
    governance.check()?;
    // Vercel doesn't have a native rollback endpoint.
    // The approach is to "promote" a previous deployment to production.
    // Vercel v13 doesn't have a direct promote, so we create a new deploy
    // pointing to the previous deployment's alias.
    let _ = (project_name, deploy_id, creds, client);
    Err(DeployError::Other(
        "Vercel rollback requires redeployment — use the Vercel dashboard for now".into(),
    ))
}

/// Check if a Vercel token is valid.
pub async fn check_token(
    creds: &Credentials,
    client: &reqwest::Client,
) -> Result<bool, DeployError> {
    let url = format!("{API_BASE}/v2/user");
    let resp = client
        .get(&url)
        .header("Authorization", auth_header(creds))
        .send()
        .await
        .map_err(|e| DeployError::Network(e.to_string()))?;

    Ok(resp.status().is_success())
}

/// Build the deploy request body for testing.
pub fn build_deploy_request_body(project_name: &str, files: &[DeployFile]) -> serde_json::Value {
    use base64::Engine;
    let files_payload: Vec<serde_json::Value> = files
        .iter()
        .map(|f| {
            let encoded = base64::engine::general_purpose::STANDARD.encode(&f.content);
            serde_json::json!({
                "file": f.path,
                "data": encoded,
                "encoding": "base64",
            })
        })
        .collect();

    serde_json::json!({
        "name": project_name,
        "files": files_payload,
        "projectSettings": {
            "framework": null,
            "buildCommand": "",
            "outputDirectory": "",
        },
        "target": "production",
    })
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_files() -> Vec<DeployFile> {
        vec![
            DeployFile {
                path: "index.html".into(),
                content: b"<html>vercel</html>".to_vec(),
                hash: "v-hash-1".into(),
            },
            DeployFile {
                path: "app.js".into(),
                content: b"console.log('hi')".to_vec(),
                hash: "v-hash-2".into(),
            },
        ]
    }

    fn sample_creds() -> Credentials {
        Credentials {
            provider: "vercel".into(),
            token: "vercel-token-abc".into(),
            account_id: None,
            expires_at: None,
        }
    }

    #[test]
    fn test_vercel_deploy_request_format() {
        let files = sample_files();
        let body = build_deploy_request_body("my-site", &files);

        assert_eq!(body["name"], "my-site");
        assert_eq!(body["target"], "production");

        let files_arr = body["files"].as_array().unwrap();
        assert_eq!(files_arr.len(), 2);
        assert_eq!(files_arr[0]["file"], "index.html");
        assert_eq!(files_arr[0]["encoding"], "base64");
        // Verify base64 encoding is present
        assert!(!files_arr[0]["data"].as_str().unwrap().is_empty());
        assert_eq!(files_arr[1]["file"], "app.js");
    }

    #[test]
    fn test_vercel_auth_header() {
        let creds = sample_creds();
        assert_eq!(auth_header(&creds), "Bearer vercel-token-abc");
    }

    #[test]
    fn test_vercel_base64_encoding() {
        use base64::Engine;
        let data = b"<html>test</html>";
        let encoded = base64::engine::general_purpose::STANDARD.encode(data);
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&encoded)
            .unwrap();
        assert_eq!(&decoded, data);
    }

    /// Live integration test — runs when VERCEL_TOKEN env var is set.
    #[test]
    fn test_vercel_deploy_live() {
        let token = match std::env::var("VERCEL_TOKEN") {
            Ok(t) if !t.is_empty() => t,
            _ => {
                eprintln!("SKIP: VERCEL_TOKEN not set");
                return;
            }
        };

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let client = reqwest::Client::new();
            let creds = Credentials {
                provider: "vercel".into(),
                token,
                account_id: None,
                expires_at: None,
            };

            let valid = check_token(&creds, &client).await.unwrap();
            assert!(valid, "Token is not valid");

            let project_name = format!(
                "nexus-test-{}",
                uuid::Uuid::new_v4().to_string().split('-').next().unwrap()
            );
            let files = vec![DeployFile {
                path: "index.html".into(),
                content: b"<!DOCTYPE html><html><body><h1>Vercel Test</h1></body></html>".to_vec(),
                hash: super::super::sha256_hex(
                    b"<!DOCTYPE html><html><body><h1>Vercel Test</h1></body></html>",
                ),
            }];

            let gov = DeployGovernance {
                agent_id: uuid::Uuid::nil(),
                capabilities: vec!["deploy.execute".into()],
                fuel_budget_usd: 1.0,
            };

            let result = deploy(&project_name, &files, &creds, &client, &gov)
                .await
                .unwrap();
            assert!(!result.url.is_empty());
            println!("Deployed to: {}", result.url);
        });
    }
}
