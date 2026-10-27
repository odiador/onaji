use super::normalizer::{EventNormalizer, NormalizationResult};
use crate::domain::{Event, Resource, ResourceType};
use crate::graph::DependencyGraph;
use notify::RecursiveMode;
use notify_debouncer_mini::new_debouncer;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel as std_channel;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::mpsc::{channel as tokio_channel, Receiver, Sender};

pub const MANIFEST_RESOURCE_ID: &str = "__onaji_manifest__";

#[derive(Debug, Error)]
pub enum WatcherError {
    #[error("Notify error: {0}")]
    Notify(#[from] notify::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub struct ReactiveWatcher {
    workspace_root: PathBuf,
}

impl ReactiveWatcher {
    pub fn new(workspace_root: impl AsRef<Path>) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_path_buf(),
        }
    }

    /// Inicia el listener de filesystem en background y emite eventos normalizados por el channel.
    pub fn start_watching(
        &self,
        dag: &DependencyGraph,
        debounce_duration: Duration,
    ) -> Result<Receiver<Event>, WatcherError> {
        self.start_watching_with_manifest(dag, None, debounce_duration)
    }

    /// Inicia el listener de filesystem con soporte opcional de observación para recarga en caliente del manifiesto.
    pub fn start_watching_with_manifest(
        &self,
        dag: &DependencyGraph,
        manifest_path: Option<&Path>,
        debounce_duration: Duration,
    ) -> Result<Receiver<Event>, WatcherError> {
        let (std_tx, std_rx) = std_channel();
        let mut debouncer = new_debouncer(debounce_duration, std_tx)?;

        let mut normalizer = EventNormalizer::new(&self.workspace_root);
        let mut file_resources = Vec::new();
        let mut http_resources = Vec::new();

        for res_id in dag.resource_indices.keys() {
            if let Some(res) = dag.get_resource(res_id) {
                match &res.resource_type {
                    ResourceType::File { path, .. } => {
                        file_resources.push(res);
                        let full_path = if path.is_absolute() {
                            path.clone()
                        } else {
                            self.workspace_root.join(path)
                        };

                        if full_path.exists() {
                            let mode = if full_path.is_dir() {
                                RecursiveMode::Recursive
                            } else {
                                RecursiveMode::NonRecursive
                            };
                            let _ = debouncer.watcher().watch(&full_path, mode);
                        }
                    }
                    ResourceType::Http {
                        url,
                        interval_ms,
                        headers,
                    } => {
                        http_resources.push((
                            res.id.clone(),
                            url.clone(),
                            *interval_ms,
                            headers.clone(),
                        ));
                    }
                    _ => {}
                }
            }
        }

        let mut owned_manifest_res = None;
        if let Some(m_path) = manifest_path {
            let full_manifest = if m_path.is_absolute() {
                m_path.to_path_buf()
            } else {
                self.workspace_root.join(m_path)
            };
            if full_manifest.exists() {
                let _ = debouncer
                    .watcher()
                    .watch(&full_manifest, RecursiveMode::NonRecursive);
                owned_manifest_res = Some(Resource::new(
                    MANIFEST_RESOURCE_ID,
                    ResourceType::File {
                        path: full_manifest,
                        debounce_ms: debounce_duration.as_millis() as u64,
                    },
                ));
            }
        }

        let mut all_resources = file_resources;
        if let Some(m_res) = &owned_manifest_res {
            all_resources.push(m_res);
        }

        normalizer.register_resources(all_resources);

        let (async_tx, async_rx): (Sender<Event>, Receiver<Event>) = tokio_channel(100);

        // Hilos Tokio en background para sondeo periódico de recursos HTTP
        for (res_id, url, interval_ms, headers) in http_resources {
            let tx = async_tx.clone();
            let effective_interval = Duration::from_millis(interval_ms.max(100));
            tokio::spawn(async move {
                let mut poller = crate::http::HttpPoller::new();
                let mut interval = tokio::time::interval(effective_interval);
                loop {
                    interval.tick().await;
                    let outcome = poller.poll_resource(&res_id, &url, &headers).await;
                    for event in outcome.emitted_events {
                        if tx.send(event).await.is_err() {
                            return;
                        }
                    }
                }
            });
        }

        // Hilo dedicado para consumir eventos del debouncer y normalizarlos
        let fs_tx = async_tx.clone();
        std::thread::spawn(move || {
            // Mantenemos el debouncer vivo en este hilo
            let _debouncer = debouncer;

            while let Ok(res) = std_rx.recv() {
                match res {
                    Ok(events) => {
                        for debounced_event in events {
                            let norm_res = normalizer.normalize_fs_event(&debounced_event.path);
                            if let NormalizationResult::Changed(event) = norm_res {
                                if fs_tx.blocking_send(event).is_err() {
                                    return;
                                }
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(async_rx)
    }
}
