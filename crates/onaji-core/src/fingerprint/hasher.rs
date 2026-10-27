use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use xxhash_rust::xxh3::{xxh3_64, Xxh3};

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

    /// Hash rápido con xxHash3 sobre el contenido del archivo usando streaming por chunks (64 KB).
    /// Garantiza consumo de memoria O(1) constante incluso para archivos gigabyte.
    pub fn compute_file(path: &Path) -> io::Result<Fingerprint> {
        let mut file = File::open(path)?;
        let mut hasher = Xxh3::new();
        let mut buffer = [0u8; 64 * 1024];

        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        Ok(Fingerprint(hasher.digest()))
    }

    /// Hash determinista recursivo para directorios (ordenado por nombres relativos normalizados).
    /// Asegura compatibilidad cross-platform (Windows/Unix) y multi-máquina.
    pub fn compute_dir(path: &Path) -> io::Result<Fingerprint> {
        Self::compute_dir_internal(path, path)
    }

    fn compute_dir_internal(base_root: &Path, current_dir: &Path) -> io::Result<Fingerprint> {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(current_dir)? {
            let entry = entry?;
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();
            // Evitar indexar directorios internos de control de versiones y caché de Onaji
            if name_str == ".git" || name_str == ".onaji" {
                continue;
            }
            entries.push(entry);
        }
        entries.sort_by_key(|e| e.path());

        let mut combined_hashes = Vec::new();
        for entry in entries {
            let entry_path = entry.path();
            let rel_name = entry_path
                .strip_prefix(base_root)
                .unwrap_or(&entry_path)
                .to_string_lossy()
                .replace('\\', "/");

            let file_type = entry.file_type()?;
            if file_type.is_symlink() {
                // Prevenir loops infinitos de recursión: no seguir symlinks recursivamente
                let target = std::fs::read_link(&entry_path).unwrap_or_default();
                combined_hashes.extend_from_slice(rel_name.as_bytes());
                combined_hashes.extend_from_slice(b":symlink:");
                combined_hashes.extend_from_slice(target.to_string_lossy().as_bytes());
            } else if file_type.is_file() {
                let file_hash = Self::compute_file(&entry_path)?;
                combined_hashes.extend_from_slice(rel_name.as_bytes());
                combined_hashes.extend_from_slice(&file_hash.0.to_le_bytes());
            } else if file_type.is_dir() {
                let dir_hash = Self::compute_dir_internal(base_root, &entry_path)?;
                combined_hashes.extend_from_slice(rel_name.as_bytes());
                combined_hashes.extend_from_slice(&dir_hash.0.to_le_bytes());
            }
        }

        Ok(Fingerprint(xxh3_64(&combined_hashes)))
    }

    pub fn compute_bytes(bytes: &[u8]) -> Fingerprint {
        Fingerprint(xxh3_64(bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_dir_symlink_loop_protection() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();

        let sub = root.join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("file.txt"), "hello").unwrap();

        #[cfg(unix)]
        {
            // Crear symlink cíclico que apunta al directorio raíz
            let link = sub.join("cycle_link");
            let _ = std::os::unix::fs::symlink(root, link);
        }

        // No debe causar stack overflow ni colgarse
        let fp = Fingerprinter::compute_dir(root).expect("compute_dir handles symlinks safely");
        assert_ne!(fp.0, 0);
    }
}
