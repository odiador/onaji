use super::normalizer::{EventNormalizer, NormalizationResult};
use crate::domain::{Event, ResourceType};
use crate::graph::DependencyGraph;
use notify::RecursiveMode;
use notify_debouncer_mini::new_debouncer;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel as std_channel;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::mpsc::{channel as tokio_channel, Receiver, Sender};

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
        let (std_tx, std_rx) = std_channel();
        let mut debouncer = new_debouncer(debounce_duration, std_tx)?;

        let mut normalizer = EventNormalizer::new(&self.workspace_root);
        let mut file_resources = Vec::new();

        for res_id in dag.resource_indices.keys() {
            if let Some(res) = dag.get_resource(res_id) {
                if let ResourceType::File { path, .. } = &res.resource_type {
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
            }
        }

        normalizer.register_resources(file_resources);

        let (async_tx, async_rx): (Sender<Event>, Receiver<Event>) = tokio_channel(100);

        // Hilo dedicado para consumir eventos del debouncer y normalizarlos
        std::thread::spawn(move || {
            // Mantenemos el debouncer vivo en este hilo
            let _debouncer = debouncer;

            while let Ok(res) = std_rx.recv() {
                match res {
                    Ok(events) => {
                        for debounced_event in events {
                            let norm_res = normalizer.normalize_fs_event(&debounced_event.path);
                            if let NormalizationResult::Changed(event) = norm_res {
                                let _ = async_tx.blocking_send(event);
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
