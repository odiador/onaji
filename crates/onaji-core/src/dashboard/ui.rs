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
    .dag-canvas { width: 100%; min-height: 340px; background: rgba(10, 15, 26, 0.7); border-radius: 8px; border: 1px dashed var(--card-border); display: flex; align-items: center; justify-content: center; position: relative; overflow: hidden; }
    .dag-node { padding: 12px 18px; border-radius: 8px; font-size: 13px; font-weight: 600; margin: 8px; display: inline-flex; align-items: center; gap: 8px; box-shadow: 0 4px 12px rgba(0,0,0,0.3); border: 1px solid var(--card-border); }
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
    <div class="logo">ONΛJI <span class="logo-badge">CLUSTER ACTIVE</span></div>
    <div class="status-pill"><span style="display:inline-block;width:8px;height:8px;border-radius:50%;background:var(--success)"></span> DAEMON READY (JSON-RPC)</div>
  </header>

  <div class="grid">
    <div class="card">
      <div class="card-title">Interactive Causal DAG Visualizer <span class="tag">Auto-Layout</span></div>
      <div class="dag-canvas" id="dag">
        <div style="display: flex; flex-direction: column; gap: 16px; align-items: center;">
          <div class="dag-node node-resource">📁 Resource: openapi_spec (api/openapi.yaml)</div>
          <div style="color:var(--text-muted)">↓</div>
          <div class="dag-node node-task" style="border-color:var(--running)">⚡ Task: validate_openapi (redocly lint)</div>
          <div style="color:var(--text-muted)">↓</div>
          <div class="dag-node node-artifact">📦 Artifact: openapi_contract</div>
          <div style="color:var(--text-muted)">↓</div>
          <div class="dag-node node-task" style="border-color:var(--success)">⚡ Task: generate_kotlin (openapi-generator)</div>
        </div>
      </div>
    </div>

    <div class="card">
      <div class="card-title">Worker Cluster Nodes <span id="worker-count" class="tag">2 Connected</span></div>
      <div class="cluster-grid">
        <div class="worker-card">
          <div>
            <div class="worker-id">worker-linux-01</div>
            <div class="worker-toolchains">
              <span class="tag">rust 1.78</span>
              <span class="tag">node 20</span>
            </div>
          </div>
          <div style="text-align:right">
            <div style="font-size:12px;color:var(--success);font-weight:700">ONLINE</div>
            <div style="font-size:11px;color:var(--text-muted)">Slots: 2 / 4</div>
          </div>
        </div>

        <div class="worker-card">
          <div>
            <div class="worker-id">worker-android-02</div>
            <div class="worker-toolchains">
              <span class="tag">android_sdk 34</span>
              <span class="tag">java 21</span>
            </div>
          </div>
          <div style="text-align:right">
            <div style="font-size:12px;color:var(--success);font-weight:700">ONLINE</div>
            <div style="font-size:11px;color:var(--text-muted)">Slots: 1 / 4</div>
          </div>
        </div>
      </div>

      <div class="card-title" style="margin-top:24px">Distributed Cache (CAS)</div>
      <div class="stat-row">
        <span>Cache Hit Ratio</span>
        <span style="font-weight:700;color:var(--success)">88.4%</span>
      </div>
      <div class="progress-bar"><div class="progress-fill" style="width:88.4%"></div></div>
      <div class="stat-row" style="margin-top:10px">
        <span>Local Tier 1 Hits</span>
        <span>142</span>
      </div>
      <div class="stat-row">
        <span>Remote Tier 2 Hits</span>
        <span>48</span>
      </div>
    </div>
  </div>
</body>
</html>
"#;
