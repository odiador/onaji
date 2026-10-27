pub const DASHBOARD_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>ONΛJI — Real-Time Control & Cluster Dashboard</title>
  <style>
    :root {
      --bg: #090d16;
      --card-bg: rgba(18, 26, 43, 0.85);
      --card-border: rgba(56, 75, 112, 0.4);
      --accent: #6366f1;
      --accent-glow: rgba(99, 102, 241, 0.35);
      --success: #10b981;
      --running: #3b82f6;
      --cached: #f59e0b;
      --failed: #ef4444;
      --text: #f3f4f6;
      --text-muted: #9ca3af;
    }
    * { box-sizing: border-box; margin: 0; padding: 0; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; }
    body { background-color: var(--bg); color: var(--text); min-height: 100vh; padding: 24px; }
    header { display: flex; justify-content: space-between; align-items: center; padding-bottom: 24px; border-bottom: 1px solid var(--card-border); margin-bottom: 24px; }
    .logo { font-size: 24px; font-weight: 800; letter-spacing: 2px; color: #fff; display: flex; align-items: center; gap: 12px; }
    .logo-badge { background: var(--accent); color: #fff; padding: 4px 10px; border-radius: 6px; font-size: 12px; }
    .status-pill { display: flex; align-items: center; gap: 8px; background: rgba(16, 185, 129, 0.15); color: var(--success); padding: 6px 14px; border-radius: 20px; border: 1px solid rgba(16, 185, 129, 0.3); font-size: 13px; font-weight: 600; }
    .grid { display: grid; grid-template-columns: 2fr 1fr; gap: 24px; margin-bottom: 24px; }
    .card { background: var(--card-bg); border: 1px solid var(--card-border); border-radius: 12px; padding: 20px; backdrop-filter: blur(12px); }
    .card-title { font-size: 16px; font-weight: 700; color: #fff; margin-bottom: 16px; display: flex; justify-content: space-between; align-items: center; }
    .dag-canvas { width: 100%; min-height: 340px; background: rgba(10, 15, 26, 0.7); border-radius: 8px; border: 1px dashed var(--card-border); padding: 20px; display: flex; flex-direction: column; gap: 16px; align-items: center; justify-content: center; position: relative; overflow-y: auto; max-height: 600px; }
    .dag-node { padding: 10px 16px; border-radius: 8px; font-size: 13px; font-weight: 600; display: inline-flex; align-items: center; gap: 8px; box-shadow: 0 4px 12px rgba(0,0,0,0.3); border: 1px solid var(--card-border); }
    .node-resource { background: #1e293b; border-color: #475569; }
    .node-task { background: #1e1b4b; border-color: var(--accent); }
    .node-artifact { background: #064e3b; border-color: var(--success); }
    .cluster-grid { display: grid; grid-template-columns: 1fr; gap: 12px; }
    .worker-card { background: rgba(255, 255, 255, 0.03); border: 1px solid var(--card-border); border-radius: 8px; padding: 12px; display: flex; justify-content: space-between; align-items: center; }
    .worker-id { font-weight: 700; font-size: 14px; }
    .worker-toolchains { display: flex; gap: 6px; margin-top: 4px; }
    .tag { font-size: 10px; background: rgba(255, 255, 255, 0.1); padding: 2px 6px; border-radius: 4px; color: var(--text-muted); }
    .stat-row { display: flex; justify-content: space-between; padding: 8px 0; border-bottom: 1px solid rgba(255, 255, 255, 0.05); font-size: 14px; }
    .progress-bar { height: 8px; background: rgba(255, 255, 255, 0.1); border-radius: 4px; overflow: hidden; margin-top: 6px; }
    .progress-fill { height: 100%; background: var(--accent); border-radius: 4px; }
  </style>
</head>
<body>
  <header>
    <div class="logo">ONΛJI <span class="logo-badge" id="project-badge">CLUSTER ACTIVE</span></div>
    <div style="display:flex;gap:12px;align-items:center;">
      <a href="/metrics" target="_blank" style="color:var(--text-muted);font-size:12px;text-decoration:none;border:1px solid var(--card-border);padding:6px 12px;border-radius:20px;display:flex;align-items:center;gap:6px;">📊 /metrics</a>
      <div class="status-pill"><span id="live-dot" style="display:inline-block;width:8px;height:8px;border-radius:50%;background:var(--success)"></span> <span id="live-status">LIVE STREAM</span></div>
    </div>
  </header>

  <div class="grid">
    <div class="card">
      <div class="card-title">Interactive Causal DAG Visualizer <span class="tag" id="dag-count">Auto-Layout</span></div>
      <div class="dag-canvas" id="dag">
        <div style="display: flex; flex-direction: column; gap: 12px; align-items: center;">
          <div class="dag-node node-resource">📁 Resource: Loading workspace...</div>
        </div>
      </div>
    </div>

    <div class="card">
      <div class="card-title">Worker Cluster Nodes <span id="worker-count" class="tag">0 Connected</span></div>
      <div class="cluster-grid" id="cluster-workers-list">
        <div style="font-size:12px;color:var(--text-muted);padding:8px;">No remote workers registered (local execution)</div>
      </div>

      <div class="card-title" style="margin-top:24px">Distributed Cache (CAS)</div>
      <div class="stat-row">
        <span>Cache Hit Ratio</span>
        <span style="font-weight:700;color:var(--success)" id="cache-ratio">88.4%</span>
      </div>
      <div class="progress-bar"><div class="progress-fill" id="cache-fill" style="width:88.4%"></div></div>
      <div class="stat-row" style="margin-top:10px">
        <span>Tracked Tasks</span>
        <span id="tasks-count">-</span>
      </div>
      <div class="stat-row">
        <span>Cached Task Records</span>
        <span id="cached-tasks-count" style="font-weight:600;color:var(--cached)">-</span>
      </div>
      <div class="stat-row">
        <span>CAS Artifact Packages</span>
        <span id="cas-count" style="font-weight:600">-</span>
      </div>
      <div class="stat-row">
        <span>CAS Disk Footprint</span>
        <span id="cas-size" style="font-weight:600">-</span>
      </div>
      <div class="stat-row">
        <span>Monitored Resources</span>
        <span id="resources-count">-</span>
      </div>
      <div style="margin-top:16px;">
        <button id="btn-prune-cas" onclick="pruneCas()" style="width:100%;padding:8px 12px;background:rgba(239, 68, 68, 0.15);border:1px solid rgba(239, 68, 68, 0.4);color:#fca5a5;border-radius:6px;font-size:12px;font-weight:600;cursor:pointer;transition:all 0.2s ease;">
          🧹 Prune CAS Storage (&lt; 500MB)
        </button>
        <div id="prune-feedback" style="font-size:11px;color:var(--text-muted);text-align:center;margin-top:6px;min-height:16px;"></div>
      </div>
    </div>
  </div>

  <script>
    function formatBytes(bytes) {
      if (!bytes || bytes === 0) return '0 B';
      const k = 1024;
      const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
      const i = Math.floor(Math.log(bytes) / Math.log(k));
      return (bytes / Math.pow(k, i)).toFixed(1) + ' ' + sizes[i];
    }

    async function updateDashboard() {
      try {
        const statusRes = await fetch('/api/status');
        if (statusRes.ok) {
          const status = await statusRes.json();
          if (status.project_name) {
            document.getElementById('project-badge').innerText = status.project_name.toUpperCase();
          }
          if (status.tasks_count !== undefined) {
            document.getElementById('tasks-count').innerText = status.tasks_count;
          }
          if (status.resources_count !== undefined) {
            document.getElementById('resources-count').innerText = status.resources_count;
          }
          if (status.cached_tasks_count !== undefined) {
            document.getElementById('cached-tasks-count').innerText = status.cached_tasks_count;
          }
          if (status.cas_artifacts_count !== undefined) {
            document.getElementById('cas-count').innerText = status.cas_artifacts_count + ' packages';
          }
          if (status.cas_size_bytes !== undefined) {
            document.getElementById('cas-size').innerText = formatBytes(status.cas_size_bytes);
          }
          if (status.cache_hit_ratio !== undefined) {
            const pct = (status.cache_hit_ratio * 100).toFixed(1) + '%';
            document.getElementById('cache-ratio').innerText = pct;
            document.getElementById('cache-fill').style.width = pct;
          }
        }

        const graphRes = await fetch('/api/graph');
        if (graphRes.ok) {
          const graph = await graphRes.json();
          if (graph.nodes && graph.nodes.length > 0) {
            const container = document.getElementById('dag');
            container.innerHTML = '';
            document.getElementById('dag-count').innerText = graph.nodes.length + ' Nodes';

            const flow = document.createElement('div');
            flow.style.display = 'flex';
            flow.style.flexDirection = 'column';
            flow.style.gap = '10px';
            flow.style.alignItems = 'center';
            flow.style.width = '100%';

            graph.nodes.forEach((node, idx) => {
              const nodeEl = document.createElement('div');
              nodeEl.className = 'dag-node';
              if (node.type === 'resource') {
                nodeEl.classList.add('node-resource');
                nodeEl.innerText = '📁 Resource: ' + node.id;
              } else if (node.type === 'task') {
                nodeEl.classList.add('node-task');
                nodeEl.innerText = '⚡ Task: ' + node.id;
              } else {
                nodeEl.classList.add('node-artifact');
                nodeEl.innerText = '📦 Artifact: ' + node.id;
              }
              flow.appendChild(nodeEl);
              if (idx < graph.nodes.length - 1) {
                const arrow = document.createElement('div');
                arrow.style.color = 'var(--text-muted)';
                arrow.style.fontSize = '12px';
                arrow.innerText = '↓';
                flow.appendChild(arrow);
              }
            });
            container.appendChild(flow);
          }
        }

        const workersRes = await fetch('/api/workers');
        if (workersRes.ok) {
          const wData = await workersRes.json();
          const container = document.getElementById('cluster-workers-list');
          if (container && wData.workers) {
            document.getElementById('worker-count').innerText = wData.workers.length + ' Connected';
            if (wData.workers.length === 0) {
              container.innerHTML = '<div style="font-size:12px;color:var(--text-muted);padding:8px;">No remote workers registered (local execution)</div>';
            } else {
              container.innerHTML = '';
              wData.workers.forEach(w => {
                const div = document.createElement('div');
                div.className = 'worker-card';
                const tcTags = Object.entries(w.toolchains || {}).map(([k, v]) => `<span class="tag">${k} ${v}</span>`).join(' ');
                div.innerHTML = `
                  <div>
                    <div class="worker-id">${w.worker_id}</div>
                    <div class="worker-toolchains">${tcTags}</div>
                  </div>
                  <div style="text-align:right">
                    <div style="font-size:12px;color:var(--success);font-weight:700">${w.status}</div>
                    <div style="font-size:11px;color:var(--text-muted)">Load: ${w.active_tasks} / ${w.max_parallel_tasks}</div>
                  </div>
                `;
                container.appendChild(div);
              });
            }
          }
        }
      } catch (e) {
        console.warn('Dashboard update failed:', e);
      }
    }
    updateDashboard();
    setInterval(updateDashboard, 5000);

    async function pruneCas() {
      const btn = document.getElementById('btn-prune-cas');
      const feedback = document.getElementById('prune-feedback');
      btn.disabled = true;
      btn.innerText = 'Pruning...';
      try {
        const res = await fetch('/api/clean?max_size=500MB', { method: 'POST' });
        if (res.ok) {
          const data = await res.json();
          feedback.style.color = 'var(--success)';
          feedback.innerText = `Pruned ${data.evicted_packages_count} packages (${formatBytes(data.freed_bytes)} freed)`;
          updateDashboard();
        } else {
          feedback.style.color = 'var(--failed)';
          feedback.innerText = 'Failed to prune CAS';
        }
      } catch (err) {
        feedback.style.color = 'var(--failed)';
        feedback.innerText = 'Error: ' + err.message;
      } finally {
        btn.disabled = false;
        btn.innerText = '🧹 Prune CAS Storage (< 500MB)';
      }
    }

    // Conexión reactiva en vivo vía Server-Sent Events (SSE)
    try {
      const evtSource = new EventSource('/api/events');
      evtSource.addEventListener('system.ready', () => {
        document.getElementById('live-status').innerText = 'LIVE STREAM';
        document.getElementById('live-dot').style.background = 'var(--success)';
      });
      evtSource.addEventListener('task.started', () => {
        updateDashboard();
      });
      evtSource.addEventListener('task.completed', () => {
        updateDashboard();
      });
      evtSource.addEventListener('task.cached', () => {
        updateDashboard();
      });
      evtSource.onerror = () => {
        document.getElementById('live-status').innerText = 'RECONNECTING';
        document.getElementById('live-dot').style.background = 'var(--cached)';
      };
    } catch (e) {
      console.warn('SSE unsupported:', e);
    }
  </script>
</body>
</html>
"#;
