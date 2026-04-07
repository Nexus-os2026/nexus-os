//! PocketBase Backend Provider — self-hosted Go backend with collection rules.
//!
//! Stack: PocketBase JS SDK. Collections defined as JSON.
//! Security: Collection access rules (similar to RLS). Uses Sonnet for rule generation.
//! Cost: ~$0.15 (Sonnet for collection rules).

use super::{
    BackendConfig, BackendError, BackendOutput, BackendProvider, GeneratedFile, PgType,
    ProviderInfo, SchemaSpec,
};
use std::fmt::Write;

pub struct PocketBaseProvider;

impl BackendProvider for PocketBaseProvider {
    fn id(&self) -> &str {
        "pocketbase"
    }

    fn name(&self) -> &str {
        "PocketBase"
    }

    fn requires_credentials(&self) -> bool {
        true
    }

    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "pocketbase".into(),
            name: "PocketBase".into(),
            description: "Self-hosted backend. Best for self-hosted apps, full control.".into(),
            requires_credentials: true,
            cost_hint: "~$0.15 (Sonnet for collection rules)".into(),
        }
    }

    fn generate(
        &self,
        spec: &SchemaSpec,
        _config: &BackendConfig,
    ) -> Result<BackendOutput, BackendError> {
        let files = vec![generate_collection_schema(spec)];

        Ok(BackendOutput {
            files,
            dependencies: vec![
                ("pocketbase".into(), "^0.21.0".into()),
            ],
            env_vars: vec![
                ("VITE_POCKETBASE_URL".into(), "http://127.0.0.1:8090".into()),
            ],
            security_files: vec![],
            setup_instructions: "Download PocketBase from pocketbase.io, run './pocketbase serve', then import the collection schema from pocketbase/pb_schema.json.".into(),
        })
    }

    fn generate_security(
        &self,
        spec: &SchemaSpec,
        _rls_provider: Option<&dyn nexus_connectors_llm::providers::LlmProvider>,
    ) -> Result<Vec<GeneratedFile>, BackendError> {
        // Generate deterministic collection rules (Sonnet would be used in production)
        // For now, generate conservative rules like the Supabase deterministic fallback
        let rules = generate_collection_rules(spec);
        Ok(vec![rules])
    }

    fn generate_auth_components(&self, _spec: &SchemaSpec) -> Vec<GeneratedFile> {
        vec![
            generate_pb_auth_provider(),
            generate_pb_login_form(),
            generate_pb_signup_form(),
            generate_pb_auth_guard(),
        ]
    }

    fn generate_data_hooks(&self, spec: &SchemaSpec) -> Vec<GeneratedFile> {
        spec.tables.iter().map(generate_pb_hook).collect()
    }

    fn generate_client(&self, _config: &BackendConfig) -> Vec<GeneratedFile> {
        vec![generate_pb_client(), generate_pb_env_example()]
    }
}

// ─── PocketBase Type Mapping ───────────────────────────────────────────────

fn pb_type(pg: &PgType) -> &'static str {
    match pg {
        PgType::Uuid => "text",
        PgType::Text => "text",
        PgType::Integer | PgType::Bigint | PgType::Float8 => "number",
        PgType::Boolean => "bool",
        PgType::Timestamptz => "autodate",
        PgType::Jsonb => "json",
        PgType::Bytea => "file",
    }
}

// ─── Collection Schema Generation ──────────────────────────────────────────

fn generate_collection_schema(spec: &SchemaSpec) -> GeneratedFile {
    let mut json = String::with_capacity(2048);
    let _ = writeln!(json, "[");

    for (t_idx, table) in spec.tables.iter().enumerate() {
        let _ = writeln!(json, "  {{");
        let _ = writeln!(json, "    \"name\": \"{}\",", table.name);
        let _ = writeln!(json, "    \"type\": \"base\",");
        let _ = writeln!(json, "    \"schema\": [");

        // Skip id, created_at, updated_at — PocketBase auto-generates these
        let custom_cols: Vec<_> = table
            .columns
            .iter()
            .filter(|c| c.name != "id" && c.name != "created_at" && c.name != "updated_at")
            .collect();

        for (i, col) in custom_cols.iter().enumerate() {
            let _ = writeln!(json, "      {{");
            let _ = writeln!(json, "        \"name\": \"{}\",", col.name);

            // Check if it's a relation (foreign key)
            if let Some(ref fk) = col.references {
                if fk.table != "auth.users" {
                    let _ = writeln!(json, "        \"type\": \"relation\",");
                    let _ = writeln!(json, "        \"required\": {},", !col.nullable);
                    let _ = writeln!(json, "        \"options\": {{");
                    let _ = writeln!(json, "          \"collectionId\": \"{}\",", fk.table);
                    let _ = writeln!(
                        json,
                        "          \"cascadeDelete\": {}",
                        matches!(fk.on_delete, super::FkAction::Cascade)
                    );
                    let _ = writeln!(json, "        }}");
                } else {
                    // user_id → relation to built-in users collection
                    let _ = writeln!(json, "        \"type\": \"relation\",");
                    let _ = writeln!(json, "        \"required\": {},", !col.nullable);
                    let _ = writeln!(json, "        \"options\": {{");
                    let _ = writeln!(json, "          \"collectionId\": \"_pb_users_auth_\",");
                    let _ = writeln!(json, "          \"cascadeDelete\": true");
                    let _ = writeln!(json, "        }}");
                }
            } else {
                let _ = writeln!(json, "        \"type\": \"{}\",", pb_type(&col.data_type));
                let _ = writeln!(json, "        \"required\": {}", !col.nullable);
            }

            if i < custom_cols.len() - 1 {
                let _ = writeln!(json, "      }},");
            } else {
                let _ = writeln!(json, "      }}");
            }
        }

        let _ = writeln!(json, "    ],");

        // Access rules
        let has_owner = table.owner_column.is_some();
        if has_owner {
            let owner = table.owner_column.as_deref().unwrap_or("user_id");
            let _ = writeln!(json, "    \"listRule\": \"@request.auth.id != ''\",");
            let _ = writeln!(json, "    \"viewRule\": \"@request.auth.id != ''\",");
            let _ = writeln!(json, "    \"createRule\": \"@request.auth.id != ''\",");
            let _ = writeln!(json, "    \"updateRule\": \"@request.auth.id = {owner}\",");
            let _ = writeln!(json, "    \"deleteRule\": \"@request.auth.id = {owner}\"");
        } else {
            let _ = writeln!(json, "    \"listRule\": \"\",");
            let _ = writeln!(json, "    \"viewRule\": \"\",");
            let _ = writeln!(json, "    \"createRule\": \"@request.auth.id != ''\",");
            let _ = writeln!(json, "    \"updateRule\": \"@request.auth.id != ''\",");
            let _ = writeln!(json, "    \"deleteRule\": \"@request.auth.id != ''\"");
        }

        if t_idx < spec.tables.len() - 1 {
            let _ = writeln!(json, "  }},");
        } else {
            let _ = writeln!(json, "  }}");
        }
    }

    let _ = writeln!(json, "]");

    GeneratedFile {
        path: "pocketbase/pb_schema.json".into(),
        content: json,
    }
}

fn generate_collection_rules(spec: &SchemaSpec) -> GeneratedFile {
    let mut content = String::with_capacity(512);
    let _ = writeln!(content, "// PocketBase Collection Access Rules");
    let _ = writeln!(
        content,
        "// Generated by Nexus Builder — review before deploying"
    );
    let _ = writeln!(content);

    for table in &spec.tables {
        let _ = writeln!(content, "// Collection: {}", table.name);
        if let Some(ref owner) = table.owner_column {
            let _ = writeln!(content, "//   listRule: @request.auth.id != ''");
            let _ = writeln!(content, "//   viewRule: @request.auth.id != ''");
            let _ = writeln!(content, "//   createRule: @request.auth.id != ''");
            let _ = writeln!(content, "//   updateRule: @request.auth.id = {owner}");
            let _ = writeln!(content, "//   deleteRule: @request.auth.id = {owner}");
        } else {
            let _ = writeln!(content, "//   listRule: (public)");
            let _ = writeln!(content, "//   viewRule: (public)");
            let _ = writeln!(content, "//   createRule: @request.auth.id != ''");
            let _ = writeln!(content, "//   updateRule: @request.auth.id != ''");
            let _ = writeln!(content, "//   deleteRule: @request.auth.id != ''");
        }
        let _ = writeln!(content);
    }

    GeneratedFile {
        path: "pocketbase/access_rules.md".into(),
        content,
    }
}

// ─── Client Init ───────────────────────────────────────────────────────────

fn generate_pb_client() -> GeneratedFile {
    GeneratedFile {
        path: "src/lib/pocketbase.ts".into(),
        content: r#"import PocketBase from 'pocketbase'

const pbUrl = import.meta.env.VITE_POCKETBASE_URL || 'http://127.0.0.1:8090'

export const pb = new PocketBase(pbUrl)

// Auto-refresh auth store
pb.autoCancellation(false)
"#
        .into(),
    }
}

fn generate_pb_env_example() -> GeneratedFile {
    GeneratedFile {
        path: ".env.local.example".into(),
        content: r#"# PocketBase connection — run PocketBase locally or point to your server
VITE_POCKETBASE_URL=http://127.0.0.1:8090
"#
        .into(),
    }
}

// ─── Auth Components ───────────────────────────────────────────────────────

fn generate_pb_auth_provider() -> GeneratedFile {
    GeneratedFile {
        path: "src/components/auth/AuthProvider.tsx".into(),
        content: r#"import { createContext, useContext, useEffect, useState, type ReactNode } from 'react'
import { pb } from '../../lib/pocketbase'
import type { RecordModel } from 'pocketbase'

interface AuthContextType {
  user: RecordModel | null
  loading: boolean
  signOut: () => void
}

const AuthContext = createContext<AuthContextType>({
  user: null,
  loading: true,
  signOut: () => {},
})

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<RecordModel | null>(pb.authStore.record)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    setUser(pb.authStore.record)
    setLoading(false)

    const unsub = pb.authStore.onChange((_token, record) => {
      setUser(record)
    })

    return () => unsub()
  }, [])

  const signOut = () => {
    pb.authStore.clear()
    setUser(null)
  }

  return (
    <AuthContext.Provider value={{ user, loading, signOut }}>
      {children}
    </AuthContext.Provider>
  )
}

export function useAuthContext() {
  return useContext(AuthContext)
}

export default AuthProvider
"#
        .into(),
    }
}

fn generate_pb_login_form() -> GeneratedFile {
    GeneratedFile {
        path: "src/components/auth/LoginForm.tsx".into(),
        content: r#"import { useState, type FormEvent } from 'react'
import { pb } from '../../lib/pocketbase'

interface LoginFormProps {
  onSuccess?: () => void
  onToggleSignUp?: () => void
}

export default function LoginForm({ onSuccess, onToggleSignUp }: LoginFormProps) {
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault()
    setError(null)
    setLoading(true)

    try {
      await pb.collection('users').authWithPassword(email, password)
      onSuccess?.()
    } catch (err: any) {
      setError(err?.message ?? 'Login failed')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div style={{ width: '100%', maxWidth: 360, margin: '0 auto' }}>
      <h2 style={{ fontSize: 20, fontWeight: 700, marginBottom: 16 }}>Sign In</h2>
      <form onSubmit={handleSubmit}>
        <label style={{ display: 'block', fontSize: 12, marginBottom: 4 }}>Email</label>
        <input type="email" value={email} onChange={(e) => setEmail(e.target.value)} required
          style={{ width: '100%', padding: '6px 8px', marginBottom: 10, boxSizing: 'border-box' }} />
        <label style={{ display: 'block', fontSize: 12, marginBottom: 4 }}>Password</label>
        <input type="password" value={password} onChange={(e) => setPassword(e.target.value)} required
          style={{ width: '100%', padding: '6px 8px', marginBottom: 10, boxSizing: 'border-box' }} />
        {error && <p style={{ color: 'red', fontSize: 12 }}>{error}</p>}
        <button type="submit" disabled={loading}
          style={{ width: '100%', padding: '8px 0', fontWeight: 600, cursor: loading ? 'default' : 'pointer' }}>
          {loading ? 'Signing in...' : 'Sign In'}
        </button>
      </form>
      {onToggleSignUp && (
        <p style={{ marginTop: 12, fontSize: 12, textAlign: 'center' }}>
          Don't have an account? <button onClick={onToggleSignUp} style={{ background: 'none', border: 'none', textDecoration: 'underline', cursor: 'pointer' }}>Sign Up</button>
        </p>
      )}
    </div>
  )
}
"#
        .into(),
    }
}

fn generate_pb_signup_form() -> GeneratedFile {
    GeneratedFile {
        path: "src/components/auth/SignUpForm.tsx".into(),
        content: r#"import { useState, type FormEvent } from 'react'
import { pb } from '../../lib/pocketbase'

interface SignUpFormProps {
  onSuccess?: () => void
  onToggleLogin?: () => void
}

export default function SignUpForm({ onSuccess, onToggleLogin }: SignUpFormProps) {
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault()
    setError(null)

    if (password !== confirmPassword) {
      setError('Passwords do not match')
      return
    }
    if (password.length < 8) {
      setError('Password must be at least 8 characters')
      return
    }

    setLoading(true)
    try {
      await pb.collection('users').create({
        email,
        password,
        passwordConfirm: confirmPassword,
      })
      await pb.collection('users').authWithPassword(email, password)
      onSuccess?.()
    } catch (err: any) {
      setError(err?.message ?? 'Sign up failed')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div style={{ width: '100%', maxWidth: 360, margin: '0 auto' }}>
      <h2 style={{ fontSize: 20, fontWeight: 700, marginBottom: 16 }}>Create Account</h2>
      <form onSubmit={handleSubmit}>
        <label style={{ display: 'block', fontSize: 12, marginBottom: 4 }}>Email</label>
        <input type="email" value={email} onChange={(e) => setEmail(e.target.value)} required
          style={{ width: '100%', padding: '6px 8px', marginBottom: 10, boxSizing: 'border-box' }} />
        <label style={{ display: 'block', fontSize: 12, marginBottom: 4 }}>Password</label>
        <input type="password" value={password} onChange={(e) => setPassword(e.target.value)} required
          style={{ width: '100%', padding: '6px 8px', marginBottom: 10, boxSizing: 'border-box' }} />
        <label style={{ display: 'block', fontSize: 12, marginBottom: 4 }}>Confirm Password</label>
        <input type="password" value={confirmPassword} onChange={(e) => setConfirmPassword(e.target.value)} required
          style={{ width: '100%', padding: '6px 8px', marginBottom: 10, boxSizing: 'border-box' }} />
        {error && <p style={{ color: 'red', fontSize: 12 }}>{error}</p>}
        <button type="submit" disabled={loading}
          style={{ width: '100%', padding: '8px 0', fontWeight: 600, cursor: loading ? 'default' : 'pointer' }}>
          {loading ? 'Creating account...' : 'Sign Up'}
        </button>
      </form>
      {onToggleLogin && (
        <p style={{ marginTop: 12, fontSize: 12, textAlign: 'center' }}>
          Already have an account? <button onClick={onToggleLogin} style={{ background: 'none', border: 'none', textDecoration: 'underline', cursor: 'pointer' }}>Sign In</button>
        </p>
      )}
    </div>
  )
}
"#
        .into(),
    }
}

fn generate_pb_auth_guard() -> GeneratedFile {
    GeneratedFile {
        path: "src/components/auth/AuthGuard.tsx".into(),
        content: r#"import { type ReactNode } from 'react'
import { useAuthContext } from './AuthProvider'

interface AuthGuardProps {
  children: ReactNode
  fallback?: ReactNode
}

export default function AuthGuard({ children, fallback }: AuthGuardProps) {
  const { user, loading } = useAuthContext()

  if (loading) {
    return (
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', minHeight: '100vh' }}>
        <span>Loading...</span>
      </div>
    )
  }

  if (!user) {
    return fallback ? <>{fallback}</> : <div style={{ padding: 24 }}>Please sign in to continue.</div>
  }

  return <>{children}</>
}
"#
        .into(),
    }
}

// ─── CRUD Hooks ────────────────────────────────────────────────────────────

fn generate_pb_hook(table: &super::TableSpec) -> GeneratedFile {
    let type_name = to_pascal_case(&table.name);
    let hook_name = format!("use{type_name}");
    let mut ts = String::with_capacity(1024);

    let auto_fields: Vec<&str> = table
        .columns
        .iter()
        .filter(|c| c.default.is_some() || c.primary_key)
        .map(|c| c.name.as_str())
        .collect();

    let omit_type = if auto_fields.is_empty() {
        type_name.clone()
    } else {
        format!(
            "Omit<{}, {}>",
            type_name,
            auto_fields
                .iter()
                .map(|f| format!("'{f}'"))
                .collect::<Vec<_>>()
                .join(" | ")
        )
    };

    let _ = writeln!(ts, "import {{ pb }} from '../lib/pocketbase'");
    let _ = writeln!(ts, "import type {{ {type_name} }} from '../types/database'");
    let _ = writeln!(ts);
    let _ = writeln!(ts, "export function {hook_name}() {{");

    // list
    let _ = writeln!(ts, "  async function list(): Promise<{type_name}[]> {{");
    let _ = writeln!(
        ts,
        "    const records = await pb.collection('{}').getFullList({{ sort: '-created' }})",
        table.name
    );
    let _ = writeln!(ts, "    return records as unknown as {type_name}[]");
    let _ = writeln!(ts, "  }}");
    let _ = writeln!(ts);

    // getById
    let _ = writeln!(
        ts,
        "  async function getById(id: string): Promise<{type_name} | null> {{"
    );
    let _ = writeln!(ts, "    try {{");
    let _ = writeln!(
        ts,
        "      const record = await pb.collection('{}').getOne(id)",
        table.name
    );
    let _ = writeln!(ts, "      return record as unknown as {type_name}");
    let _ = writeln!(ts, "    }} catch {{");
    let _ = writeln!(ts, "      return null");
    let _ = writeln!(ts, "    }}");
    let _ = writeln!(ts, "  }}");
    let _ = writeln!(ts);

    // create
    let _ = writeln!(
        ts,
        "  async function create(item: {omit_type}): Promise<{type_name}> {{"
    );
    let _ = writeln!(
        ts,
        "    const record = await pb.collection('{}').create(item)",
        table.name
    );
    let _ = writeln!(ts, "    return record as unknown as {type_name}");
    let _ = writeln!(ts, "  }}");
    let _ = writeln!(ts);

    // update
    let _ = writeln!(ts, "  async function update(id: string, updates: Partial<{type_name}>): Promise<{type_name}> {{");
    let _ = writeln!(
        ts,
        "    const record = await pb.collection('{}').update(id, updates)",
        table.name
    );
    let _ = writeln!(ts, "    return record as unknown as {type_name}");
    let _ = writeln!(ts, "  }}");
    let _ = writeln!(ts);

    // remove
    let _ = writeln!(ts, "  async function remove(id: string): Promise<void> {{");
    let _ = writeln!(ts, "    await pb.collection('{}').delete(id)", table.name);
    let _ = writeln!(ts, "  }}");
    let _ = writeln!(ts);

    let _ = writeln!(ts, "  return {{ list, getById, create, update, remove }}");
    let _ = writeln!(ts, "}}");

    GeneratedFile {
        path: format!("src/hooks/{hook_name}.ts"),
        content: ts,
    }
}

fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|part| {
            let mut c = part.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().to_string() + c.as_str(),
            }
        })
        .collect()
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{
        BackendConfig, ColumnSpec, FkAction, ForeignKey, IndexSpec, PgType, SchemaSpec, TableSpec,
    };
    use std::collections::HashMap;

    fn sample_schema() -> SchemaSpec {
        SchemaSpec {
            tables: vec![
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
                            name: "created_at".into(),
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
                    ],
                    rls_enabled: false,
                    owner_column: None,
                    indexes: vec![],
                },
            ],
            auth_enabled: true,
            storage_buckets: vec![],
        }
    }

    fn default_config() -> BackendConfig {
        BackendConfig {
            provider: "pocketbase".into(),
            options: HashMap::new(),
        }
    }

    #[test]
    fn test_pocketbase_generates_collection_json() {
        let schema = generate_collection_schema(&sample_schema());
        assert_eq!(schema.path, "pocketbase/pb_schema.json");
        assert!(schema.content.contains("\"name\": \"products\""));
        assert!(schema.content.contains("\"name\": \"cart_items\""));
        assert!(schema.content.contains("\"type\": \"base\""));
    }

    #[test]
    fn test_pocketbase_type_mapping() {
        assert_eq!(pb_type(&PgType::Text), "text");
        assert_eq!(pb_type(&PgType::Integer), "number");
        assert_eq!(pb_type(&PgType::Float8), "number");
        assert_eq!(pb_type(&PgType::Boolean), "bool");
        assert_eq!(pb_type(&PgType::Timestamptz), "autodate");
        assert_eq!(pb_type(&PgType::Jsonb), "json");
    }

    #[test]
    fn test_pocketbase_relations() {
        let schema = generate_collection_schema(&sample_schema());
        // cart_items.product_id should be a relation to products
        assert!(schema.content.contains("\"type\": \"relation\""));
        assert!(schema.content.contains("\"collectionId\": \"products\""));
    }

    #[test]
    fn test_pocketbase_auth_components() {
        let provider = PocketBaseProvider;
        let auth = provider.generate_auth_components(&sample_schema());
        assert_eq!(auth.len(), 4);
        assert!(auth.iter().any(|f| f.path.contains("AuthProvider")));
        assert!(auth.iter().any(|f| f.path.contains("LoginForm")));
        assert!(auth.iter().any(|f| f.path.contains("SignUpForm")));
        assert!(auth.iter().any(|f| f.path.contains("AuthGuard")));
        // Should use PocketBase SDK
        let provider_file = auth
            .iter()
            .find(|f| f.path.contains("AuthProvider"))
            .unwrap();
        assert!(provider_file.content.contains("pocketbase"));
    }

    #[test]
    fn test_pocketbase_crud_hooks() {
        let provider = PocketBaseProvider;
        let hooks = provider.generate_data_hooks(&sample_schema());
        assert_eq!(hooks.len(), 2);
        assert!(hooks[0].content.contains("pb.collection('products')"));
        assert!(hooks[0].content.contains("getFullList"));
        assert!(hooks[0].content.contains(".create("));
        assert!(hooks[0].content.contains(".delete("));
    }

    #[test]
    fn test_pocketbase_security_rules_present() {
        let provider = PocketBaseProvider;
        let security = provider.generate_security(&sample_schema(), None).unwrap();
        assert!(!security.is_empty());
        let rules = &security[0];
        assert!(rules.content.contains("listRule"));
        assert!(rules.content.contains("createRule"));
        assert!(rules.content.contains("updateRule"));
        assert!(rules.content.contains("deleteRule"));
    }

    #[test]
    fn test_pocketbase_collection_rules_owner() {
        let schema = generate_collection_schema(&sample_schema());
        // products has owner_column = user_id
        assert!(schema
            .content
            .contains("\"updateRule\": \"@request.auth.id = user_id\""));
        assert!(schema
            .content
            .contains("\"deleteRule\": \"@request.auth.id = user_id\""));
    }

    #[test]
    fn test_pocketbase_client_init() {
        let provider = PocketBaseProvider;
        let config = default_config();
        let client = provider.generate_client(&config);
        assert!(client.iter().any(|f| f.path == "src/lib/pocketbase.ts"));
        let pb_file = client
            .iter()
            .find(|f| f.path.contains("pocketbase.ts"))
            .unwrap();
        assert!(pb_file.content.contains("new PocketBase"));
        assert!(pb_file.content.contains("VITE_POCKETBASE_URL"));
    }

    #[test]
    fn test_pocketbase_full_generation() {
        let provider = PocketBaseProvider;
        let config = default_config();
        let output = provider.generate(&sample_schema(), &config).unwrap();
        assert!(!output.files.is_empty());
        assert!(output.dependencies.iter().any(|(n, _)| n == "pocketbase"));
    }

    #[test]
    fn test_pocketbase_user_id_maps_to_users_auth() {
        let schema = generate_collection_schema(&sample_schema());
        // user_id referencing auth.users should map to _pb_users_auth_
        assert!(schema
            .content
            .contains("\"collectionId\": \"_pb_users_auth_\""));
    }
}
