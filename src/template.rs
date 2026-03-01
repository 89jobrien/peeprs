pub fn render_html(refresh_ms: u64) -> String {
    let template = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <title>Agent Logs Dashboard</title>
  <style>
    :root {
      --bg-a: #0b1320;
      --bg-b: #0f1a2b;
      --bg-c: #0a121f;
      --card: #111b2b;
      --card-strong: #0c1624;
      --ink: #eaf1fc;
      --muted: #9fb1c8;
      --line: #203047;
      --line-soft: #1a273c;
      --accent: #4da3ff;
      --accent-soft: #1a2c45;
      --json: #40b8ff;
      --text: #f9b266;
      --good: #4bc08c;
      --shadow: 0 8px 18px rgba(1, 8, 18, 0.24);
    }

    * { box-sizing: border-box; }

    body {
      margin: 0;
      font-family: "Plus Jakarta Sans", "IBM Plex Sans", "Avenir Next", "Segoe UI", sans-serif;
      color: var(--ink);
      background:
        radial-gradient(1000px 620px at -10% -18%, #17335d 0%, transparent 58%),
        radial-gradient(1000px 620px at 112% 118%, #1b3552 0%, transparent 56%),
        linear-gradient(150deg, var(--bg-a), var(--bg-b) 54%, var(--bg-c));
      min-height: 100vh;
      padding: 14px;
    }

    body::before {
      content: "";
      position: fixed;
      inset: 0;
      pointer-events: none;
      background-image:
        linear-gradient(rgba(120, 150, 190, 0.06) 1px, transparent 1px),
        linear-gradient(90deg, rgba(120, 150, 190, 0.06) 1px, transparent 1px);
      background-size: 28px 28px;
      opacity: 0.16;
      mask-image: radial-gradient(circle at 30% 20%, black 20%, transparent 75%);
    }

    .wrap { max-width: 1480px; margin: 0 auto; }

    .header {
      display: flex;
      align-items: end;
      justify-content: space-between;
      gap: 12px;
      margin-bottom: 10px;
    }

    h1 {
      margin: 0;
      font-size: 27px;
      letter-spacing: 0.16px;
      font-weight: 700;
    }

    .sub {
      color: var(--muted);
      margin-top: 3px;
      font-size: 12px;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }

    .kpi-grid {
      display: grid;
      grid-template-columns: repeat(10, minmax(0, 1fr));
      gap: 8px;
      margin-bottom: 10px;
    }

    @media (max-width: 1240px) {
      .kpi-grid { grid-template-columns: repeat(5, minmax(0, 1fr)); }
    }

    @media (max-width: 740px) {
      .kpi-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
      .header { flex-direction: column; align-items: start; }
    }

    .card {
      background: linear-gradient(180deg, #142238 0%, var(--card) 100%);
      border: 1px solid var(--line);
      border-radius: 3px;
      padding: 10px;
      box-shadow: var(--shadow);
      animation: card-enter 180ms ease both;
    }

    .kpi {
      min-height: 72px;
      border-color: var(--line-soft);
    }

    .k {
      color: var(--muted);
      font-size: 10px;
      text-transform: uppercase;
      letter-spacing: 0.58px;
      font-weight: 600;
    }

    .v {
      font-size: 24px;
      font-weight: 700;
      margin-top: 4px;
      line-height: 1.1;
    }

    .v-small {
      font-size: 16px;
      margin-top: 7px;
      color: #d5e4f9;
      font-weight: 650;
    }

    .main {
      display: grid;
      grid-template-columns: 2fr 1fr;
      gap: 8px;
      margin-bottom: 8px;
    }

    @media (max-width: 1020px) {
      .main { grid-template-columns: 1fr; }
    }

    h2 {
      margin: 1px 0 8px 0;
      font-size: 13px;
      letter-spacing: 0.22px;
      font-weight: 680;
    }

    .panel {
      display: grid;
      gap: 8px;
    }

    .chart-card {
      height: 220px;
      display: grid;
      grid-template-rows: auto 1fr;
    }

    .bars {
      display: grid;
      align-items: end;
      height: 170px;
      gap: 2px;
      grid-template-columns: repeat(21, minmax(0, 1fr));
      padding-top: 6px;
    }

    .bar-col {
      border-radius: 2px 2px 0 0;
      background: linear-gradient(180deg, #4ca8ff 0%, #2f71c5 100%);
      min-height: 2px;
      opacity: 0.92;
      transition: opacity 140ms ease;
    }

    .bar-col:hover { opacity: 1; }

    .chart-meta {
      font-size: 11px;
      color: var(--muted);
      display: flex;
      justify-content: space-between;
      margin-top: 6px;
    }

    .split-grid {
      display: grid;
      grid-template-columns: 120px 1fr;
      gap: 10px;
      align-items: center;
    }

    .donut {
      width: 104px;
      height: 104px;
      border-radius: 50%;
      background: conic-gradient(var(--json) 0deg var(--json-deg), var(--text) var(--json-deg) 360deg);
      position: relative;
      border: 1px solid var(--line);
      margin: 0 auto;
    }

    .donut::after {
      content: "";
      position: absolute;
      inset: 20px;
      border-radius: 50%;
      background: var(--card-strong);
      border: 1px solid var(--line-soft);
    }

    .donut-center {
      position: absolute;
      inset: 0;
      display: grid;
      place-items: center;
      z-index: 1;
      font-size: 12px;
      color: #d8e6f9;
      font-weight: 650;
    }

    .legend {
      display: grid;
      gap: 8px;
      font-size: 12px;
    }

    .legend-item {
      display: grid;
      grid-template-columns: 12px 1fr auto;
      gap: 8px;
      align-items: center;
    }

    .swatch { width: 10px; height: 10px; border-radius: 1px; }

    .swatch.json { background: var(--json); }
    .swatch.text { background: var(--text); }

    .session-bars {
      display: grid;
      gap: 6px;
      margin-top: 4px;
      font-size: 11px;
    }

    .session-row {
      display: grid;
      grid-template-columns: 1fr 62px;
      gap: 8px;
      align-items: center;
    }

    .session-name {
      color: #d7e6fb;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
      margin-bottom: 3px;
    }

    .session-track {
      background: #10213a;
      height: 8px;
      border-radius: 3px;
      border: 1px solid var(--line);
      overflow: hidden;
    }

    .session-fill {
      height: 100%;
      background: linear-gradient(90deg, #468fe0, #61b2ff);
      border-radius: 3px;
    }

    .session-val {
      text-align: right;
      color: var(--muted);
      font-variant-numeric: tabular-nums;
    }

    .recent-events {
      display: grid;
      gap: 7px;
      margin-top: 4px;
    }

    .recent-item {
      border: 1px solid var(--line);
      background: #0f1b2f;
      border-radius: 3px;
      padding: 8px;
    }

    .recent-head {
      display: flex;
      justify-content: space-between;
      align-items: baseline;
      gap: 8px;
      font-size: 11px;
      color: var(--muted);
      margin-bottom: 4px;
    }

    .recent-session {
      color: #d7e6fb;
      max-width: 65%;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }

    .recent-type {
      color: #9ec5f2;
      font-size: 10px;
      letter-spacing: 0.4px;
      text-transform: uppercase;
      margin-bottom: 3px;
    }

    .recent-body {
      font-size: 12px;
      line-height: 1.35;
      color: #e9f2fd;
      word-break: break-word;
    }

    table { width: 100%; border-collapse: collapse; }
    th, td {
      text-align: left;
      font-size: 12px;
      padding: 7px 6px;
      border-bottom: 1px solid var(--line);
    }

    th {
      color: var(--muted);
      font-weight: 600;
      font-size: 11px;
      letter-spacing: 0.35px;
      text-transform: uppercase;
    }

    tr:last-child td { border-bottom: none; }
    tbody tr:hover { background: rgba(67, 110, 168, 0.19); }

    .table-card {
      max-height: 350px;
      overflow: auto;
    }

    .foot {
      margin-top: 4px;
      color: var(--muted);
      font-size: 11px;
    }

    .charts-grid {
      display: grid;
      grid-template-columns: 2fr 1fr;
      gap: 8px;
      margin-bottom: 8px;
    }

    @media (max-width: 1020px) {
      .charts-grid { grid-template-columns: 1fr; }
    }

    .sparkline-card { height: 200px; display: grid; grid-template-rows: auto 1fr; }

    .sparkline-card svg {
      width: 100%;
      height: 100%;
      display: block;
    }

    .dot-matrix { margin-top: 4px; }

    .dot-matrix-row {
      display: flex;
      align-items: center;
      gap: 4px;
      margin-bottom: 3px;
      font-size: 11px;
    }

    .dot-matrix-label {
      width: 100px;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
      color: #d7e6fb;
      flex-shrink: 0;
    }

    .dot-matrix-dots { display: flex; gap: 2px; flex-wrap: wrap; }

    .dot-matrix-dot {
      width: 8px;
      height: 8px;
      border-radius: 1px;
      background: #1a273c;
      border: 1px solid var(--line);
    }

    .dot-matrix-dot.active {
      background: var(--accent);
      border-color: var(--accent);
    }

    .stacked-bar-track {
      height: 32px;
      display: flex;
      border-radius: 3px;
      overflow: hidden;
      border: 1px solid var(--line);
      margin-top: 6px;
    }

    .stacked-bar-seg {
      display: flex;
      align-items: center;
      justify-content: center;
      font-size: 11px;
      font-weight: 600;
      color: #fff;
      min-width: 32px;
    }

    .gauge-bar-track {
      height: 24px;
      display: flex;
      border-radius: 3px;
      overflow: hidden;
      border: 1px solid var(--line);
      margin-top: 6px;
    }

    .gauge-bar-seg {
      display: flex;
      align-items: center;
      justify-content: center;
      font-size: 10px;
      font-weight: 600;
      color: #fff;
      min-width: 24px;
    }

    .gauge-labels {
      display: flex;
      justify-content: space-between;
      font-size: 11px;
      color: var(--muted);
      margin-top: 4px;
    }

    .heatmap { margin-top: 4px; }

    .heatmap-grid {
      display: grid;
      grid-template-rows: repeat(7, 1fr);
      grid-auto-flow: column;
      gap: 2px;
    }

    .heatmap-cell {
      width: 12px;
      height: 12px;
      border-radius: 1px;
      background: var(--accent);
      opacity: 0.08;
    }

    .heatmap-labels {
      display: flex;
      justify-content: space-between;
      font-size: 10px;
      color: var(--muted);
      margin-top: 4px;
    }

    .agent-tag {
      display: inline-block;
      font-size: 10px;
      font-weight: 600;
      letter-spacing: 0.4px;
      text-transform: uppercase;
      padding: 2px 7px;
      border-radius: 3px;
      background: var(--accent-soft);
      color: var(--accent);
      border: 1px solid var(--line);
      white-space: nowrap;
    }

    @keyframes card-enter {
      from {
        opacity: 0;
        transform: translateY(5px);
      }
      to {
        opacity: 1;
        transform: translateY(0);
      }
    }
  </style>
</head>
<body>
  <div class="wrap">
    <div class="header">
      <div>
        <h1>Agent Logs Dashboard</h1>
        <div class="sub" id="sub">Loading...</div>
      </div>
      <div class="sub" id="header-meta">Auto refresh</div>
    </div>

    <div class="kpi-grid">
      <div class="card kpi"><div class="k">Events</div><div class="v" id="events">-</div></div>
      <div class="card kpi"><div class="k">Sessions</div><div class="v" id="sessions">-</div></div>
      <div class="card kpi"><div class="k">Day Shards</div><div class="v" id="days">-</div></div>
      <div class="card kpi"><div class="k">Files</div><div class="v" id="files">-</div></div>
      <div class="card kpi"><div class="k">Storage</div><div class="v" id="bytes">-</div></div>
      <div class="card kpi"><div class="k">Events / Day</div><div class="v-small" id="events-per-day">-</div></div>
      <div class="card kpi"><div class="k">Events / Session</div><div class="v-small" id="events-per-session">-</div></div>
      <div class="card kpi"><div class="k">Top Session Share</div><div class="v-small" id="top-share">-</div></div>
      <div class="card kpi"><div class="k">Agents</div><div class="v-small" id="agents-count">-</div></div>
      <div class="card kpi"><div class="k">7 Day Events</div><div class="v-small" id="events-7d">-</div></div>
    </div>

    <div class="main">
      <div class="panel">
        <div class="card">
          <h2>Most Recent Events</h2>
          <div class="recent-events" id="recent-events"></div>
        </div>

        <div class="card chart-card">
          <h2>Daily Event Trend (latest 21 shards)</h2>
          <div>
            <div class="bars" id="trend-bars"></div>
            <div class="chart-meta">
              <span id="trend-min">Min: -</span>
              <span id="trend-max">Max: -</span>
              <span id="trend-total">Total: -</span>
            </div>
          </div>
        </div>

        <div class="card table-card">
          <h2>Daily Volume</h2>
          <table id="days-table">
            <thead><tr><th>Day</th><th>Events</th><th>Sessions</th><th>Files</th><th>Storage</th></tr></thead>
            <tbody></tbody>
          </table>
        </div>
      </div>

      <div class="panel">
        <div class="card">
          <h2>Type Split</h2>
          <div class="split-grid">
            <div class="donut" id="types-donut" style="--json-deg:180deg;">
              <div class="donut-center" id="donut-center">50%</div>
            </div>
            <div class="legend" id="types-legend"></div>
          </div>
        </div>

        <div class="card">
          <h2>Session Concentration (Top 8)</h2>
          <div class="session-bars" id="session-bars"></div>
        </div>

        <div class="card">
          <h2>Agent Breakdown</h2>
          <div class="session-bars" id="agent-bars"></div>
        </div>
      </div>
    </div>

    <div class="charts-grid">
      <div class="card sparkline-card">
        <h2>Daily Storage Trend</h2>
        <svg id="storage-trend" preserveAspectRatio="none"></svg>
      </div>
      <div class="card">
        <h2>Activity Heatmap</h2>
        <div class="heatmap" id="heatmap">
          <div class="heatmap-grid" id="heatmap-grid"></div>
          <div class="heatmap-labels" id="heatmap-labels"></div>
        </div>
      </div>
      <div class="card">
        <h2>Session Lifespan (Top 8)</h2>
        <div class="dot-matrix" id="dot-matrix"></div>
      </div>
      <div class="card">
        <h2>Event Type Distribution</h2>
        <div class="stacked-bar-track" id="type-dist"></div>
        <h2 style="margin-top:14px">Storage by Type</h2>
        <div class="gauge-bar-track" id="storage-gauge"></div>
        <div class="gauge-labels" id="gauge-labels"></div>
      </div>
    </div>

    <div class="card table-card">
      <h2>Top Sessions</h2>
      <table id="sessions-table">
        <thead><tr><th>Session</th><th>Agent</th><th>Events</th><th>Days</th><th>Files</th><th>Last Seen</th></tr></thead>
        <tbody></tbody>
      </table>
    </div>

    <div class="foot" id="foot"></div>
  </div>

  <script>
    const REFRESH_MS = REFRESH_MS_PLACEHOLDER;

    function fmtInt(value) {
      return new Intl.NumberFormat().format(value || 0);
    }

    function fmtBytes(bytes) {
      if (!bytes) return '0 B';
      const units = ['B', 'KB', 'MB', 'GB', 'TB'];
      let i = 0;
      let n = bytes;
      while (n >= 1024 && i < units.length - 1) {
        n /= 1024;
        i += 1;
      }
      return `${n.toFixed(n >= 10 || i === 0 ? 0 : 1)} ${units[i]}`;
    }

    function fmtPct(value) {
      if (!Number.isFinite(value)) return '0.0%';
      return `${value.toFixed(1)}%`;
    }

    function shortTs(value) {
      if (!value) return 'n/a';
      const d = new Date(value);
      if (Number.isNaN(d.getTime())) return value;
      return d.toISOString().replace('T', ' ').slice(0, 16) + 'Z';
    }

    function setText(id, text) {
      document.getElementById(id).textContent = text;
    }

    function renderDays(days) {
      const tbody = document.querySelector('#days-table tbody');
      tbody.innerHTML = '';
      for (const item of days.slice(0, 21)) {
        const tr = document.createElement('tr');
        tr.innerHTML = `<td>${item.day}</td><td>${fmtInt(item.events)}</td><td>${fmtInt(item.sessions)}</td><td>${fmtInt(item.files)}</td><td>${fmtBytes(item.bytes)}</td>`;
        tbody.appendChild(tr);
      }
    }

    function renderTypes(types) {
      const entries = Object.entries(types || {}).map(([name, rec]) => ({ name, events: rec.events || 0 }));
      const total = entries.reduce((sum, item) => sum + item.events, 0);

      const jsonEvents = (types.json && types.json.events) || 0;
      const textEvents = (types.text && types.text.events) || 0;
      const jsonPct = total > 0 ? (jsonEvents / total) * 100 : 0;
      const donut = document.getElementById('types-donut');
      donut.style.setProperty('--json-deg', `${(jsonPct / 100) * 360}deg`);
      setText('donut-center', fmtPct(jsonPct));

      const root = document.getElementById('types-legend');
      root.innerHTML = '';
      for (const item of entries) {
        const pct = total > 0 ? ((item.events / total) * 100) : 0;
        const row = document.createElement('div');
        row.className = 'legend-item';
        row.innerHTML = `
          <div class="swatch ${item.name}"></div>
          <div>${item.name.toUpperCase()} (${fmtPct(pct)})</div>
          <div>${fmtInt(item.events)}</div>
        `;
        root.appendChild(row);
      }
    }

    function renderTrend(days) {
      const items = (days || []).slice(0, 21).reverse();
      const maxEvents = Math.max(...items.map((x) => x.events || 0), 1);
      const minEvents = items.length ? Math.min(...items.map((x) => x.events || 0)) : 0;
      const totalEvents = items.reduce((sum, x) => sum + (x.events || 0), 0);

      const root = document.getElementById('trend-bars');
      root.innerHTML = '';
      for (const item of items) {
        const col = document.createElement('div');
        col.className = 'bar-col';
        const pct = Math.max(2, Math.round(((item.events || 0) / maxEvents) * 100));
        col.style.height = `${pct}%`;
        col.title = `${item.day}: ${fmtInt(item.events || 0)} events`;
        root.appendChild(col);
      }

      setText('trend-min', `Min: ${fmtInt(minEvents)}`);
      setText('trend-max', `Max: ${fmtInt(maxEvents)}`);
      setText('trend-total', `Total: ${fmtInt(totalEvents)}`);
    }

    function renderSessionBars(sessions, totalEvents) {
      const items = (sessions || []).slice(0, 8);
      const maxEvents = Math.max(...items.map((x) => x.events || 0), 1);
      const root = document.getElementById('session-bars');
      root.innerHTML = '';
      for (const item of items) {
        const pct = Math.max(2, Math.round(((item.events || 0) / maxEvents) * 100));
        const share = totalEvents > 0 ? ((item.events || 0) / totalEvents) * 100 : 0;
        const row = document.createElement('div');
        row.className = 'session-row';
        row.innerHTML = `
          <div>
            <div class="session-name">${item.session}</div>
            <div class="session-track"><div class="session-fill" style="width:${pct}%"></div></div>
          </div>
          <div class="session-val">${fmtPct(share)}</div>
        `;
        root.appendChild(row);
      }
    }

    function renderSessions(sessions) {
      const tbody = document.querySelector('#sessions-table tbody');
      tbody.innerHTML = '';
      for (const item of sessions || []) {
        const tr = document.createElement('tr');
        tr.innerHTML = `<td>${item.session}</td><td><span class="agent-tag">${item.agent || 'unknown'}</span></td><td>${fmtInt(item.events)}</td><td>${fmtInt(item.days)}</td><td>${fmtInt(item.files)}</td><td>${shortTs(item.last_seen)}</td>`;
        tbody.appendChild(tr);
      }
    }

    function renderRecentEvents(events) {
      const root = document.getElementById('recent-events');
      root.innerHTML = '';
      const items = (events || []).slice(0, 5);

      if (!items.length) {
        const empty = document.createElement('div');
        empty.className = 'recent-item';
        empty.textContent = 'No events yet.';
        root.appendChild(empty);
        return;
      }

      for (const item of items) {
        const row = document.createElement('div');
        row.className = 'recent-item';

        const head = document.createElement('div');
        head.className = 'recent-head';

        const session = document.createElement('div');
        session.className = 'recent-session';
        session.textContent = item.session || 'unknown';

        const ts = document.createElement('div');
        ts.textContent = shortTs(item.timestamp);

        head.appendChild(session);
        head.appendChild(ts);

        const eventType = document.createElement('div');
        eventType.className = 'recent-type';
        eventType.textContent = item.event_type || 'event';

        const agentTag = document.createElement('span');
        agentTag.className = 'agent-tag';
        agentTag.textContent = item.agent || 'unknown';
        agentTag.style.marginLeft = '6px';

        const body = document.createElement('div');
        body.className = 'recent-body';
        body.textContent = item.preview || '(event)';

        row.appendChild(head);
        row.appendChild(eventType);
        eventType.appendChild(agentTag);
        row.appendChild(body);
        root.appendChild(row);
      }
    }

    function renderAgentBars(agents, totalEvents) {
      const entries = Object.entries(agents || {})
        .map(([name, rec]) => ({ name, events: rec.events || 0 }))
        .sort((a, b) => b.events - a.events);
      const maxEvents = Math.max(...entries.map(x => x.events), 1);
      const root = document.getElementById('agent-bars');
      root.innerHTML = '';
      for (const item of entries) {
        const pct = Math.max(2, Math.round((item.events / maxEvents) * 100));
        const share = totalEvents > 0 ? (item.events / totalEvents) * 100 : 0;
        const row = document.createElement('div');
        row.className = 'session-row';
        row.innerHTML = `
          <div>
            <div class="session-name">${item.name}</div>
            <div class="session-track"><div class="session-fill" style="width:${pct}%"></div></div>
          </div>
          <div class="session-val">${fmtPct(share)}</div>
        `;
        root.appendChild(row);
      }
    }

    function renderDerived(data) {
      const totals = data.totals || {};
      const days = Math.max(1, totals.days || 0);
      const sessions = Math.max(1, totals.sessions || 0);
      const events = totals.events || 0;
      const top = (data.top_sessions && data.top_sessions[0] && data.top_sessions[0].events) || 0;
      const agentCount = Object.keys(data.agents || {}).length;
      const events7d = (data.days || []).slice(0, 7).reduce((sum, x) => sum + (x.events || 0), 0);

      setText('events-per-day', fmtInt(Math.round(events / days)));
      setText('events-per-session', fmtInt(Math.round(events / sessions)));
      setText('top-share', fmtPct(events > 0 ? (top / events) * 100 : 0));
      setText('agents-count', fmtInt(agentCount));
      setText('events-7d', fmtInt(events7d));
    }

    function renderStorageTrend(days) {
      const items = (days || []).slice(0, 21).reverse();
      const svg = document.getElementById('storage-trend');
      if (!items.length) { svg.innerHTML = ''; return; }
      const maxB = Math.max(...items.map(x => x.bytes || 0), 1);
      const w = 100;
      const h = 100;
      const pts = items.map((x, i) => {
        const px = items.length > 1 ? (i / (items.length - 1)) * w : w / 2;
        const py = h - ((x.bytes || 0) / maxB) * (h * 0.85);
        return `${px.toFixed(2)},${py.toFixed(2)}`;
      });
      const lineStr = pts.join(' ');
      const polyStr = `0,${h} ${lineStr} ${w},${h}`;
      svg.setAttribute('viewBox', `0 0 ${w} ${h}`);
      svg.innerHTML = `
        <defs>
          <linearGradient id="sg" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stop-color="var(--accent)" stop-opacity="0.45"/>
            <stop offset="100%" stop-color="var(--accent)" stop-opacity="0.04"/>
          </linearGradient>
        </defs>
        <polygon points="${polyStr}" fill="url(#sg)"/>
        <polyline points="${lineStr}" fill="none" stroke="var(--accent)" stroke-width="1.2" stroke-linejoin="round"/>
      `;
    }

    function renderSessionLifespan(sessions) {
      const items = (sessions || []).slice(0, 8);
      const root = document.getElementById('dot-matrix');
      root.innerHTML = '';
      if (!items.length) return;
      const maxDays = Math.max(...items.map(x => x.days || 0), 1);
      const cols = Math.min(maxDays, 30);
      for (const s of items) {
        const row = document.createElement('div');
        row.className = 'dot-matrix-row';
        const label = document.createElement('div');
        label.className = 'dot-matrix-label';
        label.textContent = s.session;
        label.title = s.session;
        row.appendChild(label);
        const dots = document.createElement('div');
        dots.className = 'dot-matrix-dots';
        const activeDots = Math.min(s.days || 0, cols);
        for (let i = 0; i < cols; i++) {
          const dot = document.createElement('div');
          dot.className = 'dot-matrix-dot' + (i < activeDots ? ' active' : '');
          dots.appendChild(dot);
        }
        row.appendChild(dots);
        root.appendChild(row);
      }
    }

    function renderTypeDistribution(types) {
      const jsonE = (types && types.json && types.json.events) || 0;
      const textE = (types && types.text && types.text.events) || 0;
      const total = jsonE + textE;
      const root = document.getElementById('type-dist');
      if (total === 0) {
        root.innerHTML = '<div class="stacked-bar-seg" style="flex:1;background:var(--line)">No data</div>';
        return;
      }
      const jsonPct = (jsonE / total) * 100;
      const textPct = (textE / total) * 100;
      root.innerHTML = `
        <div class="stacked-bar-seg" style="flex:${jsonE};background:var(--json)">${fmtPct(jsonPct)} JSON</div>
        <div class="stacked-bar-seg" style="flex:${textE};background:var(--text)">${fmtPct(textPct)} Text</div>
      `;
    }

    function renderStorageGauge(types) {
      const jsonB = (types && types.json && types.json.bytes) || 0;
      const textB = (types && types.text && types.text.bytes) || 0;
      const total = jsonB + textB;
      const track = document.getElementById('storage-gauge');
      const labels = document.getElementById('gauge-labels');
      if (total === 0) {
        track.innerHTML = '<div class="gauge-bar-seg" style="flex:1;background:var(--line)">No data</div>';
        labels.innerHTML = '';
        return;
      }
      track.innerHTML = `
        <div class="gauge-bar-seg" style="flex:${jsonB};background:var(--json)">${fmtBytes(jsonB)}</div>
        <div class="gauge-bar-seg" style="flex:${textB};background:var(--text)">${fmtBytes(textB)}</div>
      `;
      labels.innerHTML = `<span>JSON: ${fmtBytes(jsonB)}</span><span>Text: ${fmtBytes(textB)}</span>`;
    }

    function renderHeatmap(days) {
      const items = (days || []).slice(0, 21).reverse();
      const grid = document.getElementById('heatmap-grid');
      const labels = document.getElementById('heatmap-labels');
      grid.innerHTML = '';
      labels.innerHTML = '';
      if (!items.length) return;

      const maxEv = Math.max(...items.map(x => x.events || 0), 1);
      const totalCells = Math.ceil(items.length / 7) * 7;
      const cols = Math.ceil(totalCells / 7);
      grid.style.gridTemplateColumns = `repeat(${cols}, 12px)`;

      for (let i = 0; i < totalCells; i++) {
        const cell = document.createElement('div');
        cell.className = 'heatmap-cell';
        if (i < items.length) {
          const ev = items[i].events || 0;
          const opacity = 0.08 + (ev / maxEv) * 0.88;
          cell.style.opacity = opacity.toFixed(2);
          cell.title = `${items[i].day}: ${fmtInt(ev)} events`;
        } else {
          cell.style.opacity = '0.03';
        }
        grid.appendChild(cell);
      }

      if (items.length > 0) {
        labels.innerHTML = `<span>${items[0].day}</span><span>${items[items.length - 1].day}</span>`;
      }
    }

    async function refresh() {
      try {
        const response = await fetch('/api/summary', { cache: 'no-store' });
        if (!response.ok) throw new Error(`HTTP ${response.status}`);
        const data = await response.json();

        setText('sub', `Root: ${data.root}`);
        setText('header-meta', `Refresh ${Math.floor(REFRESH_MS / 1000)}s`);
        setText('events', fmtInt(data.totals.events));
        setText('sessions', fmtInt(data.totals.sessions));
        setText('days', fmtInt(data.totals.days));
        setText('files', fmtInt(data.totals.files));
        setText('bytes', fmtBytes(data.totals.bytes));

        renderDays(data.days || []);
        renderTypes(data.types || {});
        renderTrend(data.days || []);
        renderSessionBars(data.top_sessions || [], data.totals.events || 0);
        renderSessions(data.top_sessions || []);
        renderRecentEvents(data.recent_events || []);
        renderDerived(data);
        renderAgentBars(data.agents || {}, data.totals.events || 0);
        renderStorageTrend(data.days || []);
        renderSessionLifespan(data.top_sessions || []);
        renderTypeDistribution(data.types || {});
        renderStorageGauge(data.types || {});
        renderHeatmap(data.days || []);

        const newest = data.totals.newest_file_mtime || 'n/a';
        setText('foot', `Updated ${shortTs(data.generated_at)}. Newest file mtime: ${shortTs(newest)}.`);
      } catch (err) {
        setText('sub', `Failed to load data: ${String(err)}`);
      }
    }

    refresh();
    setInterval(refresh, REFRESH_MS);
  </script>
</body>
</html>
"#;

    template.replace("REFRESH_MS_PLACEHOLDER", &refresh_ms.to_string())
}

