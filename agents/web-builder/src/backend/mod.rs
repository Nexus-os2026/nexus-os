//! Backend Integration — Multi-provider backend generation from SchemaSpec.
//!
//! Pipeline: natural language → SchemaSpec → provider-specific output
//!
//! Providers:
//! - **Supabase**: Cloud Postgres + RLS + Auth (original, Phase 8A)
//! - **SQLite**: Local-first, offline-capable, no credentials needed
//! - **PocketBase**: Self-hosted Go backend with collection rules
//! - **Firebase**: Google Firestore + Auth
//!
//! Model routing:
//! - Schema parsing: gemma4:e4b (FREE)
//! - SQL/code generation: deterministic ($0)
//! - Security policies (Supabase RLS, PocketBase rules, Firestore rules): Sonnet (~$0.15)
//! - SQLite: no security generation needed ($0)

pub mod auth_components;
pub mod credentials;
pub mod data_hooks;
pub mod firebase;
pub mod pocketbase;
pub mod rls_gen;
pub mod schema_gen;
pub mod sql_gen;
pub mod sqlite;
pub mod supabase_client;
pub mod type_gen;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

// ─── Errors ─────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("schema generation failed: {0}")]
    SchemaGen(String),
    #[error("SQL generation failed: {0}")]
    SqlGen(String),
    #[error("RLS generation failed: {0}")]
    RlsGen(String),
    #[error("component generation failed: {0}")]
    ComponentGen(String),
    #[error("credential error: {0}")]
    Credential(String),
    #[error("model unavailable: {0}")]
    ModelUnavailable(String),
}

// ─── Schema Spec (Intermediate Representation) ─────────────────────────────

/// Complete backend schema specification parsed from natural language.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaSpec {
    pub tables: Vec<TableSpec>,
    pub auth_enabled: bool,
    pub storage_buckets: Vec<StorageBucketSpec>,
}

/// A single database table definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSpec {
    pub name: String,
    pub columns: Vec<ColumnSpec>,
    pub rls_enabled: bool,
    pub owner_column: Option<String>,
    pub indexes: Vec<IndexSpec>,
}

/// A single column definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnSpec {
    pub name: String,
    pub data_type: PgType,
    pub nullable: bool,
    pub default: Option<String>,
    pub primary_key: bool,
    pub references: Option<ForeignKey>,
    pub unique: bool,
}

/// PostgreSQL column types supported in schema generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PgType {
    Uuid,
    Text,
    Integer,
    Bigint,
    Float8,
    Boolean,
    Timestamptz,
    Jsonb,
    Bytea,
}

impl PgType {
    /// SQL type name for CREATE TABLE.
    pub fn sql_name(&self) -> &'static str {
        match self {
            Self::Uuid => "uuid",
            Self::Text => "text",
            Self::Integer => "integer",
            Self::Bigint => "bigint",
            Self::Float8 => "float8",
            Self::Boolean => "boolean",
            Self::Timestamptz => "timestamptz",
            Self::Jsonb => "jsonb",
            Self::Bytea => "bytea",
        }
    }

    /// TypeScript type equivalent.
    pub fn ts_type(&self) -> &'static str {
        match self {
            Self::Uuid | Self::Text | Self::Timestamptz | Self::Bytea => "string",
            Self::Integer | Self::Bigint | Self::Float8 => "number",
            Self::Boolean => "boolean",
            Self::Jsonb => "Record<string, unknown>",
        }
    }
}

/// Foreign key reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeignKey {
    pub table: String,
    pub column: String,
    pub on_delete: FkAction,
}

/// Foreign key action on delete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FkAction {
    Cascade,
    SetNull,
    Restrict,
}

impl FkAction {
    pub fn sql(&self) -> &'static str {
        match self {
            Self::Cascade => "CASCADE",
            Self::SetNull => "SET NULL",
            Self::Restrict => "RESTRICT",
        }
    }
}

/// Storage bucket specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageBucketSpec {
    pub name: String,
    pub public: bool,
    pub allowed_mime_types: Vec<String>,
    pub max_file_size_mb: u32,
}

/// Index specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexSpec {
    pub columns: Vec<String>,
    pub unique: bool,
}

// ─── Migration File ─────────────────────────────────────────────────────────

/// A single SQL migration file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationFile {
    pub filename: String,
    pub sql: String,
    pub description: String,
}

// ─── Generation Result ──────────────────────────────────────────────────────

/// Complete result of backend generation pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendGenerationResult {
    pub schema: SchemaSpec,
    pub migrations: Vec<MigrationFile>,
    pub rls_migrations: Vec<MigrationFile>,
    pub files: Vec<GeneratedFile>,
    pub cost_usd: f64,
    pub schema_hash: String,
    pub rls_hash: String,
}

/// A generated file to be added to the React project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedFile {
    pub path: String,
    pub content: String,
}

// ─── Backend Provider Trait ─────────────────────────────────────────────────

/// Information about a backend provider for UI display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub requires_credentials: bool,
    pub cost_hint: String,
}

/// Configuration for a backend provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendConfig {
    pub provider: String,
    pub options: HashMap<String, String>,
}

/// Output from a backend provider's generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendOutput {
    pub files: Vec<GeneratedFile>,
    pub dependencies: Vec<(String, String)>,
    pub env_vars: Vec<(String, String)>,
    pub security_files: Vec<GeneratedFile>,
    pub setup_instructions: String,
}

/// Trait for pluggable backend providers.
///
/// Each provider compiles the shared `SchemaSpec` into provider-specific output:
/// client init, auth components, CRUD hooks, and security policies.
pub trait BackendProvider: Send + Sync {
    /// Unique identifier: "supabase", "sqlite", "pocketbase", "firebase"
    fn id(&self) -> &str;
    /// Human-readable name
    fn name(&self) -> &str;
    /// Whether this provider needs external credentials
    fn requires_credentials(&self) -> bool;
    /// Provider info for UI display
    fn info(&self) -> ProviderInfo;

    /// Generate all backend files from a SchemaSpec.
    fn generate(
        &self,
        spec: &SchemaSpec,
        config: &BackendConfig,
    ) -> Result<BackendOutput, BackendError>;

    /// Generate security rules/policies. May use Sonnet for security-critical backends.
    fn generate_security(
        &self,
        spec: &SchemaSpec,
        rls_provider: Option<&dyn nexus_connectors_llm::providers::LlmProvider>,
    ) -> Result<Vec<GeneratedFile>, BackendError>;

    /// Generate auth components for this backend.
    fn generate_auth_components(&self, spec: &SchemaSpec) -> Vec<GeneratedFile>;

    /// Generate CRUD hooks for this backend.
    fn generate_data_hooks(&self, spec: &SchemaSpec) -> Vec<GeneratedFile>;

    /// Generate client initialization files.
    fn generate_client(&self, config: &BackendConfig) -> Vec<GeneratedFile>;
}

/// Get a backend provider by ID.
pub fn get_provider(id: &str) -> Option<Box<dyn BackendProvider>> {
    match id {
        "supabase" => None, // Supabase uses the original generate_backend() path
        "sqlite" => Some(Box::new(sqlite::SqliteProvider)),
        "pocketbase" => Some(Box::new(pocketbase::PocketBaseProvider)),
        "firebase" => Some(Box::new(firebase::FirebaseProvider)),
        _ => None,
    }
}

/// List all available backend providers (including Supabase).
pub fn list_providers() -> Vec<ProviderInfo> {
    vec![
        ProviderInfo {
            id: "supabase".into(),
            name: "Supabase".into(),
            description:
                "Cloud database + auth + storage. Best for hosted apps with real-time features."
                    .into(),
            requires_credentials: true,
            cost_hint: "~$0.15 (Sonnet for RLS policies)".into(),
        },
        sqlite::SqliteProvider.info(),
        pocketbase::PocketBaseProvider.info(),
        firebase::FirebaseProvider.info(),
    ]
}

/// Run the full backend generation pipeline for any provider.
///
/// For "supabase", delegates to the original `generate_backend()`.
/// For other providers, uses the `BackendProvider` trait.
pub fn generate_backend_v2(
    schema: &SchemaSpec,
    provider_id: &str,
    config: &BackendConfig,
    rls_provider: Option<&dyn nexus_connectors_llm::providers::LlmProvider>,
) -> Result<BackendGenerationResult, BackendError> {
    if provider_id == "supabase" {
        return generate_backend(schema, rls_provider);
    }

    let provider = get_provider(provider_id)
        .ok_or_else(|| BackendError::SchemaGen(format!("unknown provider: {provider_id}")))?;

    let output = provider.generate(schema, config)?;
    let security = provider.generate_security(schema, rls_provider)?;

    let cost_usd = if !security.is_empty() && rls_provider.is_some() {
        0.15
    } else {
        0.0
    };

    let mut files: Vec<GeneratedFile> = Vec::new();
    files.extend(output.files);
    files.extend(output.security_files);
    files.extend(security.iter().cloned());

    if schema.auth_enabled {
        files.extend(provider.generate_auth_components(schema));
    }

    files.extend(provider.generate_data_hooks(schema));

    let client_files = provider.generate_client(config);
    files.extend(client_files);

    // TypeScript types are backend-agnostic
    let type_file = type_gen::generate_typescript_types(schema);
    files.push(GeneratedFile {
        path: type_file.path,
        content: type_file.content,
    });

    // Governance hashes
    let schema_hash = compute_hash(&serde_json::to_string(schema).unwrap_or_default());
    let security_sql: String = security
        .iter()
        .map(|f| f.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let rls_hash = compute_hash(&security_sql);

    Ok(BackendGenerationResult {
        schema: schema.clone(),
        migrations: vec![], // Non-Supabase providers don't use SQL migrations
        rls_migrations: vec![],
        files,
        cost_usd,
        schema_hash,
        rls_hash,
    })
}

// ─── Orchestrator (Supabase — original) ────────────────────────────────────

/// Run the full backend generation pipeline.
///
/// 1. Parse description → SchemaSpec (gemma4:e4b, $0)
/// 2. Generate SQL migrations (deterministic, $0)
/// 3. Generate RLS policies (Sonnet, ~$0.15)
/// 4. Generate auth components (deterministic, $0)
/// 5. Generate CRUD hooks (deterministic, $0)
/// 6. Generate TypeScript types (deterministic, $0)
/// 7. Generate Supabase client setup (deterministic, $0)
/// 8. Compute governance hashes
pub fn generate_backend(
    schema: &SchemaSpec,
    rls_provider: Option<&dyn nexus_connectors_llm::providers::LlmProvider>,
) -> Result<BackendGenerationResult, BackendError> {
    // Step 2: SQL migrations
    let migrations = sql_gen::generate_migrations(schema);

    // Step 3: RLS policies
    let rls_migrations = rls_gen::generate_rls_policies(schema, rls_provider)
        .map_err(|e| BackendError::RlsGen(e.to_string()))?;

    let cost_usd = if rls_provider.is_some() { 0.15 } else { 0.0 };

    // Step 4-7: Generate all files
    let mut files: Vec<GeneratedFile> = Vec::new();

    // Supabase client setup
    let client_files = supabase_client::generate_supabase_client();
    files.extend(client_files.into_iter().map(|f| GeneratedFile {
        path: f.path,
        content: f.content,
    }));

    // Auth components (if auth enabled)
    if schema.auth_enabled {
        let auth_files = auth_components::generate_auth_components();
        files.extend(auth_files.into_iter().map(|f| GeneratedFile {
            path: f.path,
            content: f.content,
        }));
    }

    // CRUD hooks
    let hook_files = data_hooks::generate_data_hooks(schema);
    files.extend(hook_files.into_iter().map(|f| GeneratedFile {
        path: f.path,
        content: f.content,
    }));

    // TypeScript types
    let type_file = type_gen::generate_typescript_types(schema);
    files.push(GeneratedFile {
        path: type_file.path,
        content: type_file.content,
    });

    // Step 8: Governance hashes
    let schema_hash = compute_hash(&serde_json::to_string(schema).unwrap_or_default());
    let rls_sql: String = rls_migrations
        .iter()
        .map(|m| m.sql.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let rls_hash = compute_hash(&rls_sql);

    Ok(BackendGenerationResult {
        schema: schema.clone(),
        migrations,
        rls_migrations,
        files,
        cost_usd,
        schema_hash,
        rls_hash,
    })
}

/// Compute SHA-256 hex hash.
fn compute_hash(data: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    hex::encode(hasher.finalize())
}

// ─── Validation Helpers ─────────────────────────────────────────────────────

/// PostgreSQL reserved words that cannot be used as table names.
const PG_RESERVED: &[&str] = &[
    "all",
    "alter",
    "and",
    "any",
    "as",
    "begin",
    "between",
    "by",
    "case",
    "check",
    "column",
    "commit",
    "constraint",
    "create",
    "cross",
    "current",
    "database",
    "default",
    "delete",
    "distinct",
    "drop",
    "else",
    "end",
    "exists",
    "false",
    "fetch",
    "for",
    "foreign",
    "from",
    "full",
    "grant",
    "group",
    "having",
    "in",
    "index",
    "inner",
    "insert",
    "into",
    "is",
    "join",
    "key",
    "left",
    "like",
    "limit",
    "not",
    "null",
    "of",
    "on",
    "or",
    "order",
    "outer",
    "primary",
    "public",
    "references",
    "right",
    "rollback",
    "row",
    "select",
    "set",
    "table",
    "then",
    "to",
    "true",
    "union",
    "unique",
    "update",
    "user",
    "using",
    "values",
    "view",
    "when",
    "where",
    "with",
];

/// Check if a name is a valid PostgreSQL identifier.
pub fn is_valid_pg_identifier(name: &str) -> bool {
    if name.is_empty() || name.len() > 63 {
        return false;
    }
    // Must start with letter or underscore
    let first = name.chars().next().unwrap_or(' ');
    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }
    // Rest must be alphanumeric or underscore
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return false;
    }
    // Not a reserved word
    !PG_RESERVED.contains(&name.to_lowercase().as_str())
}

/// Validate a SchemaSpec for correctness.
pub fn validate_schema(spec: &SchemaSpec) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if spec.tables.is_empty() {
        errors.push("schema must define at least one table".into());
    }

    let table_names: Vec<&str> = spec.tables.iter().map(|t| t.name.as_str()).collect();

    for table in &spec.tables {
        if !is_valid_pg_identifier(&table.name) {
            errors.push(format!("invalid table name: '{}'", table.name));
        }

        // Check for duplicate column names
        let mut col_names = std::collections::HashSet::new();
        for col in &table.columns {
            if !col_names.insert(&col.name) {
                errors.push(format!(
                    "table '{}': duplicate column '{}'",
                    table.name, col.name
                ));
            }
        }

        // Validate foreign keys reference existing tables
        for col in &table.columns {
            if let Some(ref fk) = col.references {
                if !table_names.contains(&fk.table.as_str()) && fk.table != "auth.users" {
                    errors.push(format!(
                        "table '{}' column '{}': FK references unknown table '{}'",
                        table.name, col.name, fk.table
                    ));
                }
            }
        }
    }

    // Check for circular FK references (simple: A→B and B→A)
    for (i, t1) in spec.tables.iter().enumerate() {
        for t2 in spec.tables.iter().skip(i + 1) {
            let t1_refs_t2 = t1
                .columns
                .iter()
                .any(|c| c.references.as_ref().is_some_and(|fk| fk.table == t2.name));
            let t2_refs_t1 = t2
                .columns
                .iter()
                .any(|c| c.references.as_ref().is_some_and(|fk| fk.table == t1.name));
            if t1_refs_t2 && t2_refs_t1 {
                errors.push(format!(
                    "circular FK between '{}' and '{}'",
                    t1.name, t2.name
                ));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_schema() -> SchemaSpec {
        SchemaSpec {
            tables: vec![
                TableSpec {
                    name: "profiles".into(),
                    columns: vec![
                        ColumnSpec {
                            name: "id".into(),
                            data_type: PgType::Uuid,
                            nullable: false,
                            default: Some("gen_random_uuid()".into()),
                            primary_key: true,
                            references: None,
                            unique: false,
                        },
                        ColumnSpec {
                            name: "user_id".into(),
                            data_type: PgType::Uuid,
                            nullable: false,
                            default: None,
                            primary_key: false,
                            references: Some(ForeignKey {
                                table: "auth.users".into(),
                                column: "id".into(),
                                on_delete: FkAction::Cascade,
                            }),
                            unique: true,
                        },
                        ColumnSpec {
                            name: "full_name".into(),
                            data_type: PgType::Text,
                            nullable: true,
                            default: None,
                            primary_key: false,
                            references: None,
                            unique: false,
                        },
                        ColumnSpec {
                            name: "created_at".into(),
                            data_type: PgType::Timestamptz,
                            nullable: false,
                            default: Some("now()".into()),
                            primary_key: false,
                            references: None,
                            unique: false,
                        },
                        ColumnSpec {
                            name: "updated_at".into(),
                            data_type: PgType::Timestamptz,
                            nullable: false,
                            default: Some("now()".into()),
                            primary_key: false,
                            references: None,
                            unique: false,
                        },
                    ],
                    rls_enabled: true,
                    owner_column: Some("user_id".into()),
                    indexes: vec![],
                },
                TableSpec {
                    name: "products".into(),
                    columns: vec![
                        ColumnSpec {
                            name: "id".into(),
                            data_type: PgType::Uuid,
                            nullable: false,
                            default: Some("gen_random_uuid()".into()),
                            primary_key: true,
                            references: None,
                            unique: false,
                        },
                        ColumnSpec {
                            name: "user_id".into(),
                            data_type: PgType::Uuid,
                            nullable: false,
                            default: None,
                            primary_key: false,
                            references: Some(ForeignKey {
                                table: "auth.users".into(),
                                column: "id".into(),
                                on_delete: FkAction::Cascade,
                            }),
                            unique: false,
                        },
                        ColumnSpec {
                            name: "name".into(),
                            data_type: PgType::Text,
                            nullable: false,
                            default: None,
                            primary_key: false,
                            references: None,
                            unique: false,
                        },
                        ColumnSpec {
                            name: "price".into(),
                            data_type: PgType::Float8,
                            nullable: false,
                            default: Some("0".into()),
                            primary_key: false,
                            references: None,
                            unique: false,
                        },
                        ColumnSpec {
                            name: "image_url".into(),
                            data_type: PgType::Text,
                            nullable: true,
                            default: None,
                            primary_key: false,
                            references: None,
                            unique: false,
                        },
                        ColumnSpec {
                            name: "created_at".into(),
                            data_type: PgType::Timestamptz,
                            nullable: false,
                            default: Some("now()".into()),
                            primary_key: false,
                            references: None,
                            unique: false,
                        },
                        ColumnSpec {
                            name: "updated_at".into(),
                            data_type: PgType::Timestamptz,
                            nullable: false,
                            default: Some("now()".into()),
                            primary_key: false,
                            references: None,
                            unique: false,
                        },
                    ],
                    rls_enabled: true,
                    owner_column: Some("user_id".into()),
                    indexes: vec![IndexSpec {
                        columns: vec!["user_id".into()],
                        unique: false,
                    }],
                },
                TableSpec {
                    name: "cart_items".into(),
                    columns: vec![
                        ColumnSpec {
                            name: "id".into(),
                            data_type: PgType::Uuid,
                            nullable: false,
                            default: Some("gen_random_uuid()".into()),
                            primary_key: true,
                            references: None,
                            unique: false,
                        },
                        ColumnSpec {
                            name: "user_id".into(),
                            data_type: PgType::Uuid,
                            nullable: false,
                            default: None,
                            primary_key: false,
                            references: Some(ForeignKey {
                                table: "auth.users".into(),
                                column: "id".into(),
                                on_delete: FkAction::Cascade,
                            }),
                            unique: false,
                        },
                        ColumnSpec {
                            name: "product_id".into(),
                            data_type: PgType::Uuid,
                            nullable: false,
                            default: None,
                            primary_key: false,
                            references: Some(ForeignKey {
                                table: "products".into(),
                                column: "id".into(),
                                on_delete: FkAction::Cascade,
                            }),
                            unique: false,
                        },
                        ColumnSpec {
                            name: "quantity".into(),
                            data_type: PgType::Integer,
                            nullable: false,
                            default: Some("1".into()),
                            primary_key: false,
                            references: None,
                            unique: false,
                        },
                        ColumnSpec {
                            name: "created_at".into(),
                            data_type: PgType::Timestamptz,
                            nullable: false,
                            default: Some("now()".into()),
                            primary_key: false,
                            references: None,
                            unique: false,
                        },
                        ColumnSpec {
                            name: "updated_at".into(),
                            data_type: PgType::Timestamptz,
                            nullable: false,
                            default: Some("now()".into()),
                            primary_key: false,
                            references: None,
                            unique: false,
                        },
                    ],
                    rls_enabled: true,
                    owner_column: Some("user_id".into()),
                    indexes: vec![
                        IndexSpec {
                            columns: vec!["user_id".into()],
                            unique: false,
                        },
                        IndexSpec {
                            columns: vec!["product_id".into()],
                            unique: false,
                        },
                    ],
                },
            ],
            auth_enabled: true,
            storage_buckets: vec![],
        }
    }

    #[test]
    fn test_pg_type_sql_names() {
        assert_eq!(PgType::Uuid.sql_name(), "uuid");
        assert_eq!(PgType::Text.sql_name(), "text");
        assert_eq!(PgType::Float8.sql_name(), "float8");
        assert_eq!(PgType::Timestamptz.sql_name(), "timestamptz");
    }

    #[test]
    fn test_pg_type_ts_types() {
        assert_eq!(PgType::Uuid.ts_type(), "string");
        assert_eq!(PgType::Integer.ts_type(), "number");
        assert_eq!(PgType::Boolean.ts_type(), "boolean");
        assert_eq!(PgType::Jsonb.ts_type(), "Record<string, unknown>");
    }

    #[test]
    fn test_valid_pg_identifier() {
        assert!(is_valid_pg_identifier("products"));
        assert!(is_valid_pg_identifier("cart_items"));
        assert!(is_valid_pg_identifier("_private"));
        assert!(!is_valid_pg_identifier(""));
        assert!(!is_valid_pg_identifier("select")); // reserved
        assert!(!is_valid_pg_identifier("table")); // reserved
        assert!(!is_valid_pg_identifier("123abc")); // starts with digit
    }

    #[test]
    fn test_validate_schema_valid() {
        let schema = sample_schema();
        assert!(validate_schema(&schema).is_ok());
    }

    #[test]
    fn test_validate_schema_empty() {
        let schema = SchemaSpec {
            tables: vec![],
            auth_enabled: false,
            storage_buckets: vec![],
        };
        let err = validate_schema(&schema).unwrap_err();
        assert!(err.iter().any(|e| e.contains("at least one table")));
    }

    #[test]
    fn test_validate_schema_bad_table_name() {
        let schema = SchemaSpec {
            tables: vec![TableSpec {
                name: "select".into(),
                columns: vec![],
                rls_enabled: false,
                owner_column: None,
                indexes: vec![],
            }],
            auth_enabled: false,
            storage_buckets: vec![],
        };
        let err = validate_schema(&schema).unwrap_err();
        assert!(err.iter().any(|e| e.contains("invalid table name")));
    }

    #[test]
    fn test_validate_schema_unknown_fk() {
        let schema = SchemaSpec {
            tables: vec![TableSpec {
                name: "items".into(),
                columns: vec![ColumnSpec {
                    name: "category_id".into(),
                    data_type: PgType::Uuid,
                    nullable: false,
                    default: None,
                    primary_key: false,
                    references: Some(ForeignKey {
                        table: "nonexistent".into(),
                        column: "id".into(),
                        on_delete: FkAction::Cascade,
                    }),
                    unique: false,
                }],
                rls_enabled: false,
                owner_column: None,
                indexes: vec![],
            }],
            auth_enabled: false,
            storage_buckets: vec![],
        };
        let err = validate_schema(&schema).unwrap_err();
        assert!(err.iter().any(|e| e.contains("nonexistent")));
    }

    #[test]
    fn test_validate_no_circular_fk() {
        let schema = SchemaSpec {
            tables: vec![
                TableSpec {
                    name: "table_a".into(),
                    columns: vec![ColumnSpec {
                        name: "b_id".into(),
                        data_type: PgType::Uuid,
                        nullable: false,
                        default: None,
                        primary_key: false,
                        references: Some(ForeignKey {
                            table: "table_b".into(),
                            column: "id".into(),
                            on_delete: FkAction::Restrict,
                        }),
                        unique: false,
                    }],
                    rls_enabled: false,
                    owner_column: None,
                    indexes: vec![],
                },
                TableSpec {
                    name: "table_b".into(),
                    columns: vec![ColumnSpec {
                        name: "a_id".into(),
                        data_type: PgType::Uuid,
                        nullable: false,
                        default: None,
                        primary_key: false,
                        references: Some(ForeignKey {
                            table: "table_a".into(),
                            column: "id".into(),
                            on_delete: FkAction::Restrict,
                        }),
                        unique: false,
                    }],
                    rls_enabled: false,
                    owner_column: None,
                    indexes: vec![],
                },
            ],
            auth_enabled: false,
            storage_buckets: vec![],
        };
        let err = validate_schema(&schema).unwrap_err();
        assert!(err.iter().any(|e| e.contains("circular")));
    }

    #[test]
    fn test_full_backend_generation() {
        let schema = sample_schema();
        let result = generate_backend(&schema, None);
        assert!(result.is_ok(), "generation failed: {result:?}");
        let r = result.unwrap();

        // Should have migrations for each table
        assert_eq!(r.migrations.len(), 3);
        // Should have RLS migrations for each table with rls_enabled
        assert_eq!(r.rls_migrations.len(), 3);
        // Should have files: supabase client + auth components + hooks + types
        assert!(!r.files.is_empty());
        // Hashes computed
        assert!(!r.schema_hash.is_empty());
        assert!(!r.rls_hash.is_empty());
    }

    #[test]
    fn test_backend_files_integrate_with_react_project() {
        let schema = sample_schema();
        let result = generate_backend(&schema, None).unwrap();

        // All files have valid paths
        for f in &result.files {
            assert!(
                f.path.starts_with("src/") || f.path.starts_with(".env"),
                "unexpected path: {}",
                f.path
            );
            assert!(!f.content.is_empty(), "empty content for {}", f.path);
        }

        // Check expected files exist
        let paths: Vec<&str> = result.files.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"src/lib/supabase.ts"));
        assert!(paths.contains(&"src/types/database.ts"));
        assert!(paths.contains(&"src/components/auth/AuthProvider.tsx"));
        assert!(paths.contains(&"src/hooks/useProducts.ts"));
        assert!(paths.contains(&"src/hooks/useCartItems.ts"));
    }

    #[test]
    fn test_governance_signing() {
        let schema = sample_schema();
        let result = generate_backend(&schema, None).unwrap();

        // Schema hash is consistent
        let hash2 = compute_hash(&serde_json::to_string(&schema).unwrap());
        assert_eq!(result.schema_hash, hash2);

        // RLS hash is non-empty (policies were generated)
        assert!(!result.rls_hash.is_empty());
        assert_eq!(result.rls_hash.len(), 64); // SHA-256 hex
    }
}
