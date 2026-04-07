//! SQLite Backend Provider — local-first, offline-capable, zero credentials.
//!
//! Stack: sql.js (browser-compatible SQLite via WASM) or better-sqlite3 (Node).
//! No server-side security — all auth/access control at the application layer.
//! Cost: $0 (fully deterministic, no Sonnet needed).

use super::{
    BackendConfig, BackendError, BackendOutput, BackendProvider, GeneratedFile, PgType,
    ProviderInfo, SchemaSpec,
};
use std::fmt::Write;

pub struct SqliteProvider;

impl BackendProvider for SqliteProvider {
    fn id(&self) -> &str {
        "sqlite"
    }

    fn name(&self) -> &str {
        "SQLite"
    }

    fn requires_credentials(&self) -> bool {
        false
    }

    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "sqlite".into(),
            name: "SQLite".into(),
            description:
                "Local database, offline-capable. Best for desktop apps, local tools, privacy."
                    .into(),
            requires_credentials: false,
            cost_hint: "$0 (fully deterministic)".into(),
        }
    }

    fn generate(
        &self,
        spec: &SchemaSpec,
        _config: &BackendConfig,
    ) -> Result<BackendOutput, BackendError> {
        let mut files = generate_sqlite_migrations(spec);
        files.push(generate_migration_runner(spec));

        Ok(BackendOutput {
            files,
            dependencies: vec![("sql.js".into(), "^1.10.0".into())],
            env_vars: vec![],
            security_files: vec![],
            setup_instructions: "SQLite runs locally — no external service needed. The database file is created automatically on first use.".into(),
        })
    }

    fn generate_security(
        &self,
        _spec: &SchemaSpec,
        _rls_provider: Option<&dyn nexus_connectors_llm::providers::LlmProvider>,
    ) -> Result<Vec<GeneratedFile>, BackendError> {
        // SQLite has no server-side security — return empty
        Ok(vec![])
    }

    fn generate_auth_components(&self, _spec: &SchemaSpec) -> Vec<GeneratedFile> {
        vec![
            generate_sqlite_auth_provider(),
            generate_sqlite_auth_guard(),
        ]
    }

    fn generate_data_hooks(&self, spec: &SchemaSpec) -> Vec<GeneratedFile> {
        spec.tables.iter().map(generate_sqlite_hook).collect()
    }

    fn generate_client(&self, _config: &BackendConfig) -> Vec<GeneratedFile> {
        vec![generate_sqlite_client()]
    }
}

// ─── SQLite Type Mapping ───────────────────────────────────────────────────

/// Map PgType to SQLite column type.
fn sqlite_type(pg: &PgType) -> &'static str {
    match pg {
        PgType::Uuid => "TEXT",
        PgType::Text => "TEXT",
        PgType::Integer => "INTEGER",
        PgType::Bigint => "INTEGER",
        PgType::Float8 => "REAL",
        PgType::Boolean => "INTEGER",  // SQLite uses 0/1
        PgType::Timestamptz => "TEXT", // ISO 8601
        PgType::Jsonb => "TEXT",       // JSON serialized
        PgType::Bytea => "BLOB",
    }
}

/// Map PgType default to SQLite-compatible default.
fn sqlite_default(_pg: &PgType, pg_default: &str) -> String {
    match pg_default {
        "gen_random_uuid()" => String::new(), // UUID generated in JS
        "now()" => "CURRENT_TIMESTAMP".into(),
        other => other.to_string(),
    }
}

// ─── Migration Generation ──────────────────────────────────────────────────

fn generate_sqlite_migrations(spec: &SchemaSpec) -> Vec<GeneratedFile> {
    let mut files = Vec::new();
    let mut counter = 1u32;

    for table in &spec.tables {
        let mut sql = String::with_capacity(512);
        let _ = writeln!(sql, "-- {counter:03}_create_{}.sql (SQLite)", table.name);
        let _ = writeln!(sql, "CREATE TABLE IF NOT EXISTS {} (", table.name);

        let col_count = table.columns.len();
        for (i, col) in table.columns.iter().enumerate() {
            let _ = write!(sql, "    {} {}", col.name, sqlite_type(&col.data_type));

            if col.primary_key {
                let _ = write!(sql, " PRIMARY KEY");
            }
            if !col.nullable && !col.primary_key {
                let _ = write!(sql, " NOT NULL");
            }
            if col.unique {
                let _ = write!(sql, " UNIQUE");
            }
            if let Some(ref default) = col.default {
                let sqlite_def = sqlite_default(&col.data_type, default);
                if !sqlite_def.is_empty() {
                    let _ = write!(sql, " DEFAULT {sqlite_def}");
                }
            }
            // Foreign keys in SQLite use REFERENCES inline
            if let Some(ref fk) = col.references {
                // Skip auth.users references — SQLite has no auth schema
                if fk.table != "auth.users" {
                    let _ = write!(
                        sql,
                        " REFERENCES {}({}) ON DELETE {}",
                        fk.table,
                        fk.column,
                        fk.on_delete.sql()
                    );
                }
            }

            if i < col_count - 1 {
                let _ = writeln!(sql, ",");
            } else {
                let _ = writeln!(sql);
            }
        }

        let _ = writeln!(sql, ");");

        // Indexes
        for idx in &table.indexes {
            let cols = idx.columns.join(", ");
            let idx_name = format!("idx_{}_{}", table.name, idx.columns.join("_"));
            if idx.unique {
                let _ = writeln!(
                    sql,
                    "\nCREATE UNIQUE INDEX IF NOT EXISTS {idx_name} ON {}({cols});",
                    table.name
                );
            } else {
                let _ = writeln!(
                    sql,
                    "\nCREATE INDEX IF NOT EXISTS {idx_name} ON {}({cols});",
                    table.name
                );
            }
        }

        files.push(GeneratedFile {
            path: format!("migrations/{counter:03}_create_{}.sql", table.name),
            content: sql,
        });
        counter += 1;
    }

    files
}

fn generate_migration_runner(spec: &SchemaSpec) -> GeneratedFile {
    let mut ts = String::with_capacity(512);
    let _ = writeln!(ts, "import {{ db }} from './database'");
    let _ = writeln!(ts);
    let _ = writeln!(ts, "const migrations: string[] = [");

    for (i, table) in spec.tables.iter().enumerate() {
        let mut sql = String::new();
        let _ = write!(sql, "CREATE TABLE IF NOT EXISTS {} (", table.name);

        for (j, col) in table.columns.iter().enumerate() {
            let _ = write!(sql, "{} {}", col.name, sqlite_type(&col.data_type));
            if col.primary_key {
                let _ = write!(sql, " PRIMARY KEY");
            }
            if !col.nullable && !col.primary_key {
                let _ = write!(sql, " NOT NULL");
            }
            if let Some(ref default) = col.default {
                let sqlite_def = sqlite_default(&col.data_type, default);
                if !sqlite_def.is_empty() {
                    let _ = write!(sql, " DEFAULT {sqlite_def}");
                }
            }
            if j < table.columns.len() - 1 {
                let _ = write!(sql, ", ");
            }
        }
        let _ = write!(sql, ")");

        let _ = writeln!(ts, "  `{}`,", sql);
        let _ = write!(ts, "");
        if i < spec.tables.len() - 1 {
            // spacer
        }
    }

    let _ = writeln!(ts, "]");
    let _ = writeln!(ts);
    let _ = writeln!(ts, "export function runMigrations(): void {{");
    let _ = writeln!(ts, "  for (const sql of migrations) {{");
    let _ = writeln!(ts, "    db.run(sql)");
    let _ = writeln!(ts, "  }}");
    let _ = writeln!(ts, "}}");

    GeneratedFile {
        path: "src/lib/migrations.ts".into(),
        content: ts,
    }
}

// ─── Client Init ───────────────────────────────────────────────────────────

fn generate_sqlite_client() -> GeneratedFile {
    GeneratedFile {
        path: "src/lib/database.ts".into(),
        content: r#"import initSqlJs, { type Database } from 'sql.js'

let _db: Database | null = null

export async function getDb(): Promise<Database> {
  if (_db) return _db

  const SQL = await initSqlJs({
    locateFile: (file: string) => `https://sql.js.org/dist/${file}`,
  })

  // Try to load persisted database from localStorage
  const saved = localStorage.getItem('nexus_sqlite_db')
  if (saved) {
    const buf = Uint8Array.from(atob(saved), (c) => c.charCodeAt(0))
    _db = new SQL.Database(buf)
  } else {
    _db = new SQL.Database()
  }

  return _db
}

/** Persist the current database to localStorage. */
export function persistDb(): void {
  if (!_db) return
  const data = _db.export()
  const str = btoa(String.fromCharCode(...data))
  localStorage.setItem('nexus_sqlite_db', str)
}

/** Convenience: get db synchronously (must call getDb() first). */
export const db = {
  run(sql: string, params?: any[]): void {
    if (!_db) throw new Error('Database not initialized — call getDb() first')
    _db.run(sql, params)
    persistDb()
  },
  exec(sql: string, params?: any[]): any[] {
    if (!_db) throw new Error('Database not initialized — call getDb() first')
    return _db.exec(sql, params)
  },
  getAll<T = Record<string, unknown>>(sql: string, params?: any[]): T[] {
    if (!_db) throw new Error('Database not initialized — call getDb() first')
    const stmt = _db.prepare(sql)
    if (params) stmt.bind(params)
    const rows: T[] = []
    while (stmt.step()) {
      rows.push(stmt.getAsObject() as T)
    }
    stmt.free()
    return rows
  },
  getOne<T = Record<string, unknown>>(sql: string, params?: any[]): T | null {
    const rows = this.getAll<T>(sql, params)
    return rows[0] ?? null
  },
}
"#
        .into(),
    }
}

// ─── Auth Components ───────────────────────────────────────────────────────

fn generate_sqlite_auth_provider() -> GeneratedFile {
    GeneratedFile {
        path: "src/components/auth/AuthProvider.tsx".into(),
        content: r#"import { createContext, useContext, useState, type ReactNode } from 'react'

interface User {
  id: string
  email: string
}

interface AuthContextType {
  user: User | null
  loading: boolean
  signIn: (email: string, password: string) => Promise<void>
  signUp: (email: string, password: string) => Promise<void>
  signOut: () => void
}

const AuthContext = createContext<AuthContextType>({
  user: null,
  loading: false,
  signIn: async () => {},
  signUp: async () => {},
  signOut: () => {},
})

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null)
  const [loading] = useState(false)

  const signIn = async (email: string, _password: string) => {
    // SQLite local auth — in production, hash passwords with bcrypt
    // This is a minimal scaffold; replace with your auth logic
    setUser({ id: crypto.randomUUID(), email })
  }

  const signUp = async (email: string, _password: string) => {
    setUser({ id: crypto.randomUUID(), email })
  }

  const signOut = () => setUser(null)

  return (
    <AuthContext.Provider value={{ user, loading, signIn, signUp, signOut }}>
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

fn generate_sqlite_auth_guard() -> GeneratedFile {
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

fn generate_sqlite_hook(table: &super::TableSpec) -> GeneratedFile {
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

    let insert_cols: Vec<&str> = table
        .columns
        .iter()
        .filter(|c| c.default.is_none() && !c.primary_key)
        .map(|c| c.name.as_str())
        .collect();

    let _ = writeln!(ts, "import {{ db }} from '../lib/database'");
    let _ = writeln!(ts, "import type {{ {type_name} }} from '../types/database'");
    let _ = writeln!(ts);
    let _ = writeln!(ts, "export function {hook_name}() {{");

    // list
    let _ = writeln!(ts, "  function list(): {type_name}[] {{");
    let _ = writeln!(
        ts,
        "    return db.getAll<{type_name}>('SELECT * FROM {} ORDER BY rowid DESC')",
        table.name
    );
    let _ = writeln!(ts, "  }}");
    let _ = writeln!(ts);

    // getById
    let _ = writeln!(ts, "  function getById(id: string): {type_name} | null {{");
    let _ = writeln!(
        ts,
        "    return db.getOne<{type_name}>('SELECT * FROM {} WHERE id = ?', [id])",
        table.name
    );
    let _ = writeln!(ts, "  }}");
    let _ = writeln!(ts);

    // create
    let _ = writeln!(ts, "  function create(item: {omit_type}): {type_name} {{");
    let _ = writeln!(ts, "    const id = crypto.randomUUID()");
    if insert_cols.is_empty() {
        let _ = writeln!(
            ts,
            "    db.run('INSERT INTO {} (id) VALUES (?)', [id])",
            table.name
        );
    } else {
        let cols_str = std::iter::once("id")
            .chain(insert_cols.iter().copied())
            .collect::<Vec<_>>()
            .join(", ");
        let placeholders = std::iter::once("?")
            .chain(insert_cols.iter().map(|_| "?"))
            .collect::<Vec<_>>()
            .join(", ");
        let vals = insert_cols
            .iter()
            .map(|c| format!("(item as any).{c}"))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            ts,
            "    db.run('INSERT INTO {} ({cols_str}) VALUES ({placeholders})', [id, {vals}])",
            table.name
        );
    }
    let _ = writeln!(ts, "    return getById(id)!");
    let _ = writeln!(ts, "  }}");
    let _ = writeln!(ts);

    // update
    let _ = writeln!(
        ts,
        "  function update(id: string, updates: Partial<{type_name}>): {type_name} {{"
    );
    let _ = writeln!(
        ts,
        "    const entries = Object.entries(updates).filter(([k]) => k !== 'id')"
    );
    let _ = writeln!(ts, "    if (entries.length === 0) return getById(id)!");
    let _ = writeln!(
        ts,
        "    const sets = entries.map(([k]) => `${{k}} = ?`).join(', ')"
    );
    let _ = writeln!(ts, "    const vals = entries.map(([, v]) => v)");
    let _ = writeln!(
        ts,
        "    db.run(`UPDATE {} SET ${{sets}} WHERE id = ?`, [...vals, id])",
        table.name
    );
    let _ = writeln!(ts, "    return getById(id)!");
    let _ = writeln!(ts, "  }}");
    let _ = writeln!(ts);

    // remove
    let _ = writeln!(ts, "  function remove(id: string): void {{");
    let _ = writeln!(
        ts,
        "    db.run('DELETE FROM {} WHERE id = ?', [id])",
        table.name
    );
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
            provider: "sqlite".into(),
            options: HashMap::new(),
        }
    }

    #[test]
    fn test_sqlite_generates_migrations() {
        let files = generate_sqlite_migrations(&sample_schema());
        assert_eq!(files.len(), 2);
        assert!(files[0]
            .content
            .contains("CREATE TABLE IF NOT EXISTS products"));
        assert!(files[1]
            .content
            .contains("CREATE TABLE IF NOT EXISTS cart_items"));
    }

    #[test]
    fn test_sqlite_uuid_as_text() {
        let files = generate_sqlite_migrations(&sample_schema());
        assert!(files[0].content.contains("id TEXT PRIMARY KEY"));
    }

    #[test]
    fn test_sqlite_timestamptz_as_text() {
        let files = generate_sqlite_migrations(&sample_schema());
        assert!(files[0].content.contains("created_at TEXT"));
    }

    #[test]
    fn test_sqlite_no_rls() {
        let provider = SqliteProvider;
        let security = provider.generate_security(&sample_schema(), None).unwrap();
        assert!(
            security.is_empty(),
            "SQLite should not generate security files"
        );
    }

    #[test]
    fn test_sqlite_client_init() {
        let client = generate_sqlite_client();
        assert_eq!(client.path, "src/lib/database.ts");
        assert!(client.content.contains("sql.js"));
        assert!(client.content.contains("getDb"));
    }

    #[test]
    fn test_sqlite_crud_hooks() {
        let provider = SqliteProvider;
        let hooks = provider.generate_data_hooks(&sample_schema());
        assert_eq!(hooks.len(), 2);
        assert_eq!(hooks[0].path, "src/hooks/useProducts.ts");
        assert!(hooks[0].content.contains("SELECT * FROM products"));
        assert!(hooks[0].content.contains("INSERT INTO products"));
        assert!(hooks[0].content.contains("DELETE FROM products"));
    }

    #[test]
    fn test_sqlite_no_credentials_required() {
        let provider = SqliteProvider;
        assert!(!provider.requires_credentials());
    }

    #[test]
    fn test_sqlite_float_as_real() {
        let files = generate_sqlite_migrations(&sample_schema());
        assert!(files[0].content.contains("price REAL"));
    }

    #[test]
    fn test_sqlite_foreign_key_inline() {
        let files = generate_sqlite_migrations(&sample_schema());
        assert!(
            files[1]
                .content
                .contains("REFERENCES products(id) ON DELETE CASCADE"),
            "cart_items.product_id should reference products"
        );
    }

    #[test]
    fn test_sqlite_skips_auth_users_fk() {
        let files = generate_sqlite_migrations(&sample_schema());
        // products.user_id references auth.users — should be skipped in SQLite
        assert!(
            !files[0].content.contains("REFERENCES auth.users"),
            "SQLite should skip auth.users foreign key"
        );
    }

    #[test]
    fn test_sqlite_full_generation() {
        let provider = SqliteProvider;
        let config = default_config();
        let output = provider.generate(&sample_schema(), &config).unwrap();
        assert!(!output.files.is_empty());
        assert!(output.security_files.is_empty());
        assert!(output.dependencies.iter().any(|(name, _)| name == "sql.js"));
    }

    #[test]
    fn test_sqlite_auth_components_generated() {
        let provider = SqliteProvider;
        let schema = sample_schema();
        let auth = provider.generate_auth_components(&schema);
        assert_eq!(auth.len(), 2);
        assert!(auth.iter().any(|f| f.path.contains("AuthProvider")));
        assert!(auth.iter().any(|f| f.path.contains("AuthGuard")));
        // Should NOT import supabase
        for f in &auth {
            assert!(
                !f.content.contains("supabase"),
                "SQLite auth should not reference supabase"
            );
        }
    }
}
