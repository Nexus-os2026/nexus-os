//! Content-free `hm_event` writes (P3-HM-CONTRACT-02 §5.3, §15).
//!
//! Fields carry only counts, class names, generations and sequences. A
//! subject is an identifier, never authorisation: no code here reads an
//! event to decide anything.

use nexus_persistence::NexusDatabase;

use super::write::is_migrated;
use super::HmError;

/// At most this many missing events of each kind per back-fill run.
pub const BACKFILL_BOUND: i64 = 4096;

/// `policy:<agent>:<generation>`: the authority subject of one
/// `hm_policy_change` row.
pub(in crate::hm) fn policy_subject(agent_id: &str, policy_generation: i64) -> String {
    format!("policy:{agent_id}:{policy_generation}")
}

/// Writes one authority-subject event in its own transaction, after the
/// change it describes has committed (§15.3). Returns whether it was
/// written; a failure never changes the committed change.
pub(in crate::hm) fn after_commit(
    db: &NexusDatabase,
    subject: &str,
    kind: &str,
    agent_id: Option<&str>,
    fields: serde_json::Value,
) -> bool {
    db.hm()
        .immediate(|tx| tx.insert_event(subject, kind, agent_id, &fields.to_string()))
        .is_ok()
}

/// Writes one observational event with subject `obs:<kind>:<128-bit random
/// id>` (§5.3). Best effort: a lost observational event is never back-filled
/// and changes no decision.
pub(in crate::hm) fn observational(
    db: &NexusDatabase,
    kind: &str,
    agent_id: Option<&str>,
    fields: serde_json::Value,
) -> bool {
    let mut id = [0u8; 16];
    if getrandom::getrandom(&mut id).is_err() {
        return false;
    }
    let subject = format!("obs:{kind}:{}", hex::encode(id));
    db.hm()
        .immediate(|tx| tx.insert_event(&subject, kind, agent_id, &fields.to_string()))
        .is_ok()
}

/// `tombstone:<record_table>:<record_id>`: the authority subject of one
/// `hm_t0_tombstone` row.
pub(in crate::hm) fn tombstone_subject(record_table: &str, record_id: &str) -> String {
    format!("tombstone:{record_table}:{record_id}")
}

/// The event kind that describes a policy change, by its cause (§5.3).
/// `t0_baseline` changes are described by `migration:t0` (A9).
fn policy_change_kind(cause: &str) -> Option<&'static str> {
    match cause {
        "row_created" => Some("memory.policy_row_created"),
        "governed_tighten" => Some("memory.policy_tightened"),
        "config_tighten" => Some("memory.policy_tightened_by_config"),
        "owner_loosen" => Some("memory.policy_loosened"),
        _ => None,
    }
}

/// What one back-fill run inserted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackfillReport {
    pub policy_events: usize,
    pub tombstone_events: usize,
    /// False when a kind reached `BACKFILL_BOUND`; the next run continues.
    pub complete: bool,
}

/// The back-fill reconciler (§5.3 item 3): in one `BEGIN IMMEDIATE`,
/// inserts the missing authority-subject event of every `hm_policy_change`
/// row (except `t0_baseline`) and every `hm_t0_tombstone` row. It only adds
/// audit rows and never reads audit to decide authority; `UNIQUE(subject)`
/// makes it idempotent. Run at start and after a failed after-commit
/// audit write.
pub fn backfill_authority_events(db: &NexusDatabase) -> Result<BackfillReport, HmError> {
    db.hm()
        .immediate(|tx| {
            if !is_migrated(tx)? {
                return Ok(BackfillReport {
                    policy_events: 0,
                    tombstone_events: 0,
                    complete: true,
                });
            }
            let changes = tx.policy_changes_without_event("t0_baseline", BACKFILL_BOUND)?;
            let mut policy_events = 0;
            for c in &changes {
                let Some(kind) = policy_change_kind(&c.cause) else {
                    continue;
                };
                tx.insert_event(
                    &policy_subject(&c.agent_id, c.policy_generation),
                    kind,
                    Some(&c.agent_id),
                    &serde_json::json!({
                        "policy_generation": c.policy_generation,
                        "local_only": c.local_only,
                    })
                    .to_string(),
                )?;
                policy_events += 1;
            }
            let tombstones = tx.tombstones_without_event(BACKFILL_BOUND)?;
            for t in &tombstones {
                tx.insert_event(
                    &tombstone_subject(&t.record_table, &t.record_id),
                    "memory.t0_member_deleted",
                    t.agent_id.as_deref(),
                    &serde_json::json!({ "record_count": 1 }).to_string(),
                )?;
            }
            Ok(BackfillReport {
                policy_events,
                tombstone_events: tombstones.len(),
                complete: (changes.len() as i64) < BACKFILL_BOUND
                    && (tombstones.len() as i64) < BACKFILL_BOUND,
            })
        })
        .map_err(HmError::Store)
}
