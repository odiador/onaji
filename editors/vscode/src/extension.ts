import * as vscode from "vscode";
import { DaemonManager } from "./daemonManager";
import { TasksTreeProvider, ResourcesTreeProvider } from "./treeProvider";

let daemonManager: DaemonManager | null = null;

export async function activate(context: vscode.ExtensionContext) {
  const workspaceFolders = vscode.workspace.workspaceFolders;
  if (!workspaceFolders || workspaceFolders.length === 0) {
    return;
  }

  const workspaceRoot = workspaceFolders[0].uri.fsPath;
  const outputChannel = vscode.window.createOutputChannel("Onaji Automation");
  context.subscriptions.push(outputChannel);

  daemonManager = new DaemonManager(workspaceRoot, outputChannel);

  const tasksProvider = new TasksTreeProvider();
  const resourcesProvider = new ResourcesTreeProvider();

  vscode.window.registerTreeDataProvider("onaji.tasksView", tasksProvider);
  vscode.window.registerTreeDataProvider("onaji.resourcesView", resourcesProvider);

  const statusBarItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
  statusBarItem.text = "$(sync~spin) Onaji: Connecting...";
  statusBarItem.show();
  context.subscriptions.push(statusBarItem);

  async function syncState() {
    const client = daemonManager?.getClient();
    if (!client) return;
    try {
      const graph = await client.getGraph();
      resourcesProvider.setResources(graph.resources);
      tasksProvider.setTasks(graph.tasks.map((t) => ({ label: t, state: "Ready", duration: 0 })));
    } catch (err: any) {
      outputChannel.appendLine(`[Onaji] Sync state error: ${err.message}`);
    }
  }

  try {
    const client = await daemonManager.start();
    statusBarItem.text = `$(check) Onaji`;
    statusBarItem.tooltip = `Onaji Reactive Automation Daemon active in workspace`;
    await syncState();
  } catch (err: any) {
    statusBarItem.text = "$(error) Onaji: Disconnected";
    statusBarItem.tooltip = `Daemon error: ${err.message}. Click to restart.`;
    statusBarItem.command = "onaji.restartDaemon";
    outputChannel.appendLine(`[Onaji] Daemon startup error: ${err.message}`);
  }

  // Watch onaji.yaml for changes to re-sync
  const watcher = vscode.workspace.createFileSystemWatcher("**/onaji.yaml");
  watcher.onDidChange(async () => {
    outputChannel.appendLine("[Onaji] onaji.yaml changed, reloading state...");
    await syncState();
  });
  context.subscriptions.push(watcher);

  // Register user commands
  context.subscriptions.push(
    vscode.commands.registerCommand("onaji.runPipeline", async () => {
      const client = daemonManager?.getClient();
      if (!client) {
        vscode.window.showErrorMessage("Onaji Daemon is not running.");
        return;
      }

      statusBarItem.text = "$(sync~spin) Onaji: Running...";
      try {
        const result = await client.runPipeline();
        tasksProvider.setTasks(
          result.task_states.map((t) => ({
            label: t.task,
            state: t.state,
            duration: t.duration_ms,
          }))
        );

        if (result.success) {
          statusBarItem.text = `$(check) Onaji (${result.total_duration_ms}ms)`;
          vscode.window.showInformationMessage(`Onaji Pipeline completed in ${result.total_duration_ms}ms`);
        } else {
          statusBarItem.text = "$(error) Onaji: Failed";
          vscode.window.showErrorMessage("Onaji Pipeline execution failed.");
        }
      } catch (err: any) {
        statusBarItem.text = "$(error) Onaji: Error";
        vscode.window.showErrorMessage(`Execution failed: ${err.message}`);
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("onaji.restartDaemon", async () => {
      if (!daemonManager) return;
      statusBarItem.text = "$(sync~spin) Onaji: Restarting...";
      try {
        await daemonManager.restart();
        statusBarItem.text = "$(check) Onaji";
        await syncState();
        vscode.window.showInformationMessage("Onaji Daemon restarted successfully.");
      } catch (err: any) {
        statusBarItem.text = "$(error) Onaji: Failed";
        vscode.window.showErrorMessage(`Failed to restart daemon: ${err.message}`);
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("onaji.showGraph", async () => {
      const client = daemonManager?.getClient();
      if (!client) return;
      const graph = await client.getGraph();
      const doc = await vscode.workspace.openTextDocument({
        content: JSON.stringify(graph, null, 2),
        language: "json",
      });
      await vscode.window.showTextDocument(doc);
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("onaji.openLogs", () => {
      outputChannel.show();
    })
  );
}

export function deactivate() {
  if (daemonManager) {
    daemonManager.stop();
    daemonManager = null;
  }
}
