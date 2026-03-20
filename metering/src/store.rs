//! SQLite-backed storage for usage records and budget alerts.

use rusqlite::{params, Connection};
use std::path::Path;

use crate::error::MeteringError;
use crate::types::{BudgetAlert, UsageRecord};

/// Persistent metering storage backed by SQLite.
pub struct MeteringStore {
    conn: Connection,
}

impl MeteringStore {
    /// Open (or create) the metering database at `path`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, MeteringError> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    /// Create an in-memory store (for testing).
    pub fn in_memory() -> Result<Self, MeteringError> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<(), MeteringError> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS usage_records (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                workspace_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                agent_did TEXT NOT NULL,
                resource_type TEXT NOT NULL,
                quantity REAL NOT NULL,
                unit TEXT NOT NULL,
                cost_estimate_usd REAL,
                metadata TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_usage_workspace_time
                ON usage_records(workspace_id, timestamp);
            CREATE INDEX IF NOT EXISTS idx_usage_agent_time
                ON usage_records(agent_did, timestamp);

            CREATE TABLE IF NOT EXISTS budget_alerts (
                id TEXT PRIMARY KEY,
                workspace_id TEXT NOT NULL,
                metric TEXT NOT NULL,
                threshold REAL NOT NULL,
                period TEXT NOT NULL,
                notification_channels TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1
            );
            ",
        )?;
        Ok(())
    }

    /// Insert a usage record.
    pub fn insert_record(&self, record: &UsageRecord) -> Result<(), MeteringError> {
        let resource_json = serde_json::to_string(&record.resource_type)?;
        let meta_json = record
            .metadata
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;

        self.conn.execute(
            "INSERT INTO usage_records (id, timestamp, workspace_id, user_id, agent_did, resource_type, quantity, unit, cost_estimate_usd, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                record.id.to_string(),
                record.timestamp.to_rfc3339(),
                record.workspace_id,
                record.user_id,
                record.agent_did,
                resource_json,
                record.quantity,
                record.unit,
                record.cost_estimate_usd,
                meta_json,
            ],
        )?;
        Ok(())
    }

    /// Query records for a workspace within a time range.
    pub fn query_records(
        &self,
        workspace_id: &str,
        start: &str,
        end: &str,
    ) -> Result<Vec<UsageRecord>, MeteringError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, timestamp, workspace_id, user_id, agent_did, resource_type, quantity, unit, cost_estimate_usd, metadata
             FROM usage_records
             WHERE workspace_id = ?1 AND timestamp >= ?2 AND timestamp <= ?3
             ORDER BY timestamp ASC",
        )?;

        let mut records = Vec::new();
        let mut rows = stmt.query(params![workspace_id, start, end])?;
        while let Some(row) = rows.next()? {
            let id_str: String = row.get(0)?;
            let ts_str: String = row.get(1)?;
            let ws: String = row.get(2)?;
            let uid: String = row.get(3)?;
            let agent: String = row.get(4)?;
            let rt_str: String = row.get(5)?;
            let quantity: f64 = row.get(6)?;
            let unit: String = row.get(7)?;
            let cost: Option<f64> = row.get(8)?;
            let meta_str: Option<String> = row.get(9)?;

            let id =
                uuid::Uuid::parse_str(&id_str).map_err(|e| MeteringError::Parse(e.to_string()))?;
            let timestamp = chrono::DateTime::parse_from_rfc3339(&ts_str)
                .map_err(|e| MeteringError::Parse(e.to_string()))?
                .with_timezone(&chrono::Utc);
            let resource_type: crate::types::ResourceType = serde_json::from_str(&rt_str)?;
            let metadata = meta_str.map(|s| serde_json::from_str(&s)).transpose()?;

            records.push(UsageRecord {
                id,
                timestamp,
                workspace_id: ws,
                user_id: uid,
                agent_did: agent,
                resource_type,
                quantity,
                unit,
                cost_estimate_usd: cost,
                metadata,
            });
        }
        Ok(records)
    }

    /// Get total quantity for a resource category in a workspace/time range.
    pub fn sum_quantity(
        &self,
        workspace_id: &str,
        start: &str,
        end: &str,
        resource_category: &str,
    ) -> Result<f64, MeteringError> {
        let mut stmt = self.conn.prepare(
            "SELECT COALESCE(SUM(quantity), 0.0) FROM usage_records
             WHERE workspace_id = ?1 AND timestamp >= ?2 AND timestamp <= ?3
             AND resource_type LIKE ?4",
        )?;

        let pattern = format!("%\"{resource_category}\"%");
        let total: f64 =
            stmt.query_row(params![workspace_id, start, end, pattern], |row| row.get(0))?;
        Ok(total)
    }

    /// Get total estimated cost in a workspace/time range.
    pub fn sum_cost(
        &self,
        workspace_id: &str,
        start: &str,
        end: &str,
    ) -> Result<f64, MeteringError> {
        let mut stmt = self.conn.prepare(
            "SELECT COALESCE(SUM(cost_estimate_usd), 0.0) FROM usage_records
             WHERE workspace_id = ?1 AND timestamp >= ?2 AND timestamp <= ?3",
        )?;

        let total: f64 = stmt.query_row(params![workspace_id, start, end], |row| row.get(0))?;
        Ok(total)
    }

    /// Top N agents by cost in a workspace/time range.
    pub fn top_agents(
        &self,
        workspace_id: &str,
        start: &str,
        end: &str,
        limit: usize,
    ) -> Result<Vec<(String, u64, f64)>, MeteringError> {
        let mut stmt = self.conn.prepare(
            "SELECT agent_did, COUNT(*) as cnt, COALESCE(SUM(cost_estimate_usd), 0.0) as total_cost
             FROM usage_records
             WHERE workspace_id = ?1 AND timestamp >= ?2 AND timestamp <= ?3
             GROUP BY agent_did
             ORDER BY total_cost DESC
             LIMIT ?4",
        )?;

        let rows = stmt.query_map(params![workspace_id, start, end, limit as i64], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(MeteringError::from)
    }

    /// Save a budget alert.
    pub fn save_alert(&self, alert: &BudgetAlert) -> Result<(), MeteringError> {
        let metric_json = serde_json::to_string(&alert.metric)?;
        let period_json = serde_json::to_string(&alert.period)?;
        let channels_json = serde_json::to_string(&alert.notification_channels)?;

        self.conn.execute(
            "INSERT OR REPLACE INTO budget_alerts (id, workspace_id, metric, threshold, period, notification_channels, enabled)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                alert.id.to_string(),
                alert.workspace_id,
                metric_json,
                alert.threshold,
                period_json,
                channels_json,
                alert.enabled as i32,
            ],
        )?;
        Ok(())
    }

    /// List alerts for a workspace.
    pub fn list_alerts(&self, workspace_id: &str) -> Result<Vec<BudgetAlert>, MeteringError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, workspace_id, metric, threshold, period, notification_channels, enabled
             FROM budget_alerts WHERE workspace_id = ?1",
        )?;

        let rows = stmt.query_map(params![workspace_id], |row| {
            let id_str: String = row.get(0)?;
            let metric_str: String = row.get(2)?;
            let period_str: String = row.get(4)?;
            let channels_str: String = row.get(5)?;
            let enabled: i32 = row.get(6)?;

            Ok((
                id_str,
                row.get(1)?,
                metric_str,
                row.get(3)?,
                period_str,
                channels_str,
                enabled,
            ))
        })?;

        let mut alerts = Vec::new();
        for row in rows {
            let (id_str, workspace_id, metric_str, threshold, period_str, channels_str, enabled): (
                String,
                String,
                String,
                f64,
                String,
                String,
                i32,
            ) = row?;

            alerts.push(BudgetAlert {
                id: uuid::Uuid::parse_str(&id_str)
                    .map_err(|e| MeteringError::Parse(e.to_string()))?,
                workspace_id,
                metric: serde_json::from_str(&metric_str)?,
                threshold,
                period: serde_json::from_str(&period_str)?,
                notification_channels: serde_json::from_str(&channels_str)?,
                enabled: enabled != 0,
            });
        }
        Ok(alerts)
    }

    /// Delete a budget alert by ID.
    pub fn delete_alert(&self, alert_id: &uuid::Uuid) -> Result<bool, MeteringError> {
        let count = self.conn.execute(
            "DELETE FROM budget_alerts WHERE id = ?1",
            params![alert_id.to_string()],
        )?;
        Ok(count > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ResourceType, TimePeriod};

    #[test]
    fn insert_and_query_records() {
        let store = MeteringStore::in_memory().unwrap();

        let record = UsageRecord::new("ws-1", "user-1", "agent-1", ResourceType::ApiCalls, 42.0)
            .with_cost(0.0042);

        store.insert_record(&record).unwrap();

        let results = store
            .query_records("ws-1", "2000-01-01T00:00:00Z", "2100-01-01T00:00:00Z")
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].quantity, 42.0);
    }

    #[test]
    fn sum_cost_works() {
        let store = MeteringStore::in_memory().unwrap();

        for i in 0..5 {
            let record =
                UsageRecord::new("ws-1", "user-1", "agent-1", ResourceType::ApiCalls, 10.0)
                    .with_cost(1.0 + i as f64);
            store.insert_record(&record).unwrap();
        }

        let total = store
            .sum_cost("ws-1", "2000-01-01T00:00:00Z", "2100-01-01T00:00:00Z")
            .unwrap();
        // 1 + 2 + 3 + 4 + 5 = 15
        assert!((total - 15.0).abs() < 0.001);
    }

    #[test]
    fn budget_alert_crud() {
        let store = MeteringStore::in_memory().unwrap();

        let alert = BudgetAlert::new("ws-1", ResourceType::ApiCalls, 1000.0, TimePeriod::Month);
        store.save_alert(&alert).unwrap();

        let alerts = store.list_alerts("ws-1").unwrap();
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].threshold, 1000.0);

        store.delete_alert(&alert.id).unwrap();
        let alerts = store.list_alerts("ws-1").unwrap();
        assert!(alerts.is_empty());
    }
}
