use crate::domain::{Event, EventId, EventType, Resource, ResourceId, ResourceType};
use crate::fingerprint::{Fingerprint, Fingerprinter};
use std::collections::HashMap;
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
    workspace_root: PathBuf,
}

impl EventNormalizer {
    pub fn new(workspace_root: impl AsRef<Path>) -> Self {
        Self {
            resource_paths: HashMap::new(),
            fingerprints: HashMap::new(),
            workspace_root: workspace_root.as_ref().to_path_buf(),
        }
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
                self.resource_paths.insert(lookup_path, res.id.clone());

                if let Ok(fp) = Fingerprinter::compute_path(&full_path) {
                    self.fingerprints.insert(res.id.clone(), fp);
                }
            }
        }
    }

    /// Normaliza un evento crudo del sistema de archivos comprobando cambio real de contenido.
    pub fn normalize_fs_event(&mut self, event_path: &Path) -> NormalizationResult {
        let lookup_path = event_path
            .canonicalize()
            .unwrap_or_else(|_| event_path.to_path_buf());

        // Buscamos coincidencia directa o si event_path es hijo de un directorio observado
        let matched_resource = self.find_matching_resource(&lookup_path);

        let resource_id = match matched_resource {
            Some(id) => id,
            None => return NormalizationResult::UnrelatedPath(event_path.to_path_buf()),
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        if !lookup_path.exists() {
            self.fingerprints.remove(&resource_id);
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
                if let Some(old_fp) = self.fingerprints.get(&resource_id) {
                    if *old_fp == new_fp {
                        // El contenido NO ha cambiado (touch o re-escritura idéntica)
                        return NormalizationResult::IgnoredUnchanged { resource_id };
                    }
                }

                // El contenido cambió realmente
                self.fingerprints.insert(resource_id.clone(), new_fp);
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

    fn find_matching_resource(&self, path: &Path) -> Option<ResourceId> {
        if let Some(id) = self.resource_paths.get(path) {
            return Some(id.clone());
        }

        // Si el recurso es un directorio contenedor
        for (res_path, id) in &self.resource_paths {
            if path.starts_with(res_path) {
                return Some(id.clone());
            }
        }

        None
    }
}
