# Onaji Reactive Automation (VS Code Extension)

Universal Reactive Automation Engine interface for Visual Studio Code.

## Features

- **Daemon Integration**: Auto-discovers and connects to the Onaji JSON-RPC Daemon.
- **Pipeline Tasks View**: Monitor live states (`✓`, `⚡ cached`, `○ blocked`, `✗ failed`) in the ActivityBar.
- **Observed Resources View**: Real-time inspection of watched files and HTTP endpoints.
- **Dependency Graph Visualizer**: Inspect topological DAG models.
- **Interactive Execution**: Run pipelines or restart the daemon with one click.

## Getting Started

1. Ensure `onaji` is installed on your system or run `onaji init` in your project root.
2. Open any workspace containing an `onaji.yaml` manifest.
3. Open the Onaji ActivityBar panel to trigger reactive automations.
