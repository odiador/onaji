use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSignature {
    pub sig_type: String,
    pub signature_hex: String,
    pub public_key_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseArtifact {
    pub filename: String,
    pub os: String,
    pub arch: String,
    pub sha256: String,
    pub size_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<ArtifactSignature>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseManifest {
    pub version: String,
    pub protocol_version: String,
    pub release_date: String,
    pub artifacts: Vec<ReleaseArtifact>,
}

impl ReleaseManifest {
    pub fn new(version: impl Into<String>, protocol_version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            protocol_version: protocol_version.into(),
            release_date: "2026-08-26T11:45:00Z".to_string(),
            artifacts: Vec::new(),
        }
    }

    pub fn add_artifact(&mut self, artifact: ReleaseArtifact) {
        self.artifacts.push(artifact);
    }

    pub fn find_artifact(&self, os: &str, arch: &str) -> Option<&ReleaseArtifact> {
        self.artifacts
            .iter()
            .find(|a| a.os.eq_ignore_ascii_case(os) && a.arch.eq_ignore_ascii_case(arch))
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}
