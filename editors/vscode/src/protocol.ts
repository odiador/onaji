export interface RpcRequest {
  jsonrpc: "2.0";
  id: number;
  method: string;
  params?: any;
}

export interface RpcResponse<T = any> {
  jsonrpc: "2.0";
  id?: number;
  result?: T;
  error?: {
    code: number;
    message: string;
    data?: any;
  };
}

export interface InitializeResult {
  engine_version: string;
  active_resources: number;
  active_tasks: number;
}

export interface GraphGetResult {
  project_name: string;
  resources: string[];
  artifacts: string[];
  tasks: string[];
  mermaid?: string;
}

export interface TaskStateSummaryDto {
  task: string;
  state: string;
  duration_ms: number;
}

export interface PipelineRunResult {
  success: boolean;
  total_duration_ms: number;
  task_states: TaskStateSummaryDto[];
}
