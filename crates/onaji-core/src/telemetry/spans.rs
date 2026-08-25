use super::events::SpanEvent;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpanKind {
    PipelineRun,
    ImpactAnalysis,
    TaskExecute,
    CacheLookup,
    PluginExecute,
    ArtifactWrite,
    ResourcePoll,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpanStatus {
    Ok,
    Cached,
    Blocked,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanRecord {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub kind: SpanKind,
    pub start_time_ms: u64,
    pub end_time_ms: Option<u64>,
    pub duration_ms: Option<u64>,
    pub status: SpanStatus,
    pub attributes: HashMap<String, String>,
    pub events: Vec<SpanEvent>,
}

impl SpanRecord {
    pub fn new(
        id: impl Into<String>,
        parent_id: Option<String>,
        name: impl Into<String>,
        kind: SpanKind,
    ) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            id: id.into(),
            parent_id,
            name: name.into(),
            kind,
            start_time_ms: now_ms,
            end_time_ms: None,
            duration_ms: None,
            status: SpanStatus::Ok,
            attributes: HashMap::new(),
            events: Vec::new(),
        }
    }

    pub fn set_attr(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.attributes.insert(key.into(), value.into());
    }

    pub fn add_event(&mut self, event: SpanEvent) {
        self.events.push(event);
    }

    pub fn complete(&mut self, status: SpanStatus) {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        self.end_time_ms = Some(now_ms);
        self.duration_ms = Some(now_ms.saturating_sub(self.start_time_ms));
        self.status = status;
    }
}
