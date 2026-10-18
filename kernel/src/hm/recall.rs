//! Route-first recall (P3-HM-CONTRACT-02 §12.3).
//!
//! The context's route class is fixed before recall. For a `non_local`
//! unit the query selects, in SQL and before `ORDER BY`, only the agent's
//! rows labelled `UNRESTRICTED_ORIGIN`; the §5.2 binding is then recomputed
//! while walking the ordered cursor, and the walk stops when `limit` bound
//! rows are collected. A withheld row therefore never displaces an eligible
//! one. Under AD-4 no grant is joined. A `local` unit sees every label.
//! Recall narrows what is offered; it never authorises transmission
//! (§12.4 decides that).

use std::fmt;

use nexus_persistence::hm::RecallRow;

use super::context::{DispatchContext, Principal, RecalledRecord, RecordRef, RouteClass};
use super::events;
use super::issuer::DispatchIssuer;
use super::write::is_migrated;
use super::{bind, digest_columns_of, EvaluatedLabel, OriginLabel, RecordTable};

/// What to recall: one in-scope table of the context's own agent, at most
/// `limit` records (capped by `restrictions.max_records`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecallQuery {
    pub table: RecordTable,
    pub limit: usize,
}

/// A refused recall: nothing was read into the working set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecallRefused {
    WrongIssuer,
    NotAnAgent,
    /// The unit carries no retained data (class AC, §12.1).
    RetainedDataNotPermitted,
    NotMigrated,
    Store,
}

impl fmt::Display for RecallRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "recall refused: {self:?}")
    }
}

impl std::error::Error for RecallRefused {}

/// The record as a verbatim item: its content rendered from the
/// digest-covered columns other than the owner.
fn recalled(table: RecordTable, r: RecallRow, label: EvaluatedLabel) -> RecalledRecord {
    let names = digest_columns_of(table);
    let content = names
        .iter()
        .zip(&r.values)
        .filter(|(name, _)| **name != "agent_id")
        .map(|(name, v)| format!("{name}: {}", v.as_deref().unwrap_or("")))
        .collect::<Vec<_>>()
        .join("\n");
    let source_kind = r
        .label
        .as_ref()
        .and_then(|l| l.source_kind.clone())
        .unwrap_or_else(|| "unlabelled".to_string());
    RecalledRecord {
        reference: RecordRef {
            table,
            record_id: r.row.record_id,
        },
        agent_id: r.row.agent_id,
        label,
        source_kind,
        content,
    }
}

/// The evaluated label of a walked row (§5.2).
fn evaluate_row(r: &RecallRow) -> EvaluatedLabel {
    let label = r.label.as_ref().and_then(|l| {
        Some((
            l.agent_id.as_deref()?,
            l.origin.as_deref()?,
            l.content_sha256.as_deref()?,
        ))
    });
    bind(label, &r.row)
}

struct Walked {
    records: Vec<RecalledRecord>,
    /// UNBOUND rows the walk met.
    unbound_met: usize,
    /// Non-local only: rows the query excluded (label missing or not
    /// `UNRESTRICTED_ORIGIN`), and how many of them had no label row.
    excluded: i64,
    unlabelled: i64,
}

impl DispatchIssuer {
    /// Route-first recall of the context's own agent (§12.3). Every
    /// returned record enters the working set as a verbatim item.
    pub fn recall(
        &self,
        ctx: &mut DispatchContext,
        query: RecallQuery,
    ) -> Result<Vec<RecalledRecord>, RecallRefused> {
        if !self.issued(ctx) {
            return Err(RecallRefused::WrongIssuer);
        }
        let Principal::Agent(agent_id) = ctx.principal else {
            return Err(RecallRefused::NotAnAgent);
        };
        if !ctx.restrictions.retained_data_permitted {
            return Err(RecallRefused::RetainedDataNotPermitted);
        }
        let limit = query.limit.min(ctx.restrictions.max_records);
        // Fixed before recall: the route class of the unit at this moment.
        let route = ctx.route_class;
        let table = query.table;
        let agent = agent_id.to_string();
        let unrestricted = OriginLabel::UnrestrictedOrigin.class_name();

        let walked = self
            .db()
            .hm()
            .read(|tx| {
                if !is_migrated(tx)? {
                    return Ok(None);
                }
                let origin_filter = match route {
                    RouteClass::NonLocal => Some(unrestricted),
                    RouteClass::Local => None,
                };
                let mut records = Vec::new();
                let mut unbound_met = 0usize;
                if limit > 0 {
                    tx.recall_walk(table.table_name(), &agent, origin_filter, &mut |r| {
                        let label = evaluate_row(&r);
                        if label == EvaluatedLabel::Unbound {
                            unbound_met += 1;
                            if route == RouteClass::NonLocal {
                                // Withheld: never counted toward the limit.
                                return true;
                            }
                        }
                        records.push(recalled(table, r, label));
                        records.len() < limit
                    })?;
                }
                let (excluded, unlabelled) = match route {
                    RouteClass::NonLocal => {
                        tx.recall_withheld_counts(table.table_name(), &agent, unrestricted)?
                    }
                    RouteClass::Local => (0, 0),
                };
                Ok(Some(Walked {
                    records,
                    unbound_met,
                    excluded,
                    unlabelled,
                }))
            })
            .map_err(|_| RecallRefused::Store)?;
        let Some(w) = walked else {
            return Err(RecallRefused::NotMigrated);
        };

        // Observational, after the read; best effort (§5.3).
        if route == RouteClass::NonLocal {
            events::observational(
                self.db(),
                "memory.recall_filtered",
                Some(&agent),
                serde_json::json!({
                    "route_class": route.class_name(),
                    "eligible_count": w.records.len(),
                    "withheld_count": w.excluded + w.unbound_met as i64,
                }),
            );
        }
        let unbound = w.unbound_met as i64 + w.unlabelled;
        if unbound > 0 {
            events::observational(
                self.db(),
                "memory.unlabelled_record_observed",
                Some(&agent),
                serde_json::json!({
                    "route_class": route.class_name(),
                    "unbound_count": unbound,
                }),
            );
        }

        for r in &w.records {
            ctx.working_set
                .verbatim
                .push((r.reference.clone(), r.label));
        }
        Ok(w.records)
    }
}
