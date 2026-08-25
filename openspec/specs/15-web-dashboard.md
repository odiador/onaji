# 15 — Onaji Web Dashboard & Real-Time Inspection Server

## 1. Abstract
The Onaji Web Dashboard is a zero-dependency embedded monitoring server providing real-time causal DAG visualization, cluster worker metrics, cache telemetry, distributed scheduling state, and visual task explanation (`onaji explain`).

---

## 2. Server Architecture & Protocol

```text
 ┌───────────────────────────────────────────────────────────┐
 │                       Onaji Daemon                        │
 │           Core DAG • Scheduler • Tracing • Cluster        │
 └─────────────────────────────┬─────────────────────────────┘
                               │
               ┌───────────────┴───────────────┐
               ▼                               ▼
       [HTTP REST Endpoints]           [Server-Sent Events]
       • GET /                         • Real-time task state
       • GET /api/status               • Worker heartbeats
       • GET /api/graph                • Cache hit metrics
       • GET /api/explain?task=...
               │                               │
               └───────────────┬───────────────┘
                               ▼
 ┌───────────────────────────────────────────────────────────┐
 │                   Embedded Web SPA UI                     │
 │   • Interactive DAG (SVG Canvas)                          │
 │   • Worker Cluster Status & Toolchain Grid                │
 │   • Multi-Tier Cache Hit Meter (Local / Remote)           │
 │   • Priority Queue Backpressure Inspector                 │
 │   • Visual Task Causal Trace & Explanation                │
 └───────────────────────────────────────────────────────────┘
```

---

## 3. Endpoints Contract

* `GET /`: Returns the self-contained single-page application with modern dark mode aesthetic.
* `GET /api/status`: JSON summary of engine state, connected workers, cache statistics, and running pipelines.
* `GET /api/graph`: JSON representation of DAG resources, artifacts, tasks, and dependency edges.
* `GET /api/explain?task=<id>`: JSON causal trace with root cause resource, intermediate tasks, cache status, worker assignment, and duration.
