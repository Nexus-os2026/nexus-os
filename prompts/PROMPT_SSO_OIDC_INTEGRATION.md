# PROMPT: SSO/OIDC Integration for Nexus OS

## Context
Nexus OS needs enterprise SSO via OIDC (OpenID Connect) and SAML 2.0 for production deployment. This integrates with Keycloak, Azure AD/Entra, Okta, and Google Workspace.

## Objective
Create a `nexus-auth` crate that provides OIDC/SAML authentication, session management, and role-based user administration layered on top of the existing capability-based agent ACL.

## Architecture

The auth system has two layers:
1. **User Authentication**: Who is the human? (OIDC/SAML → User identity)
2. **Agent Authorization**: What can this user's agents do? (Existing capability ACL)

```
User (OIDC token) → nexus-auth → Session → User Role → Agent Scope → Capability ACL
```

## Implementation Steps

### Step 1: Create the nexus-auth crate

```bash
cd crates
cargo new nexus-auth --lib
```

Add to workspace Cargo.toml.

**Dependencies (Cargo.toml):**
```toml
[dependencies]
openidconnect = "4"
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
jsonwebtoken = "9"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
uuid = { version = "1", features = ["v4"] }
thiserror = "2"
tracing = "0.1"
base64 = "0.22"
chrono = { version = "0.4", features = ["serde"] }
```

### Step 2: Core types

```rust
// crates/nexus-auth/src/lib.rs

pub mod oidc;
pub mod session;
pub mod roles;
pub mod config;
pub mod error;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuthenticatedUser {
    pub id: String,
    pub email: String,
    pub name: String,
    pub role: UserRole,
    pub workspace_id: Option<String>,
    pub session_id: uuid::Uuid,
    pub authenticated_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum UserRole {
    Admin,      // Full system access, user management, policy editing
    Operator,   // Agent deployment, configuration, HITL approval
    Viewer,     // Read-only access to dashboards and audit trails
    Auditor,    // Read-only access to audit trails and compliance reports
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuthConfig {
    pub provider: AuthProvider,
    pub issuer_url: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    pub role_mapping: std::collections::HashMap<String, UserRole>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum AuthProvider {
    Oidc,
    Saml,
    Local, // For desktop mode — OS-level auth
}
```

### Step 3: OIDC implementation

Use the `openidconnect` crate to implement the OIDC authorization code flow:
1. Discovery: Fetch `.well-known/openid-configuration` from issuer
2. Authorization: Redirect to IdP with PKCE challenge
3. Callback: Exchange code for tokens
4. Validation: Verify ID token signature, expiry, audience
5. Role extraction: Map IdP roles/groups to Nexus UserRole
6. Session creation: Issue session with expiry

### Step 4: Session management

```rust
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<Uuid, AuthenticatedUser>>>,
    max_session_duration: Duration,
    cleanup_interval: Duration,
}
```

- In-memory sessions with configurable TTL
- Automatic cleanup of expired sessions
- Session refresh via OIDC refresh tokens
- Concurrent access via RwLock

### Step 5: Tauri commands

Add these Tauri commands:
```rust
#[tauri::command]
async fn auth_login(state: State<'_, AppState>) -> Result<AuthRedirectUrl, NexusError>

#[tauri::command]
async fn auth_callback(state: State<'_, AppState>, code: String, state_param: String) -> Result<AuthenticatedUser, NexusError>

#[tauri::command]
async fn auth_logout(state: State<'_, AppState>, session_id: String) -> Result<(), NexusError>

#[tauri::command]
async fn auth_session(state: State<'_, AppState>, session_id: String) -> Result<AuthenticatedUser, NexusError>

#[tauri::command]
async fn auth_users_list(state: State<'_, AppState>) -> Result<Vec<UserSummary>, NexusError>
```

### Step 6: Middleware integration

Add authentication check to all existing Tauri commands:
- Desktop mode: OS-level user (no OIDC required)
- Server mode: Require valid OIDC session for all API calls
- Hybrid mode: Desktop UI authenticated locally, server calls require OIDC

### Step 7: Frontend

Create `frontend/src/pages/Login/` page with:
- OIDC login button (redirects to IdP)
- Session status indicator in header
- Role-based UI rendering (hide admin features for Viewer role)

### Step 8: Configuration

Add to config.toml:
```toml
[auth]
provider = "oidc"
issuer_url = "https://keycloak.example.com/realms/nexus"
client_id = "nexus-os"
client_secret_env = "NEXUS_OIDC_CLIENT_SECRET"
scopes = ["openid", "profile", "email", "roles"]
session_duration_hours = 8

[auth.role_mapping]
"nexus-admin" = "Admin"
"nexus-operator" = "Operator"
"nexus-viewer" = "Viewer"
"nexus-auditor" = "Auditor"
```

## Testing
- Unit test: Token validation with mock JWKS
- Unit test: Role mapping
- Unit test: Session expiry
- Integration test: Full OIDC flow with test Keycloak instance

## Finish
Run `cargo fmt` and `cargo clippy` on `nexus-auth` crate only.
Do NOT use `--all-features`. Do NOT run workspace-wide tests.
