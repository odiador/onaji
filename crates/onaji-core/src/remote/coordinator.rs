use super::worker::{TcpWorkerClient, WorkerHandle};
use crate::runner::RunnerError;
use onaji_protocol::messages::worker::{TaskAssignment, TaskExecutionReport, WorkerRegistration};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

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

            // Incrementar contador de tareas activas del worker seleccionado
            {
                let mut loads = self.active_tasks.write().unwrap();
                *loads.entry(worker_id.clone()).or_insert(0) += 1;
            }

            let report = worker.execute_assignment(&assignment).await;

            // Decrementar contador de tareas activas al finalizar
            {
                let mut loads = self.active_tasks.write().unwrap();
                if let Some(val) = loads.get_mut(&worker_id) {
                    *val = val.saturating_sub(1);
                }
            }

            if report.success {
                return Ok(report);
            }

            // En caso de fallo, guardar el reporte y continuar al siguiente worker compatible si existe
            last_report = Some(report);
        }

        Ok(last_report.unwrap())
    }
}
