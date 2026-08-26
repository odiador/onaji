# CLI Command Reference

| Command | Description |
|---|---|
| `onaji onboard` | Interactive project detection, adapter discovery, doctor check, and scaffold |
| `onaji init [--force]` | Auto-detect toolchains and generate a starter `onaji.yaml` |
| `onaji check` | Validate syntax, acyclicity, and dependency graph completeness |
| `onaji run [--resource <id>] [--remote] [--ci]` | Execute affected DAG tasks with caching and scheduling |
| `onaji explain <task>` | Print causal explanation, trigger reason, and cache status for a task |
| `onaji watch [--debounce <ms>]` | Run reactive file-watcher daemon |
| `onaji dashboard [--port <p>] [--open]` | Start embedded real-time web UI and DAG visualizer |
| `onaji doctor` | Run diagnostics on toolchains, adapters, permissions, and paths |
| `onaji worker <start\|status\|list>` | Manage worker cluster nodes and remote executors |
| `onaji security check` | Audit authentication, command sandboxing, and execution policies |
| `onaji workspace <list\|check\|graph\|impact>` | Multi-workspace monorepo inspection and cross-impact |
| `onaji plugin <list\|search\|info\|add\|update\|remove>` | Manage external plugins and lockfile |
