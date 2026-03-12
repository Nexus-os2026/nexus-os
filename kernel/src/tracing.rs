//! OpenTelemetry-style distributed tracing for agent decision chains.
//!
//! Lightweight tracing system that follows OTel concepts (traces, spans, events)
//! without pulling in the full opentelemetry crate.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Status of a span.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpanStatus {
    Ok,
    Error(String),
    InProgress,
}

/// A timestamped event within a span.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanEvent {
    pub name: String,
    pub timestamp: u64,
    pub attributes: HashMap<String, serde_json::Value>,
}

/// A single unit of work in a trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub operation_name: String,
    pub agent_id: Option<String>,
    pub start_time: u64,
    pub end_time: Option<u64>,
    pub duration_ms: Option<u64>,
    pub status: SpanStatus,
    pub attributes: HashMap<String, serde_json::Value>,
    pub events: Vec<SpanEvent>,
}

/// A complete trace consisting of related spans.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trace {
    pub trace_id: String,
    pub root_span_id: String,
    pub spans: Vec<Span>,
    pub total_duration_ms: Option<u64>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Engine that manages traces and spans across agent decision chains.
#[derive(Debug, Clone)]
pub struct TracingEngine {
    /// Active (not yet ended) spans keyed by span_id.
    active_spans: HashMap<String, Span>,
    /// Mapping from trace_id to root_span_id for traces that haven't been completed.
    trace_roots: HashMap<String, String>,
    /// Completed traces.
    completed_traces: Vec<Trace>,
    /// Maximum number of completed traces to retain.
    max_traces: usize,
}

impl TracingEngine {
    pub fn new(max_traces: usize) -> Self {
        Self {
            active_spans: HashMap::new(),
            trace_roots: HashMap::new(),
            completed_traces: Vec::new(),
            max_traces,
        }
    }

    /// Start a new trace by creating a root span. Returns `(trace_id, span_id)`.
    pub fn start_trace(
        &mut self,
        operation_name: &str,
        agent_id: Option<&str>,
    ) -> (String, String) {
        let trace_id = Uuid::new_v4().to_string();
        let span_id = Uuid::new_v4().to_string();

        let span = Span {
            trace_id: trace_id.clone(),
            span_id: span_id.clone(),
            parent_span_id: None,
            operation_name: operation_name.to_string(),
            agent_id: agent_id.map(|s| s.to_string()),
            start_time: now_ms(),
            end_time: None,
            duration_ms: None,
            status: SpanStatus::InProgress,
            attributes: HashMap::new(),
            events: Vec::new(),
        };

        self.active_spans.insert(span_id.clone(), span);
        self.trace_roots.insert(trace_id.clone(), span_id.clone());

        (trace_id, span_id)
    }

    /// Start a child span within an existing trace.
    pub fn start_span(
        &mut self,
        trace_id: &str,
        parent_span_id: &str,
        operation_name: &str,
        agent_id: Option<&str>,
    ) -> String {
        let span_id = Uuid::new_v4().to_string();

        let span = Span {
            trace_id: trace_id.to_string(),
            span_id: span_id.clone(),
            parent_span_id: Some(parent_span_id.to_string()),
            operation_name: operation_name.to_string(),
            agent_id: agent_id.map(|s| s.to_string()),
            start_time: now_ms(),
            end_time: None,
            duration_ms: None,
            status: SpanStatus::InProgress,
            attributes: HashMap::new(),
            events: Vec::new(),
        };

        self.active_spans.insert(span_id.clone(), span);
        span_id
    }

    /// Add an event to an active span.
    pub fn add_event(
        &mut self,
        span_id: &str,
        name: &str,
        attributes: HashMap<String, serde_json::Value>,
    ) {
        if let Some(span) = self.active_spans.get_mut(span_id) {
            span.events.push(SpanEvent {
                name: name.to_string(),
                timestamp: now_ms(),
                attributes,
            });
        }
    }

    /// Set an attribute on an active span.
    pub fn set_attribute(&mut self, span_id: &str, key: &str, value: serde_json::Value) {
        if let Some(span) = self.active_spans.get_mut(span_id) {
            span.attributes.insert(key.to_string(), value);
        }
    }

    /// End a span, computing its duration.
    pub fn end_span(&mut self, span_id: &str, status: SpanStatus) {
        if let Some(span) = self.active_spans.get_mut(span_id) {
            let end = now_ms();
            span.end_time = Some(end);
            span.duration_ms = Some(end.saturating_sub(span.start_time));
            span.status = status;
        }
    }

    /// End a trace: collect all spans belonging to it, compute total duration, and
    /// move the trace to `completed_traces`. Returns the completed [`Trace`] if found.
    pub fn end_trace(&mut self, trace_id: &str) -> Option<Trace> {
        let root_span_id = self.trace_roots.remove(trace_id)?;

        // Drain all active spans belonging to this trace.
        let span_ids: Vec<String> = self
            .active_spans
            .iter()
            .filter(|(_, s)| s.trace_id == trace_id)
            .map(|(id, _)| id.clone())
            .collect();

        let mut spans: Vec<Span> = Vec::new();
        for id in span_ids {
            if let Some(mut span) = self.active_spans.remove(&id) {
                // Auto-end any spans that weren't explicitly ended.
                if span.end_time.is_none() {
                    let end = now_ms();
                    span.end_time = Some(end);
                    span.duration_ms = Some(end.saturating_sub(span.start_time));
                    span.status = SpanStatus::Ok;
                }
                spans.push(span);
            }
        }

        let total_duration_ms = if spans.is_empty() {
            None
        } else {
            let min_start = spans.iter().map(|s| s.start_time).min().unwrap_or(0);
            let max_end = spans.iter().filter_map(|s| s.end_time).max().unwrap_or(0);
            Some(max_end.saturating_sub(min_start))
        };

        let trace = Trace {
            trace_id: trace_id.to_string(),
            root_span_id,
            spans,
            total_duration_ms,
        };

        // Evict oldest trace if at capacity.
        if self.completed_traces.len() >= self.max_traces {
            self.completed_traces.remove(0);
        }
        self.completed_traces.push(trace.clone());

        Some(trace)
    }

    /// Get a completed trace by ID.
    pub fn get_trace(&self, trace_id: &str) -> Option<&Trace> {
        self.completed_traces
            .iter()
            .find(|t| t.trace_id == trace_id)
    }

    /// List completed traces, most recent first.
    pub fn list_traces(&self, limit: usize) -> Vec<&Trace> {
        self.completed_traces.iter().rev().take(limit).collect()
    }

    /// Search completed traces by operation name (matches any span in the trace).
    pub fn search_traces(&self, operation_name: &str) -> Vec<&Trace> {
        self.completed_traces
            .iter()
            .filter(|t| {
                t.spans
                    .iter()
                    .any(|s| s.operation_name.contains(operation_name))
            })
            .collect()
    }

    /// Number of currently active (open) spans.
    pub fn active_span_count(&self) -> usize {
        self.active_spans.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_start_and_end_trace() {
        let mut engine = TracingEngine::new(100);
        let (trace_id, span_id) = engine.start_trace("root-op", Some("agent-1"));
        assert_eq!(engine.active_span_count(), 1);

        engine.end_span(&span_id, SpanStatus::Ok);
        let trace = engine.end_trace(&trace_id).unwrap();

        assert_eq!(trace.trace_id, trace_id);
        assert_eq!(trace.root_span_id, span_id);
        assert_eq!(trace.spans.len(), 1);
        assert!(trace.total_duration_ms.is_some());
    }

    #[test]
    fn test_nested_spans() {
        let mut engine = TracingEngine::new(100);
        let (trace_id, root_id) = engine.start_trace("root", None);
        let child_id = engine.start_span(&trace_id, &root_id, "child-op", Some("agent-2"));
        let grandchild_id =
            engine.start_span(&trace_id, &child_id, "grandchild-op", Some("agent-3"));

        assert_eq!(engine.active_span_count(), 3);

        engine.end_span(&grandchild_id, SpanStatus::Ok);
        engine.end_span(&child_id, SpanStatus::Ok);
        engine.end_span(&root_id, SpanStatus::Ok);

        let trace = engine.end_trace(&trace_id).unwrap();
        assert_eq!(trace.spans.len(), 3);

        let child = trace.spans.iter().find(|s| s.span_id == child_id).unwrap();
        assert_eq!(child.parent_span_id.as_deref(), Some(root_id.as_str()));

        let gc = trace
            .spans
            .iter()
            .find(|s| s.span_id == grandchild_id)
            .unwrap();
        assert_eq!(gc.parent_span_id.as_deref(), Some(child_id.as_str()));
    }

    #[test]
    fn test_span_events() {
        let mut engine = TracingEngine::new(100);
        let (trace_id, span_id) = engine.start_trace("op", None);

        let mut attrs = HashMap::new();
        attrs.insert("key".to_string(), serde_json::json!("value"));
        engine.add_event(&span_id, "my-event", attrs);

        engine.end_span(&span_id, SpanStatus::Ok);
        let trace = engine.end_trace(&trace_id).unwrap();

        assert_eq!(trace.spans[0].events.len(), 1);
        assert_eq!(trace.spans[0].events[0].name, "my-event");
        assert_eq!(
            trace.spans[0].events[0].attributes.get("key"),
            Some(&serde_json::json!("value"))
        );
    }

    #[test]
    fn test_span_attributes() {
        let mut engine = TracingEngine::new(100);
        let (trace_id, span_id) = engine.start_trace("op", None);

        engine.set_attribute(&span_id, "model", serde_json::json!("gpt-4"));
        engine.set_attribute(&span_id, "tokens", serde_json::json!(150));

        engine.end_span(&span_id, SpanStatus::Ok);
        let trace = engine.end_trace(&trace_id).unwrap();

        assert_eq!(
            trace.spans[0].attributes.get("model"),
            Some(&serde_json::json!("gpt-4"))
        );
        assert_eq!(
            trace.spans[0].attributes.get("tokens"),
            Some(&serde_json::json!(150))
        );
    }

    #[test]
    fn test_search_traces() {
        let mut engine = TracingEngine::new(100);

        let (t1, s1) = engine.start_trace("llm-call", Some("a1"));
        engine.end_span(&s1, SpanStatus::Ok);
        engine.end_trace(&t1);

        let (t2, s2) = engine.start_trace("file-read", Some("a2"));
        engine.end_span(&s2, SpanStatus::Ok);
        engine.end_trace(&t2);

        let (t3, s3) = engine.start_trace("llm-embedding", Some("a3"));
        engine.end_span(&s3, SpanStatus::Ok);
        engine.end_trace(&t3);

        let results = engine.search_traces("llm");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_max_traces_eviction() {
        let mut engine = TracingEngine::new(3);

        for i in 0..5 {
            let (tid, sid) = engine.start_trace(&format!("op-{i}"), None);
            engine.end_span(&sid, SpanStatus::Ok);
            engine.end_trace(&tid);
        }

        assert_eq!(engine.completed_traces.len(), 3);
        // Oldest traces (op-0, op-1) should be evicted.
        assert!(engine
            .completed_traces
            .iter()
            .all(|t| t.spans[0].operation_name != "op-0" && t.spans[0].operation_name != "op-1"));
    }

    #[test]
    fn test_duration_calculation() {
        let mut engine = TracingEngine::new(100);
        let (_trace_id, span_id) = engine.start_trace("timed-op", None);

        // Manually set start_time to verify duration.
        if let Some(span) = engine.active_spans.get_mut(&span_id) {
            span.start_time = 1000;
        }

        engine.end_span(&span_id, SpanStatus::Ok);

        let span = engine.active_spans.get(&span_id).unwrap();
        assert!(span.duration_ms.is_some());
        assert!(span.end_time.unwrap() >= 1000);
    }

    #[test]
    fn test_error_status() {
        let mut engine = TracingEngine::new(100);
        let (trace_id, span_id) = engine.start_trace("fail-op", None);

        engine.end_span(&span_id, SpanStatus::Error("timeout".to_string()));
        let trace = engine.end_trace(&trace_id).unwrap();

        match &trace.spans[0].status {
            SpanStatus::Error(msg) => assert_eq!(msg, "timeout"),
            _ => panic!("expected Error status"),
        }
    }

    #[test]
    fn test_list_traces() {
        let mut engine = TracingEngine::new(100);
        for i in 0..5 {
            let (tid, sid) = engine.start_trace(&format!("list-op-{i}"), None);
            engine.end_span(&sid, SpanStatus::Ok);
            engine.end_trace(&tid);
        }

        let listed = engine.list_traces(3);
        assert_eq!(listed.len(), 3);
        // Most recent first.
        assert!(listed[0].spans[0].operation_name.contains("4"));
    }

    #[test]
    fn test_get_trace() {
        let mut engine = TracingEngine::new(100);
        let (tid, sid) = engine.start_trace("get-op", None);
        engine.end_span(&sid, SpanStatus::Ok);
        engine.end_trace(&tid);

        assert!(engine.get_trace(&tid).is_some());
        assert!(engine.get_trace("nonexistent").is_none());
    }

    #[test]
    fn test_auto_end_spans_on_trace_end() {
        let mut engine = TracingEngine::new(100);
        let (trace_id, root_id) = engine.start_trace("root", None);
        let _child_id = engine.start_span(&trace_id, &root_id, "child", None);
        // Don't explicitly end spans — end_trace should auto-close them.
        let trace = engine.end_trace(&trace_id).unwrap();
        assert_eq!(trace.spans.len(), 2);
        for span in &trace.spans {
            assert!(span.end_time.is_some());
        }
    }

    #[test]
    fn test_end_nonexistent_trace() {
        let mut engine = TracingEngine::new(100);
        assert!(engine.end_trace("does-not-exist").is_none());
    }

    #[test]
    fn test_add_event_to_nonexistent_span() {
        let mut engine = TracingEngine::new(100);
        // Should not panic.
        engine.add_event("nope", "evt", HashMap::new());
        engine.set_attribute("nope", "k", serde_json::json!(1));
        engine.end_span("nope", SpanStatus::Ok);
    }
}
