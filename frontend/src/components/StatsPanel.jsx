import { useState, useEffect, useRef } from 'react';
import { wsUrl } from '../api/client';

export default function StatsPanel({ containerId }) {
  const [stats, setStats] = useState(null);
  const [connected, setConnected] = useState(false);
  const wsRef = useRef(null);

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
        const data = JSON.parse(e.data);
        setStats(data);
      } catch {
        // ignore parse errors
      }
    };

    return () => {
      ws.close();
    };
  }, [containerId]);

  if (!connected && !stats) {
    return <div className="text-secondary loading-center">Connecting to stats stream...</div>;
  }

  const cpuPercent = stats?.cpu_stats
    ? calculateCPU(stats.cpu_stats, stats.precpu_stats)
    : 0;
  const memUsage = stats?.memory_stats?.usage || 0;
  const memLimit = stats?.memory_stats?.limit || 1;
  const memPercent = memLimit > 0 ? (memUsage / memLimit) * 100 : 0;
  const netIO = stats?.networks
    ? Object.values(stats.networks).reduce(
        (acc, n) => ({ rx: (acc.rx || 0) + (n.rx_bytes || 0), tx: (acc.tx || 0) + (n.tx_bytes || 0) }),
        { rx: 0, tx: 0 }
      )
    : { rx: 0, tx: 0 };
  const blkIO = stats?.blkio_stats?.io_service_bytes_recursive || [];

  return (
    <div>
      <div style={{
        display: 'flex',
        alignItems: 'center',
        gap: '8px',
        marginBottom: '16px',
        padding: '4px 0',
      }}>
        <div style={{
          width: '8px', height: '8px', borderRadius: '50%',
          background: connected ? 'var(--green)' : 'var(--red)',
        }} />
        <span style={{ fontSize: '0.75rem', color: 'var(--text-secondary)' }}>
          {connected ? 'Live' : 'Disconnected'}
        </span>
      </div>

      {/* CPU */}
      <div className="card mb-16">
        <h3>CPU</h3>
        <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
          <div style={{
            flex: 1,
            height: '24px',
            background: 'var(--bg-tertiary)',
            borderRadius: '12px',
            overflow: 'hidden',
          }}>
            <div style={{
              width: `${Math.min(cpuPercent, 100)}%`,
              height: '100%',
              background: cpuPercent > 80 ? 'var(--red)' : cpuPercent > 50 ? 'var(--yellow)' : 'var(--green)',
              borderRadius: '12px',
              transition: 'width 0.5s ease',
            }} />
          </div>
          <span className="mono" style={{ fontWeight: 600, minWidth: '60px', textAlign: 'right' }}>
            {cpuPercent.toFixed(1)}%
          </span>
        </div>
      </div>

      {/* Memory */}
      <div className="card mb-16">
        <h3>Memory</h3>
        <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
          <div style={{
            flex: 1,
            height: '24px',
            background: 'var(--bg-tertiary)',
            borderRadius: '12px',
            overflow: 'hidden',
          }}>
            <div style={{
              width: `${Math.min(memPercent, 100)}%`,
              height: '100%',
              background: memPercent > 80 ? 'var(--red)' : memPercent > 50 ? 'var(--yellow)' : 'var(--green)',
              borderRadius: '12px',
              transition: 'width 0.5s ease',
            }} />
          </div>
          <span className="mono" style={{ fontWeight: 600, minWidth: '60px', textAlign: 'right' }}>
            {memPercent.toFixed(1)}%
          </span>
        </div>
        <div className="text-secondary mono" style={{ fontSize: '0.75rem', marginTop: '8px' }}>
          {formatBytes(memUsage)} / {formatBytes(memLimit)}
        </div>
      </div>

      {/* Network I/O */}
      <div className="card mb-16">
        <h3>Network I/O</h3>
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '12px' }}>
          <div>
            <div className="text-secondary" style={{ fontSize: '0.75rem', marginBottom: '4px' }}>RX</div>
            <div className="mono" style={{ color: 'var(--accent)' }}>{formatBytes(netIO.rx)}</div>
          </div>
          <div>
            <div className="text-secondary" style={{ fontSize: '0.75rem', marginBottom: '4px' }}>TX</div>
            <div className="mono" style={{ color: 'var(--green)' }}>{formatBytes(netIO.tx)}</div>
          </div>
        </div>
      </div>

      {/* Block I/O */}
      {blkIO.length > 0 && (
        <div className="card">
          <h3>Block I/O</h3>
          {blkIO.map((io, i) => (
            <div key={i} className="mono" style={{ fontSize: '0.75rem', marginBottom: '4px' }}>
              {io.op}: {formatBytes(io.value)}
            </div>
          ))}
        </div>
      )}
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
  if (!bytes || bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}
