import * as vscode from "vscode";

export class OnajiCodeLensProvider implements vscode.CodeLensProvider {
  provideCodeLenses(
    document: vscode.TextDocument,
    _token: vscode.CancellationToken
  ): vscode.CodeLens[] | Thenable<vscode.CodeLens[]> {
    const codeLenses: vscode.CodeLens[] = [];
    const text = document.getText();
    const lines = text.split("\n");

    let inTasksSection = false;

    for (let i = 0; i < lines.length; i++) {
      const line = lines[i];

      if (line.match(/^tasks:\s*$/)) {
        inTasksSection = true;
        continue;
      }

      if (inTasksSection && line.match(/^[a-zA-Z0-9_-]+:\s*$/) && !line.startsWith("  ")) {
        inTasksSection = false;
      }

      if (inTasksSection) {
        const taskMatch = line.match(/^\s\s([a-zA-Z0-9_-]+):\s*$/);
        if (taskMatch) {
          const taskId = taskMatch[1];
          const range = new vscode.Range(i, 0, i, line.length);

          codeLenses.push(
            new vscode.CodeLens(range, {
              title: "▶ Run Task",
              command: "onaji.runPipeline",
              arguments: [taskId],
              tooltip: `Execute reactive task '${taskId}'`,
            })
          );

          codeLenses.push(
            new vscode.CodeLens(range, {
              title: "🔍 Explain Impact",
              command: "onaji.openVisualGraph",
              arguments: [taskId],
              tooltip: `Inspect causal dependency tree for '${taskId}'`,
            })
          );
        }
      }
    }

    return codeLenses;
  }
}
