# OpenSpec 02: Client-Engine IPC Protocol (JSON-RPC 2.0)

**Status**: Approved  
**Package**: `dev.odiador.onaji.protocol`  
**Domain**: Inter-Process Communication, IDE Integration, Event Streaming  

---

## 1. Protocol Overview

Onaji utiliza **JSON-RPC 2.0** estructurado sobre flujos bidireccionales (`stdio` para CLI/subprocesos o Unix Domain Sockets/Named Pipes para daemons persistentes).

Este desacoplamiento permite que clientes como la extensión de VS Code, plugins de JetBrains o interfaces TUI sean **clientes delgados** que solo consumen y comandan el estado del daemon.

---

## 2. Core RPC Methods

### `onaji/initialize`
Inicializa la sesión con el workspace.

* **Request**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "onaji/initialize",
  "params": {
    "workspaceRoot": "/path/to/project",
    "clientInfo": {
      "name": "vscode-onaji",
      "version": "0.1.0"
    }
  }
}
```

* **Response**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "engineVersion": "0.1.0",
    "activePipelines": 2,
    "registeredResources": 4
  }
}
```

---

### `onaji/runPipeline`
Dispara manualmente la ejecución de un pipeline o tarea específica.

* **Request**:
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "onaji/runPipeline",
  "params": {
    "pipelineId": "sync_api_contracts",
    "force": true
  }
}
```

---

## 3. Server-Sent Notifications (Stream de Eventos)

El motor emite notificaciones asíncronas para actualizar la UI del editor en tiempo real:

### `onaji/pipelineStateChanged`
```json
{
  "jsonrpc": "2.0",
  "method": "onaji/pipelineStateChanged",
  "params": {
    "pipelineId": "sync_api_contracts",
    "step": "generate_kotlin_client",
    "status": "running",
    "timestamp": 1755897600
  }
}
```

### `onaji/taskOutput`
```json
{
  "jsonrpc": "2.0",
  "method": "onaji/taskOutput",
  "params": {
    "pipelineId": "sync_api_contracts",
    "step": "gradle_check",
    "stream": "stdout",
    "line": "BUILD SUCCESSFUL in 3s"
  }
}
```
