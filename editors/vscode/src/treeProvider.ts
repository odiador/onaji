import * as vscode from "vscode";
import { OnajiDaemonClient } from "./client";

export class OnajiTaskTreeItem extends vscode.TreeItem {
  constructor(
    public readonly label: string,
    public readonly state: string = "Pending",
    public readonly durationMs: number = 0
  ) {
    super(label, vscode.TreeItemCollapsibleState.None);
    this.description = state === "Cached" ? "⚡ cached" : state === "Success" ? `✓ ${durationMs}ms` : state;
    this.iconPath = new vscode.ThemeIcon(
      state === "Success" ? "check" : state === "Cached" ? "zap" : state === "Failed" ? "error" : "circle-outline"
    );
  }
}

export class OnajiResourceTreeItem extends vscode.TreeItem {
  constructor(public readonly label: string) {
    super(label, vscode.TreeItemCollapsibleState.None);
    this.iconPath = new vscode.ThemeIcon("file-code");
  }
}

export class TasksTreeProvider implements vscode.TreeDataProvider<OnajiTaskTreeItem> {
  private _onDidChangeTreeData = new vscode.EventEmitter<OnajiTaskTreeItem | undefined | void>();
  readonly onDidChangeTreeData = this._onDidChangeTreeData.event;
  private tasks: { label: string; state: string; duration: number }[] = [];

  public setTasks(tasks: { label: string; state: string; duration: number }[]): void {
    this.tasks = tasks;
    this._onDidChangeTreeData.fire();
  }

  getTreeItem(element: OnajiTaskTreeItem): vscode.TreeItem {
    return element;
  }

  getChildren(): Thenable<OnajiTaskTreeItem[]> {
    return Promise.resolve(this.tasks.map((t) => new OnajiTaskTreeItem(t.label, t.state, t.duration)));
  }
}

export class ResourcesTreeProvider implements vscode.TreeDataProvider<OnajiResourceTreeItem> {
  private _onDidChangeTreeData = new vscode.EventEmitter<OnajiResourceTreeItem | undefined | void>();
  readonly onDidChangeTreeData = this._onDidChangeTreeData.event;
  private resources: string[] = [];

  public setResources(resources: string[]): void {
    this.resources = resources;
    this._onDidChangeTreeData.fire();
  }

  getTreeItem(element: OnajiResourceTreeItem): vscode.TreeItem {
    return element;
  }

  getChildren(): Thenable<OnajiResourceTreeItem[]> {
    return Promise.resolve(this.resources.map((r) => new OnajiResourceTreeItem(r)));
  }
}
