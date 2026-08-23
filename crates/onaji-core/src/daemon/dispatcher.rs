use crate::adapter::{AdapterContext, AdapterRegistry};
use crate::cache::ExecutionCache;
use crate::config::ConfigLoader;
use crate::domain::{ResourceId, TaskId};
use crate::graph::{DependencyGraph, ImpactAnalyzer};
use crate::scheduler::TaskScheduler;
use onaji_protocol::error::{ErrorCode, RpcError};
use onaji_protocol::messages::*;
use onaji_protocol::ServerCapabilities;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct Dispatcher {
    workspace_root: PathBuf,
    dag: Option<DependencyGraph>,
    cache: Arc<Mutex<ExecutionCache>>,
    registry: AdapterRegistry,
}

impl Dispatcher {
    pub fn new(workspace_root: impl AsRef<Path>) -> Self {
        let root = workspace_root.as_ref().to_path_buf();
        let config_path = root.join("onaji.yaml");

        let dag = if config_path.exists() {
            ConfigLoader::load_from_file(&config_path)
                .map(|(_, d)| d)
                .ok()
        } else {
            None
        };

        Self {
            workspace_root: root,
            dag,
            cache: Arc::new(Mutex::new(ExecutionCache::new())),
            registry: AdapterRegistry::with_builtins(),
        }
    }

    pub async fn dispatch_request(&mut self, req: &RpcRequest) -> RpcResponse {
        let id = req.id;
        match req.method.as_str() {
            "initialize" => self.handle_initialize(id, req.params.as_ref()),
            "graph/get" => self.handle_graph_get(id),
            "impact/get" => self.handle_impact_get(id, req.params.as_ref()),
            "pipeline/run" => self.handle_pipeline_run(id, req.params.as_ref()).await,
            "validate" => self.handle_validate(id, req.params.as_ref()),
            _ => RpcResponse::error(
                id,
                RpcError::new(
                    ErrorCode::MethodNotFound,
                    format!("Method '{}' not found", req.method),
                ),
            ),
        }
    }

    fn handle_initialize(
        &self,
        id: Option<u64>,
        params: Option<&serde_json::Value>,
    ) -> RpcResponse {
        let _init_params: InitializeParams =
            match params.and_then(|p| serde_json::from_value(p.clone()).ok()) {
                Some(p) => p,
                None => {
                    return RpcResponse::error(
                        id,
                        RpcError::new(ErrorCode::InvalidParams, "Invalid InitializeParams"),
                    )
                }
            };

        let active_resources = self
            .dag
            .as_ref()
            .map(|d| d.resource_indices.len())
            .unwrap_or(0);
        let active_tasks = self.dag.as_ref().map(|d| d.task_indices.len()).unwrap_or(0);

        let result = InitializeResult {
            engine_version: onaji_protocol::PROTOCOL_VERSION.to_string(),
            capabilities: ServerCapabilities {
                reactive_watch: true,
                impact_analysis: true,
                graph_inspection: true,
                adapter_extensibility: true,
            },
            active_resources,
            active_tasks,
        };

        RpcResponse::success(id, serde_json::to_value(result).unwrap())
    }

    fn handle_graph_get(&self, id: Option<u64>) -> RpcResponse {
        let dag = match &self.dag {
            Some(d) => d,
            None => {
                return RpcResponse::error(
                    id,
                    RpcError::new(ErrorCode::ConfigError, "No active DAG loaded in workspace"),
                )
            }
        };

        let result = GraphGetResult {
            project_name: self
                .workspace_root
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            resources: dag
                .resource_indices
                .keys()
                .map(|r| r.as_str().to_string())
                .collect(),
            artifacts: dag
                .artifact_indices
                .keys()
                .map(|a| a.as_str().to_string())
                .collect(),
            tasks: dag
                .task_indices
                .keys()
                .map(|t| t.as_str().to_string())
                .collect(),
            mermaid: None,
        };

        RpcResponse::success(id, serde_json::to_value(result).unwrap())
    }

    fn handle_impact_get(
        &self,
        id: Option<u64>,
        params: Option<&serde_json::Value>,
    ) -> RpcResponse {
        let dag = match &self.dag {
            Some(d) => d,
            None => {
                return RpcResponse::error(
                    id,
                    RpcError::new(ErrorCode::ConfigError, "No active DAG loaded"),
                )
            }
        };

        let impact_params: ImpactGetParams =
            match params.and_then(|p| serde_json::from_value(p.clone()).ok()) {
                Some(p) => p,
                None => {
                    return RpcResponse::error(
                        id,
                        RpcError::new(
                            ErrorCode::InvalidParams,
                            "Missing ImpactGetParams with 'resource'",
                        ),
                    )
                }
            };

        let analyzer = ImpactAnalyzer::new(dag);
        let res_id = ResourceId::new(&impact_params.resource);
        match analyzer.analyze_resource_change(&res_id) {
            Some(report) => {
                let result = ImpactGetResult {
                    root_resource: impact_params.resource,
                    affected_tasks: report.affected_tasks.into_iter().map(|t| t.0).collect(),
                    unaffected_tasks: report.unaffected_tasks.into_iter().map(|t| t.0).collect(),
                };
                RpcResponse::success(id, serde_json::to_value(result).unwrap())
            }
            None => RpcResponse::error(
                id,
                RpcError::new(
                    ErrorCode::InvalidParams,
                    format!("Resource '{}' not found in DAG", impact_params.resource),
                ),
            ),
        }
    }

    async fn handle_pipeline_run(
        &self,
        id: Option<u64>,
        params: Option<&serde_json::Value>,
    ) -> RpcResponse {
        let dag = match &self.dag {
            Some(d) => d,
            None => {
                return RpcResponse::error(
                    id,
                    RpcError::new(ErrorCode::ConfigError, "No active DAG loaded"),
                )
            }
        };

        let run_params: PipelineRunParams = params
            .and_then(|p| serde_json::from_value(p.clone()).ok())
            .unwrap_or_default();

        let tasks_to_run: Vec<TaskId> = if let Some(res_str) = run_params.resource {
            let res_id = ResourceId::new(res_str);
            let analyzer = ImpactAnalyzer::new(dag);
            match analyzer.analyze_resource_change(&res_id) {
                Some(report) => report.affected_tasks,
                None => Vec::new(),
            }
        } else if let Some(t_list) = run_params.tasks {
            t_list.into_iter().map(TaskId::new).collect()
        } else {
            dag.task_indices.keys().cloned().collect()
        };

        let mut cache_guard = self.cache.lock().await;
        let mut scheduler = TaskScheduler::new(dag, &self.workspace_root, &mut cache_guard);

        match scheduler.execute_tasks(&tasks_to_run).await {
            Ok(summary) => {
                let task_states: Vec<TaskStateSummaryDto> = summary
                    .results
                    .into_iter()
                    .map(|r| TaskStateSummaryDto {
                        task: r.task_id.0,
                        state: r.state.to_string(),
                        duration_ms: r.duration_ms,
                    })
                    .collect();

                let result = PipelineRunResult {
                    success: summary.success,
                    total_duration_ms: summary.total_duration_ms,
                    task_states,
                };
                RpcResponse::success(id, serde_json::to_value(result).unwrap())
            }
            Err(err) => RpcResponse::error(
                id,
                RpcError::new(ErrorCode::TaskExecutionFailed, err.to_string()),
            ),
        }
    }

    fn handle_validate(&self, id: Option<u64>, params: Option<&serde_json::Value>) -> RpcResponse {
        let val_params: ValidateParams = params
            .and_then(|p| serde_json::from_value(p.clone()).ok())
            .unwrap_or_default();

        let target_path = val_params.target.as_ref().map(Path::new);
        let ctx = AdapterContext::new(&self.workspace_root);
        let ctx = if let Some(p) = target_path {
            ctx.with_resource_path(p)
        } else {
            ctx
        };

        let detections = self.registry.detect_all(&ctx);
        if let Some((adapter_id, _)) = detections.first() {
            if let Some(adapter) = self.registry.get(adapter_id) {
                match adapter.validate(&ctx) {
                    Ok(val) => {
                        let dto = ValidationResultDto {
                            is_valid: val.is_valid,
                            errors: val.errors,
                            warnings: val.warnings,
                        };
                        return RpcResponse::success(id, serde_json::to_value(dto).unwrap());
                    }
                    Err(err) => {
                        return RpcResponse::error(
                            id,
                            RpcError::new(ErrorCode::InternalError, err.to_string()),
                        )
                    }
                }
            }
        }

        RpcResponse::error(
            id,
            RpcError::new(
                ErrorCode::InvalidParams,
                "No matching adapter for target validation",
            ),
        )
    }
}
