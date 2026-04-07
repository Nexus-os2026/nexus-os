//! Schema Generation — natural language → SchemaSpec via gemma4:e4b.

use super::{
    BackendError, ColumnSpec, FkAction, ForeignKey, IndexSpec, PgType, SchemaSpec,
    StorageBucketSpec, TableSpec,
};
use nexus_connectors_llm::providers::LlmProvider;
use serde::Deserialize;

/// Prompt for natural language → schema parsing.
fn build_schema_prompt(description: &str) -> String {
    format!(
        r#"You are a database architect. Parse the user's description into a JSON schema specification.
Respond ONLY with a valid JSON object, no markdown, no explanation.

RULES:
- Every table MUST have: id uuid PRIMARY KEY DEFAULT gen_random_uuid(), created_at timestamptz DEFAULT now(), updated_at timestamptz DEFAULT now()
- Tables with user-owned data MUST have: user_id uuid REFERENCES auth.users(id) ON DELETE CASCADE
- Use snake_case for all table and column names
- Table names must NOT be PostgreSQL reserved words
- Set rls_enabled=true for all tables with user data
- Set owner_column to the column that references auth.uid() (usually "user_id")
- If the description mentions auth, login, signup, or users, set auth_enabled=true
- data_type must be one of: Uuid, Text, Integer, Bigint, Float8, Boolean, Timestamptz, Jsonb, Bytea
- on_delete must be one of: Cascade, SetNull, Restrict

USER DESCRIPTION: {description}

RESPONSE FORMAT:
{{
  "tables": [
    {{
      "name": "table_name",
      "columns": [
        {{
          "name": "column_name",
          "data_type": "Text",
          "nullable": false,
          "default": null,
          "primary_key": false,
          "references": null,
          "unique": false
        }}
      ],
      "rls_enabled": true,
      "owner_column": "user_id",
      "indexes": [{{ "columns": ["user_id"], "unique": false }}]
    }}
  ],
  "auth_enabled": true,
  "storage_buckets": []
}}"#
    )
}

/// Parse response from LLM.
#[derive(Debug, Deserialize)]
struct LlmSchemaResponse {
    tables: Vec<LlmTableSpec>,
    auth_enabled: Option<bool>,
    storage_buckets: Option<Vec<LlmBucketSpec>>,
}

#[derive(Debug, Deserialize)]
struct LlmTableSpec {
    name: String,
    columns: Vec<LlmColumnSpec>,
    rls_enabled: Option<bool>,
    owner_column: Option<String>,
    indexes: Option<Vec<LlmIndexSpec>>,
}

#[derive(Debug, Deserialize)]
struct LlmColumnSpec {
    name: String,
    data_type: String,
    nullable: Option<bool>,
    default: Option<String>,
    primary_key: Option<bool>,
    references: Option<LlmForeignKey>,
    unique: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct LlmForeignKey {
    table: String,
    column: String,
    on_delete: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LlmIndexSpec {
    columns: Vec<String>,
    unique: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct LlmBucketSpec {
    name: String,
    public: Option<bool>,
    allowed_mime_types: Option<Vec<String>>,
    max_file_size_mb: Option<u32>,
}

/// Parse data type string from LLM response.
fn parse_pg_type(s: &str) -> PgType {
    match s.to_lowercase().as_str() {
        "uuid" => PgType::Uuid,
        "text" | "varchar" | "string" => PgType::Text,
        "integer" | "int" | "int4" => PgType::Integer,
        "bigint" | "int8" => PgType::Bigint,
        "float8" | "float" | "double" | "numeric" | "decimal" => PgType::Float8,
        "boolean" | "bool" => PgType::Boolean,
        "timestamptz" | "timestamp" | "datetime" => PgType::Timestamptz,
        "jsonb" | "json" => PgType::Jsonb,
        "bytea" | "bytes" | "binary" => PgType::Bytea,
        _ => PgType::Text, // safe fallback
    }
}

fn parse_fk_action(s: &str) -> FkAction {
    match s.to_lowercase().as_str() {
        "cascade" => FkAction::Cascade,
        "setnull" | "set_null" | "set null" => FkAction::SetNull,
        "restrict" => FkAction::Restrict,
        _ => FkAction::Cascade,
    }
}

/// Convert LLM response into validated SchemaSpec.
fn convert_llm_response(resp: LlmSchemaResponse) -> SchemaSpec {
    let tables = resp
        .tables
        .into_iter()
        .map(|t| {
            let columns = t
                .columns
                .into_iter()
                .map(|c| ColumnSpec {
                    name: c.name,
                    data_type: parse_pg_type(&c.data_type),
                    nullable: c.nullable.unwrap_or(false),
                    default: c.default,
                    primary_key: c.primary_key.unwrap_or(false),
                    references: c.references.map(|fk| ForeignKey {
                        table: fk.table,
                        column: fk.column,
                        on_delete: parse_fk_action(&fk.on_delete.unwrap_or_default()),
                    }),
                    unique: c.unique.unwrap_or(false),
                })
                .collect();

            let indexes = t
                .indexes
                .unwrap_or_default()
                .into_iter()
                .map(|i| IndexSpec {
                    columns: i.columns,
                    unique: i.unique.unwrap_or(false),
                })
                .collect();

            TableSpec {
                name: t.name,
                columns,
                rls_enabled: t.rls_enabled.unwrap_or(false),
                owner_column: t.owner_column,
                indexes,
            }
        })
        .collect();

    let storage_buckets = resp
        .storage_buckets
        .unwrap_or_default()
        .into_iter()
        .map(|b| StorageBucketSpec {
            name: b.name,
            public: b.public.unwrap_or(false),
            allowed_mime_types: b.allowed_mime_types.unwrap_or_default(),
            max_file_size_mb: b.max_file_size_mb.unwrap_or(10),
        })
        .collect();

    SchemaSpec {
        tables,
        auth_enabled: resp.auth_enabled.unwrap_or(false),
        storage_buckets,
    }
}

/// Parse a natural language description into a SchemaSpec using an LLM.
pub fn parse_schema_description(
    description: &str,
    provider: &dyn LlmProvider,
) -> Result<SchemaSpec, BackendError> {
    let prompt = build_schema_prompt(description);

    let response = provider
        .query(&prompt, 4096, crate::model_router::OLLAMA_LARGE)
        .map_err(|e| BackendError::SchemaGen(format!("LLM query failed: {e}")))?;

    let text = response.output_text.trim();
    // Strip markdown fences
    let json_text = if text.starts_with("```") {
        let after = text.find('\n').map(|i| &text[i + 1..]).unwrap_or(text);
        after.trim_end().strip_suffix("```").unwrap_or(after).trim()
    } else {
        text
    };

    let parsed: LlmSchemaResponse = serde_json::from_str(json_text)
        .map_err(|e| BackendError::SchemaGen(format!("JSON parse failed: {e}")))?;

    let spec = convert_llm_response(parsed);

    // Validate
    super::validate_schema(&spec).map_err(|errors| {
        BackendError::SchemaGen(format!("validation failed: {}", errors.join("; ")))
    })?;

    Ok(spec)
}

/// Build a SchemaSpec from a structured description (no LLM needed, for testing/fallback).
pub fn build_schema_from_tables(
    table_defs: Vec<TableSpec>,
    auth_enabled: bool,
) -> Result<SchemaSpec, BackendError> {
    let spec = SchemaSpec {
        tables: table_defs,
        auth_enabled,
        storage_buckets: vec![],
    };
    super::validate_schema(&spec)
        .map_err(|e| BackendError::SchemaGen(format!("validation: {}", e.join("; "))))?;
    Ok(spec)
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pg_type_variants() {
        assert_eq!(parse_pg_type("Uuid"), PgType::Uuid);
        assert_eq!(parse_pg_type("text"), PgType::Text);
        assert_eq!(parse_pg_type("varchar"), PgType::Text);
        assert_eq!(parse_pg_type("Integer"), PgType::Integer);
        assert_eq!(parse_pg_type("float8"), PgType::Float8);
        assert_eq!(parse_pg_type("Boolean"), PgType::Boolean);
        assert_eq!(parse_pg_type("timestamptz"), PgType::Timestamptz);
        assert_eq!(parse_pg_type("jsonb"), PgType::Jsonb);
        assert_eq!(parse_pg_type("unknown"), PgType::Text);
    }

    #[test]
    fn test_parse_fk_action_variants() {
        assert_eq!(parse_fk_action("Cascade"), FkAction::Cascade);
        assert_eq!(parse_fk_action("set_null"), FkAction::SetNull);
        assert_eq!(parse_fk_action("Restrict"), FkAction::Restrict);
        assert_eq!(parse_fk_action("unknown"), FkAction::Cascade);
    }

    #[test]
    fn test_convert_llm_response_simple() {
        let resp = LlmSchemaResponse {
            tables: vec![LlmTableSpec {
                name: "products".into(),
                columns: vec![
                    LlmColumnSpec {
                        name: "id".into(),
                        data_type: "Uuid".into(),
                        nullable: Some(false),
                        default: Some("gen_random_uuid()".into()),
                        primary_key: Some(true),
                        references: None,
                        unique: None,
                    },
                    LlmColumnSpec {
                        name: "name".into(),
                        data_type: "Text".into(),
                        nullable: Some(false),
                        default: None,
                        primary_key: None,
                        references: None,
                        unique: None,
                    },
                    LlmColumnSpec {
                        name: "price".into(),
                        data_type: "Float8".into(),
                        nullable: Some(false),
                        default: Some("0".into()),
                        primary_key: None,
                        references: None,
                        unique: None,
                    },
                ],
                rls_enabled: Some(true),
                owner_column: None,
                indexes: None,
            }],
            auth_enabled: Some(true),
            storage_buckets: None,
        };

        let spec = convert_llm_response(resp);
        assert_eq!(spec.tables.len(), 1);
        assert!(spec.auth_enabled);
        assert_eq!(spec.tables[0].name, "products");
        assert_eq!(spec.tables[0].columns.len(), 3);
        assert!(spec.tables[0].rls_enabled);
    }

    #[test]
    fn test_parse_with_auth_keywords() {
        // The prompt builder includes auth detection
        let prompt = build_schema_prompt("I need user authentication and a products table");
        assert!(prompt.contains("auth"));
        assert!(prompt.contains("auth_enabled"));
    }

    #[test]
    fn test_parse_with_relationships() {
        let resp = LlmSchemaResponse {
            tables: vec![
                LlmTableSpec {
                    name: "users_extra".into(),
                    columns: vec![LlmColumnSpec {
                        name: "id".into(),
                        data_type: "Uuid".into(),
                        nullable: Some(false),
                        default: Some("gen_random_uuid()".into()),
                        primary_key: Some(true),
                        references: None,
                        unique: None,
                    }],
                    rls_enabled: None,
                    owner_column: None,
                    indexes: None,
                },
                LlmTableSpec {
                    name: "posts".into(),
                    columns: vec![
                        LlmColumnSpec {
                            name: "id".into(),
                            data_type: "Uuid".into(),
                            nullable: Some(false),
                            default: Some("gen_random_uuid()".into()),
                            primary_key: Some(true),
                            references: None,
                            unique: None,
                        },
                        LlmColumnSpec {
                            name: "author_id".into(),
                            data_type: "Uuid".into(),
                            nullable: Some(false),
                            default: None,
                            primary_key: None,
                            references: Some(LlmForeignKey {
                                table: "users_extra".into(),
                                column: "id".into(),
                                on_delete: Some("Cascade".into()),
                            }),
                            unique: None,
                        },
                    ],
                    rls_enabled: Some(true),
                    owner_column: Some("author_id".into()),
                    indexes: None,
                },
            ],
            auth_enabled: Some(false),
            storage_buckets: None,
        };

        let spec = convert_llm_response(resp);
        assert_eq!(spec.tables.len(), 2);
        let posts = &spec.tables[1];
        let author_col = posts
            .columns
            .iter()
            .find(|c| c.name == "author_id")
            .unwrap();
        assert!(author_col.references.is_some());
        let fk = author_col.references.as_ref().unwrap();
        assert_eq!(fk.table, "users_extra");
        assert_eq!(fk.on_delete, FkAction::Cascade);
    }

    #[test]
    fn test_adds_standard_columns_validation() {
        // Schema with standard id/created_at/updated_at passes validation
        let spec = SchemaSpec {
            tables: vec![TableSpec {
                name: "items".into(),
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
            }],
            auth_enabled: false,
            storage_buckets: vec![],
        };
        assert!(super::super::validate_schema(&spec).is_ok());
    }

    #[test]
    fn test_build_schema_from_tables() {
        let tables = vec![TableSpec {
            name: "items".into(),
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
        }];
        let result = build_schema_from_tables(tables, true);
        assert!(result.is_ok());
        assert!(result.unwrap().auth_enabled);
    }
}
