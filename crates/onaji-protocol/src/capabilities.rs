use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ClientCapabilities {
    pub streaming_logs: bool,
    pub interactive_prompts: bool,
    pub notifications: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ServerCapabilities {
    pub reactive_watch: bool,
    pub impact_analysis: bool,
    pub graph_inspection: bool,
    pub adapter_extensibility: bool,
}
