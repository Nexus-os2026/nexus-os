//! Firebase Backend Provider — Google Firestore + Firebase Auth.
//!
//! Stack: Firebase JS SDK v9+ (modular imports).
//! Security: Firestore security rules. Uses Sonnet for rule generation.
//! Cost: ~$0.15 (Sonnet for Firestore rules).

use super::{
    BackendConfig, BackendError, BackendOutput, BackendProvider, GeneratedFile, PgType,
    ProviderInfo, SchemaSpec,
};
use std::fmt::Write;

pub struct FirebaseProvider;

impl BackendProvider for FirebaseProvider {
    fn id(&self) -> &str {
        "firebase"
    }

    fn name(&self) -> &str {
        "Firebase"
    }

    fn requires_credentials(&self) -> bool {
        true
    }

    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "firebase".into(),
            name: "Firebase".into(),
            description: "Google cloud backend. Best for Google ecosystem, mobile apps.".into(),
            requires_credentials: true,
            cost_hint: "~$0.15 (Sonnet for Firestore rules)".into(),
        }
    }

    fn generate(
        &self,
        spec: &SchemaSpec,
        _config: &BackendConfig,
    ) -> Result<BackendOutput, BackendError> {
        let files = vec![generate_firestore_rules(spec), generate_firebase_json()];

        Ok(BackendOutput {
            files,
            dependencies: vec![
                ("firebase".into(), "^10.12.0".into()),
            ],
            env_vars: vec![
                ("VITE_FIREBASE_API_KEY".into(), "your-api-key".into()),
                ("VITE_FIREBASE_AUTH_DOMAIN".into(), "your-project.firebaseapp.com".into()),
                ("VITE_FIREBASE_PROJECT_ID".into(), "your-project-id".into()),
                ("VITE_FIREBASE_STORAGE_BUCKET".into(), "your-project.appspot.com".into()),
                ("VITE_FIREBASE_MESSAGING_SENDER_ID".into(), "123456789".into()),
                ("VITE_FIREBASE_APP_ID".into(), "1:123456789:web:abc123".into()),
            ],
            security_files: vec![],
            setup_instructions: "Create a Firebase project at console.firebase.google.com, enable Firestore and Authentication, then copy your web app config to .env.local.".into(),
        })
    }

    fn generate_security(
        &self,
        spec: &SchemaSpec,
        _rls_provider: Option<&dyn nexus_connectors_llm::providers::LlmProvider>,
    ) -> Result<Vec<GeneratedFile>, BackendError> {
        // Generate deterministic Firestore rules (Sonnet would be used in production)
        Ok(vec![generate_firestore_rules(spec)])
    }

    fn generate_auth_components(&self, _spec: &SchemaSpec) -> Vec<GeneratedFile> {
        vec![
            generate_firebase_auth_provider(),
            generate_firebase_login_form(),
            generate_firebase_signup_form(),
            generate_firebase_auth_guard(),
        ]
    }

    fn generate_data_hooks(&self, spec: &SchemaSpec) -> Vec<GeneratedFile> {
        spec.tables.iter().map(generate_firestore_hook).collect()
    }

    fn generate_client(&self, _config: &BackendConfig) -> Vec<GeneratedFile> {
        vec![generate_firebase_client(), generate_firebase_env_example()]
    }
}

// ─── Firestore Type Mapping ────────────────────────────────────────────────

/// Map PgType to Firestore field type name.
pub fn firestore_type(pg: &PgType) -> &'static str {
    match pg {
        PgType::Uuid => "string",
        PgType::Text => "string",
        PgType::Integer | PgType::Bigint | PgType::Float8 => "number",
        PgType::Boolean => "boolean",
        PgType::Timestamptz => "timestamp",
        PgType::Jsonb => "map",
        PgType::Bytea => "bytes",
    }
}

// ─── Firestore Security Rules ──────────────────────────────────────────────

fn generate_firestore_rules(spec: &SchemaSpec) -> GeneratedFile {
    let mut rules = String::with_capacity(1024);
    let _ = writeln!(rules, "rules_version = '2';");
    let _ = writeln!(rules, "service cloud.firestore {{");
    let _ = writeln!(rules, "  match /databases/{{database}}/documents {{");

    for table in &spec.tables {
        let _ = writeln!(rules);
        let _ = writeln!(rules, "    // Collection: {}", table.name);
        let _ = writeln!(rules, "    match /{}/ {{{} Id}} {{", table.name, table.name);

        if let Some(ref owner) = table.owner_column {
            let _ = writeln!(rules, "      allow read: if request.auth != null;");
            let _ = writeln!(
                rules,
                "      allow create: if request.auth != null && request.resource.data.{owner} == request.auth.uid;"
            );
            let _ = writeln!(
                rules,
                "      allow update, delete: if request.auth != null && resource.data.{owner} == request.auth.uid;"
            );
        } else {
            let _ = writeln!(rules, "      allow read: if true;");
            let _ = writeln!(rules, "      allow create: if request.auth != null;");
            let _ = writeln!(
                rules,
                "      allow update, delete: if request.auth != null;"
            );
        }

        let _ = writeln!(rules, "    }}");
    }

    let _ = writeln!(rules, "  }}");
    let _ = writeln!(rules, "}}");

    GeneratedFile {
        path: "firebase/firestore.rules".into(),
        content: rules,
    }
}

fn generate_firebase_json() -> GeneratedFile {
    GeneratedFile {
        path: "firebase/firebase.json".into(),
        content: r#"{
  "firestore": {
    "rules": "firestore.rules",
    "indexes": "firestore.indexes.json"
  },
  "hosting": {
    "public": "dist",
    "ignore": ["firebase.json", "**/.*", "**/node_modules/**"],
    "rewrites": [
      { "source": "**", "destination": "/index.html" }
    ]
  }
}
"#
        .into(),
    }
}

// ─── Client Init ───────────────────────────────────────────────────────────

fn generate_firebase_client() -> GeneratedFile {
    GeneratedFile {
        path: "src/lib/firebase.ts".into(),
        content: r#"import { initializeApp } from 'firebase/app'
import { getFirestore } from 'firebase/firestore'
import { getAuth } from 'firebase/auth'

const firebaseConfig = {
  apiKey: import.meta.env.VITE_FIREBASE_API_KEY,
  authDomain: import.meta.env.VITE_FIREBASE_AUTH_DOMAIN,
  projectId: import.meta.env.VITE_FIREBASE_PROJECT_ID,
  storageBucket: import.meta.env.VITE_FIREBASE_STORAGE_BUCKET,
  messagingSenderId: import.meta.env.VITE_FIREBASE_MESSAGING_SENDER_ID,
  appId: import.meta.env.VITE_FIREBASE_APP_ID,
}

if (!firebaseConfig.apiKey) {
  throw new Error(
    'Missing Firebase environment variables. ' +
    'Copy .env.local.example to .env.local and fill in your Firebase project credentials.'
  )
}

const app = initializeApp(firebaseConfig)

export const db = getFirestore(app)
export const auth = getAuth(app)
"#
        .into(),
    }
}

fn generate_firebase_env_example() -> GeneratedFile {
    GeneratedFile {
        path: ".env.local.example".into(),
        content:
            r#"# Firebase connection — get these from Firebase Console → Project Settings → Web App
VITE_FIREBASE_API_KEY=your-api-key
VITE_FIREBASE_AUTH_DOMAIN=your-project.firebaseapp.com
VITE_FIREBASE_PROJECT_ID=your-project-id
VITE_FIREBASE_STORAGE_BUCKET=your-project.appspot.com
VITE_FIREBASE_MESSAGING_SENDER_ID=123456789
VITE_FIREBASE_APP_ID=1:123456789:web:abc123
"#
            .into(),
    }
}

// ─── Auth Components ───────────────────────────────────────────────────────

fn generate_firebase_auth_provider() -> GeneratedFile {
    GeneratedFile {
        path: "src/components/auth/AuthProvider.tsx".into(),
        content: r#"import { createContext, useContext, useEffect, useState, type ReactNode } from 'react'
import { auth } from '../../lib/firebase'
import { onAuthStateChanged, signOut as fbSignOut, type User } from 'firebase/auth'

interface AuthContextType {
  user: User | null
  loading: boolean
  signOut: () => Promise<void>
}

const AuthContext = createContext<AuthContextType>({
  user: null,
  loading: true,
  signOut: async () => {},
})

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    const unsub = onAuthStateChanged(auth, (u) => {
      setUser(u)
      setLoading(false)
    })
    return () => unsub()
  }, [])

  const signOut = async () => {
    await fbSignOut(auth)
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

fn generate_firebase_login_form() -> GeneratedFile {
    GeneratedFile {
        path: "src/components/auth/LoginForm.tsx".into(),
        content: r#"import { useState, type FormEvent } from 'react'
import { auth } from '../../lib/firebase'
import { signInWithEmailAndPassword } from 'firebase/auth'

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
      await signInWithEmailAndPassword(auth, email, password)
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

fn generate_firebase_signup_form() -> GeneratedFile {
    GeneratedFile {
        path: "src/components/auth/SignUpForm.tsx".into(),
        content: r#"import { useState, type FormEvent } from 'react'
import { auth } from '../../lib/firebase'
import { createUserWithEmailAndPassword } from 'firebase/auth'

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
    if (password.length < 6) {
      setError('Password must be at least 6 characters')
      return
    }

    setLoading(true)
    try {
      await createUserWithEmailAndPassword(auth, email, password)
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

fn generate_firebase_auth_guard() -> GeneratedFile {
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

fn generate_firestore_hook(table: &super::TableSpec) -> GeneratedFile {
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

    let _ = writeln!(ts, "import {{ db }} from '../lib/firebase'");
    let _ = writeln!(ts, "import {{");
    let _ = writeln!(
        ts,
        "  collection, doc, getDocs, getDoc, addDoc, updateDoc, deleteDoc,"
    );
    let _ = writeln!(ts, "  query, orderBy,");
    let _ = writeln!(ts, "}} from 'firebase/firestore'");
    let _ = writeln!(ts, "import type {{ {type_name} }} from '../types/database'");
    let _ = writeln!(ts);
    let _ = writeln!(ts, "const COLLECTION = '{}'", table.name);
    let _ = writeln!(ts);
    let _ = writeln!(ts, "export function {hook_name}() {{");

    // list
    let _ = writeln!(ts, "  async function list(): Promise<{type_name}[]> {{");
    let _ = writeln!(
        ts,
        "    const q = query(collection(db, COLLECTION), orderBy('created_at', 'desc'))"
    );
    let _ = writeln!(ts, "    const snap = await getDocs(q)");
    let _ = writeln!(
        ts,
        "    return snap.docs.map((d) => ({{ id: d.id, ...d.data() }})) as {type_name}[]"
    );
    let _ = writeln!(ts, "  }}");
    let _ = writeln!(ts);

    // getById
    let _ = writeln!(
        ts,
        "  async function getById(id: string): Promise<{type_name} | null> {{"
    );
    let _ = writeln!(ts, "    const snap = await getDoc(doc(db, COLLECTION, id))");
    let _ = writeln!(ts, "    if (!snap.exists()) return null");
    let _ = writeln!(
        ts,
        "    return {{ id: snap.id, ...snap.data() }} as {type_name}"
    );
    let _ = writeln!(ts, "  }}");
    let _ = writeln!(ts);

    // create
    let _ = writeln!(
        ts,
        "  async function create(item: {omit_type}): Promise<{type_name}> {{"
    );
    let _ = writeln!(
        ts,
        "    const ref = await addDoc(collection(db, COLLECTION), {{"
    );
    let _ = writeln!(ts, "      ...item,");
    let _ = writeln!(ts, "      created_at: new Date().toISOString(),");
    let _ = writeln!(ts, "      updated_at: new Date().toISOString(),");
    let _ = writeln!(ts, "    }})");
    let _ = writeln!(
        ts,
        "    return {{ id: ref.id, ...item }} as unknown as {type_name}"
    );
    let _ = writeln!(ts, "  }}");
    let _ = writeln!(ts);

    // update
    let _ = writeln!(ts, "  async function update(id: string, updates: Partial<{type_name}>): Promise<{type_name}> {{");
    let _ = writeln!(ts, "    const ref = doc(db, COLLECTION, id)");
    let _ = writeln!(
        ts,
        "    await updateDoc(ref, {{ ...updates, updated_at: new Date().toISOString() }})"
    );
    let _ = writeln!(ts, "    const updated = await getById(id)");
    let _ = writeln!(ts, "    return updated!");
    let _ = writeln!(ts, "  }}");
    let _ = writeln!(ts);

    // remove
    let _ = writeln!(ts, "  async function remove(id: string): Promise<void> {{");
    let _ = writeln!(ts, "    await deleteDoc(doc(db, COLLECTION, id))");
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
            provider: "firebase".into(),
            options: HashMap::new(),
        }
    }

    #[test]
    fn test_firebase_generates_firestore_rules() {
        let rules = generate_firestore_rules(&sample_schema());
        assert_eq!(rules.path, "firebase/firestore.rules");
        assert!(rules.content.contains("rules_version = '2'"));
        assert!(rules.content.contains("service cloud.firestore"));
        assert!(rules.content.contains("match /products/"));
        assert!(rules.content.contains("match /cart_items/"));
    }

    #[test]
    fn test_firebase_type_mapping() {
        assert_eq!(firestore_type(&PgType::Text), "string");
        assert_eq!(firestore_type(&PgType::Integer), "number");
        assert_eq!(firestore_type(&PgType::Float8), "number");
        assert_eq!(firestore_type(&PgType::Boolean), "boolean");
        assert_eq!(firestore_type(&PgType::Timestamptz), "timestamp");
        assert_eq!(firestore_type(&PgType::Jsonb), "map");
    }

    #[test]
    fn test_firebase_auth_components() {
        let provider = FirebaseProvider;
        let auth = provider.generate_auth_components(&sample_schema());
        assert_eq!(auth.len(), 4);
        assert!(auth.iter().any(|f| f.path.contains("AuthProvider")));
        assert!(auth.iter().any(|f| f.path.contains("LoginForm")));
        assert!(auth.iter().any(|f| f.path.contains("SignUpForm")));
        // Should use Firebase Auth SDK
        let login = auth.iter().find(|f| f.path.contains("LoginForm")).unwrap();
        assert!(login.content.contains("signInWithEmailAndPassword"));
    }

    #[test]
    fn test_firebase_crud_hooks() {
        let provider = FirebaseProvider;
        let hooks = provider.generate_data_hooks(&sample_schema());
        assert_eq!(hooks.len(), 2);
        assert!(hooks[0].content.contains("collection(db, COLLECTION)"));
        assert!(hooks[0].content.contains("addDoc"));
        assert!(hooks[0].content.contains("deleteDoc"));
        assert!(hooks[0].content.contains("getDocs"));
    }

    #[test]
    fn test_firebase_security_uses_auth_uid() {
        let rules = generate_firestore_rules(&sample_schema());
        // products has owner_column → rules should reference request.auth.uid
        assert!(rules.content.contains("request.auth.uid"));
        assert!(rules.content.contains("request.auth != null"));
    }

    #[test]
    fn test_firebase_document_structure() {
        // Each table maps to a Firestore collection
        let rules = generate_firestore_rules(&sample_schema());
        assert!(rules.content.contains("match /products/"));
        assert!(rules.content.contains("match /cart_items/"));
    }

    #[test]
    fn test_firebase_client_init() {
        let provider = FirebaseProvider;
        let config = default_config();
        let client = provider.generate_client(&config);
        let firebase_file = client
            .iter()
            .find(|f| f.path.contains("firebase.ts"))
            .unwrap();
        assert!(firebase_file.content.contains("initializeApp"));
        assert!(firebase_file.content.contains("getFirestore"));
        assert!(firebase_file.content.contains("getAuth"));
        assert!(firebase_file.content.contains("VITE_FIREBASE_API_KEY"));
    }

    #[test]
    fn test_firebase_full_generation() {
        let provider = FirebaseProvider;
        let config = default_config();
        let output = provider.generate(&sample_schema(), &config).unwrap();
        assert!(!output.files.is_empty());
        assert!(output.dependencies.iter().any(|(n, _)| n == "firebase"));
    }

    #[test]
    fn test_firebase_owner_rules_use_user_id() {
        let rules = generate_firestore_rules(&sample_schema());
        // products has owner_column = user_id
        assert!(rules
            .content
            .contains("resource.data.user_id == request.auth.uid"));
    }

    #[test]
    fn test_firebase_non_owner_rules_public_read() {
        let rules = generate_firestore_rules(&sample_schema());
        // cart_items has no owner_column → public read
        assert!(rules.content.contains("allow read: if true"));
    }
}
