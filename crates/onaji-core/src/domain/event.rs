use super::resource::ResourceId;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EventId(pub String);

impl EventId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl fmt::Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventType {
    Created,
    Modified,
    Deleted,
    Available,
    Degraded { status: Option<u16>, reason: String },
    Unavailable { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub id: EventId,
    pub resource_id: ResourceId,
    pub event_type: EventType,
    pub timestamp_epoch_ms: u64,
    pub payload_hash: Option<u64>,
}

impl Event {
    pub fn new(
        id: impl Into<EventId>,
        resource_id: impl Into<ResourceId>,
        event_type: EventType,
        timestamp_epoch_ms: u64,
    ) -> Self {
        Self {
            id: id.into(),
            resource_id: resource_id.into(),
            event_type,
            timestamp_epoch_ms,
            payload_hash: None,
        }
    }

    pub fn with_payload_hash(mut self, hash: u64) -> Self {
        self.payload_hash = Some(hash);
        self
    }
}
