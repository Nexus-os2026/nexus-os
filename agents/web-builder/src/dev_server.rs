//! Vite Dev Server Manager — spawns and manages Vite child processes for React preview.
//!
//! Lifecycle: prepare (write files) → install_deps (npm install) → start (spawn vite)
//!   → write_file (trigger HMR) → stop (kill process)
//!
//! Only ONE dev server runs per project at a time. The process is killed on
//! project close, project switch, and app exit.

use crate::react_gen::ReactProject;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use thiserror::Error;

// ─── Errors ─────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum DevServerError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("npm install failed: {0}")]
    NpmInstallFailed(String),
    #[error("vite startup failed: {0}")]
    ViteStartFailed(String),
    #[error("port {0} is not available")]
    PortUnavailable(u16),
    #[error("server not running")]
    NotRunning,
    #[error("timeout waiting for vite to start")]
    StartupTimeout,
    #[error("governance denied: {0}")]
    GovernanceDenied(String),
}

// ─── Status ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DevServerStatus {
    Stopped,
    Starting,
    Running { url: String },
    Error { message: String },
}

// ─── Port Management ────────────────────────────────────────────────────────

const PORT_RANGE_START: u16 = 15173;
const PORT_RANGE_END: u16 = 15183;

/// Find an available port in the range.
pub fn find_available_port() -> Result<u16, DevServerError> {
    for port in PORT_RANGE_START..=PORT_RANGE_END {
        if is_port_available(port) {
            return Ok(port);
        }
    }
    Err(DevServerError::PortUnavailable(PORT_RANGE_START))
}

/// Check if a TCP port is available by attempting to bind to it.
fn is_port_available(port: u16) -> bool {
    std::net::TcpListener::bind(("127.0.0.1", port)).is_ok()
}

// ─── Dev Server ─────────────────────────────────────────────────────────────

/// Manages a Vite dev server child process.
pub struct DevServer {
    port: u16,
    process: Option<Child>,
    project_dir: PathBuf,
    status: DevServerStatus,
    host: String,
}

impl DevServer {
    /// Create a new stopped dev server for a project directory.
    pub fn new(project_dir: PathBuf) -> Self {
        DevServer {
            port: 0,
            process: None,
            project_dir,
            status: DevServerStatus::Stopped,
            host: "127.0.0.1".into(),
        }
    }

    /// Write React project files to disk.
    pub fn prepare(project: &ReactProject, project_dir: &Path) -> Result<(), DevServerError> {
        for file in &project.files {
            let full_path = project_dir.join(&file.path);
            if let Some(parent) = full_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&full_path, &file.content)?;
        }
        Ok(())
    }

    /// Run `npm install` if needed (skip if node_modules exists and package.json unchanged).
    ///
    /// Requires `process.exec` capability — spawns an external npm process.
    pub fn install_deps(project_dir: &Path, capabilities: &[&str]) -> Result<(), DevServerError> {
        if !nexus_kernel::capabilities::has_capability(capabilities.iter().copied(), "process.exec")
        {
            return Err(DevServerError::GovernanceDenied(
                "npm install requires 'process.exec' capability".into(),
            ));
        }
        let node_modules = project_dir.join("node_modules");
        let pkg_json = project_dir.join("package.json");

        // Skip if node_modules exists and is newer than package.json
        if node_modules.exists() {
            let nm_meta = std::fs::metadata(&node_modules).ok();
            let pkg_meta = std::fs::metadata(&pkg_json).ok();
            if let (Some(nm), Some(pkg)) = (nm_meta, pkg_meta) {
                if let (Ok(nm_time), Ok(pkg_time)) = (nm.modified(), pkg.modified()) {
                    if nm_time >= pkg_time {
                        return Ok(());
                    }
                }
            }
        }

        eprintln!(
            "[nexus-builder][governance] npm_install dir={}",
            project_dir.display()
        );
        let output = Command::new("npm")
            .arg("install")
            .current_dir(project_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DevServerError::NpmInstallFailed(
                stderr.chars().take(500).collect(),
            ));
        }

        Ok(())
    }

    /// Spawn the Vite dev server as a child process.
    ///
    /// Returns the URL (e.g., "http://127.0.0.1:15173").
    /// Waits up to 15 seconds for Vite to report ready.
    /// Requires `process.exec` capability — spawns an external npx process.
    pub fn start(&mut self, capabilities: &[&str]) -> Result<String, DevServerError> {
        if !nexus_kernel::capabilities::has_capability(capabilities.iter().copied(), "process.exec")
        {
            return Err(DevServerError::GovernanceDenied(
                "vite dev server requires 'process.exec' capability".into(),
            ));
        }
        // Kill any existing process first
        self.stop_internal();

        self.port = find_available_port()?;
        self.status = DevServerStatus::Starting;

        eprintln!(
            "[nexus-builder][governance] vite_start dir={} port={}",
            self.project_dir.display(),
            self.port
        );
        let mut child = Command::new("npx")
            .args([
                "vite",
                "--port",
                &self.port.to_string(),
                "--strictPort",
                "--host",
                &self.host,
            ])
            .current_dir(&self.project_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| DevServerError::ViteStartFailed(e.to_string()))?;

        // Read stdout to detect the "ready" line
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| DevServerError::ViteStartFailed("no stdout".into()))?;

        let url = format!("http://{}:{}", self.host, self.port);
        let url_clone = url.clone();
        let reader = BufReader::new(stdout);

        // Wait for Vite to print "Local:" or the URL
        let (tx, rx) = std::sync::mpsc::channel();
        let _reader_thread = std::thread::spawn(move || {
            for line in reader.lines() {
                match line {
                    Ok(l) => {
                        if l.contains("Local:") || l.contains(&url_clone) || l.contains("ready in")
                        {
                            let _ = tx.send(true);
                            return;
                        }
                    }
                    Err(_) => break,
                }
            }
            let _ = tx.send(false);
        });

        // Wait with timeout
        let timeout = std::time::Duration::from_secs(15);
        match rx.recv_timeout(timeout) {
            Ok(true) => {
                self.process = Some(child);
                self.status = DevServerStatus::Running { url: url.clone() };
                Ok(url)
            }
            Ok(false) => {
                let _ = child.kill();
                self.status = DevServerStatus::Error {
                    message: "Vite exited before ready".into(),
                };
                Err(DevServerError::ViteStartFailed(
                    "Vite exited before ready".into(),
                ))
            }
            Err(_) => {
                // Timeout — Vite may still be starting, keep the process but report error
                let _ = child.kill();
                self.status = DevServerStatus::Error {
                    message: "Timeout waiting for Vite".into(),
                };
                Err(DevServerError::StartupTimeout)
            }
        }
    }

    /// Kill the dev server process gracefully.
    pub fn stop(&mut self) -> Result<(), DevServerError> {
        self.stop_internal();
        Ok(())
    }

    /// Internal stop — kills the child process.
    fn stop_internal(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.status = DevServerStatus::Stopped;
        self.port = 0;
    }

    /// Write a single file to disk (triggers Vite HMR).
    pub fn write_file(&self, relative_path: &str, content: &str) -> Result<(), DevServerError> {
        let full_path = self.project_dir.join(relative_path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&full_path, content)?;
        Ok(())
    }

    /// Get current status.
    pub fn status(&self) -> &DevServerStatus {
        &self.status
    }

    /// Get the active port (0 if not running).
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Enable LAN sharing by rebinding to 0.0.0.0.
    pub fn enable_lan_sharing(&mut self) {
        self.host = "0.0.0.0".into();
    }

    /// Disable LAN sharing by rebinding to localhost.
    pub fn disable_lan_sharing(&mut self) {
        self.host = "127.0.0.1".into();
    }

    /// Get the LAN URL if sharing is enabled.
    pub fn lan_url(&self) -> Option<String> {
        if self.host != "0.0.0.0" {
            return None;
        }
        get_local_ip().map(|ip| format!("http://{}:{}", ip, self.port))
    }
}

impl Drop for DevServer {
    fn drop(&mut self) {
        self.stop_internal();
    }
}

// ─── Thread-safe Server Registry ────────────────────────────────────────────

/// Global registry of active dev servers (one per project).
/// Used by Tauri commands to manage server lifecycle.
#[derive(Clone, Default)]
pub struct DevServerRegistry {
    servers: Arc<Mutex<std::collections::HashMap<String, DevServer>>>,
}

impl DevServerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get or create a dev server for a project.
    pub fn get_or_create(&self, project_id: &str, project_dir: PathBuf) -> Result<(), String> {
        let mut servers = self.servers.lock().map_err(|e| e.to_string())?;
        if !servers.contains_key(project_id) {
            servers.insert(project_id.to_string(), DevServer::new(project_dir));
        }
        Ok(())
    }

    /// Start the dev server for a project. Returns the URL.
    ///
    /// Requires `process.exec` capability (delegated to [`DevServer::start`]).
    pub fn start(&self, project_id: &str, capabilities: &[&str]) -> Result<String, String> {
        let mut servers = self.servers.lock().map_err(|e| e.to_string())?;
        let server = servers
            .get_mut(project_id)
            .ok_or_else(|| format!("no server for project {project_id}"))?;
        server.start(capabilities).map_err(|e| e.to_string())
    }

    /// Stop the dev server for a project.
    pub fn stop(&self, project_id: &str) -> Result<(), String> {
        let mut servers = self.servers.lock().map_err(|e| e.to_string())?;
        if let Some(server) = servers.get_mut(project_id) {
            server.stop().map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Stop ALL dev servers (called on app exit).
    pub fn stop_all(&self) {
        if let Ok(mut servers) = self.servers.lock() {
            for (_, server) in servers.iter_mut() {
                server.stop_internal();
            }
            servers.clear();
        }
    }

    /// Get the status of a project's dev server.
    pub fn status(&self, project_id: &str) -> DevServerStatus {
        let servers = self.servers.lock().ok();
        servers
            .and_then(|s| s.get(project_id).map(|srv| srv.status().clone()))
            .unwrap_or(DevServerStatus::Stopped)
    }

    /// Write a file via a project's dev server (triggers HMR).
    pub fn write_file(
        &self,
        project_id: &str,
        relative_path: &str,
        content: &str,
    ) -> Result<(), String> {
        let servers = self.servers.lock().map_err(|e| e.to_string())?;
        let server = servers
            .get(project_id)
            .ok_or_else(|| format!("no server for project {project_id}"))?;
        server
            .write_file(relative_path, content)
            .map_err(|e| e.to_string())
    }
}

// ─── Utilities ──────────────────────────────────────────────────────────────

/// Get the machine's local IP address for LAN sharing.
fn get_local_ip() -> Option<String> {
    // Simple approach: try to connect to a public IP (doesn't actually send data)
    // and read the local address
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let addr = socket.local_addr().ok()?;
    Some(addr.ip().to_string())
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::react_gen::ReactProjectFile;

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("nexus-dev-server-test-{}", uuid::Uuid::new_v4()))
    }

    fn mock_react_project() -> ReactProject {
        ReactProject {
            files: vec![
                ReactProjectFile {
                    path: "package.json".into(),
                    content: r#"{"name":"test","private":true,"version":"1.0.0"}"#.into(),
                },
                ReactProjectFile {
                    path: "src/main.tsx".into(),
                    content: "import React from 'react'\nconsole.log('hello')".into(),
                },
                ReactProjectFile {
                    path: "src/components/Hero.tsx".into(),
                    content: "export default function Hero() { return <div>Hero</div> }".into(),
                },
            ],
            project_name: "test".into(),
            template_id: "saas_landing".into(),
        }
    }

    #[test]
    fn test_prepare_writes_all_files() {
        let dir = temp_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let project = mock_react_project();

        DevServer::prepare(&project, &dir).unwrap();

        assert!(dir.join("package.json").exists());
        assert!(dir.join("src/main.tsx").exists());
        assert!(dir.join("src/components/Hero.tsx").exists());

        let content = std::fs::read_to_string(dir.join("package.json")).unwrap();
        assert!(content.contains("test"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_prepare_creates_directory_structure() {
        let dir = temp_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let project = mock_react_project();

        DevServer::prepare(&project, &dir).unwrap();

        assert!(dir.join("src").is_dir());
        assert!(dir.join("src/components").is_dir());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_install_deps_skips_when_node_modules_exists() {
        let dir = temp_dir();
        std::fs::create_dir_all(&dir).unwrap();

        // Create package.json and node_modules (fake)
        std::fs::write(dir.join("package.json"), "{}").unwrap();
        std::fs::create_dir_all(dir.join("node_modules")).unwrap();
        // Touch node_modules to make it newer
        std::fs::write(dir.join("node_modules/.keep"), "").unwrap();

        // This should return Ok without actually running npm
        let result = DevServer::install_deps(&dir, &["process.exec"]);
        assert!(result.is_ok());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_port_selection_avoids_conflict() {
        // Bind to a port in our range to simulate conflict
        let listener = std::net::TcpListener::bind(("127.0.0.1", PORT_RANGE_START)).ok();
        let port = find_available_port();

        if listener.is_some() {
            // The first port is taken, so we should get a different one
            assert!(port.is_ok());
            let p = port.unwrap();
            assert!(p >= PORT_RANGE_START);
            assert!(p <= PORT_RANGE_END);
        }
        // If bind failed (port already in use by something else), that's fine too
    }

    #[test]
    fn test_write_file_updates_disk() {
        let dir = temp_dir();
        std::fs::create_dir_all(&dir).unwrap();

        let server = DevServer::new(dir.clone());
        server
            .write_file("src/index.css", ":root { --color-primary: #ff0000; }")
            .unwrap();

        let content = std::fs::read_to_string(dir.join("src/index.css")).unwrap();
        assert!(content.contains("#ff0000"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_status_transitions() {
        let dir = temp_dir();
        std::fs::create_dir_all(&dir).unwrap();

        let server = DevServer::new(dir.clone());
        assert!(matches!(server.status(), DevServerStatus::Stopped));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_registry_stop_all() {
        let registry = DevServerRegistry::new();
        let dir = temp_dir();
        std::fs::create_dir_all(&dir).unwrap();

        registry.get_or_create("test-proj", dir.clone()).unwrap();
        registry.stop_all();

        assert!(matches!(
            registry.status("test-proj"),
            DevServerStatus::Stopped
        ));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Helper: returns true if `node` and `npm` are reachable on PATH.
    fn node_available() -> bool {
        std::process::Command::new("node")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
            && std::process::Command::new("npm")
                .arg("--version")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
    }

    #[test]
    fn test_start_returns_url() {
        if !node_available() {
            eprintln!("SKIP: node/npm not on PATH");
            return;
        }

        let dir = temp_dir();
        std::fs::create_dir_all(&dir).unwrap();

        // Write a minimal Vite project
        std::fs::write(
            dir.join("package.json"),
            r#"{"name":"test","private":true,"type":"module","devDependencies":{"vite":"^5.3.4"}}"#,
        )
        .unwrap();
        std::fs::write(dir.join("index.html"), "<html><body>test</body></html>").unwrap();

        DevServer::install_deps(&dir, &["process.exec"]).unwrap();
        let mut server = DevServer::new(dir.clone());
        let url = server.start(&["process.exec"]);
        assert!(url.is_ok(), "start failed: {url:?}");
        assert!(url.unwrap().starts_with("http://127.0.0.1:"));

        server.stop().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_stop_kills_process() {
        if !node_available() {
            eprintln!("SKIP: node/npm not on PATH");
            return;
        }

        let dir = temp_dir();
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("package.json"),
            r#"{"name":"test","private":true,"type":"module","devDependencies":{"vite":"^5.3.4"}}"#,
        )
        .unwrap();
        std::fs::write(dir.join("index.html"), "<html><body>test</body></html>").unwrap();

        DevServer::install_deps(&dir, &["process.exec"]).unwrap();
        let mut server = DevServer::new(dir.clone());
        let _ = server.start(&["process.exec"]);
        server.stop().unwrap();
        assert!(matches!(server.status(), DevServerStatus::Stopped));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
