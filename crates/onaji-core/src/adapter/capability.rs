use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    Detect,
    Inspect,
    Validate,
    Generate,
    Execute,
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Capability::Detect => write!(f, "Detect"),
            Capability::Inspect => write!(f, "Inspect"),
            Capability::Validate => write!(f, "Validate"),
            Capability::Generate => write!(f, "Generate"),
            Capability::Execute => write!(f, "Execute"),
        }
    }
}
