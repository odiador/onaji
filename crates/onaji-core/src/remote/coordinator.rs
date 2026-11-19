use super::worker::{TcpWorkerClient, WorkerHandle};
use crate::runner::RunnerError;
use onaji_protocol::messages::worker::{TaskAssignment, TaskExecutionReport, WorkerRegistration};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone)]
pub struct WorkerHealthStatus {
    pub registration: WorkerRegistration,
    pub active_tasks: usize,
    pub online: bool,
    pub latency_ms: Option<u64>,
}

struct ActiveTaskGuard {
    active_tasks: Arc<RwLock<HashMap<String, usize>>>,
    worker_id: String,
}

impl ActiveTaskGuard {
    fn acquire(active_tasks: Arc<RwLock<HashMap<String, usize>>>, worker_id: String) -> Self {
        if let Ok(mut loads) = active_tasks.write() {
            *loads.entry(worker_id.clone()).or_insert(0) += 1;
        }
        Self {
            active_tasks,
            worker_id,
        }
    }
}

impl Drop for ActiveTaskGuard {
    fn drop(&mut self) {
        if let Ok(mut loads) = self.active_tasks.write() {
            if let Some(val) = loads.get_mut(&self.worker_id) {
                *val = val.saturating_sub(1);
            }
        }
    }
}

pub struct ClusterCoordinator {
    workers: Arc<RwLock<HashMap<String, Arc<dyn WorkerHandle>>>>,
    active_tasks: Arc<RwLock<HashMap<String, usize>>>,
}

impl Default for ClusterCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl ClusterCoordinator {
    pub fn new() -> Self {
        Self {
            workers: Arc::new(RwLock::new(HashMap::new())),
            active_tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn register_worker(&self, worker: Arc<dyn WorkerHandle>) {
        let mut map = self.workers.write().unwrap();
        map.insert(worker.registration().worker_id.clone(), worker);
    }

    pub fn register_tcp_worker(
        &self,
        worker_id: impl Into<String>,
        address: impl Into<String>,
        toolchains: HashMap<String, String>,
    ) {
        let client = Arc::new(TcpWorkerClient::new(worker_id, address, toolchains));
        self.register_worker(client);
    }

    pub fn register_manifest_workers(&self, manifest: &crate::config::RawManifest) -> usize {
        let mut count = 0;
        if let Some(cluster) = &manifest.cluster {
            for w in &cluster.workers {
                self.register_tcp_worker(&w.id, &w.address, w.toolchains.clone());
                count += 1;
            }
        }
        count
    }

    pub fn unregister_worker(&self, worker_id: &str) {
        let mut map = self.workers.write().unwrap();
        map.remove(worker_id);
        let mut loads = self.active_tasks.write().unwrap();
        loads.remove(worker_id);
    }

    pub fn list_workers(&self) -> Vec<WorkerRegistration> {
        let map = self.workers.read().unwrap();
        map.values().map(|w| w.registration().clone()).collect()
    }

    pub fn worker_load(&self, worker_id: &str) -> usize {
        let loads = self.active_tasks.read().unwrap();
        loads.get(worker_id).copied().unwrap_or(0)
    }

    pub async fn inspect_workers_health(
        &self,
        timeout: std::time::Duration,
    ) -> Vec<WorkerHealthStatus> {
        let snapshot: Vec<(Arc<dyn WorkerHandle>, usize)> = {
            let map = self.workers.read().unwrap();
            let loads = self.active_tasks.read().unwrap();
            map.values()
                .map(|w| {
                    let active = loads.get(&w.registration().worker_id).copied().unwrap_or(0);
                    (Arc::clone(w), active)
                })
                .collect()
        };

        let mut handles = Vec::with_capacity(snapshot.len());
        for (worker, active_tasks) in snapshot {
            handles.push(tokio::spawn(async move {
                let reg = worker.registration().clone();
                let latency_ms = worker.ping_latency(timeout).await;
                WorkerHealthStatus {
                    registration: reg,
                    active_tasks,
                    online: latency_ms.is_some(),
                    latency_ms,
                }
            }));
        }

        let mut out = Vec::with_capacity(handles.len());
        for h in handles {
            if let Ok(status) = h.await {
                out.push(status);
            }
        }
        out.sort_by(|a, b| a.registration.worker_id.cmp(&b.registration.worker_id));
        out
    }

    /// Retorna todos los workers compatibles con el toolchain requerido,
    /// ordenados por menor carga activa (algoritmo least-loaded).
    pub fn find_compatible_workers(
        &self,
        required_toolchain: Option<&str>,
    ) -> Vec<Arc<dyn WorkerHandle>> {
        let map = self.workers.read().unwrap();
        let loads = self.active_tasks.read().unwrap();

        let mut compatible: Vec<Arc<dyn WorkerHandle>> = map
            .values()
            .filter(|w| {
                if let Some(req) = required_toolchain {
                    w.registration().toolchains.contains_key(req)
                } else {
                    true
                }
            })
            .cloned()
            .collect();

        compatible.sort_by_key(|w| loads.get(&w.registration().worker_id).copied().unwrap_or(0));
        compatible
    }

    pub fn find_compatible_worker(
        &self,
        required_toolchain: Option<&str>,
    ) -> Option<Arc<dyn WorkerHandle>> {
        self.find_compatible_workers(required_toolchain)
            .into_iter()
            .next()
    }

    pub async fn dispatch_task(
        &self,
        assignment: TaskAssignment,
    ) -> Result<TaskExecutionReport, RunnerError> {
        // Detect toolchain requirement from command
        let req_toolchain = if assignment.action_cmd.contains("gradle") {
            Some("android_sdk")
        } else if assignment.action_cmd.contains("cargo") {
            Some("rust")
        } else {
            None
        };

        let candidates = self.find_compatible_workers(req_toolchain);
        if candidates.is_empty() {
            return Err(RunnerError::ExecutionFailed {
                code: Some(127),
                stderr: format!(
                    "No remote worker available with required toolchain capability '{:?}'",
                    req_toolchain
                ),
            });
        }

        let mut last_report = None;

        for worker in candidates {
            let worker_id = worker.registration().worker_id.clone();

            let _guard = ActiveTaskGuard::acquire(Arc::clone(&self.active_tasks), worker_id);
            let report = worker.execute_assignment(&assignment).await;
            drop(_guard);

            if report.success {
                return Ok(report);
            }

            // En caso de fallo, guardar el reporte y continuar al siguiente worker compatible si existe
            last_report = Some(report);
        }

        Ok(last_report.unwrap())
    }
}
