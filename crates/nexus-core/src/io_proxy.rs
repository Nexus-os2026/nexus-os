use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IoRequest {
    pub kind: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IoResponse {
    pub ok: bool,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IoEvent {
    pub request_id: String,
    pub request: IoRequest,
    pub response: IoResponse,
}

pub trait IoProxy: Send + Sync {
    fn execute(&self, req: IoRequest) -> anyhow::Result<IoResponse>;
}
