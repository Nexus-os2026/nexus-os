//! RLS Policy Generation — SchemaSpec → Row Level Security policies.
//!
//! **SECURITY-CRITICAL**: Uses Sonnet when available. Falls back to deterministic
//! template-based generation (not an LLM fallback — a safe, conservative default).
//! A wrong RLS policy exposes user data. We never ship unvalidated RLS.

use super::{MigrationFile, SchemaSpec};
use crate::model_router::SONNET;
use nexus_connectors_llm::providers::LlmProvider;
use std::fmt::Write;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RlsGenError {
    #[error("RLS generation failed: {0}")]
    GenerationFailed(String),
    #[error("RLS validation failed: {0}")]
    ValidationFailed(String),
}

/// Generate RLS policies for all tables with rls_enabled=true.
///
/// If a Sonnet-capable provider is given, uses it for nuanced policies.
/// Otherwise falls back to safe, deterministic template-based policies.
pub fn generate_rls_policies(
    spec: &SchemaSpec,
    provider: Option<&dyn LlmProvider>,
) -> Result<Vec<MigrationFile>, RlsGenError> {
    let rls_tables: Vec<_> = spec.tables.iter().filter(|t| t.rls_enabled).collect();

    if rls_tables.is_empty() {
        return Ok(vec![]);
    }

    if let Some(prov) = provider {
        // Try Sonnet-based generation
        match generate_rls_with_sonnet(spec, prov) {
            Ok(migrations) => {
                // Validate every migration
                validate_rls_policies(&migrations, spec)?;
                return Ok(migrations);
            }
            Err(e) => {
                eprintln!("[rls-gen] Sonnet generation failed, using deterministic fallback: {e}");
            }
        }
    }

    // Deterministic fallback — safe, conservative policies
    let migrations = generate_rls_deterministic(spec);
    validate_rls_policies(&migrations, spec)?;
    Ok(migrations)
}

/// Generate RLS using Sonnet (security-critical model).
fn generate_rls_with_sonnet(
    spec: &SchemaSpec,
    provider: &dyn LlmProvider,
) -> Result<Vec<MigrationFile>, RlsGenError> {
    let prompt = build_rls_prompt(spec);

    let response = provider
        .query(&prompt, 4096, SONNET)
        .map_err(|e| RlsGenError::GenerationFailed(format!("Sonnet query failed: {e}")))?;

    let text = response.output_text.trim();
    // Strip markdown fences
    let sql = if text.starts_with("```") {
        let after = text.find('\n').map(|i| &text[i + 1..]).unwrap_or(text);
        after.trim_end().strip_suffix("```").unwrap_or(after).trim()
    } else {
        text
    };

    // Split into per-table migrations
    let rls_tables: Vec<_> = spec.tables.iter().filter(|t| t.rls_enabled).collect();
    let base_num = spec.tables.len() as u32 + 1;

    let mut migrations = Vec::new();
    // If Sonnet returned a single SQL block, split by table or use as one migration
    if rls_tables.len() == 1 {
        migrations.push(MigrationFile {
            filename: format!("{:03}_rls_{}.sql", base_num, rls_tables[0].name),
            sql: sql.to_string(),
            description: format!("RLS policies for {}", rls_tables[0].name),
        });
    } else {
        // Try to split by "-- RLS" comments or table names
        let mut current_sql = String::new();
        let mut current_table_idx = 0;

        for line in sql.lines() {
            // Check if this line starts a new table's policies
            let switches_table = rls_tables
                .iter()
                .position(|t| {
                    line.to_lowercase()
                        .contains(&format!("policy on public.{}", t.name))
                        || line.to_lowercase().contains(&format!("-- rls {}", t.name))
                        || line
                            .to_lowercase()
                            .contains(&format!("-- {} policies", t.name))
                })
                .filter(|&idx| idx > current_table_idx);

            if let Some(new_idx) = switches_table {
                if !current_sql.trim().is_empty() {
                    let t = rls_tables[current_table_idx];
                    migrations.push(MigrationFile {
                        filename: format!(
                            "{:03}_rls_{}.sql",
                            base_num + current_table_idx as u32,
                            t.name
                        ),
                        sql: current_sql.clone(),
                        description: format!("RLS policies for {}", t.name),
                    });
                }
                current_sql.clear();
                current_table_idx = new_idx;
            }
            let _ = writeln!(current_sql, "{line}");
        }

        // Flush last
        if !current_sql.trim().is_empty() && current_table_idx < rls_tables.len() {
            let t = rls_tables[current_table_idx];
            migrations.push(MigrationFile {
                filename: format!(
                    "{:03}_rls_{}.sql",
                    base_num + current_table_idx as u32,
                    t.name
                ),
                sql: current_sql,
                description: format!("RLS policies for {}", t.name),
            });
        }

        // If splitting failed, put everything in one file
        if migrations.is_empty() {
            migrations.push(MigrationFile {
                filename: format!("{base_num:03}_rls_all.sql"),
                sql: sql.to_string(),
                description: "RLS policies for all tables".into(),
            });
        }
    }

    Ok(migrations)
}

/// Build the RLS prompt for Sonnet.
fn build_rls_prompt(spec: &SchemaSpec) -> String {
    let mut prompt = String::with_capacity(2048);
    let _ = writeln!(
        prompt,
        "You are a security engineer specializing in PostgreSQL Row Level Security."
    );
    let _ = writeln!(
        prompt,
        "Generate RLS policies for the following database schema."
    );
    let _ = writeln!(
        prompt,
        "Respond ONLY with SQL statements, no markdown fences, no explanation.\n"
    );

    let _ = writeln!(prompt, "SCHEMA:");
    for table in &spec.tables {
        if !table.rls_enabled {
            continue;
        }
        let cols: Vec<String> = table
            .columns
            .iter()
            .map(|c| format!("{} {}", c.name, c.data_type.sql_name()))
            .collect();
        let _ = writeln!(
            prompt,
            "  TABLE public.{} ({})",
            table.name,
            cols.join(", ")
        );
        if let Some(ref owner) = table.owner_column {
            let _ = writeln!(prompt, "    owner_column: {owner} (references auth.uid())");
        }
    }

    let _ = writeln!(prompt, "\nRULES:");
    let _ = writeln!(
        prompt,
        "- Every policy MUST use auth.uid() for access checks (never hardcoded UUIDs)"
    );
    let _ = writeln!(
        prompt,
        "- Tables with owner_column: owner can SELECT/INSERT/UPDATE/DELETE their own rows"
    );
    let _ = writeln!(prompt, "- Use USING clause for SELECT/DELETE policies");
    let _ = writeln!(prompt, "- Use WITH CHECK clause for INSERT/UPDATE policies");
    let _ = writeln!(prompt, "- Policy names: {{table}}_select_own, {{table}}_insert_own, {{table}}_update_own, {{table}}_delete_own");
    let _ = writeln!(
        prompt,
        "- Each CREATE POLICY must specify the table with ON public.{{table}}"
    );
    let _ = writeln!(
        prompt,
        "- Do NOT create PERMISSIVE policies without a USING check"
    );

    let _ = writeln!(prompt, "\nEXAMPLE:");
    let _ = writeln!(
        prompt,
        "CREATE POLICY products_select_own ON public.products"
    );
    let _ = writeln!(prompt, "    FOR SELECT USING (auth.uid() = user_id);");
    let _ = writeln!(
        prompt,
        "CREATE POLICY products_insert_own ON public.products"
    );
    let _ = writeln!(prompt, "    FOR INSERT WITH CHECK (auth.uid() = user_id);");

    prompt
}

/// Deterministic fallback: generate safe, conservative RLS policies.
///
/// Pattern: owner-only access for all operations on tables with owner_column.
fn generate_rls_deterministic(spec: &SchemaSpec) -> Vec<MigrationFile> {
    let rls_tables: Vec<_> = spec.tables.iter().filter(|t| t.rls_enabled).collect();
    let base_num = spec.tables.len() as u32 + 1;
    let mut migrations = Vec::new();

    for (i, table) in rls_tables.iter().enumerate() {
        let mut sql = String::with_capacity(512);
        let _ = writeln!(sql, "-- RLS policies for {}", table.name);
        let _ = writeln!(
            sql,
            "-- Generated deterministically (model: none, cost: $0)"
        );
        let _ = writeln!(sql);

        let owner_col = table.owner_column.as_deref().unwrap_or("user_id");

        // SELECT: owner can read own rows
        let _ = writeln!(
            sql,
            "CREATE POLICY {name}_select_own ON public.{name}",
            name = table.name
        );
        let _ = writeln!(sql, "    FOR SELECT USING (auth.uid() = {owner_col});");
        let _ = writeln!(sql);

        // INSERT: owner can insert own rows
        let _ = writeln!(
            sql,
            "CREATE POLICY {name}_insert_own ON public.{name}",
            name = table.name
        );
        let _ = writeln!(sql, "    FOR INSERT WITH CHECK (auth.uid() = {owner_col});");
        let _ = writeln!(sql);

        // UPDATE: owner can update own rows
        let _ = writeln!(
            sql,
            "CREATE POLICY {name}_update_own ON public.{name}",
            name = table.name
        );
        let _ = writeln!(
            sql,
            "    FOR UPDATE USING (auth.uid() = {owner_col}) WITH CHECK (auth.uid() = {owner_col});"
        );
        let _ = writeln!(sql);

        // DELETE: owner can delete own rows
        let _ = writeln!(
            sql,
            "CREATE POLICY {name}_delete_own ON public.{name}",
            name = table.name
        );
        let _ = writeln!(sql, "    FOR DELETE USING (auth.uid() = {owner_col});");

        migrations.push(MigrationFile {
            filename: format!("{:03}_rls_{}.sql", base_num + i as u32, table.name),
            sql,
            description: format!("RLS policies for {} (deterministic)", table.name),
        });
    }

    migrations
}

/// Validate generated RLS policies for safety.
fn validate_rls_policies(
    migrations: &[MigrationFile],
    spec: &SchemaSpec,
) -> Result<(), RlsGenError> {
    let rls_tables: Vec<_> = spec
        .tables
        .iter()
        .filter(|t| t.rls_enabled)
        .map(|t| t.name.as_str())
        .collect();

    // Every RLS policy must reference auth.uid()
    for m in migrations {
        if !m.sql.contains("auth.uid()") {
            return Err(RlsGenError::ValidationFailed(format!(
                "migration '{}' does not reference auth.uid() — unsafe",
                m.filename
            )));
        }
    }

    // Every rls_enabled table should have at least one policy
    let all_sql: String = migrations
        .iter()
        .map(|m| m.sql.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    for table_name in &rls_tables {
        if !all_sql.contains(&format!("public.{table_name}")) {
            return Err(RlsGenError::ValidationFailed(format!(
                "table '{table_name}' has rls_enabled but no policies generated"
            )));
        }
    }

    // No bare PERMISSIVE without a check
    let lower = all_sql.to_lowercase();
    if lower.contains("permissive") && !lower.contains("using") {
        return Err(RlsGenError::ValidationFailed(
            "PERMISSIVE policy without USING clause detected — unsafe".into(),
        ));
    }

    Ok(())
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{ColumnSpec, FkAction, ForeignKey, PgType, TableSpec};

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
                    ],
                    rls_enabled: true,
                    owner_column: Some("user_id".into()),
                    indexes: vec![],
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
                    ],
                    rls_enabled: true,
                    owner_column: Some("user_id".into()),
                    indexes: vec![],
                },
            ],
            auth_enabled: true,
            storage_buckets: vec![],
        }
    }

    #[test]
    fn test_rls_references_auth_uid() {
        let migrations = generate_rls_deterministic(&sample_spec());
        for m in &migrations {
            assert!(
                m.sql.contains("auth.uid()"),
                "migration '{}' must reference auth.uid()",
                m.filename
            );
        }
    }

    #[test]
    fn test_rls_covers_all_tables() {
        let spec = sample_spec();
        let migrations = generate_rls_deterministic(&spec);
        // Both products and cart_items should have policies
        let all_sql: String = migrations
            .iter()
            .map(|m| m.sql.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(all_sql.contains("public.products"));
        assert!(all_sql.contains("public.cart_items"));
    }

    #[test]
    fn test_rls_no_permissive_without_check() {
        let migrations = generate_rls_deterministic(&sample_spec());
        let result = validate_rls_policies(&migrations, &sample_spec());
        assert!(
            result.is_ok(),
            "deterministic policies should pass validation"
        );
    }

    #[test]
    fn test_owner_only_pattern() {
        let migrations = generate_rls_deterministic(&sample_spec());
        let sql = &migrations[0].sql;
        assert!(
            sql.contains("USING (auth.uid() = user_id)"),
            "should have owner-only USING clause"
        );
    }

    #[test]
    fn test_rls_has_crud_policies() {
        let migrations = generate_rls_deterministic(&sample_spec());
        let sql = &migrations[0].sql;
        assert!(sql.contains("FOR SELECT"), "should have SELECT policy");
        assert!(sql.contains("FOR INSERT"), "should have INSERT policy");
        assert!(sql.contains("FOR UPDATE"), "should have UPDATE policy");
        assert!(sql.contains("FOR DELETE"), "should have DELETE policy");
    }

    #[test]
    fn test_rls_validation_rejects_missing_auth_uid() {
        let bad_migration = vec![MigrationFile {
            filename: "bad.sql".into(),
            sql: "CREATE POLICY open ON public.products FOR SELECT USING (true);".into(),
            description: "bad".into(),
        }];
        let spec = sample_spec();
        let result = validate_rls_policies(&bad_migration, &spec);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("auth.uid()"));
    }

    #[test]
    fn test_rls_migration_numbering() {
        let spec = sample_spec();
        let migrations = generate_rls_deterministic(&spec);
        // Base num = spec.tables.len() + 1 = 3
        assert_eq!(migrations[0].filename, "003_rls_products.sql");
        assert_eq!(migrations[1].filename, "004_rls_cart_items.sql");
    }

    #[test]
    fn test_generate_rls_policies_no_provider() {
        let spec = sample_spec();
        let result = generate_rls_policies(&spec, None);
        assert!(result.is_ok());
        let migrations = result.unwrap();
        assert_eq!(migrations.len(), 2);
    }

    #[test]
    fn test_rls_prompt_contains_security_rules() {
        let spec = sample_spec();
        let prompt = build_rls_prompt(&spec);
        assert!(prompt.contains("auth.uid()"));
        assert!(prompt.contains("PERMISSIVE"));
        assert!(prompt.contains("USING"));
        assert!(prompt.contains("WITH CHECK"));
    }
}
