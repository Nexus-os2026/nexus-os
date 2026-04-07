//! Netlify Deploy API implementation.
//!
//! Flow:
//! 1. Create deploy with file digest map → POST /api/v1/sites/{site_id}/deploys
//! 2. Upload required files → PUT /api/v1/deploys/{deploy_id}/files/{path}
//! 3. Poll deploy status until "ready"
//!
//! API base: https://api.netlify.com
//! Auth: Bearer token in Authorization header

use super::{
    now_iso8601, Credentials, DeployError, DeployFile, DeployGovernance, DeployResult, SiteInfo,
};
use std::collections::HashMap;

const API_BASE: &str = "https://api.netlify.com";

/// Build authorization header value.
fn auth_header(creds: &Credentials) -> String {
    format!("Bearer {}", creds.token)
}

/// Create a new site on Netlify.
///
/// POST /api/v1/sites with { "name": "<name>" }
/// Returns site_id and default URL.
pub async fn create_site(
    name: &str,
    creds: &Credentials,
    client: &reqwest::Client,
    governance: &DeployGovernance,
) -> Result<SiteInfo, DeployError> {
    governance.check()?;
    eprintln!(
        "[nexus-deploy][governance] netlify::create_site site={name} agent={}",
        governance.agent_id
    );
    let url = format!("{API_BASE}/api/v1/sites");
    let body = serde_json::json!({ "name": name });

    let resp = client
        .post(&url)
        .header("Authorization", auth_header(creds))
        .json(&body)
        .send()
        .await
        .map_err(|e| DeployError::Network(e.to_string()))?;

    let status = resp.status().as_u16();
    if status == 401 {
        return Err(DeployError::InvalidToken);
    }
    if status == 422 {
        return Err(DeployError::SiteNameTaken(name.to_string()));
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

    Ok(SiteInfo {
        id: json["id"].as_str().unwrap_or("").to_string(),
        name: json["name"].as_str().unwrap_or(name).to_string(),
        url: json["ssl_url"]
            .as_str()
            .or_else(|| json["url"].as_str())
            .unwrap_or("")
            .to_string(),
        provider: "netlify".into(),
    })
}

/// List sites for the authenticated Netlify user.
pub async fn list_sites(
    creds: &Credentials,
    client: &reqwest::Client,
) -> Result<Vec<SiteInfo>, DeployError> {
    let url = format!("{API_BASE}/api/v1/sites");
    let resp = client
        .get(&url)
        .header("Authorization", auth_header(creds))
        .send()
        .await
        .map_err(|e| DeployError::Network(e.to_string()))?;

    let status = resp.status().as_u16();
    if status == 401 {
        return Err(DeployError::InvalidToken);
    }
    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(DeployError::ProviderApi {
            status,
            message: body,
        });
    }

    let json: Vec<serde_json::Value> = resp
        .json()
        .await
        .map_err(|e| DeployError::Network(e.to_string()))?;

    Ok(json
        .iter()
        .map(|s| SiteInfo {
            id: s["id"].as_str().unwrap_or("").to_string(),
            name: s["name"].as_str().unwrap_or("").to_string(),
            url: s["ssl_url"]
                .as_str()
                .or_else(|| s["url"].as_str())
                .unwrap_or("")
                .to_string(),
            provider: "netlify".into(),
        })
        .collect())
}

/// Deploy files to a Netlify site.
///
/// 1. POST file digest map to create a deploy
/// 2. Upload each required file
/// 3. Poll until deploy is "ready"
pub async fn deploy(
    site_id: &str,
    files: &[DeployFile],
    creds: &Credentials,
    client: &reqwest::Client,
    governance: &DeployGovernance,
) -> Result<DeployResult, DeployError> {
    governance.check()?;
    eprintln!(
        "[nexus-deploy][governance] netlify::deploy site={site_id} agent={}",
        governance.agent_id
    );
    let start = std::time::Instant::now();

    // Step 1: Create deploy with file digests
    let mut file_digests: HashMap<String, String> = HashMap::new();
    for f in files {
        let path = if f.path.starts_with('/') {
            f.path.clone()
        } else {
            format!("/{}", f.path)
        };
        file_digests.insert(path, f.hash.clone());
    }

    let deploy_body = serde_json::json!({
        "files": file_digests,
    });

    let url = format!("{API_BASE}/api/v1/sites/{site_id}/deploys");
    let resp = client
        .post(&url)
        .header("Authorization", auth_header(creds))
        .json(&deploy_body)
        .send()
        .await
        .map_err(|e| DeployError::Network(e.to_string()))?;

    let status = resp.status().as_u16();
    if status == 401 {
        return Err(DeployError::InvalidToken);
    }
    if status == 404 {
        return Err(DeployError::SiteNotFound(site_id.to_string()));
    }
    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(DeployError::ProviderApi {
            status,
            message: body,
        });
    }

    let deploy_json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| DeployError::Network(e.to_string()))?;

    let deploy_id = deploy_json["id"].as_str().unwrap_or("").to_string();

    // Step 2: Upload files that Netlify needs
    // Netlify returns "required" array — files it doesn't already have.
    // For simplicity, upload all files (dedup optimization later).
    let required: Vec<String> = deploy_json["required"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    for file in files {
        // Only upload files Netlify needs (by hash), or all if required list is empty
        if !required.is_empty() && !required.contains(&file.hash) {
            continue;
        }

        let file_path = if file.path.starts_with('/') {
            file.path.clone()
        } else {
            format!("/{}", file.path)
        };

        let upload_url = format!("{API_BASE}/api/v1/deploys/{deploy_id}/files{file_path}");

        let resp = client
            .put(&upload_url)
            .header("Authorization", auth_header(creds))
            .header("Content-Type", "application/octet-stream")
            .body(file.content.clone())
            .send()
            .await
            .map_err(|e| DeployError::Network(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(DeployError::ProviderApi {
                status,
                message: format!("upload {}: {body}", file.path),
            });
        }
    }

    // Step 3: Poll until ready (max 60 seconds)
    let poll_url = format!("{API_BASE}/api/v1/deploys/{deploy_id}");
    let mut live_url = String::new();
    for _ in 0..60 {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

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
            let state = json["state"].as_str().unwrap_or("");
            if state == "ready" {
                live_url = json["ssl_url"]
                    .as_str()
                    .or_else(|| json["url"].as_str())
                    .unwrap_or("")
                    .to_string();
                break;
            }
            if state == "error" {
                return Err(DeployError::ProviderApi {
                    status: 500,
                    message: json["error_message"]
                        .as_str()
                        .unwrap_or("deploy failed")
                        .to_string(),
                });
            }
        }
    }

    let build_hash = super::compute_build_hash(files);
    Ok(DeployResult {
        deploy_id,
        url: live_url,
        provider: "netlify".into(),
        site_id: site_id.to_string(),
        timestamp: now_iso8601(),
        build_hash,
        duration_ms: start.elapsed().as_millis() as u64,
    })
}

/// Rollback a Netlify site to a previous deploy.
///
/// POST /api/v1/sites/{site_id}/rollback
pub async fn rollback(
    site_id: &str,
    _deploy_id: &str,
    creds: &Credentials,
    client: &reqwest::Client,
    governance: &DeployGovernance,
) -> Result<DeployResult, DeployError> {
    governance.check()?;
    eprintln!(
        "[nexus-deploy][governance] netlify::rollback site={site_id} agent={}",
        governance.agent_id
    );
    let start = std::time::Instant::now();
    let url = format!("{API_BASE}/api/v1/sites/{site_id}/rollback");

    let resp = client
        .post(&url)
        .header("Authorization", auth_header(creds))
        .send()
        .await
        .map_err(|e| DeployError::Network(e.to_string()))?;

    let status = resp.status().as_u16();
    if status == 401 {
        return Err(DeployError::InvalidToken);
    }
    if status == 404 {
        return Err(DeployError::SiteNotFound(site_id.to_string()));
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
        deploy_id: json["id"].as_str().unwrap_or("").to_string(),
        url: json["ssl_url"]
            .as_str()
            .or_else(|| json["url"].as_str())
            .unwrap_or("")
            .to_string(),
        provider: "netlify".into(),
        site_id: site_id.to_string(),
        timestamp: now_iso8601(),
        build_hash: String::new(),
        duration_ms: start.elapsed().as_millis() as u64,
    })
}

/// Check if a Netlify token is valid by making a lightweight API call.
pub async fn check_token(
    creds: &Credentials,
    client: &reqwest::Client,
) -> Result<bool, DeployError> {
    let url = format!("{API_BASE}/api/v1/user");
    let resp = client
        .get(&url)
        .header("Authorization", auth_header(creds))
        .send()
        .await
        .map_err(|e| DeployError::Network(e.to_string()))?;

    Ok(resp.status().is_success())
}

// ─── Request Format Helpers (for testing) ──────────────────────────────────

/// Build the deploy request body for testing.
pub fn build_deploy_request_body(files: &[DeployFile]) -> serde_json::Value {
    let mut file_digests: HashMap<String, String> = HashMap::new();
    for f in files {
        let path = if f.path.starts_with('/') {
            f.path.clone()
        } else {
            format!("/{}", f.path)
        };
        file_digests.insert(path, f.hash.clone());
    }
    serde_json::json!({ "files": file_digests })
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_files() -> Vec<DeployFile> {
        vec![
            DeployFile {
                path: "index.html".into(),
                content: b"<html>hi</html>".to_vec(),
                hash: "aaa111".into(),
            },
            DeployFile {
                path: "assets/style.css".into(),
                content: b"body{}".to_vec(),
                hash: "bbb222".into(),
            },
        ]
    }

    fn sample_creds() -> Credentials {
        Credentials {
            provider: "netlify".into(),
            token: "test-token-123".into(),
            account_id: None,
            expires_at: None,
        }
    }

    #[test]
    fn test_netlify_deploy_request_format() {
        let files = sample_files();
        let body = build_deploy_request_body(&files);

        let files_map = body["files"].as_object().unwrap();
        assert_eq!(files_map.len(), 2);
        assert_eq!(files_map["/index.html"].as_str().unwrap(), "aaa111");
        assert_eq!(files_map["/assets/style.css"].as_str().unwrap(), "bbb222");
    }

    #[test]
    fn test_netlify_auth_header() {
        let creds = sample_creds();
        let header = auth_header(&creds);
        assert_eq!(header, "Bearer test-token-123");
    }

    #[test]
    fn test_netlify_handles_401() {
        // Simulate the error path
        let err = DeployError::InvalidToken;
        let msg = err.to_string();
        assert!(msg.contains("invalid token") || msg.contains("expired"));
    }

    #[test]
    fn test_netlify_handles_422() {
        let err = DeployError::SiteNameTaken("mysite".into());
        let msg = err.to_string();
        assert!(msg.contains("mysite"));
        assert!(msg.contains("taken"));
    }

    /// Live integration test — runs when NETLIFY_TOKEN env var is set.
    #[test]
    fn test_netlify_deploy_live() {
        let token = match std::env::var("NETLIFY_TOKEN") {
            Ok(t) if !t.is_empty() => t,
            _ => {
                eprintln!("SKIP: NETLIFY_TOKEN not set");
                return;
            }
        };

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let client = reqwest::Client::new();
            let creds = Credentials {
                provider: "netlify".into(),
                token,
                account_id: None,
                expires_at: None,
            };
            let gov = DeployGovernance {
                agent_id: uuid::Uuid::nil(),
                capabilities: vec!["deploy.execute".into()],
                fuel_budget_usd: 1.0,
            };

            // Check token validity
            let valid = check_token(&creds, &client).await.unwrap();
            assert!(valid, "Token is not valid");

            // Create a test site
            let site_name = format!(
                "nexus-test-{}",
                uuid::Uuid::new_v4().to_string().split('-').next().unwrap()
            );
            let site = create_site(&site_name, &creds, &client, &gov)
                .await
                .unwrap();
            assert!(!site.id.is_empty());

            // Deploy a simple page
            let files = vec![DeployFile {
                path: "index.html".into(),
                content: b"<!DOCTYPE html><html><body><h1>Nexus Test Deploy</h1></body></html>"
                    .to_vec(),
                hash: super::super::sha256_hex(
                    b"<!DOCTYPE html><html><body><h1>Nexus Test Deploy</h1></body></html>",
                ),
            }];

            let result = deploy(&site.id, &files, &creds, &client, &gov)
                .await
                .unwrap();
            assert!(!result.url.is_empty());
            assert_eq!(result.provider, "netlify");
            println!("Deployed to: {}", result.url);
        });
    }
}
