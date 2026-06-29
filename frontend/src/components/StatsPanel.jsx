import { useState, useEffect, useRef } from 'react';
import { wsUrl } from '../api/client';
import {
  AreaChart, Area, LineChart, Line, XAxis, YAxis, CartesianGrid,
  Tooltip, ResponsiveContainer, Legend,
} from 'recharts';

const MAX_POINTS = 60;

const LS_KEY = 'marionette_stats_thresholds';

function loadThresholds() {
  try {
    const raw = localStorage.getItem(LS_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      return {
        cpu: typeof parsed.cpu === 'number' ? parsed.cpu : 80,
        mem: typeof parsed.mem === 'number' ? parsed.mem : 90,
      };
    }
  } catch { /* ignore */ }
  return { cpu: 80, mem: 90 };
}

function saveThresholds(t) {
  try {
    localStorage.setItem(LS_KEY, JSON.stringify(t));
  } catch { /* ignore */ }
}

export default function StatsPanel({ containerId }) {
  const [history, setHistory] = useState([]);
  const [connected, setConnected] = useState(false);
  const [thresholds, setThresholds] = useState(loadThresholds);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const wsRef = useRef(null);
  const prevNetRef = useRef(null); // for rate calculation

  useEffect(() => {
    if (!containerId) return;

    const url = wsUrl(`/api/containers/${containerId}/stats`);
    const ws = new WebSocket(url);
    wsRef.current = ws;

    ws.onopen = () => setConnected(true);
    ws.onclose = () => setConnected(false);
    ws.onerror = () => setConnected(false);

    ws.onmessage = (e) => {
      try {
        const stats = JSON.parse(e.data);
        const now = Date.now();

        // CPU
        const cpuPercent = calculateCPU(stats.cpu_stats, stats.precpu_stats);

        // Memory
        const memUsage = stats.memory_stats?.usage || 0;
        const memLimit = stats.memory_stats?.limit || 1;
        const memPercent = memLimit > 0 ? (memUsage / memLimit) * 100 : 0;

        // Network rate (bytes/sec)
        let netRxRate = 0;
        let netTxRate = 0;
        const networks = stats.networks;
        if (networks) {
          const netTotal = Object.values(networks).reduce(
            (acc, n) => ({ rx: (acc.rx || 0) + (n.rx_bytes || 0), tx: (acc.tx || 0) + (n.tx_bytes || 0) }),
            { rx: 0, tx: 0 }
          );
          if (prevNetRef.current) {
            const elapsed = (now - prevNetRef.current.time) / 1000;
            if (elapsed > 0) {
              netRxRate = (netTotal.rx - prevNetRef.current.rx) / elapsed;
              netTxRate = (netTotal.tx - prevNetRef.current.tx) / elapsed;
            }
          }
          prevNetRef.current = { ...netTotal, time: now };
        }

        // Block I/O cumulative
        const blkRead = stats.blkio_stats?.io_service_bytes_recursive
          ?.filter(io => io.op === 'read')
          ?.reduce((s, io) => s + (io.value || 0), 0) || 0;
        const blkWrite = stats.blkio_stats?.io_service_bytes_recursive
          ?.filter(io => io.op === 'write')
          ?.reduce((s, io) => s + (io.value || 0), 0) || 0;

        const point = {
          time: now,
          cpu: Number(cpuPercent.toFixed(1)),
          memPct: Number(memPercent.toFixed(1)),
          memAbs: memUsage,
          netRx: Number(netRxRate.toFixed(0)),
          netTx: Number(netTxRate.toFixed(0)),
          blkRead,
          blkWrite,
        };

        setHistory(prev => {
          const next = [...prev, point];
          return next.length > MAX_POINTS ? next.slice(next.length - MAX_POINTS) : next;
        });
      } catch {
        // ignore parse errors
      }
    };

    return () => {
      ws.close();
    };
  }, [containerId]);

  const latest = history.length > 0 ? history[history.length - 1] : null;
  const cpuAlert = latest && latest.cpu >= thresholds.cpu;
  const memAlert = latest && latest.memPct >= thresholds.mem;

  const handleThresholdChange = (key, value) => {
    const next = { ...thresholds, [key]: Number(value) };
    setThresholds(next);
    saveThresholds(next);
  };

  if (!connected && history.length === 0) {
    return <div style={{ color: 'var(--pico-muted-color)', textAlign: 'center', padding: '48px' }}>
      Connecting to stats stream...
    </div>;
  }

  const formatTime = (ts) => {
    const d = new Date(ts);
    return `${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')}:${d.getSeconds().toString().padStart(2, '0')}`;
  };

  const chartTheme = {
    grid: 'var(--card-border)',
    text: 'var(--pico-muted-color, #8b949e)',
    cpu: cpuAlert ? 'var(--red, #f85149)' : 'var(--accent, #58a6ff)',
    mem: memAlert ? 'var(--red, #f85149)' : 'var(--green, #3fb950)',
    rx: 'var(--accent, #58a6ff)',
    tx: 'var(--green, #3fb950)',
  };

  return (
    <div>
      {/* Connection indicator + thresholds gear */}
      <div style={{
        display: 'flex', alignItems: 'center', gap: '8px',
        marginBottom: '16px', padding: '4px 0',
      }}>
        <div style={{
          width: '8px', height: '8px', borderRadius: '50%',
          background: connected ? 'var(--green)' : 'var(--red)',
        }} />
        <span style={{ fontSize: '0.75rem', color: 'var(--pico-muted-color)' }}>
          {connected ? 'Live' : 'Disconnected'}
        </span>
        {latest && (
          <span style={{ fontSize: '0.75rem', color: 'var(--pico-muted-color)', marginLeft: 'auto' }}>
            CPU: {latest.cpu}% &nbsp; Mem: {latest.memPct}%
          </span>
        )}
        <button
          className="btn-sm"
          onClick={() => setSettingsOpen(o => !o)}
          title="Alert thresholds"
          style={{
            background: 'transparent', border: '1px solid var(--card-border)',
            borderRadius: '6px', cursor: 'pointer', fontSize: '0.9rem',
            color: 'var(--pico-muted-color)', padding: '2px 6px',
            display: 'flex', alignItems: 'center', justifyContent: 'center',
          }}
          aria-label="Threshold settings"
        >
          &#9881;
        </button>
        {(cpuAlert || memAlert) && (
          <span style={{
            fontSize: '0.65rem', fontWeight: 600, borderRadius: '4px',
            padding: '2px 6px', background: 'var(--red)', color: '#fff',
          }}>
            ALERT
          </span>
        )}
      </div>

      {/* Threshold settings panel */}
      {settingsOpen && (
        <div style={{
          background: 'var(--card-bg)',
          border: '1px solid var(--card-border)',
          borderRadius: '8px', padding: '12px 16px',
          marginBottom: '16px',
        }}>
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '12px' }}>
            <span style={{ fontWeight: 600, fontSize: '0.85rem' }}>Alert Thresholds</span>
            <button
              onClick={() => setSettingsOpen(false)}
              style={{
                background: 'none', border: 'none', cursor: 'pointer',
                color: 'var(--pico-muted-color)', fontSize: '1rem', lineHeight: 1,
              }}
              aria-label="Close settings"
            >
              &times;
            </button>
          </div>
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))', gap: '12px' }}>
            {/* CPU threshold */}
            <div>
              <label style={{ fontSize: '0.75rem', color: 'var(--pico-muted-color)', display: 'block', marginBottom: '4px' }}>
                CPU Threshold: <strong>{thresholds.cpu}%</strong>
              </label>
              <input
                type="range"
                min="0" max="100"
                value={thresholds.cpu}
                onChange={e => handleThresholdChange('cpu', e.target.value)}
                style={{ width: '100%' }}
              />
            </div>
            {/* Memory threshold */}
            <div>
              <label style={{ fontSize: '0.75rem', color: 'var(--pico-muted-color)', display: 'block', marginBottom: '4px' }}>
                Memory Threshold: <strong>{thresholds.mem}%</strong>
              </label>
              <input
                type="range"
                min="0" max="100"
                value={thresholds.mem}
                onChange={e => handleThresholdChange('mem', e.target.value)}
                style={{ width: '100%' }}
              />
            </div>
          </div>
          <div style={{ fontSize: '0.7rem', color: 'var(--pico-muted-color)', marginTop: '8px' }}>
            When usage crosses the threshold, the chart line turns red and an alert badge appears.
          </div>
        </div>
      )}

      {/* Charts grid */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(360px, 1fr))', gap: '16px', marginBottom: '16px' }}>
        {/* CPU Chart */}
        <div className="card" style={{ position: 'relative' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '8px' }}>
            <h3 style={{ margin: 0, fontSize: '0.9rem', color: 'var(--pico-muted-color)' }}>CPU %</h3>
            {cpuAlert && (
              <span style={{
                fontSize: '0.6rem', fontWeight: 700, borderRadius: '3px',
                padding: '1px 5px', background: 'var(--red)', color: '#fff',
                textTransform: 'uppercase', letterSpacing: '0.5px',
              }}>
                &ge;{thresholds.cpu}%
              </span>
            )}
          </div>
          <ResponsiveContainer width="100%" height={200}>
            <AreaChart data={history} margin={{ top: 4, right: 4, left: -8, bottom: 0 }}>
              <defs>
                <linearGradient id="cpuGrad" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stopColor={chartTheme.cpu} stopOpacity={0.3} />
                  <stop offset="100%" stopColor={chartTheme.cpu} stopOpacity={0} />
                </linearGradient>
              </defs>
              <CartesianGrid stroke={chartTheme.grid} strokeDasharray="3 3" />
              <XAxis dataKey="time" tickFormatter={formatTime} fontSize={10} stroke={chartTheme.text} interval="preserveStartEnd" />
              <YAxis fontSize={10} stroke={chartTheme.text} domain={[0, 'auto']} tickFormatter={v => `${v}%`} />
              <Tooltip labelFormatter={formatTime} formatter={(v) => [`${v}%`, 'CPU']} />
              <Area type="monotone" dataKey="cpu" stroke={chartTheme.cpu} fill="url(#cpuGrad)" strokeWidth={2} isAnimationActive={false} dot={false} />
            </AreaChart>
          </ResponsiveContainer>
        </div>

        {/* Memory Chart */}
        <div className="card" style={{ position: 'relative' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '8px' }}>
            <h3 style={{ margin: 0, fontSize: '0.9rem', color: 'var(--pico-muted-color)' }}>Memory</h3>
            {memAlert && (
              <span style={{
                fontSize: '0.6rem', fontWeight: 700, borderRadius: '3px',
                padding: '1px 5px', background: 'var(--red)', color: '#fff',
                textTransform: 'uppercase', letterSpacing: '0.5px',
              }}>
                &ge;{thresholds.mem}%
              </span>
            )}
          </div>
          <ResponsiveContainer width="100%" height={200}>
            <AreaChart data={history} margin={{ top: 4, right: 4, left: -8, bottom: 0 }}>
              <defs>
                <linearGradient id="memGrad" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stopColor={chartTheme.mem} stopOpacity={0.3} />
                  <stop offset="100%" stopColor={chartTheme.mem} stopOpacity={0} />
                </linearGradient>
              </defs>
              <CartesianGrid stroke={chartTheme.grid} strokeDasharray="3 3" />
              <XAxis dataKey="time" tickFormatter={formatTime} fontSize={10} stroke={chartTheme.text} interval="preserveStartEnd" />
              <YAxis fontSize={10} stroke={chartTheme.text} domain={[0, 100]} tickFormatter={v => `${v}%`} />
              <Tooltip labelFormatter={formatTime} formatter={(v) => [`${v}%`, 'Memory']} />
              <Area type="monotone" dataKey="memPct" stroke={chartTheme.mem} fill="url(#memGrad)" strokeWidth={2} isAnimationActive={false} dot={false} />
            </AreaChart>
          </ResponsiveContainer>
          {latest && (
            <div style={{ fontSize: '0.75rem', color: 'var(--pico-muted-color)', marginTop: '4px' }}>
              {formatBytes(latest.memAbs)} / {formatBytes(latest.memAbs / (latest.memPct / 100) || 0)}
            </div>
          )}
        </div>
      </div>

      {/* Network Chart — full width */}
      <div className="card">
        <h3 style={{ margin: '0 0 8px 0', fontSize: '0.9rem', color: 'var(--pico-muted-color)' }}>Network I/O</h3>
        <ResponsiveContainer width="100%" height={200}>
          <LineChart data={history} margin={{ top: 4, right: 4, left: -8, bottom: 0 }}>
            <CartesianGrid stroke={chartTheme.grid} strokeDasharray="3 3" />
            <XAxis dataKey="time" tickFormatter={formatTime} fontSize={10} stroke={chartTheme.text} interval="preserveStartEnd" />
            <YAxis fontSize={10} stroke={chartTheme.text} tickFormatter={formatBytes} />
            <Tooltip labelFormatter={formatTime} formatter={(v, name) => [formatBytes(v), name === 'netRx' ? 'RX' : 'TX']} />
            <Legend formatter={(v) => v === 'netRx' ? 'RX' : 'TX'} />
            <Line type="monotone" dataKey="netRx" stroke={chartTheme.rx} strokeWidth={2} isAnimationActive={false} dot={false} />
            <Line type="monotone" dataKey="netTx" stroke={chartTheme.tx} strokeWidth={2} isAnimationActive={false} dot={false} />
          </LineChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
}

function calculateCPU(cpu, precpu) {
  if (!cpu || !precpu) return 0;
  const cpuDelta = (cpu.cpu_usage?.total_usage || 0) - (precpu.cpu_usage?.total_usage || 0);
  const systemDelta = (cpu.system_cpu_usage || 0) - (precpu.system_cpu_usage || 0);
  const numCpus = cpu.online_cpus || 1;
  if (systemDelta <= 0 || cpuDelta <= 0) return 0;
  return (cpuDelta / systemDelta) * numCpus * 100;
}

function formatBytes(bytes) {
  if (!bytes || bytes === 0) return '0 B/s';
  if (bytes < 0) bytes = 0;
  const k = 1024;
  const sizes = ['B/s', 'KB/s', 'MB/s', 'GB/s'];
  const i = Math.floor(Math.log(Math.max(bytes, 1)) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, Math.min(i, sizes.length - 1))).toFixed(1)) + ' ' + sizes[Math.min(i, sizes.length - 1)];
}
