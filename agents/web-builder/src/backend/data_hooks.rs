//! CRUD Hook Generation — one React hook per table (deterministic).

use super::{GeneratedFile, SchemaSpec};
use std::fmt::Write;

/// Generate React hooks with typed CRUD operations for each table.
pub fn generate_data_hooks(spec: &SchemaSpec) -> Vec<GeneratedFile> {
    spec.tables
        .iter()
        .map(|table| generate_hook_for_table(&table.name, &to_pascal_case(&table.name), table))
        .collect()
}

fn generate_hook_for_table(
    table_name: &str,
    type_name: &str,
    table: &super::TableSpec,
) -> GeneratedFile {
    let hook_name = format!("use{type_name}");
    let mut ts = String::with_capacity(1024);

    // Determine which fields to omit from insert
    let auto_fields: Vec<&str> = table
        .columns
        .iter()
        .filter(|c| c.default.is_some() || c.primary_key)
        .map(|c| c.name.as_str())
        .collect();

    let omit_type = if auto_fields.is_empty() {
        type_name.to_string()
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

    let _ = writeln!(ts, "import {{ supabase }} from '../lib/supabase'");
    let _ = writeln!(ts, "import type {{ {type_name} }} from '../types/database'");
    let _ = writeln!(ts);
    let _ = writeln!(ts, "export function {hook_name}() {{");

    // list
    let _ = writeln!(ts, "  async function list(): Promise<{type_name}[]> {{");
    let _ = writeln!(ts, "    const {{ data, error }} = await supabase");
    let _ = writeln!(ts, "      .from('{table_name}')");
    let _ = writeln!(ts, "      .select('*')");
    let _ = writeln!(ts, "      .order('created_at', {{ ascending: false }})");
    let _ = writeln!(ts, "    if (error) throw error");
    let _ = writeln!(ts, "    return (data ?? []) as {type_name}[]");
    let _ = writeln!(ts, "  }}");
    let _ = writeln!(ts);

    // getById
    let _ = writeln!(
        ts,
        "  async function getById(id: string): Promise<{type_name} | null> {{"
    );
    let _ = writeln!(ts, "    const {{ data, error }} = await supabase");
    let _ = writeln!(ts, "      .from('{table_name}')");
    let _ = writeln!(ts, "      .select('*')");
    let _ = writeln!(ts, "      .eq('id', id)");
    let _ = writeln!(ts, "      .single()");
    let _ = writeln!(ts, "    if (error) throw error");
    let _ = writeln!(ts, "    return data as {type_name} | null");
    let _ = writeln!(ts, "  }}");
    let _ = writeln!(ts);

    // create
    let _ = writeln!(
        ts,
        "  async function create(item: {omit_type}): Promise<{type_name}> {{"
    );
    let _ = writeln!(ts, "    const {{ data, error }} = await supabase");
    let _ = writeln!(ts, "      .from('{table_name}')");
    let _ = writeln!(ts, "      .insert(item)");
    let _ = writeln!(ts, "      .select()");
    let _ = writeln!(ts, "      .single()");
    let _ = writeln!(ts, "    if (error) throw error");
    let _ = writeln!(ts, "    return data as {type_name}");
    let _ = writeln!(ts, "  }}");
    let _ = writeln!(ts);

    // update
    let _ = writeln!(
        ts,
        "  async function update(id: string, updates: Partial<{type_name}>): Promise<{type_name}> {{"
    );
    let _ = writeln!(ts, "    const {{ data, error }} = await supabase");
    let _ = writeln!(ts, "      .from('{table_name}')");
    let _ = writeln!(ts, "      .update(updates)");
    let _ = writeln!(ts, "      .eq('id', id)");
    let _ = writeln!(ts, "      .select()");
    let _ = writeln!(ts, "      .single()");
    let _ = writeln!(ts, "    if (error) throw error");
    let _ = writeln!(ts, "    return data as {type_name}");
    let _ = writeln!(ts, "  }}");
    let _ = writeln!(ts);

    // remove
    let _ = writeln!(ts, "  async function remove(id: string): Promise<void> {{");
    let _ = writeln!(ts, "    const {{ error }} = await supabase");
    let _ = writeln!(ts, "      .from('{table_name}')");
    let _ = writeln!(ts, "      .delete()");
    let _ = writeln!(ts, "      .eq('id', id)");
    let _ = writeln!(ts, "    if (error) throw error");
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

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{ColumnSpec, PgType, TableSpec};

    fn sample_spec() -> SchemaSpec {
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
                            name: "name".into(),
                            data_type: PgType::Text,
                            nullable: false,
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
                },
                TableSpec {
                    name: "cart_items".into(),
                    columns: vec![ColumnSpec {
                        name: "id".into(),
                        data_type: PgType::Uuid,
                        nullable: false,
                        default: Some("gen_random_uuid()".into()),
                        primary_key: true,
                        references: None,
                        unique: false,
                    }],
                    rls_enabled: false,
                    owner_column: None,
                    indexes: vec![],
                },
            ],
            auth_enabled: false,
            storage_buckets: vec![],
        }
    }

    #[test]
    fn test_generates_hook_per_table() {
        let hooks = generate_data_hooks(&sample_spec());
        assert_eq!(hooks.len(), 2);
        assert_eq!(hooks[0].path, "src/hooks/useProducts.ts");
        assert_eq!(hooks[1].path, "src/hooks/useCartItems.ts");
    }

    #[test]
    fn test_hook_has_crud_operations() {
        let hooks = generate_data_hooks(&sample_spec());
        let content = &hooks[0].content;
        assert!(content.contains("async function list()"));
        assert!(content.contains("async function getById(id: string)"));
        assert!(content.contains("async function create("));
        assert!(content.contains("async function update(id: string"));
        assert!(content.contains("async function remove(id: string)"));
    }

    #[test]
    fn test_hook_typed_correctly() {
        let hooks = generate_data_hooks(&sample_spec());
        let content = &hooks[0].content;
        assert!(content.contains("import type { Products }"));
        assert!(content.contains("Promise<Products[]>"));
        assert!(content.contains("Promise<Products | null>"));
    }

    #[test]
    fn test_hook_uses_supabase_client() {
        let hooks = generate_data_hooks(&sample_spec());
        let content = &hooks[0].content;
        assert!(content.contains("import { supabase }"));
        assert!(content.contains(".from('products')"));
    }

    #[test]
    fn test_hook_omits_auto_fields_in_create() {
        let hooks = generate_data_hooks(&sample_spec());
        let content = &hooks[0].content;
        // Should omit id and created_at from create parameter type
        assert!(content.contains("Omit<Products"));
        assert!(content.contains("'id'"));
        assert!(content.contains("'created_at'"));
    }
}
