//! Deploy module — one-click static site deployment to Netlify, Cloudflare Pages, and Vercel.
//!
//! Provides a provider-agnostic trait, shared types, file collection from build output,
//! and deterministic build hashing for governance. Credentials are encrypted on disk.
//! Every deploy is logged to the governance audit trail with build hash and timestamp,
//! but credential tokens are NEVER included in logs or exports.

pub mod cloudflare;
pub mod credentials;
// Phase 7B: Deploy History + URL Management
pub mod diff;
pub mod history;
pub mod manifest;
pub mod netlify;
pub mod qr;
pub mod vercel;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use thiserror::Error;

// Re-export the legacy types so existing code doesn't break.
// The old deploy.rs had DeployProvider, DeploymentResult, Deployer, deploy_to.
// We preserve those names from the legacy module.
pub use legacy::{deploy_to, DeployProvider as LegacyDeployProvider, Deployer, DeploymentResult};

// ─── Errors ────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum DeployError {
    #[error("credential error: {0}")]
    Credential(String),
    #[error("provider API error ({status}): {message}")]
    ProviderApi { status: u16, message: String },
    #[error("site name already taken: {0}")]
    SiteNameTaken(String),
    #[error("site not found: {0}")]
    SiteNotFound(String),
    #[error("invalid token or expired credentials")]
    InvalidToken,
    #[error("account ID required for this provider")]
    AccountIdRequired,
    #[error("build output not found: {0}")]
    BuildOutputNotFound(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

// ─── Bounded provider API client (Final Gate decisions C and E) ─────────────

/// The longest one provider API request may take, from connecting until its
/// whole body is read.
pub const API_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// The most response bytes one provider API request reads into memory.
pub const MAX_API_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

/// The most bytes of a failed request's body kept for its error.
pub const MAX_API_ERROR_BODY_BYTES: usize = 64 * 1024;

/// The client for the provider API calls that carry the stored deploy token
/// (the desktop's token check and site listing): no redirect is followed
/// (reqwest would re-send a 307/308 request, token header included unless it
/// is Authorization, to another host), no Referer is sent, and each request
/// ends within `API_REQUEST_TIMEOUT`.
pub fn api_client() -> Result<reqwest::Client, DeployError> {
    api_client_with(API_REQUEST_TIMEOUT)
}

/// [`api_client`] with a given total timeout.
fn api_client_with(timeout: std::time::Duration) -> Result<reqwest::Client, DeployError> {
    reqwest::Client::builder()
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .referer(false)
        .build()
        .map_err(|e| DeployError::Network(e.without_url().to_string()))
}

/// At most `max` bytes of a provider response, parsed as JSON. A body
/// declared or streamed past `max` is refused without the rest being read;
/// errors name no URL.
pub(crate) async fn read_json_capped<T: serde::de::DeserializeOwned>(
    mut resp: reqwest::Response,
    max: usize,
) -> Result<T, DeployError> {
    let too_large = || DeployError::Network(format!("the response is larger than {max} bytes"));
    if resp
        .content_length()
        .is_some_and(|length| length > max as u64)
    {
        return Err(too_large());
    }
    let mut body = Vec::new();
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| DeployError::Network(e.without_url().to_string()))?
    {
        if chunk.len() > max - body.len() {
            return Err(too_large());
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|e| DeployError::Network(e.to_string()))
}

/// At most `max` bytes of a failed request's body, as text, for its error.
pub(crate) async fn error_text(mut resp: reqwest::Response, max: usize) -> String {
    let mut body = Vec::new();
    while let Ok(Some(chunk)) = resp.chunk().await {
        let room = max - body.len();
        body.extend_from_slice(&chunk[..chunk.len().min(room)]);
        if body.len() == max {
            break;
        }
    }
    String::from_utf8_lossy(&body).into_owned()
}

// ─── Shared Types ──────────────────────────────────────────────────────────

/// A single file to be deployed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployFile {
    /// Relative path, e.g., "index.html", "assets/style.css"
    pub path: String,
    /// File content as bytes
    #[serde(skip)]
    pub content: Vec<u8>,
    /// SHA-256 hex digest of this file's content
    pub hash: String,
}

/// Result of a successful deploy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployResult {
    pub deploy_id: String,
    pub url: String,
    pub provider: String,
    pub site_id: String,
    pub timestamp: String,
    pub build_hash: String,
    pub duration_ms: u64,
}

/// Information about a site on a hosting provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteInfo {
    pub id: String,
    pub name: String,
    pub url: String,
    pub provider: String,
}

/// Information about a specific deploy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployInfo {
    pub deploy_id: String,
    pub url: String,
    pub created_at: String,
    pub build_hash: String,
}

/// Credentials for a deploy provider. The token field is NEVER
/// included in Debug output, logs, or governance exports.
#[derive(Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub provider: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub token: String,
    /// Cloudflare requires account_id in addition to the token.
    pub account_id: Option<String>,
    pub expires_at: Option<String>,
}

// Custom Debug that redacts the token
impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("provider", &self.provider)
            .field("token", &"***REDACTED***")
            .field("account_id", &self.account_id)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

// ─── Governance ───────────────────────────────────────────────────────────

/// Governance context required for deploy operations.
///
/// Deploy is a Tier2+ governed operation: it pushes code to production
/// infrastructure under the user's cloud accounts. Callers must supply
/// this context to prove the operation is authorized.
#[derive(Debug, Clone)]
pub struct DeployGovernance {
    /// Agent or user that initiated the deploy.
    pub agent_id: uuid::Uuid,
    /// Capabilities held by the agent (must include `deploy.execute`).
    pub capabilities: Vec<String>,
    /// Maximum cost in USD this deploy may incur (bandwidth, compute).
    pub fuel_budget_usd: f64,
}

impl DeployGovernance {
    /// Verify the agent has the `deploy.execute` capability.
    pub fn check(&self) -> Result<(), DeployError> {
        if self
            .capabilities
            .iter()
            .any(|c| c == "deploy.execute" || c == "*")
        {
            Ok(())
        } else {
            Err(DeployError::Other(format!(
                "Agent {} lacks 'deploy.execute' capability",
                self.agent_id
            )))
        }
    }
}

// ─── File Collection ───────────────────────────────────────────────────────

/// Collect all static files from a build output directory into DeployFile vec.
pub fn collect_deploy_files(output_dir: &Path) -> Result<Vec<DeployFile>, DeployError> {
    if !output_dir.exists() {
        return Err(DeployError::BuildOutputNotFound(
            output_dir.display().to_string(),
        ));
    }

    let mut files = Vec::new();
    collect_recursive(output_dir, output_dir, &mut files)?;
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

fn collect_recursive(
    base: &Path,
    dir: &Path,
    files: &mut Vec<DeployFile>,
) -> Result<(), DeployError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_recursive(base, &path, files)?;
        } else {
            let relative = path
                .strip_prefix(base)
                .map_err(|e| DeployError::Other(e.to_string()))?
                .to_string_lossy()
                // Normalize to forward slashes for all platforms
                .replace('\\', "/");
            let content = std::fs::read(&path)?;
            let hash = sha256_hex(&content);
            files.push(DeployFile {
                path: relative,
                content,
                hash,
            });
        }
    }
    Ok(())
}

/// Compute a deterministic build hash from all deploy files.
/// Sorts by path, then hashes (path + content_hash) pairs.
pub fn compute_build_hash(files: &[DeployFile]) -> String {
    let mut hasher = Sha256::new();
    let mut sorted: Vec<&DeployFile> = files.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    for f in sorted {
        hasher.update(f.path.as_bytes());
        hasher.update(b":");
        hasher.update(f.hash.as_bytes());
        hasher.update(b"\n");
    }
    hex::encode(hasher.finalize())
}

/// SHA-256 hex digest of arbitrary bytes.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Get ISO-8601 timestamp (UTC).
pub fn now_iso8601() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    secs_to_iso8601(secs)
}

fn secs_to_iso8601(secs: u64) -> String {
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    let mut y = 1970i64;
    let mut remaining_days = days as i64;
    loop {
        let days_in_year = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
            366
        } else {
            365
        };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let month_days: [i64; 12] = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 0usize;
    for (i, &md) in month_days.iter().enumerate() {
        if remaining_days < md {
            m = i;
            break;
        }
        remaining_days -= md;
    }

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m + 1,
        remaining_days + 1,
        hours,
        minutes,
        seconds
    )
}

// ─── Legacy deploy module (preserves old API from deploy.rs) ───────────────

mod legacy {
    use nexus_sdk::audit::{AuditEvent, AuditTrail, EventType};
    use nexus_sdk::errors::AgentError;
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use std::path::Path;
    use uuid::Uuid;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub enum DeployProvider {
        Local,
        GitHubPages,
        Vercel,
        Netlify,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct DeploymentResult {
        pub provider: DeployProvider,
        pub url: String,
        pub command: String,
    }

    pub struct Deployer {
        agent_id: Uuid,
        audit: AuditTrail,
    }

    impl Default for Deployer {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Deployer {
        pub fn new() -> Self {
            Self {
                agent_id: Uuid::new_v4(),
                audit: AuditTrail::new(),
            }
        }

        pub fn deploy_to(
            &mut self,
            provider: DeployProvider,
            project: impl AsRef<Path>,
        ) -> Result<DeploymentResult, AgentError> {
            let project = project.as_ref();
            if !project.exists() {
                return Err(AgentError::SupervisorError(format!(
                    "deploy project path '{}' does not exist",
                    project.display()
                )));
            }

            let project_name = project
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("site");

            let result = match provider {
                DeployProvider::Local => DeploymentResult {
                    provider,
                    url: "http://127.0.0.1:4173".to_string(),
                    command:
                        "npm run build && npm run preview -- --host 127.0.0.1 --port 4173"
                            .to_string(),
                },
                DeployProvider::GitHubPages => DeploymentResult {
                    provider,
                    url: format!("https://example.github.io/{project_name}/"),
                    command: "git checkout -b gh-pages && npm run build && git add dist && git commit -m \"deploy\" && git push origin gh-pages"
                        .to_string(),
                },
                DeployProvider::Vercel => DeploymentResult {
                    provider,
                    url: format!("https://{project_name}.vercel.app"),
                    command: "vercel --prod".to_string(),
                },
                DeployProvider::Netlify => DeploymentResult {
                    provider,
                    url: format!("https://{project_name}.netlify.app"),
                    command: "netlify deploy --prod --dir=dist".to_string(),
                },
            };

            if let Err(e) = self.audit.append_event(
                self.agent_id,
                EventType::ToolCall,
                json!({
                    "step": "deploy",
                    "provider": format!("{:?}", provider),
                    "project": project.to_string_lossy().to_string(),
                    "url": result.url,
                }),
            ) {
                tracing::error!("Audit append failed: {e}");
            }

            Ok(result)
        }

        pub fn audit_events(&self) -> &[AuditEvent] {
            self.audit.events()
        }
    }

    pub fn deploy_to(
        provider: DeployProvider,
        project: impl AsRef<Path>,
    ) -> Result<DeploymentResult, AgentError> {
        let mut deployer = Deployer::new();
        deployer.deploy_to(provider, project)
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {

    // ── Final Gate decisions C and E: the bounded provider API client ──

    /// Answer one loopback request (read whole) with `head` and `body`, all
    /// at once or one byte per `drip`.
    fn serve(
        head: String,
        body: Vec<u8>,
        drip: Option<std::time::Duration>,
    ) -> (String, std::thread::JoinHandle<()>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(30)))
                .unwrap();
            let mut request = Vec::new();
            let mut byte = [0u8; 1];
            while !request.ends_with(b"\r\n\r\n") && matches!(stream.read(&mut byte), Ok(1)) {
                request.push(byte[0]);
            }
            if stream.write_all(head.as_bytes()).is_err() {
                return;
            }
            match drip {
                None => {
                    let _ = stream.write_all(&body);
                }
                Some(interval) => {
                    for byte in body {
                        std::thread::sleep(interval);
                        if stream.write_all(&[byte]).is_err() {
                            return;
                        }
                    }
                }
            }
        });
        (base, server)
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    /// The deploy token check and site listing: a redirect is returned as its
    /// status (the target is never contacted), a body past the cap is
    /// refused, declared or not, and a body dripped past the total timeout
    /// is abandoned.
    #[test]
    fn p0_r1_deploy_api_requests_are_bounded_and_follow_no_redirect() {
        let rt = runtime();
        let target = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        target.set_nonblocking(true).unwrap();
        let target_url = format!("http://{}", target.local_addr().unwrap());
        for code in [
            "301 Moved Permanently",
            "307 Temporary Redirect",
            "308 Permanent Redirect",
        ] {
            let (base, server) = serve(
                format!("HTTP/1.1 {code}\r\nLocation: {target_url}/collect\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"),
                Vec::new(),
                None,
            );
            let status = rt.block_on(async {
                api_client()
                    .unwrap()
                    .get(format!("{base}/api/v1/sites"))
                    .header("Authorization", "Bearer fg-deploy-token")
                    .send()
                    .await
                    .unwrap()
                    .status()
                    .as_u16()
            });
            assert_eq!(status.to_string(), code[..3]);
            server.join().unwrap();
        }
        assert!(matches!(
            target.accept(),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
        ));

        for declared in [true, false] {
            let body = format!("[\"{}\"]", "a".repeat(2048)).into_bytes();
            let head = if declared {
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
            } else {
                "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n".to_string()
            };
            let (base, server) = serve(head, body, None);
            let result: Result<Vec<String>, DeployError> = rt.block_on(async {
                let resp = api_client().unwrap().get(base).send().await.unwrap();
                read_json_capped(resp, 1024).await
            });
            assert!(
                matches!(&result, Err(DeployError::Network(reason)) if reason == "the response is larger than 1024 bytes"),
                "{result:?}"
            );
            server.join().unwrap();
        }
        let (base, server) = serve(
            "HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\n".to_string(),
            b"[\"site\"]".to_vec(),
            None,
        );
        let sites: Vec<String> = rt.block_on(async {
            let resp = api_client().unwrap().get(base).send().await.unwrap();
            read_json_capped(resp, 1024).await.unwrap()
        });
        assert_eq!(sites, ["site"]);
        server.join().unwrap();

        let (base, server) = serve(
            "HTTP/1.1 200 OK\r\nContent-Length: 18\r\nConnection: close\r\n\r\n".to_string(),
            b"[\"dripped-slowly\"]".to_vec(),
            Some(std::time::Duration::from_millis(300)),
        );
        let started = std::time::Instant::now();
        let result: Result<Vec<String>, DeployError> = rt.block_on(async {
            let resp = api_client_with(std::time::Duration::from_secs(1))
                .unwrap()
                .get(base)
                .send()
                .await
                .unwrap();
            read_json_capped(resp, 1024).await
        });
        assert!(result.is_err(), "a 5.4 s body must not be read within 1 s");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(4),
            "{:?}",
            started.elapsed()
        );
        server.join().unwrap();

        for (file, source) in [
            ("netlify.rs", include_str!("netlify.rs")),
            ("cloudflare.rs", include_str!("cloudflare.rs")),
            ("vercel.rs", include_str!("vercel.rs")),
        ] {
            let source = source.replace("\r\n", "\n");
            let list = source.split("pub async fn list_sites(").nth(1).unwrap();
            let list = list.split("\n}\n").next().unwrap();
            assert!(
                list.contains("super::read_json_capped(resp, super::MAX_API_RESPONSE_BYTES)"),
                "{file}"
            );
            for unbounded in [".json()", ".text()", ".bytes()"] {
                assert!(!list.contains(unbounded), "{file}: {unbounded}");
            }
        }
    }
    use super::*;
    use std::fs;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "nexus-deploy-test-{}-{}",
            name,
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_collect_deploy_files_from_directory() {
        let dir = temp_dir("collect");
        fs::write(dir.join("index.html"), b"<html>hello</html>").unwrap();
        fs::create_dir_all(dir.join("assets")).unwrap();
        fs::write(dir.join("assets").join("style.css"), b"body{margin:0}").unwrap();

        let files = collect_deploy_files(&dir).unwrap();
        assert_eq!(files.len(), 2);

        // Sorted by path
        assert_eq!(files[0].path, "assets/style.css");
        assert_eq!(files[1].path, "index.html");

        // Hashes are hex SHA-256
        assert_eq!(files[0].hash.len(), 64);
        assert_eq!(files[1].hash.len(), 64);

        // Content matches
        assert_eq!(files[0].content, b"body{margin:0}");
        assert_eq!(files[1].content, b"<html>hello</html>");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_collect_deploy_files_nonexistent_dir() {
        let result = collect_deploy_files(Path::new("/tmp/nonexistent-deploy-dir-99999"));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"), "got: {err}");
    }

    #[test]
    fn test_compute_build_hash_deterministic() {
        let files_a = vec![
            DeployFile {
                path: "b.txt".into(),
                content: vec![],
                hash: "abc123".into(),
            },
            DeployFile {
                path: "a.txt".into(),
                content: vec![],
                hash: "def456".into(),
            },
        ];
        // Same files in different order
        let files_b = vec![
            DeployFile {
                path: "a.txt".into(),
                content: vec![],
                hash: "def456".into(),
            },
            DeployFile {
                path: "b.txt".into(),
                content: vec![],
                hash: "abc123".into(),
            },
        ];

        let hash_a = compute_build_hash(&files_a);
        let hash_b = compute_build_hash(&files_b);
        assert_eq!(hash_a, hash_b);
        assert_eq!(hash_a.len(), 64);
    }

    #[test]
    fn test_compute_build_hash_changes_with_content() {
        let files_1 = vec![DeployFile {
            path: "index.html".into(),
            content: vec![],
            hash: "hash_v1".into(),
        }];
        let files_2 = vec![DeployFile {
            path: "index.html".into(),
            content: vec![],
            hash: "hash_v2".into(),
        }];

        assert_ne!(compute_build_hash(&files_1), compute_build_hash(&files_2));
    }

    #[test]
    fn test_sha256_hex() {
        let hash = sha256_hex(b"hello");
        assert_eq!(hash.len(), 64);
        assert_eq!(
            hash,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_credentials_debug_redacts_token() {
        let creds = Credentials {
            provider: "netlify".into(),
            token: "super-secret-token-123".into(),
            account_id: None,
            expires_at: None,
        };
        let debug_str = format!("{creds:?}");
        assert!(
            !debug_str.contains("super-secret"),
            "Token leaked in Debug: {debug_str}"
        );
        assert!(debug_str.contains("REDACTED"));
    }

    #[test]
    fn test_now_iso8601_format() {
        let ts = now_iso8601();
        assert!(ts.contains('T'));
        assert!(ts.ends_with('Z'));
        assert!(ts.starts_with("20"));
    }

    // Legacy API tests (preserved from old deploy.rs)
    #[test]
    fn test_legacy_deploy_to_netlify() {
        let dir = temp_dir("legacy");
        let result = deploy_to(LegacyDeployProvider::Netlify, &dir).unwrap();
        assert_eq!(result.provider, LegacyDeployProvider::Netlify);
        assert!(result.url.contains("netlify.app"));
        assert!(result.command.contains("netlify"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_legacy_deploy_nonexistent_path() {
        let result = deploy_to(
            LegacyDeployProvider::Vercel,
            "/tmp/nonexistent-deploy-path-99999",
        );
        assert!(result.is_err());
    }
}
