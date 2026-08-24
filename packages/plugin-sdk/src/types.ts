export type Capability = "Detect" | "Inspect" | "Validate" | "Generate" | "Execute";

export interface WorkspaceFileSystem {
  exists(relativePath: string): Promise<boolean>;
  readFile(relativePath: string, encoding?: BufferEncoding): Promise<string>;
  writeFile(relativePath: string, content: string): Promise<void>;
  listDir(relativePath: string): Promise<string[]>;
  resolve(relativePath: string): string;
}

export interface Logger {
  info(message: string, ...args: any[]): void;
  warn(message: string, ...args: any[]): void;
  error(message: string, ...args: any[]): void;
  debug(message: string, ...args: any[]): void;
}

export interface PluginContext {
  workspaceRoot: string;
  resourcePath?: string;
  fs: WorkspaceFileSystem;
  logger: Logger;
}

export interface DetectionResult {
  matched: boolean;
  confidence: number;
  detected_paths: string[];
  details?: string;
}

export interface ValidationResult {
  is_valid: boolean;
  errors: string[];
  warnings: string[];
}

export interface InspectionProperty {
  key: string;
  value: string;
}

export interface InspectionResult {
  title: string;
  version?: string;
  summary: string;
  properties: InspectionProperty[];
  children: string[];
}

export interface TaskExecutionResult {
  task_id: string;
  state: "Success" | "Failed" | "Blocked" | "Cached";
  exit_code?: number;
  stdout: string;
  stderr: string;
  duration_ms: number;
}

export interface PluginDefinition {
  name: string;
  version: string;
  protocol?: string;
  capabilities: Capability[];

  detect?(ctx: PluginContext): Promise<DetectionResult> | DetectionResult;
  validate?(ctx: PluginContext): Promise<ValidationResult> | ValidationResult;
  inspect?(ctx: PluginContext): Promise<InspectionResult> | InspectionResult;
  execute?(ctx: PluginContext, target: string): Promise<TaskExecutionResult> | TaskExecutionResult;
}

export interface RpcRequest {
  jsonrpc: "2.0";
  id: number | string | null;
  method: string;
  params?: any;
}

export interface RpcResponse<T = any> {
  jsonrpc: "2.0";
  id: number | string | null;
  result?: T;
  error?: {
    code: number;
    message: string;
    data?: any;
  };
}
