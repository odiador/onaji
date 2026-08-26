use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub struct ChecksumVerifier;

impl ChecksumVerifier {
    pub fn compute_sha256(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }

    pub fn verify(bytes: &[u8], expected_hex: &str) -> bool {
        let computed = Self::compute_sha256(bytes);
        computed.eq_ignore_ascii_case(expected_hex.trim())
    }

    pub fn parse_sha256sums(content: &str) -> HashMap<String, String> {
        let mut map = HashMap::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let sha = parts[0].to_string();
                let filename = parts[1].trim_start_matches('*').to_string();
                map.insert(filename, sha);
            }
        }
        map
    }

    pub fn format_sha256sums(entries: &[(String, String)]) -> String {
        let mut output = String::new();
        for (filename, sha) in entries {
            output.push_str(&format!("{}  {}\n", sha, filename));
        }
        output
    }
}
