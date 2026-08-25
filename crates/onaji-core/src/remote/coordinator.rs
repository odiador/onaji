use super::worker::WorkerDaemon;
use crate::runner::RunnerError;
use onaji_protocol::messages::worker::{TaskAssignment, TaskExecutionReport, WorkerRegistration};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub struct ClusterCoordinator {
    workers: Arc<RwLock<HashMap<String, Arc<WorkerDaemon>>>>,
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
        }
    }

    pub fn register_worker(&self, worker: Arc<WorkerDaemon>) {
        let mut map = self.workers.write().unwrap();
        map.insert(worker.registration.worker_id.clone(), worker);
    }

    pub fn unregister_worker(&self, worker_id: &str) {
        let mut map = self.workers.write().unwrap();
        map.remove(worker_id);
    }

    pub fn list_workers(&self) -> Vec<WorkerRegistration> {
        let map = self.workers.read().unwrap();
        map.values().map(|w| w.registration.clone()).collect()
    }

    pub fn find_compatible_worker(&self, required_toolchain: Option<&str>) -> Option<Arc<WorkerDaemon>> {
        let map = self.workers.read().unwrap();
        for worker in map.values() {
            if let Some(req) = required_toolchain {
                if worker.registration.toolchains.contains_key(req) {
                    return Some(worker.clone());
                }
            } else {
                return Some(worker.clone());
            }
        }
        None
    }

    pub async fn dispatch_task(&self, assignment: TaskAssignment) -> Result<TaskExecutionReport, RunnerError> {
        // Detect toolchain requirement from command
        let req_toolchain = if assignment.action_cmd.contains("gradle") {
            Some("android_sdk")
        } else if assignment.action_cmd.contains("cargo") {
            Some("rust")
        } else {
            None
        };

        let worker = self
            .find_compatible_worker(req_toolchain)
            .ok_or_else(|| RunnerError::ExecutionFailed {
                code: Some(127),
                stderr: format!(
                    "No remote worker available with required toolchain capability '{:?}'",
                    req_toolchain
                ),
            })?;

        let report = worker.execute_assignment(&assignment);
        Ok(report)
    }
}
