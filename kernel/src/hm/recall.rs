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

use nexus_persistence::hm::{RecallFilter, RecallRow};

use super::context::{DispatchContext, Principal, RecalledRecord, RecordRef, RouteClass};
use super::events;
use super::issuer::DispatchIssuer;
use super::write::{is_migrated, SourceKind};
use super::{bind, digest_columns_of, EvaluatedLabel, OriginLabel, RecordTable};

/// What to recall: one in-scope table of the context's own agent, at most
/// `limit` records (capped by `restrictions.max_records`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecallQuery {
    pub table: RecordTable,
    pub limit: usize,
}

/// A route-first recall with the search semantics of its read site
/// (mission R8): the eligibility filter of the route and every narrowing
/// condition below are applied in the query before the ORDER BY; the §5.2
/// binding is checked while walking the ordered rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecallSpec<'a> {
    pub table: RecordTable,
    /// Records returned, at most `restrictions.max_records`.
    pub limit: usize,
    /// `column = value` on a content column (for example
    /// `agent_memory.memory_type`, `strategy_scores.goal_type`).
    pub column_eq: Option<(&'a str, &'a str)>,
    /// The record's role, read from its label (§6.6).
    pub source_kind: Option<SourceKind>,
    /// A content column must contain this text (compared lowercased).
    pub contains: Option<(&'a str, &'a str)>,
    /// `agent_memory` keyword ranking (the semantics of
    /// `AgentMemoryManager::recall_relevant`): `limit * 3` eligible
    /// candidates are read in recall order, ranked by how many of these
    /// words their key and value contain (stable), and the best `limit` are
    /// returned. Only the returned records enter the working set.
    pub rank_words: Option<&'a str>,
    /// Newest record first instead of the table's recall order.
    pub newest_first: bool,
}

impl<'a> RecallSpec<'a> {
    /// The plain recall of a table: its recall order, no condition.
    pub fn table(table: RecordTable, limit: usize) -> Self {
        Self {
            table,
            limit,
            column_eq: None,
            source_kind: None,
            contains: None,
            rank_words: None,
            newest_first: false,
        }
    }

    /// `agent_memory` rows of one `memory_type`, newest first.
    pub fn memory_type(memory_type: &'a str, limit: usize) -> Self {
        Self {
            column_eq: Some(("memory_type", memory_type)),
            newest_first: true,
            ..Self::table(RecordTable::AgentMemory, limit)
        }
    }
}

/// A record read to decide how the backend acts (a route, a parameter, a
/// strategy to record), never placed in a prompt and never added to the
/// working set. It is read route-first like any recall: a non-local unit
/// reads only eligible records. A write whose content comes from it names
/// it as a parent, so its label joins (§6.3).
pub struct ControlRecord {
    reference: RecordRef,
    label: EvaluatedLabel,
    values: Vec<(&'static str, Option<String>)>,
}

/// Content-free: the values are never formatted.
impl std::fmt::Debug for ControlRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ControlRecord")
            .field("reference", &self.reference)
            .field("label", &self.label)
            .finish_non_exhaustive()
    }
}

impl ControlRecord {
    pub fn record_ref(&self) -> &RecordRef {
        &self.reference
    }

    /// The label evaluated when the record was read.
    pub fn label(&self) -> EvaluatedLabel {
        self.label
    }

    /// One digest-covered column of the record (never its owner).
    pub fn field(&self, column: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|(name, _)| *name == column && *name != "agent_id")
            .and_then(|(_, v)| v.as_deref())
    }
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

/// The record as a control value: its digest-covered columns by name.
fn control(table: RecordTable, r: RecallRow, label: EvaluatedLabel) -> ControlRecord {
    let names = digest_columns_of(table);
    ControlRecord {
        reference: RecordRef {
            table,
            record_id: r.row.record_id,
        },
        label,
        values: names.iter().copied().zip(r.values).collect(),
    }
}

/// `AgentMemoryManager::recall_relevant`'s keyword score of an
/// `agent_memory` row: how many of `words` its lowercased key and value
/// contain.
fn keyword_score(table: RecordTable, r: &RecallRow, words: &[String]) -> usize {
    let names = digest_columns_of(table);
    let column = |name: &str| {
        names
            .iter()
            .position(|n| *n == name)
            .and_then(|i| r.values.get(i).cloned().flatten())
            .unwrap_or_default()
            .to_lowercase()
    };
    let combined = format!("{} {}", column("key"), column("value_json"));
    words
        .iter()
        .filter(|w| combined.contains(w.as_str()))
        .count()
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

impl DispatchIssuer {
    /// Route-first recall of the context's own agent (§12.3). Every
    /// returned record enters the working set as a verbatim item.
    pub fn recall(
        &self,
        ctx: &mut DispatchContext,
        query: RecallQuery,
    ) -> Result<Vec<RecalledRecord>, RecallRefused> {
        self.recall_spec(ctx, &RecallSpec::table(query.table, query.limit))
    }

    /// Route-first recall with a read site's own search semantics (§12.3,
    /// mission R8). Every returned record enters the working set as a
    /// verbatim item.
    pub fn recall_spec(
        &self,
        ctx: &mut DispatchContext,
        spec: &RecallSpec<'_>,
    ) -> Result<Vec<RecalledRecord>, RecallRefused> {
        let table = spec.table;
        let limit = spec.limit.min(ctx.restrictions.max_records);
        let words: Vec<String> = spec
            .rank_words
            .map(|q| {
                q.to_lowercase()
                    .split_whitespace()
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        let candidates = if words.is_empty() { limit } else { limit * 3 };
        let mut walked = self.walk(ctx, spec, candidates)?;
        if !words.is_empty() {
            walked.sort_by_key(|(r, _)| std::cmp::Reverse(keyword_score(table, r, &words)));
        }
        walked.truncate(limit);
        let records: Vec<RecalledRecord> = walked
            .into_iter()
            .map(|(r, label)| recalled(table, r, label))
            .collect();
        for r in &records {
            ctx.working_set
                .verbatim
                .push((r.reference.clone(), r.label));
        }
        Ok(records)
    }

    /// Route-first control read (§12.3): at most `spec.limit` records, in
    /// the spec's order, for a decision that never places them in a prompt.
    /// Nothing enters the working set.
    pub fn control_records(
        &self,
        ctx: &DispatchContext,
        spec: &RecallSpec<'_>,
    ) -> Result<Vec<ControlRecord>, RecallRefused> {
        let limit = spec.limit.min(ctx.restrictions.max_records);
        Ok(self
            .walk(ctx, spec, limit)?
            .into_iter()
            .map(|(r, label)| control(spec.table, r, label))
            .collect())
    }

    /// The route-first walk shared by recall and control reads: at most
    /// `limit` bound rows (any row for a `local` unit), with the
    /// observational events of §15.1.
    fn walk(
        &self,
        ctx: &DispatchContext,
        spec: &RecallSpec<'_>,
        limit: usize,
    ) -> Result<Vec<(RecallRow, EvaluatedLabel)>, RecallRefused> {
        if !self.issued(ctx) {
            return Err(RecallRefused::WrongIssuer);
        }
        let Principal::Agent(agent_id) = ctx.principal else {
            return Err(RecallRefused::NotAnAgent);
        };
        if !ctx.restrictions.retained_data_permitted {
            return Err(RecallRefused::RetainedDataNotPermitted);
        }
        // Fixed before recall: the route class of the unit at this moment.
        let route = ctx.route_class;
        let table = spec.table;
        let agent = agent_id.to_string();
        let unrestricted = OriginLabel::UnrestrictedOrigin.class_name();
        let lowered = spec.contains.map(|(c, n)| (c, n.to_lowercase()));
        let filter = RecallFilter {
            column_eq: spec.column_eq,
            source_kind: spec.source_kind.map(SourceKind::name),
            contains: lowered.as_ref().map(|(c, n)| (*c, n.as_str())),
            newest_first: spec.newest_first,
        };

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
                    tx.recall_walk_filtered(
                        table.table_name(),
                        &agent,
                        origin_filter,
                        &filter,
                        &mut |r| {
                            let label = evaluate_row(&r);
                            if label == EvaluatedLabel::Unbound {
                                unbound_met += 1;
                                if route == RouteClass::NonLocal {
                                    // Withheld: never counted toward the limit.
                                    return true;
                                }
                            }
                            records.push((r, label));
                            records.len() < limit
                        },
                    )?;
                }
                let (excluded, unlabelled) = match route {
                    RouteClass::NonLocal => tx.recall_withheld_counts_filtered(
                        table.table_name(),
                        &agent,
                        unrestricted,
                        &filter,
                    )?,
                    RouteClass::Local => (0, 0),
                };
                Ok(Some((records, unbound_met, excluded, unlabelled)))
            })
            .map_err(|_| RecallRefused::Store)?;
        let Some((records, unbound_met, excluded, unlabelled)) = walked else {
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
                    "eligible_count": records.len(),
                    "withheld_count": excluded + unbound_met as i64,
                }),
            );
        }
        let unbound = unbound_met as i64 + unlabelled;
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
        Ok(records)
    }
}
