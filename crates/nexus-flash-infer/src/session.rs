use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::autoconfig::{auto_configure, OptimalConfig};
use crate::budget::MemoryBudget;
use crate::error::FlashError;
use crate::profiler::ModelProfile;
use crate::types::{HardwareInfo, InferencePreference, SessionInfo, SessionStatus};

/// Manages inference sessions with a shared memory budget.
pub struct SessionManager {
    hw: HardwareInfo,
    sessions: Arc<RwLock<HashMap<String, InferenceSession>>>,
    total_budget: MemoryBudget,
    allocated_mb: Arc<AtomicU64>,
}

/// An inference session record.
pub struct InferenceSession {
    pub id: String,
    pub model_path: String,
    pub profile: ModelProfile,
    pub config: OptimalConfig,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub tokens_generated: u64,
    pub status: SessionStatus,
}

impl SessionManager {
    /// Create a new session manager for given hardware.
    pub fn new(hw: HardwareInfo) -> Self {
        let dummy_profile = ModelProfile {
            name: String::new(),
            architecture: String::new(),
            total_params: 0,
            file_size_mb: 0,
            quantization: String::new(),
            is_moe: false,
            num_experts: 0,
            num_active_experts: 0,
            num_layers: 0,
            num_kv_heads: 0,
            head_dim: 0,
            dense_weight_size_mb: 0,
            expert_weight_size_mb: 0,
            single_expert_mb: 0.0,
            total_experts: 0,
            active_params: 0,
            flops_per_token: 0,
        };
        let total_budget = MemoryBudget::calculate(&hw, &dummy_profile, 0);

        Self {
            hw,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            total_budget,
            allocated_mb: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Memory footprint for a session (dense weights + KV cache).
    fn session_footprint(config: &OptimalConfig) -> u64 {
        config.budget.model_dense_mb + config.budget.kv_cache_mb
    }

    /// Create a new inference session (validates memory budget).
    pub async fn create_session(
        &self,
        model_path: &str,
        profile: ModelProfile,
        preference: InferencePreference,
    ) -> Result<String, FlashError> {
        let config = auto_configure(&self.hw, &profile, preference)?;

        let needed_mb = Self::session_footprint(&config);
        let remaining = self.remaining_budget_mb();

        if needed_mb > remaining {
            // MoE models stream expert weights via mmap — the session footprint
            // can exceed the RAM budget without causing OOM.  Warn, don't block.
            tracing::warn!(
                needed_mb,
                remaining,
                "session footprint exceeds RAM budget — mmap will stream from disk"
            );
        }

        let id = uuid::Uuid::new_v4().to_string();
        let session = InferenceSession {
            id: id.clone(),
            model_path: model_path.to_string(),
            profile,
            config,
            created_at: chrono::Utc::now(),
            tokens_generated: 0,
            status: SessionStatus::Ready,
        };

        self.allocated_mb.fetch_add(needed_mb, Ordering::Relaxed);

        let mut sessions = self.sessions.write().await;
        sessions.insert(id.clone(), session);

        Ok(id)
    }

    /// List active sessions.
    pub async fn list_sessions(&self) -> Vec<SessionInfo> {
        let sessions = self.sessions.read().await;
        sessions
            .values()
            .map(|s| SessionInfo {
                id: s.id.clone(),
                model_path: s.model_path.clone(),
                model_name: s.profile.name.clone(),
                memory_used_mb: Self::session_footprint(&s.config),
                tokens_generated: s.tokens_generated,
                status: s.status.clone(),
                created_at: s.created_at,
            })
            .collect()
    }

    /// Unload a session to free memory.
    pub async fn unload_session(&self, id: &str) -> Result<(), FlashError> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .remove(id)
            .ok_or_else(|| FlashError::SessionNotFound(id.to_string()))?;

        let freed = Self::session_footprint(&session.config);
        let current = self.allocated_mb.load(Ordering::Relaxed);
        self.allocated_mb
            .store(current.saturating_sub(freed), Ordering::Relaxed);

        Ok(())
    }

    /// Get remaining memory budget in MB.
    pub fn remaining_budget_mb(&self) -> u64 {
        let total_available = self
            .total_budget
            .total_system_ram_mb
            .saturating_sub(self.total_budget.os_reserved_mb)
            .saturating_sub(self.total_budget.app_overhead_mb)
            .saturating_sub(self.total_budget.safety_margin_mb);

        let allocated = self.allocated_mb.load(Ordering::Relaxed);
        total_available.saturating_sub(allocated)
    }

    /// Clear all sessions (used by frontend on page navigation).
    pub async fn clear_all(&self) {
        let mut sessions = self.sessions.write().await;
        sessions.clear();
        self.allocated_mb.store(0, Ordering::Relaxed);
    }

    /// Get the hardware info.
    pub fn hardware(&self) -> &HardwareInfo {
        &self.hw
    }
}
