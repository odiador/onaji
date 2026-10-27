use std::path::{Path, PathBuf};

pub fn resolve_workspace_root(config_path: &Path) -> PathBuf {
    match config_path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    }
}

pub fn sanitize_id(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub fn dir_size(path: &Path) -> u64 {
    let mut total = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_dir() {
                    total += dir_size(&entry.path());
                } else {
                    total += meta.len();
                }
            }
        }
    }
    total
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

pub fn parse_size(input: &str) -> Option<u64> {
    let s = input.trim().to_uppercase();
    if let Some(num) = s.strip_suffix("GB").or_else(|| s.strip_suffix('G')) {
        let val: f64 = num.trim().parse().ok()?;
        Some((val * 1024.0 * 1024.0 * 1024.0) as u64)
    } else if let Some(num) = s.strip_suffix("MB").or_else(|| s.strip_suffix('M')) {
        let val: f64 = num.trim().parse().ok()?;
        Some((val * 1024.0 * 1024.0) as u64)
    } else if let Some(num) = s.strip_suffix("KB").or_else(|| s.strip_suffix('K')) {
        let val: f64 = num.trim().parse().ok()?;
        Some((val * 1024.0) as u64)
    } else if let Some(num) = s.strip_suffix('B') {
        num.trim().parse().ok()
    } else {
        s.parse().ok()
    }
}
