use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use xxhash_rust::xxh3::xxh3_64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Fingerprint(pub u64);

impl Fingerprint {
    pub fn new(hash: u64) -> Self {
        Self(hash)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }

    pub fn to_hex(&self) -> String {
        format!("{:016x}", self.0)
    }
}

impl std::fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

pub struct Fingerprinter;

impl Fingerprinter {
    /// Calcula la huella de contenido para un archivo o directorio.
    pub fn compute_path(path: &Path) -> io::Result<Fingerprint> {
        if !path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Path does not exist: {}", path.display()),
            ));
        }

        if path.is_file() {
            Self::compute_file(path)
        } else if path.is_dir() {
            Self::compute_dir(path)
        } else {
            Err(io::Error::other(format!(
                "Unsupported file type: {}",
                path.display()
            )))
        }
    }

    /// Hash rápido con xxHash3 sobre el contenido del archivo.
    pub fn compute_file(path: &Path) -> io::Result<Fingerprint> {
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        Ok(Fingerprint(xxh3_64(&buffer)))
    }

    /// Hash determinista recursivo para directorios (ordenado por nombres de archivo).
    pub fn compute_dir(path: &Path) -> io::Result<Fingerprint> {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            entries.push(entry.path());
        }
        entries.sort();

        let mut combined_hashes = Vec::new();
        for entry_path in entries {
            if entry_path.is_file() {
                let file_hash = Self::compute_file(&entry_path)?;
                combined_hashes.extend_from_slice(entry_path.to_string_lossy().as_bytes());
                combined_hashes.extend_from_slice(&file_hash.0.to_le_bytes());
            } else if entry_path.is_dir() {
                let dir_hash = Self::compute_dir(&entry_path)?;
                combined_hashes.extend_from_slice(entry_path.to_string_lossy().as_bytes());
                combined_hashes.extend_from_slice(&dir_hash.0.to_le_bytes());
            }
        }

        Ok(Fingerprint(xxh3_64(&combined_hashes)))
    }

    pub fn compute_bytes(bytes: &[u8]) -> Fingerprint {
        Fingerprint(xxh3_64(bytes))
    }
}
