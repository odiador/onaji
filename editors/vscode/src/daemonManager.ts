import * as vscode from "vscode";
import * as path from "path";
import * as fs from "fs";
import * as os from "os";
import { OnajiDaemonClient } from "./client";

export class DaemonManager {
  private client: OnajiDaemonClient | null = null;
  private isRestarting = false;

  constructor(private workspaceRoot: string, private outputChannel: vscode.OutputChannel) {}

  public async findOrResolveExecutable(): Promise<string | null> {
    const config = vscode.workspace.getConfiguration("onaji");
    const customPath = config.get<string>("executablePath");
    if (customPath && fs.existsSync(customPath)) {
      return customPath;
    }

    const candidates = [
      "onaji",
      path.join(os.homedir(), ".onaji", "bin", "onaji"),
      "/usr/local/bin/onaji",
      "/opt/homebrew/bin/onaji",
      path.join(this.workspaceRoot, "target", "debug", "onaji"),
      path.join(this.workspaceRoot, "..", "..", "target", "debug", "onaji"),
    ];

    for (const cand of candidates) {
      if (cand === "onaji") continue;
      if (fs.existsSync(cand)) {
        return cand;
      }
    }

    return "onaji";
  }

  public async start(): Promise<OnajiDaemonClient> {
    const executable = await this.findOrResolveExecutable();
    this.outputChannel.appendLine(`[Onaji] Starting Daemon using binary: ${executable || "onaji"}`);

    this.client = new OnajiDaemonClient(this.workspaceRoot, executable || "onaji");
    await this.client.start();
    this.outputChannel.appendLine(`[Onaji] Daemon connected successfully.`);
    return this.client;
  }

  public async restart(): Promise<OnajiDaemonClient> {
    if (this.isRestarting) {
      throw new Error("Daemon is already restarting");
    }
    this.isRestarting = true;
    try {
      if (this.client) {
        this.client.stop();
        this.client = null;
      }
      return await this.start();
    } finally {
      this.isRestarting = false;
    }
  }

  public getClient(): OnajiDaemonClient | null {
    return this.client;
  }

  public stop(): void {
    if (this.client) {
      this.client.stop();
      this.client = null;
    }
  }
}
