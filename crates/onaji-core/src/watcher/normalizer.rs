use crate::domain::{Event, EventId, EventType, Resource, ResourceId, ResourceType};
use crate::fingerprint::{Fingerprint, Fingerprinter};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NormalizationResult {
    Changed(Event),
    IgnoredUnchanged { resource_id: ResourceId },
    UnrelatedPath(PathBuf),
}

pub struct EventNormalizer {
    resource_paths: HashMap<PathBuf, ResourceId>,
    fingerprints: HashMap<ResourceId, Fingerprint>,
    file_fingerprints: HashMap<PathBuf, Fingerprint>,
    excluded_paths: HashSet<PathBuf>,
    workspace_root: PathBuf,
}

impl EventNormalizer {
    pub fn new(workspace_root: impl AsRef<Path>) -> Self {
        let root = workspace_root.as_ref().to_path_buf();
        let mut excluded = HashSet::new();
        let onaji_dir = root.join(".onaji");
        if let Ok(canon) = onaji_dir.canonicalize() {
            excluded.insert(canon);
        } else {
            excluded.insert(onaji_dir);
        }

        Self {
            resource_paths: HashMap::new(),
            fingerprints: HashMap::new(),
            file_fingerprints: HashMap::new(),
            excluded_paths: excluded,
            workspace_root: root,
        }
    }

    /// Excluye rutas específicas o directorios de generación interna para prevenir feedback loops
    pub fn exclude_path(&mut self, path: impl AsRef<Path>) {
        let full = if path.as_ref().is_absolute() {
            path.as_ref().to_path_buf()
        } else {
            self.workspace_root.join(path.as_ref())
        };
        let lookup = full.canonicalize().unwrap_or(full);
        self.excluded_paths.insert(lookup);
    }

    /// Registra los recursos observables e inicializa sus huellas digitales base.
    pub fn register_resources<'a>(&mut self, resources: impl IntoIterator<Item = &'a Resource>) {
        for res in resources {
            if let ResourceType::File { path, .. } = &res.resource_type {
                let full_path = if path.is_absolute() {
                    path.clone()
                } else {
                    self.workspace_root.join(path)
                };

                // Canonicalizamos si es posible para matching exacto
                let lookup_path = full_path
                    .canonicalize()
                    .unwrap_or_else(|_| full_path.clone());
                self.resource_paths
                    .insert(lookup_path.clone(), res.id.clone());

                if let Ok(fp) = Fingerprinter::compute_path(&full_path) {
                    self.fingerprints.insert(res.id.clone(), fp);
                }

                if full_path.is_file() {
                    if let Ok(fp) = Fingerprinter::compute_file(&full_path) {
                        self.file_fingerprints.insert(lookup_path, fp);
                    }
                } else if full_path.is_dir() {
                    self.scan_and_fingerprint_files(&full_path);
                }
            }
        }
    }

    fn scan_and_fingerprint_files(&mut self, dir: &Path) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Ok(fp) = Fingerprinter::compute_file(&path) {
                        let canon = path.canonicalize().unwrap_or_else(|_| path.clone());
                        self.file_fingerprints.insert(canon, fp);
                    }
                } else if path.is_dir() {
                    self.scan_and_fingerprint_files(&path);
                }
            }
        }
    }

    /// Normaliza un evento crudo del sistema de archivos comprobando cambio real de contenido.
    pub fn normalize_fs_event(&mut self, event_path: &Path) -> NormalizationResult {
        // Ignorar mutaciones en .onaji o rutas excluidas para prevenir bucles de retroalimentación
        if event_path.components().any(|c| c.as_os_str() == ".onaji") {
            return NormalizationResult::UnrelatedPath(event_path.to_path_buf());
        }

        let lookup_path = event_path
            .canonicalize()
            .unwrap_or_else(|_| event_path.to_path_buf());

        if lookup_path.components().any(|c| c.as_os_str() == ".onaji")
            || self
                .excluded_paths
                .iter()
                .any(|exc| lookup_path.starts_with(exc) || event_path.starts_with(exc))
        {
            return NormalizationResult::UnrelatedPath(event_path.to_path_buf());
        }

        // Buscamos coincidencia directa o si event_path es hijo de un directorio observado
        let matched_resource = self.find_matching_resource(&lookup_path);

        let (res_path, resource_id) = match matched_resource {
            Some(pair) => pair,
            None => return NormalizationResult::UnrelatedPath(event_path.to_path_buf()),
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        if !lookup_path.exists() {
            self.file_fingerprints.remove(&lookup_path);
            if lookup_path == res_path {
                self.fingerprints.remove(&resource_id);
            }
            let event = Event::new(
                EventId::new(format!("ev-{}", now)),
                resource_id,
                EventType::Deleted,
                now,
            );
            return NormalizationResult::Changed(event);
        }

        match Fingerprinter::compute_path(&lookup_path) {
            Ok(new_fp) => {
                // Comprobar si este archivo específico no ha variado en contenido
                if let Some(old_fp) = self.file_fingerprints.get(&lookup_path) {
                    if *old_fp == new_fp {
                        // El contenido NO ha cambiado (touch o re-escritura idéntica)
                        return NormalizationResult::IgnoredUnchanged { resource_id };
                    }
                } else if lookup_path == res_path {
                    if let Some(old_fp) = self.fingerprints.get(&resource_id) {
                        if *old_fp == new_fp {
                            return NormalizationResult::IgnoredUnchanged { resource_id };
                        }
                    }
                }

                // El contenido cambió realmente
                self.file_fingerprints.insert(lookup_path.clone(), new_fp);
                if lookup_path == res_path {
                    self.fingerprints.insert(resource_id.clone(), new_fp);
                } else if res_path.is_dir() {
                    if let Ok(dir_fp) = Fingerprinter::compute_dir(&res_path) {
                        self.fingerprints.insert(resource_id.clone(), dir_fp);
                    }
                }

                let event = Event::new(
                    EventId::new(format!("ev-{}", now)),
                    resource_id,
                    EventType::Modified,
                    now,
                )
                .with_payload_hash(new_fp.as_u64());

                NormalizationResult::Changed(event)
            }
            Err(_) => NormalizationResult::UnrelatedPath(event_path.to_path_buf()),
        }
    }

    fn find_matching_resource(&self, path: &Path) -> Option<(PathBuf, ResourceId)> {
        if let Some(id) = self.resource_paths.get(path) {
            return Some((path.to_path_buf(), id.clone()));
        }

        // Si el recurso es un directorio contenedor
        for (res_path, id) in &self.resource_paths {
            if path.starts_with(res_path) {
                return Some((res_path.clone(), id.clone()));
            }
        }

        None
    }
}
