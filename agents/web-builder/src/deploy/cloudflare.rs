//! Cloudflare Pages Direct Upload API implementation.
//!
//! Flow:
//! 1. Create project (if first deploy): POST /client/v4/accounts/{account_id}/pages/projects
//! 2. Create deployment: POST /client/v4/accounts/{account_id}/pages/projects/{name}/deployments
//!    with files as multipart form upload
//! 3. Poll until deployment status is "active"
//!
//! API base: https://api.cloudflare.com
//! Auth: Bearer token in Authorization header
//! Requires: account_id (stored alongside token in Credentials)

use super::{
    now_iso8601, Credentials, DeployError, DeployFile, DeployGovernance, DeployResult, SiteInfo,
};

const API_BASE: &str = "https://api.cloudflare.com";

fn auth_header(creds: &Credentials) -> String {
    format!("Bearer {}", creds.token)
}

fn require_account_id(creds: &Credentials) -> Result<&str, DeployError> {
    creds
        .account_id
        .as_deref()
        .filter(|id| !id.is_empty())
        .ok_or(DeployError::AccountIdRequired)
}

/// Create a new Cloudflare Pages project.
///
/// Requires `deploy.execute` capability — modifies production infrastructure.
pub async fn create_site(
    name: &str,
    creds: &Credentials,
    client: &reqwest::Client,
    governance: &DeployGovernance,
) -> Result<SiteInfo, DeployError> {
    governance.check()?;
    eprintln!(
        "[nexus-deploy][governance] cloudflare::create_site name={name} agent={}",
        governance.agent_id
    );
    let account_id = require_account_id(creds)?;
    let url = format!("{API_BASE}/client/v4/accounts/{account_id}/pages/projects");

    let body = serde_json::json!({
        "name": name,
        "production_branch": "main",
    });

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
        if body.contains("already exists") || body.contains("duplicat") {
            return Err(DeployError::SiteNameTaken(name.to_string()));
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

    let result = &json["result"];
    Ok(SiteInfo {
        id: result["name"].as_str().unwrap_or(name).to_string(),
        name: result["name"].as_str().unwrap_or(name).to_string(),
        url: result["subdomain"]
            .as_str()
            .map(|s| format!("https://{s}"))
            .unwrap_or_else(|| format!("https://{name}.pages.dev")),
        provider: "cloudflare".into(),
    })
}

/// List Cloudflare Pages projects.
pub async fn list_sites(
    creds: &Credentials,
    client: &reqwest::Client,
) -> Result<Vec<SiteInfo>, DeployError> {
    eprintln!("[nexus-deploy][governance] cloudflare::list_sites");
    let account_id = require_account_id(creds)?;
    let url = format!("{API_BASE}/client/v4/accounts/{account_id}/pages/projects");

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

    let sites = json["result"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|p| SiteInfo {
                    id: p["name"].as_str().unwrap_or("").to_string(),
                    name: p["name"].as_str().unwrap_or("").to_string(),
                    url: p["subdomain"]
                        .as_str()
                        .map(|s| format!("https://{s}"))
                        .unwrap_or_default(),
                    provider: "cloudflare".into(),
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(sites)
}

/// Deploy files to Cloudflare Pages via direct upload.
///
/// Uses multipart form data: each file is a form field with the relative path as key.
/// Requires `deploy.execute` capability — this pushes to production infrastructure.
pub async fn deploy(
    project_name: &str,
    files: &[DeployFile],
    creds: &Credentials,
    client: &reqwest::Client,
    governance: &DeployGovernance,
) -> Result<DeployResult, DeployError> {
    governance.check()?;
    eprintln!(
        "[nexus-deploy][governance] cloudflare::deploy project={project_name} files={} agent={}",
        files.len(),
        governance.agent_id,
    );
    let start = std::time::Instant::now();
    let account_id = require_account_id(creds)?;

    let url = format!(
        "{API_BASE}/client/v4/accounts/{account_id}/pages/projects/{project_name}/deployments"
    );

    // Build multipart form
    let mut form = reqwest::multipart::Form::new();
    for file in files {
        let part = reqwest::multipart::Part::bytes(file.content.clone())
            .file_name(file.path.clone())
            .mime_str("application/octet-stream")
            .map_err(|e| DeployError::Other(e.to_string()))?;
        form = form.part(file.path.clone(), part);
    }

    let resp = client
        .post(&url)
        .header("Authorization", auth_header(creds))
        .multipart(form)
        .send()
        .await
        .map_err(|e| DeployError::Network(e.to_string()))?;

    let status = resp.status().as_u16();
    if status == 401 || status == 403 {
        return Err(DeployError::InvalidToken);
    }
    if status == 404 {
        return Err(DeployError::SiteNotFound(project_name.to_string()));
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

    let result = &json["result"];
    let deploy_id = result["id"].as_str().unwrap_or("").to_string();
    let deploy_url = result["url"].as_str().unwrap_or("").to_string();

    // Poll until active (max 60 seconds)
    let mut live_url = deploy_url;
    for _ in 0..60 {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        let poll_url = format!(
            "{API_BASE}/client/v4/accounts/{account_id}/pages/projects/{project_name}/deployments/{deploy_id}"
        );
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
            let stage = json["result"]["latest_stage"]["name"]
                .as_str()
                .unwrap_or("");
            let stage_status = json["result"]["latest_stage"]["status"]
                .as_str()
                .unwrap_or("");
            if stage == "deploy" && stage_status == "success" {
                if let Some(url) = json["result"]["url"].as_str() {
                    live_url = url.to_string();
                }
                break;
            }
            if stage_status == "failure" {
                return Err(DeployError::ProviderApi {
                    status: 500,
                    message: "Cloudflare deployment failed".into(),
                });
            }
        }
    }

    let build_hash = super::compute_build_hash(files);
    Ok(DeployResult {
        deploy_id,
        url: live_url,
        provider: "cloudflare".into(),
        site_id: project_name.to_string(),
        timestamp: now_iso8601(),
        build_hash,
        duration_ms: start.elapsed().as_millis() as u64,
    })
}

/// Rollback Cloudflare Pages to a previous deployment.
///
/// Requires `deploy.execute` capability — modifies production infrastructure.
pub async fn rollback(
    project_name: &str,
    deploy_id: &str,
    creds: &Credentials,
    client: &reqwest::Client,
    governance: &DeployGovernance,
) -> Result<DeployResult, DeployError> {
    governance.check()?;
    eprintln!(
        "[nexus-deploy][governance] cloudflare::rollback project={project_name} deploy_id={deploy_id} agent={}",
        governance.agent_id,
    );
    let start = std::time::Instant::now();
    let account_id = require_account_id(creds)?;

    let url = format!(
        "{API_BASE}/client/v4/accounts/{account_id}/pages/projects/{project_name}/deployments/{deploy_id}/rollback"
    );

    let resp = client
        .post(&url)
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

    Ok(DeployResult {
        deploy_id: json["result"]["id"]
            .as_str()
            .unwrap_or(deploy_id)
            .to_string(),
        url: json["result"]["url"].as_str().unwrap_or("").to_string(),
        provider: "cloudflare".into(),
        site_id: project_name.to_string(),
        timestamp: now_iso8601(),
        build_hash: String::new(),
        duration_ms: start.elapsed().as_millis() as u64,
    })
}

/// Check if a Cloudflare token is valid.
pub async fn check_token(
    creds: &Credentials,
    client: &reqwest::Client,
) -> Result<bool, DeployError> {
    let url = format!("{API_BASE}/client/v4/user/tokens/verify");
    let resp = client
        .get(&url)
        .header("Authorization", auth_header(creds))
        .send()
        .await
        .map_err(|e| DeployError::Network(e.to_string()))?;

    Ok(resp.status().is_success())
}

/// Build the deploy request metadata for testing.
pub fn build_deploy_metadata(files: &[DeployFile]) -> serde_json::Value {
    serde_json::json!({
        "file_count": files.len(),
        "files": files.iter().map(|f| serde_json::json!({
            "path": f.path,
            "hash": f.hash,
            "size": f.content.len(),
        })).collect::<Vec<_>>(),
    })
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_creds_no_account() -> Credentials {
        Credentials {
            provider: "cloudflare".into(),
            token: "cf-token-123".into(),
            account_id: None,
            expires_at: None,
        }
    }

    fn sample_creds_with_account() -> Credentials {
        Credentials {
            provider: "cloudflare".into(),
            token: "cf-token-123".into(),
            account_id: Some("acct-abc".into()),
            expires_at: None,
        }
    }

    fn sample_files() -> Vec<DeployFile> {
        vec![
            DeployFile {
                path: "index.html".into(),
                content: b"<html>cf</html>".to_vec(),
                hash: "cf-hash-1".into(),
            },
            DeployFile {
                path: "style.css".into(),
                content: b"body{}".to_vec(),
                hash: "cf-hash-2".into(),
            },
        ]
    }

    #[test]
    fn test_cloudflare_requires_account_id() {
        let creds = sample_creds_no_account();
        let result = require_account_id(&creds);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("account"), "got: {err}");
    }

    #[test]
    fn test_cloudflare_account_id_accepted() {
        let creds = sample_creds_with_account();
        let result = require_account_id(&creds);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "acct-abc");
    }

    #[test]
    fn test_cloudflare_deploy_request_format() {
        let files = sample_files();
        let meta = build_deploy_metadata(&files);
        assert_eq!(meta["file_count"], 2);
        let file_list = meta["files"].as_array().unwrap();
        assert_eq!(file_list[0]["path"], "index.html");
        assert_eq!(file_list[1]["path"], "style.css");
    }

    #[test]
    fn test_cloudflare_auth_header() {
        let creds = sample_creds_with_account();
        assert_eq!(auth_header(&creds), "Bearer cf-token-123");
    }

    /// Live integration test — runs when CF_TOKEN + CF_ACCOUNT_ID env vars are set.
    #[test]
    fn test_cloudflare_deploy_live() {
        let token = match std::env::var("CF_TOKEN") {
            Ok(t) if !t.is_empty() => t,
            _ => {
                eprintln!("SKIP: CF_TOKEN not set");
                return;
            }
        };
        let account_id = match std::env::var("CF_ACCOUNT_ID") {
            Ok(a) if !a.is_empty() => a,
            _ => {
                eprintln!("SKIP: CF_ACCOUNT_ID not set");
                return;
            }
        };

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let client = reqwest::Client::new();
            let creds = Credentials {
                provider: "cloudflare".into(),
                token,
                account_id: Some(account_id),
                expires_at: None,
            };

            let valid = check_token(&creds, &client).await.unwrap();
            assert!(valid, "Token is not valid");

            let project_name = format!(
                "nexus-test-{}",
                uuid::Uuid::new_v4().to_string().split('-').next().unwrap()
            );
            let gov = DeployGovernance {
                agent_id: uuid::Uuid::nil(),
                capabilities: vec!["deploy.execute".into()],
                fuel_budget_usd: 1.0,
            };
            let site = create_site(&project_name, &creds, &client, &gov)
                .await
                .unwrap();
            assert!(!site.id.is_empty());

            let files = vec![DeployFile {
                path: "index.html".into(),
                content: b"<!DOCTYPE html><html><body><h1>CF Test</h1></body></html>".to_vec(),
                hash: super::super::sha256_hex(
                    b"<!DOCTYPE html><html><body><h1>CF Test</h1></body></html>",
                ),
            }];

            let result = deploy(&project_name, &files, &creds, &client, &gov)
                .await
                .unwrap();
            assert!(!result.url.is_empty());
            println!("Deployed to: {}", result.url);
        });
    }
}
