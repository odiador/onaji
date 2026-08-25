use crate::cache::{CacheKey, TaskState};
use crate::domain::{Action, Task};
use crate::runner::{ProcessExecutor, RunnerError, TaskExecutionResult};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

#[async_trait]
pub trait ExecutionBackend: Send + Sync {
    async fn execute_task(
        &self,
        task: &Task,
        cache_key: Option<CacheKey>,
        env: &HashMap<String, String>,
    ) -> Result<TaskExecutionResult, RunnerError>;
}

pub struct LocalExecutionBackend<'a> {
    workspace_root: &'a Path,
}

impl<'a> LocalExecutionBackend<'a> {
    pub fn new(workspace_root: &'a Path) -> Self {
        Self { workspace_root }
    }
}

#[async_trait]
impl<'a> ExecutionBackend for LocalExecutionBackend<'a> {
    async fn execute_task(
        &self,
        task: &Task,
        cache_key: Option<CacheKey>,
        _env: &HashMap<String, String>,
    ) -> Result<TaskExecutionResult, RunnerError> {
        ProcessExecutor::execute(task, self.workspace_root, cache_key).await
    }
}

pub struct RemoteExecutionBackend {
    coordinator: Arc<super::coordinator::ClusterCoordinator>,
}

impl RemoteExecutionBackend {
    pub fn new(coordinator: Arc<super::coordinator::ClusterCoordinator>) -> Self {
        Self { coordinator }
    }
}

#[async_trait]
impl ExecutionBackend for RemoteExecutionBackend {
    async fn execute_task(
        &self,
        task: &Task,
        cache_key: Option<CacheKey>,
        env: &HashMap<String, String>,
    ) -> Result<TaskExecutionResult, RunnerError> {
        let cmd = match &task.action {
            Action::Command { run, .. } => run.clone(),
            Action::Builtin { adapter, .. } => format!("builtin:{}", adapter),
        };

        let assignment = onaji_protocol::messages::worker::TaskAssignment {
            task_id: task.id.to_string(),
            workspace_id: "default".to_string(),
            action_cmd: cmd,
            env: env.clone(),
            cache_key: cache_key
                .as_ref()
                .map(|k| k.to_string())
                .unwrap_or_default(),
            timeout_ms: 30000,
        };

        let report = self.coordinator.dispatch_task(assignment).await?;
        let state = if report.success {
            TaskState::Success
        } else {
            TaskState::Failed
        };

        Ok(TaskExecutionResult {
            task_id: task.id.clone(),
            state,
            exit_code: report.exit_code,
            stdout: report.stdout,
            stderr: report.stderr,
            duration_ms: report.duration_ms,
            cache_key,
        })
    }
}
