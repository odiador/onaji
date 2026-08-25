use super::events::SpanEvent;
use super::metrics::OnajiMetrics;
use super::spans::{SpanKind, SpanRecord, SpanStatus};
use std::sync::{Arc, RwLock};

pub trait TelemetryCollector: Send + Sync {
    fn start_span(&self, id: &str, parent_id: Option<&str>, name: &str, kind: SpanKind) -> String;

    fn finish_span(&self, span_id: &str, status: SpanStatus);
    fn set_attribute(&self, span_id: &str, key: &str, value: &str);
    fn add_event(&self, span_id: &str, event: SpanEvent);
    fn get_spans(&self) -> Vec<SpanRecord>;
    fn get_metrics(&self) -> OnajiMetrics;
    fn record_metric(&self, is_cached: bool, is_success: bool, is_blocked: bool);
    fn explain_task(&self, task_id: &str) -> String;
}

#[derive(Default)]
struct InnerTraceState {
    spans: Vec<SpanRecord>,
    metrics: OnajiMetrics,
}

#[derive(Clone, Default)]
pub struct InMemoryTraceCollector {
    state: Arc<RwLock<InnerTraceState>>,
}

impl InMemoryTraceCollector {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(InnerTraceState::default())),
        }
    }

    pub fn to_json(&self) -> String {
        let state = self.state.read().unwrap();
        serde_json::to_string_pretty(&state.spans).unwrap_or_default()
    }
}

impl TelemetryCollector for InMemoryTraceCollector {
    fn start_span(&self, id: &str, parent_id: Option<&str>, name: &str, kind: SpanKind) -> String {
        let record = SpanRecord::new(id, parent_id.map(|s| s.to_string()), name, kind);
        let span_id = record.id.clone();
        let mut state = self.state.write().unwrap();
        state.spans.push(record);
        span_id
    }

    fn finish_span(&self, span_id: &str, status: SpanStatus) {
        let mut state = self.state.write().unwrap();
        if let Some(span) = state.spans.iter_mut().find(|s| s.id == span_id) {
            span.complete(status);
        }
    }

    fn set_attribute(&self, span_id: &str, key: &str, value: &str) {
        let mut state = self.state.write().unwrap();
        if let Some(span) = state.spans.iter_mut().find(|s| s.id == span_id) {
            span.set_attr(key, value);
        }
    }

    fn add_event(&self, span_id: &str, event: SpanEvent) {
        let mut state = self.state.write().unwrap();
        if let Some(span) = state.spans.iter_mut().find(|s| s.id == span_id) {
            span.add_event(event);
        }
    }

    fn get_spans(&self) -> Vec<SpanRecord> {
        let state = self.state.read().unwrap();
        state.spans.clone()
    }

    fn get_metrics(&self) -> OnajiMetrics {
        let state = self.state.read().unwrap();
        state.metrics.clone()
    }

    fn record_metric(&self, is_cached: bool, is_success: bool, is_blocked: bool) {
        let mut state = self.state.write().unwrap();
        state
            .metrics
            .record_task_result(is_cached, is_success, is_blocked);
    }

    fn explain_task(&self, task_id: &str) -> String {
        let state = self.state.read().unwrap();
        let task_span = state.spans.iter().find(|s| {
            s.kind == SpanKind::TaskExecute
                && s.attributes.get("task.id").map(|s| s.as_str()) == Some(task_id)
        });

        match task_span {
            Some(span) => {
                let status_str = match &span.status {
                    SpanStatus::Ok => "Success",
                    SpanStatus::Cached => "Cached (Zero Re-computation)",
                    SpanStatus::Blocked => "Blocked by failed dependency",
                    SpanStatus::Error(e) => {
                        return format!("Task '{}' failed with error: {}", task_id, e)
                    }
                };

                let mut explanation = format!(
                    "Task: {}\nStatus: {}\nDuration: {}ms\n",
                    task_id,
                    status_str,
                    span.duration_ms.unwrap_or_default()
                );
                if let Some(trigger) = span.attributes.get("causal.trigger") {
                    explanation.push_str(&format!("Triggered by Resource: {}\n", trigger));
                }
                if let Some(tier) = span.attributes.get("cache.tier") {
                    explanation.push_str(&format!("Cache Tier: {}\n", tier));
                }
                if let Some(key) = span.attributes.get("cache.key") {
                    explanation.push_str(&format!("Cache Key: {}\n", key));
                }
                explanation
            }
            None => format!("No execution telemetry found for task '{}'.", task_id),
        }
    }
}
