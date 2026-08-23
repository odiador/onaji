use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub struct DetectionResult {
    pub matched: bool,
    pub confidence: f32,
    pub detected_paths: Vec<PathBuf>,
    pub details: Option<String>,
}

impl DetectionResult {
    pub fn none() -> Self {
        Self {
            matched: false,
            confidence: 0.0,
            detected_paths: Vec::new(),
            details: None,
        }
    }

    pub fn matched(paths: Vec<PathBuf>, confidence: f32, details: impl Into<String>) -> Self {
        Self {
            matched: true,
            confidence,
            detected_paths: paths,
            details: Some(details.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ValidationResult {
    pub fn ok() -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn failure(errors: Vec<String>) -> Self {
        Self {
            is_valid: false,
            errors,
            warnings: Vec::new(),
        }
    }

    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectionProperty {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectionResult {
    pub title: String,
    pub version: Option<String>,
    pub summary: String,
    pub properties: Vec<InspectionProperty>,
    pub children: Vec<String>,
}

impl InspectionResult {
    pub fn new(title: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            version: None,
            summary: summary.into(),
            properties: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn with_version(mut self, ver: impl Into<String>) -> Self {
        self.version = Some(ver.into());
        self
    }

    pub fn add_property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.push(InspectionProperty {
            key: key.into(),
            value: value.into(),
        });
        self
    }

    pub fn add_child(mut self, child: impl Into<String>) -> Self {
        self.children.push(child.into());
        self
    }
}
