import * as vscode from "vscode";
import { OnajiDaemonClient } from "./client";
import { TasksTreeProvider, ResourcesTreeProvider } from "./treeProvider";

let client: OnajiDaemonClient | null = null;

export async function activate(context: vscode.ExtensionContext) {
  const workspaceFolders = vscode.workspace.workspaceFolders;
  if (!workspaceFolders || workspaceFolders.length === 0) {
    return;
  }

  const workspaceRoot = workspaceFolders[0].uri.fsPath;
  client = new OnajiDaemonClient(workspaceRoot);

  const tasksProvider = new TasksTreeProvider();
  const resourcesProvider = new ResourcesTreeProvider();

  vscode.window.registerTreeDataProvider("onaji.tasksView", tasksProvider);
  vscode.window.registerTreeDataProvider("onaji.resourcesView", resourcesProvider);

  const statusBarItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
  statusBarItem.text = "$(sync~spin) Onaji: Starting...";
  statusBarItem.show();
  context.subscriptions.push(statusBarItem);

  try {
    const initResult = await client.start();
    statusBarItem.text = `$(check) Onaji v${initResult.engine_version}`;
    statusBarItem.tooltip = `Watching ${initResult.active_resources} resources across ${initResult.active_tasks} tasks`;

    const graph = await client.getGraph();
    resourcesProvider.setResources(graph.resources);
    tasksProvider.setTasks(graph.tasks.map((t) => ({ label: t, state: "Ready", duration: 0 })));
  } catch (err: any) {
    statusBarItem.text = "$(error) Onaji: Daemon Error";
    vscode.window.showErrorMessage(`Failed to connect to Onaji Daemon: ${err.message}`);
  }

  // Register commands
  context.subscriptions.push(
    vscode.commands.registerCommand("onaji.runPipeline", async () => {
      if (!client) return;
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
          statusBarItem.text = `$(check) Onaji: Completed in ${result.total_duration_ms}ms`;
          vscode.window.showInformationMessage(`Onaji Pipeline completed in ${result.total_duration_ms}ms`);
        } else {
          statusBarItem.text = "$(error) Onaji: Pipeline Failed";
          vscode.window.showErrorMessage("Onaji Pipeline failed during execution");
        }
      } catch (err: any) {
        vscode.window.showErrorMessage(`Execution failed: ${err.message}`);
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("onaji.showGraph", async () => {
      if (!client) return;
      const graph = await client.getGraph();
      const doc = await vscode.workspace.openTextDocument({
        content: JSON.stringify(graph, null, 2),
        language: "json",
      });
      await vscode.window.showTextDocument(doc);
    })
  );
}

export function deactivate() {
  if (client) {
    client.stop();
    client = null;
  }
}
