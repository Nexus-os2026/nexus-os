//! SQL Migration Generation — SchemaSpec → SQL files (deterministic, no LLM).

use super::{MigrationFile, SchemaSpec};
use std::fmt::Write;

/// Generate CREATE TABLE SQL migrations from a SchemaSpec.
///
/// Fully deterministic — no LLM needed once the spec is validated.
pub fn generate_migrations(spec: &SchemaSpec) -> Vec<MigrationFile> {
    let mut migrations = Vec::new();
    let mut counter = 1u32;

    for table in &spec.tables {
        let mut sql = String::with_capacity(1024);

        // CREATE TABLE
        let _ = writeln!(sql, "-- {counter:03}_create_{}.sql", table.name);
        let _ = writeln!(sql, "CREATE TABLE IF NOT EXISTS public.{} (", table.name);

        // Columns
        let col_count = table.columns.len();
        for (i, col) in table.columns.iter().enumerate() {
            let _ = write!(sql, "    {} {}", col.name, col.data_type.sql_name());

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
                let _ = write!(sql, " DEFAULT {default}");
            }
            if let Some(ref fk) = col.references {
                let _ = write!(
                    sql,
                    " REFERENCES {}({}) ON DELETE {}",
                    fk.table,
                    fk.column,
                    fk.on_delete.sql()
                );
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
                    "\nCREATE UNIQUE INDEX IF NOT EXISTS {idx_name} ON public.{}({cols});",
                    table.name
                );
            } else {
                let _ = writeln!(
                    sql,
                    "\nCREATE INDEX IF NOT EXISTS {idx_name} ON public.{}({cols});",
                    table.name
                );
            }
        }

        // Enable RLS
        if table.rls_enabled {
            let _ = writeln!(
                sql,
                "\nALTER TABLE public.{} ENABLE ROW LEVEL SECURITY;",
                table.name
            );
        }

        // Updated_at trigger
        let has_updated_at = table.columns.iter().any(|c| c.name == "updated_at");
        if has_updated_at {
            let _ = writeln!(sql, "\n-- Auto-update updated_at timestamp");
            let _ = writeln!(
                sql,
                "CREATE OR REPLACE FUNCTION public.update_{}_updated_at()",
                table.name
            );
            let _ = writeln!(sql, "RETURNS trigger AS $$");
            let _ = writeln!(sql, "BEGIN");
            let _ = writeln!(sql, "    NEW.updated_at = now();");
            let _ = writeln!(sql, "    RETURN NEW;");
            let _ = writeln!(sql, "END;");
            let _ = writeln!(sql, "$$ LANGUAGE plpgsql;");
            let _ = writeln!(sql);
            let _ = writeln!(
                sql,
                "DROP TRIGGER IF EXISTS trigger_{}_updated_at ON public.{};",
                table.name, table.name
            );
            let _ = writeln!(sql, "CREATE TRIGGER trigger_{}_updated_at", table.name);
            let _ = writeln!(sql, "    BEFORE UPDATE ON public.{}", table.name);
            let _ = writeln!(sql, "    FOR EACH ROW");
            let _ = writeln!(
                sql,
                "    EXECUTE FUNCTION public.update_{}_updated_at();",
                table.name
            );
        }

        migrations.push(MigrationFile {
            filename: format!("{counter:03}_create_{}.sql", table.name),
            sql,
            description: format!("Create {} table", table.name),
        });

        counter += 1;
    }

    migrations
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{ColumnSpec, FkAction, ForeignKey, IndexSpec, PgType, TableSpec};

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
                    ],
                    rls_enabled: false,
                    owner_column: None,
                    indexes: vec![IndexSpec {
                        columns: vec!["product_id".into()],
                        unique: false,
                    }],
                },
            ],
            auth_enabled: true,
            storage_buckets: vec![],
        }
    }

    #[test]
    fn test_generates_create_table() {
        let migrations = generate_migrations(&sample_spec());
        assert!(!migrations.is_empty());
        let sql = &migrations[0].sql;
        assert!(
            sql.contains("CREATE TABLE IF NOT EXISTS public.products"),
            "should have CREATE TABLE"
        );
        assert!(sql.contains("name text NOT NULL"), "should have columns");
    }

    #[test]
    fn test_generates_indexes() {
        let migrations = generate_migrations(&sample_spec());
        let sql = &migrations[0].sql;
        assert!(
            sql.contains("CREATE INDEX IF NOT EXISTS idx_products_user_id"),
            "should create index on FK column"
        );
    }

    #[test]
    fn test_generates_rls_enable() {
        let migrations = generate_migrations(&sample_spec());
        let sql = &migrations[0].sql;
        assert!(
            sql.contains("ENABLE ROW LEVEL SECURITY"),
            "should enable RLS on products"
        );
        // cart_items has rls_enabled = false
        let sql2 = &migrations[1].sql;
        assert!(
            !sql2.contains("ENABLE ROW LEVEL SECURITY"),
            "should NOT enable RLS on cart_items"
        );
    }

    #[test]
    fn test_migration_numbering() {
        let migrations = generate_migrations(&sample_spec());
        assert_eq!(migrations[0].filename, "001_create_products.sql");
        assert_eq!(migrations[1].filename, "002_create_cart_items.sql");
    }

    #[test]
    fn test_if_not_exists() {
        let migrations = generate_migrations(&sample_spec());
        for m in &migrations {
            assert!(
                m.sql.contains("IF NOT EXISTS"),
                "migration {} should use IF NOT EXISTS",
                m.filename
            );
        }
    }

    #[test]
    fn test_foreign_keys_correct() {
        let migrations = generate_migrations(&sample_spec());
        let sql = &migrations[0].sql;
        assert!(
            sql.contains("REFERENCES auth.users(id) ON DELETE CASCADE"),
            "should have correct FK reference"
        );
        let sql2 = &migrations[1].sql;
        assert!(
            sql2.contains("REFERENCES products(id) ON DELETE CASCADE"),
            "should reference products"
        );
    }

    #[test]
    fn test_updated_at_trigger() {
        let migrations = generate_migrations(&sample_spec());
        let sql = &migrations[0].sql;
        assert!(
            sql.contains("EXECUTE FUNCTION public.update_products_updated_at()"),
            "should have updated_at trigger"
        );
    }
}
