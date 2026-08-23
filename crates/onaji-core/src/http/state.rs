use crate::domain::Event;
use crate::fingerprint::Fingerprint;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceAvailability {
    Available,
    Degraded { status: Option<u16>, reason: String },
    Unavailable { reason: String },
}

impl fmt::Display for ResourceAvailability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResourceAvailability::Available => write!(f, "Available"),
            ResourceAvailability::Degraded { status, reason } => {
                write!(f, "Degraded (status: {:?}, reason: {})", status, reason)
            }
            ResourceAvailability::Unavailable { reason } => {
                write!(f, "Unavailable ({})", reason)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentState {
    Unchanged,
    Modified { fingerprint: Fingerprint },
    Unknown,
}

#[derive(Debug, Clone, Default)]
pub struct HttpResourceState {
    pub last_etag: Option<String>,
    pub last_modified: Option<String>,
    pub last_fingerprint: Option<Fingerprint>,
    pub last_availability: Option<ResourceAvailability>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpPollOutcome {
    pub availability: ResourceAvailability,
    pub content_state: ContentState,
    pub emitted_events: Vec<Event>,
}
