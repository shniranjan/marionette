import { useState, useEffect, useRef, useCallback } from 'react';
import { wsUrl } from '../api/client';

const MAX_LINES = 10000;

// Parse an ISO timestamp prefix from a Docker log line (e.g. "2024-01-15T10:30:45.123456789Z ...")
const TIMESTAMP_RE = /^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z)\s/;

export default function LogViewer({ containerId }) {
  const [lines, setLines] = useState([]);
  const [autoScroll, setAutoScroll] = useState(true);
  const [filter, setFilter] = useState('');
  const [connected, setConnected] = useState(false);
  const [showTimestamps, setShowTimestamps] = useState(true);
  const containerRef = useRef(null);
  const wsRef = useRef(null);

  useEffect(() => {
    if (!containerId) return;

    const url = wsUrl(`/api/containers/${containerId}/logs`);
    const ws = new WebSocket(url);
    wsRef.current = ws;

    ws.onopen = () => setConnected(true);
    ws.onclose = () => setConnected(false);
    ws.onerror = () => setConnected(false);

    ws.onmessage = (e) => {
      try {
        const data = JSON.parse(e.data);
        const text = data.stream || data.error || e.data;
        setLines((prev) => {
          const next = [...prev, text];
          return next.length > MAX_LINES ? next.slice(next.length - MAX_LINES) : next;
        });
      } catch {
        setLines((prev) => {
          const next = [...prev, e.data];
          return next.length > MAX_LINES ? next.slice(next.length - MAX_LINES) : next;
        });
      }
    };

    return () => {
      ws.close();
    };
  }, [containerId]);

  useEffect(() => {
    if (autoScroll && containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
  }, [lines, autoScroll]);

  const clearLogs = useCallback(() => {
    setLines([]);
  }, []);

  const filtered = filter
    ? lines.filter((l) => l.toLowerCase().includes(filter.toLowerCase()))
    : lines;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      {/* Toolbar */}
      <div style={{
        display: 'flex',
        gap: '8px',
        alignItems: 'center',
        padding: '8px 0',
        borderBottom: '1px solid var(--border)',
        marginBottom: '8px',
      }}>
        <div style={{
          width: '8px',
          height: '8px',
          borderRadius: '50%',
          background: connected ? 'var(--green)' : 'var(--red)',
          flexShrink: 0,
        }} />
        <span style={{ fontSize: '0.75rem', color: 'var(--text-secondary)' }}>
          {connected ? 'Connected' : 'Disconnected'}
        </span>
        <span style={{ fontSize: '0.75rem', color: 'var(--text-secondary)' }}>
          ({lines.length} lines)
        </span>
        <div style={{ flex: 1 }} />
        <input
          type="text"
          placeholder="Filter..."
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          style={{ width: '180px', padding: '4px 8px', fontSize: '0.75rem' }}
        />
        <label style={{ display: 'flex', alignItems: 'center', gap: '4px', fontSize: '0.75rem', cursor: 'pointer', color: 'var(--text-secondary)' }}>
          <input
            type="checkbox"
            checked={showTimestamps}
            onChange={(e) => setShowTimestamps(e.target.checked)}
          />
          Timestamps
        </label>
        <label style={{ display: 'flex', alignItems: 'center', gap: '4px', fontSize: '0.75rem', cursor: 'pointer', color: 'var(--text-secondary)' }}>
          <input
            type="checkbox"
            checked={autoScroll}
            onChange={(e) => setAutoScroll(e.target.checked)}
          />
          Auto-scroll
        </label>
        <button className="btn-sm" onClick={clearLogs}>Clear</button>
        <button
          className="btn-sm"
          onClick={() => {
            const ep = new URLSearchParams(window.location.search).get('endpoint') || 'local';
            window.open(`/api/containers/${containerId}/logs/download?tail=all&timestamps=true&endpoint=${encodeURIComponent(ep)}`);
          }}
        >
          Download
        </button>
      </div>

      {/* Log output */}
      <div
        ref={containerRef}
        className="log-output"
        style={{
          flex: 1,
          overflow: 'auto',
          background: 'var(--bg-tertiary)',
          borderRadius: '6px',
          padding: '12px',
          minHeight: '300px',
        }}
      >
        {filtered.length === 0 && (
          <div className="text-secondary" style={{ textAlign: 'center', padding: '24px' }}>
            {connected ? 'Waiting for logs...' : 'Connecting...'}
          </div>
        )}
        {filtered.map((line, i) => {
          const match = showTimestamps ? TIMESTAMP_RE.exec(line) : null;
          return (
            <div key={i} style={{ whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}>
              {match ? (
                <>
                  <span style={{ color: 'var(--text-muted)', opacity: 0.6 }}>{match[1]} </span>
                  {line.slice(match[0].length)}
                </>
              ) : (
                line
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
