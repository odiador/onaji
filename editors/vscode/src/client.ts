import { spawn, ChildProcess } from "child_process";
import * as readline from "readline";
import { RpcRequest, RpcResponse, InitializeResult, GraphGetResult, PipelineRunResult } from "./protocol";

export class OnajiDaemonClient {
  private process: ChildProcess | null = null;
  private nextId = 1;
  private pendingRequests = new Map<number, { resolve: (val: any) => void; reject: (err: any) => void }>();

  constructor(private workspaceRoot: string, private binaryPath: string = "onaji") {}

  public async start(): Promise<InitializeResult> {
    this.process = spawn(this.binaryPath, ["daemon", "--stdio", "-c", `${this.workspaceRoot}/onaji.yaml`], {
      cwd: this.workspaceRoot,
      stdio: ["pipe", "pipe", "pipe"],
    });

    const rl = readline.createInterface({
      input: this.process.stdout!,
      crlfDelay: Infinity,
    });

    rl.on("line", (line) => {
      const trimmed = line.trim();
      if (!trimmed) return;
      try {
        const response: RpcResponse = JSON.parse(trimmed);
        if (response.id && this.pendingRequests.has(response.id)) {
          const { resolve, reject } = this.pendingRequests.get(response.id)!;
          this.pendingRequests.delete(response.id);
          if (response.error) {
            reject(new Error(response.error.message));
          } else {
            resolve(response.result);
          }
        }
      } catch (err) {
        console.error("Failed to parse daemon response:", line, err);
      }
    });

    return this.sendRequest<InitializeResult>("initialize", {
      workspace_root: this.workspaceRoot,
      client_info: { name: "vscode-onaji", version: "0.1.0" },
    });
  }

  public async getGraph(): Promise<GraphGetResult> {
    return this.sendRequest<GraphGetResult>("graph/get");
  }

  public async runPipeline(resource?: string): Promise<PipelineRunResult> {
    return this.sendRequest<PipelineRunResult>("pipeline/run", { resource });
  }

  public stop(): void {
    if (this.process) {
      this.process.kill();
      this.process = null;
    }
  }

  private sendRequest<T>(method: string, params?: any): Promise<T> {
    return new Promise((resolve, reject) => {
      if (!this.process || !this.process.stdin) {
        return reject(new Error("Daemon is not running"));
      }

      const id = this.nextId++;
      this.pendingRequests.set(id, { resolve, reject });

      const request: RpcRequest = {
        jsonrpc: "2.0",
        id,
        method,
        params,
      };

      this.process.stdin.write(JSON.stringify(request) + "\n");
    });
  }
}
