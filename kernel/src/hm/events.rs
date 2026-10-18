//! Content-free `hm_event` writes (P3-HM-CONTRACT-02 §5.3, §15).
//!
//! Fields carry only counts, class names, generations and sequences. A
//! subject is an identifier, never authorisation: no code here reads an
//! event to decide anything.

use nexus_persistence::NexusDatabase;

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
