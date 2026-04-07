//! TypeScript Type Generation — SchemaSpec → database.ts (fully deterministic).

use super::{GeneratedFile, SchemaSpec};
use std::fmt::Write;

/// Generate TypeScript interface definitions from a SchemaSpec.
///
/// Fully deterministic — maps PgType → TypeScript type directly.
pub fn generate_typescript_types(spec: &SchemaSpec) -> GeneratedFile {
    let mut ts = String::with_capacity(1024);

    let _ = writeln!(
        ts,
        "// Auto-generated TypeScript types from Nexus Builder backend schema"
    );
    let _ = writeln!(ts, "// Do not edit manually — regenerate via 'Add Backend'");
    let _ = writeln!(ts);

    for table in &spec.tables {
        let interface_name = to_pascal_case(&table.name);
        let _ = writeln!(ts, "export interface {interface_name} {{");

        for col in &table.columns {
            let ts_type = col.data_type.ts_type();
            let optional = if col.nullable { "?" } else { "" };
            let _ = writeln!(ts, "  {}{optional}: {ts_type};", col.name);
        }

        let _ = writeln!(ts, "}}");
        let _ = writeln!(ts);
    }

    // Database type (for Supabase client generic)
    let _ = writeln!(ts, "export interface Database {{");
    let _ = writeln!(ts, "  public: {{");
    let _ = writeln!(ts, "    Tables: {{");
    for table in &spec.tables {
        let interface_name = to_pascal_case(&table.name);
        let _ = writeln!(ts, "      {}: {{", table.name);
        let _ = writeln!(ts, "        Row: {interface_name};");

        // Insert type: omit id, created_at, updated_at
        let _ = write!(ts, "        Insert: Omit<{interface_name}");
        let omit_fields: Vec<&str> = table
            .columns
            .iter()
            .filter(|c| c.default.is_some() || c.primary_key)
            .map(|c| c.name.as_str())
            .collect();
        if !omit_fields.is_empty() {
            let _ = write!(
                ts,
                ", {}",
                omit_fields
                    .iter()
                    .map(|f| format!("'{f}'"))
                    .collect::<Vec<_>>()
                    .join(" | ")
            );
        }
        let _ = writeln!(ts, ">;");

        // Update type: all fields optional except id
        let _ = writeln!(ts, "        Update: Partial<{interface_name}>;");
        let _ = writeln!(ts, "      }};");
    }
    let _ = writeln!(ts, "    }};");
    let _ = writeln!(ts, "  }};");
    let _ = writeln!(ts, "}}");

    GeneratedFile {
        path: "src/types/database.ts".into(),
        content: ts,
    }
}

/// Convert snake_case to PascalCase.
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

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{ColumnSpec, PgType, TableSpec};

    fn sample_spec() -> SchemaSpec {
        SchemaSpec {
            tables: vec![TableSpec {
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
                        name: "description".into(),
                        data_type: PgType::Text,
                        nullable: true,
                        default: None,
                        primary_key: false,
                        references: None,
                        unique: false,
                    },
                    ColumnSpec {
                        name: "active".into(),
                        data_type: PgType::Boolean,
                        nullable: false,
                        default: Some("true".into()),
                        primary_key: false,
                        references: None,
                        unique: false,
                    },
                    ColumnSpec {
                        name: "metadata".into(),
                        data_type: PgType::Jsonb,
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
                ],
                rls_enabled: false,
                owner_column: None,
                indexes: vec![],
            }],
            auth_enabled: false,
            storage_buckets: vec![],
        }
    }

    #[test]
    fn test_generates_interface_per_table() {
        let file = generate_typescript_types(&sample_spec());
        assert!(file.content.contains("export interface Products"));
    }

    #[test]
    fn test_pg_to_ts_type_mapping() {
        let file = generate_typescript_types(&sample_spec());
        // uuid → string
        assert!(file.content.contains("id: string;"));
        // text → string
        assert!(file.content.contains("name: string;"));
        // float8 → number
        assert!(file.content.contains("price: number;"));
        // boolean → boolean
        assert!(file.content.contains("active: boolean;"));
        // timestamptz → string
        assert!(file.content.contains("created_at: string;"));
        // jsonb → Record
        assert!(file.content.contains("metadata?: Record<string, unknown>;"));
    }

    #[test]
    fn test_nullable_columns_optional() {
        let file = generate_typescript_types(&sample_spec());
        // description is nullable → optional
        assert!(file.content.contains("description?: string;"));
        // name is not nullable → required
        assert!(file.content.contains("name: string;"));
        assert!(!file.content.contains("name?: string;"));
    }

    #[test]
    fn test_output_path() {
        let file = generate_typescript_types(&sample_spec());
        assert_eq!(file.path, "src/types/database.ts");
    }

    #[test]
    fn test_database_type_generated() {
        let file = generate_typescript_types(&sample_spec());
        assert!(file.content.contains("export interface Database"));
        assert!(file.content.contains("Row: Products;"));
        assert!(file.content.contains("Insert: Omit<Products"));
        assert!(file.content.contains("Update: Partial<Products>;"));
    }

    #[test]
    fn test_pascal_case_conversion() {
        assert_eq!(to_pascal_case("products"), "Products");
        assert_eq!(to_pascal_case("cart_items"), "CartItems");
        assert_eq!(to_pascal_case("user_profiles"), "UserProfiles");
    }
}
