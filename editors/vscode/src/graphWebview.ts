import * as vscode from "vscode";
import { OnajiDaemonClient } from "./client";

export class GraphWebviewPanel {
  public static currentPanel: GraphWebviewPanel | undefined;
  private readonly _panel: vscode.WebviewPanel;
  private readonly _extensionUri: vscode.Uri;
  private _disposables: vscode.Disposable[] = [];

  public static createOrShow(extensionUri: vscode.Uri, client: OnajiDaemonClient | null) {
    const column = vscode.window.activeTextEditor
      ? vscode.window.activeTextEditor.viewColumn
      : undefined;

    if (GraphWebviewPanel.currentPanel) {
      GraphWebviewPanel.currentPanel._panel.reveal(column);
      if (client) {
        GraphWebviewPanel.currentPanel.updateGraph(client);
      }
      return;
    }

    const panel = vscode.window.createWebviewPanel(
      "onajiGraph",
      "Onaji Causal Graph",
      column || vscode.ViewColumn.One,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
      }
    );

    GraphWebviewPanel.currentPanel = new GraphWebviewPanel(panel, extensionUri, client);
  }

  private constructor(
    panel: vscode.WebviewPanel,
    extensionUri: vscode.Uri,
    client: OnajiDaemonClient | null
  ) {
    this._panel = panel;
    this._extensionUri = extensionUri;

    this._panel.onDidDispose(() => this.dispose(), null, this._disposables);

    this._panel.webview.onDidReceiveMessage(
      async (message) => {
        switch (message.command) {
          case "runTask":
            if (client) {
              vscode.window.showInformationMessage(`Running task '${message.taskId}'...`);
              try {
                await client.runPipeline();
                this.updateGraph(client);
              } catch (err: any) {
                vscode.window.showErrorMessage(`Failed to run task: ${err.message}`);
              }
            }
            return;
          case "refresh":
            if (client) {
              this.updateGraph(client);
            }
            return;
        }
      },
      null,
      this._disposables
    );

    if (client) {
      this.updateGraph(client);
    }
  }

  public async updateGraph(client: OnajiDaemonClient) {
    try {
      const graph = await client.getGraph();
      this._panel.webview.html = this._getHtmlForWebview(graph);
    } catch (err: any) {
      this._panel.webview.html = `<html><body style="font-family:sans-serif;color:#f87171;padding:20px;"><h3>Error loading graph</h3><p>${err.message}</p></body></html>`;
    }
  }

  private _getHtmlForWebview(graph: any) {
    const resources = graph.resources || [];
    const tasks = graph.tasks || [];
    const artifacts = graph.artifacts || [];

    const nodesJson = JSON.stringify({ resources, tasks, artifacts });

    return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>Onaji Causal Dependency DAG</title>
  <style>
    :root {
      --bg: #090d16;
      --card-bg: rgba(18, 24, 38, 0.85);
      --border: rgba(255, 255, 255, 0.08);
      --accent: #6366f1;
      --accent-glow: rgba(99, 102, 241, 0.35);
      --success: #10b981;
      --text: #f1f5f9;
      --text-muted: #94a3b8;
    }
    body {
      margin: 0;
      padding: 24px;
      background: var(--bg);
      color: var(--text);
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
      overflow-x: hidden;
    }
    .header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 24px;
      padding-bottom: 16px;
      border-bottom: 1px solid var(--border);
    }
    .title {
      font-size: 20px;
      font-weight: 700;
      letter-spacing: -0.02em;
      display: flex;
      align-items: center;
      gap: 10px;
    }
    .badge {
      font-size: 11px;
      font-weight: 600;
      padding: 3px 8px;
      border-radius: 9999px;
      background: var(--accent-glow);
      color: #a5b4fc;
      border: 1px solid rgba(99, 102, 241, 0.3);
    }
    .btn {
      background: var(--accent);
      color: #fff;
      border: none;
      padding: 8px 14px;
      border-radius: 6px;
      font-size: 13px;
      font-weight: 600;
      cursor: pointer;
      transition: all 0.2s;
    }
    .btn:hover {
      background: #4f46e5;
      box-shadow: 0 0 12px var(--accent-glow);
    }
    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
      gap: 20px;
    }
    .col {
      background: var(--card-bg);
      border: 1px solid var(--border);
      border-radius: 12px;
      padding: 16px;
      backdrop-filter: blur(12px);
    }
    .col-title {
      font-size: 12px;
      font-weight: 700;
      text-transform: uppercase;
      letter-spacing: 0.05em;
      color: var(--text-muted);
      margin-bottom: 14px;
      display: flex;
      align-items: center;
      justify-content: space-between;
    }
    .node-card {
      background: rgba(255, 255, 255, 0.03);
      border: 1px solid var(--border);
      border-radius: 8px;
      padding: 12px;
      margin-bottom: 10px;
      transition: all 0.2s;
      position: relative;
    }
    .node-card:hover {
      border-color: rgba(99, 102, 241, 0.4);
      transform: translateY(-1px);
    }
    .node-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      font-weight: 600;
      font-size: 14px;
    }
    .node-meta {
      font-size: 12px;
      color: var(--text-muted);
      margin-top: 4px;
    }
    .tag {
      font-size: 10px;
      padding: 2px 6px;
      border-radius: 4px;
      background: rgba(255, 255, 255, 0.06);
      color: var(--text-muted);
    }
    .run-btn {
      font-size: 11px;
      padding: 4px 8px;
      border-radius: 4px;
      background: rgba(16, 185, 129, 0.15);
      color: #34d399;
      border: 1px solid rgba(16, 185, 129, 0.3);
      cursor: pointer;
      margin-top: 8px;
    }
    .run-btn:hover {
      background: rgba(16, 185, 129, 0.3);
    }
  </style>
</head>
<body>
  <div class="header">
    <div class="title">
      <span>ONΛJI Causal Graph</span>
      <span class="badge">Live DAG</span>
    </div>
    <button class="btn" onclick="refresh()">Sync Graph</button>
  </div>

  <div class="grid">
    <div class="col">
      <div class="col-title">
        <span>Observed Resources</span>
        <span class="tag">${resources.length}</span>
      </div>
      ${resources.map((r: string) => `
        <div class="node-card">
          <div class="node-header">
            <span>📄 ${r}</span>
            <span class="tag">Resource</span>
          </div>
          <div class="node-meta">Triggers causal invalidation</div>
        </div>
      `).join("")}
    </div>

    <div class="col">
      <div class="col-title">
        <span>Reactive Tasks</span>
        <span class="tag">${tasks.length}</span>
      </div>
      ${tasks.map((t: string) => `
        <div class="node-card">
          <div class="node-header">
            <span>⚡ ${t}</span>
            <span class="tag" style="color:#34d399;">Ready</span>
          </div>
          <div class="node-meta">Cached / Content-addressed</div>
          <button class="run-btn" onclick="runTask('${t}')">▶ Execute Task</button>
        </div>
      `).join("")}
    </div>

    <div class="col">
      <div class="col-title">
        <span>Monitored Artifacts</span>
        <span class="tag">${artifacts.length}</span>
      </div>
      ${artifacts.map((a: string) => `
        <div class="node-card">
          <div class="node-header">
            <span>📦 ${a}</span>
            <span class="tag">Artifact</span>
          </div>
          <div class="node-meta">Fingerprinted & pinned in CAS</div>
        </div>
      `).join("")}
    </div>
  </div>

  <script>
    const vscode = acquireVsCodeApi();
    function refresh() {
      vscode.postMessage({ command: 'refresh' });
    }
    function runTask(taskId) {
      vscode.postMessage({ command: 'runTask', taskId: taskId });
    }
  </script>
</body>
</html>`;
  }

  public dispose() {
    GraphWebviewPanel.currentPanel = undefined;
    this._panel.dispose();
    while (this._disposables.length) {
      const x = this._disposables.pop();
      if (x) {
        x.dispose();
      }
    }
  }
}
