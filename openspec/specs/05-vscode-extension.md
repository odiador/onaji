# OpenSpec 05: Visual Studio Code Extension Architecture

**Status**: Approved  
**Package**: `dev.odiador.onaji.vscode`  
**Domain**: Editor UI, Language Client, TreeViews, Status Bar, Webview DAG Visualizer  

---

## 1. Design & Responsibilities

La extensión de VS Code es un **cliente UI ultraliviano** desarrollado en TypeScript. Su rol es puramente de visualización y control:

1. **Gestión del Ciclo de Vida del Daemon**:
   - Detecta si el binario `onaji` está en el `PATH` o en el bundle de la extensión.
   - Si no está, ofrece auto-instalación / descarga de releases multiplataforma o guía al desarrollador.
   - Inicia o se conecta al daemon vía JSON-RPC 2.0 (stdio o socket local).
2. **UI & Integraciones**:
   - **Activity Bar View Container**: TreeView con pipelines activos, estado de recursos observados y logs en tiempo real.
   - **Status Bar Item**: Indicador rápido del estado del motor (Idle, Syncing, Failed, Degraded).
   - **DAG Interactive Visualizer (Webview)**: Diagrama reactivo del grafo de dependencias y flujo de tareas.
   - **Comandos de Paleta**: `Onaji: Run Pipeline`, `Onaji: Restart Daemon`, `Onaji: Show Logs`, `Onaji: Validate Config`.
