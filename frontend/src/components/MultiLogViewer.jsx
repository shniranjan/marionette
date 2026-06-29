import { useState, useEffect, useRef, useCallback } from 'react';
import { wsUrl } from '../api/client';

const MAX_LINES = 10000;

// Color palette for container names — distinct, readable on dark backgrounds
const CONTAINER_COLORS = [
  '#4fc3f7', // light blue
  '#a5d6a7', // light green
  '#ffcc80', // orange
  '#ef9a9a', // red
  '#ce93d8', // purple
  '#80cbc4', // teal
  '#fff176', // yellow
  '#b0bec5', // blue-grey
];

// Resolve a deterministic color for a container name
function colorForName(name, idx) {
  if (idx < CONTAINER_COLORS.length) {
    return CONTAINER_COLORS[idx];
  }
  // Hash fallback
  let hash = 0;
  for (let i = 0; i < name.length; i++) {
    hash = name.charCodeAt(i) + ((hash << 5) - hash);
  }
  const hue = Math.abs(hash) % 360;
  return `hsl(${hue}, 60%, 72%)`;
}

// Parse an ISO timestamp prefix from a Docker log line
const TIMESTAMP_RE = /^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z)\s/;

export default function MultiLogViewer({ containerIds, onClose }) {
  const [lines, setLines] = useState([]);
  const [autoScroll, setAutoScroll] = useState(true);
  const [filter, setFilter] = useState('');
  const [connected, setConnected] = useState(false);
  const [showTimestamps, setShowTimestamps] = useState(true);
  const containerRef = useRef(null);
  const wsRef = useRef(null);

  useEffect(() => {
    if (!containerIds || containerIds.length === 0) return;

    const idsParam = containerIds.join(',');
    const url = wsUrl(`/api/containers/logs/merged?ids=${encodeURIComponent(idsParam)}`);
    const ws = new WebSocket(url);
    wsRef.current = ws;

    ws.onopen = () => setConnected(true);
    ws.onclose = () => setConnected(false);
    ws.onerror = () => setConnected(false);

    ws.onmessage = (e) => {
      try {
        const data = JSON.parse(e.data);
        // data.stream, data.container, data.containerId, data.eof, data.error
        const entry = {
          text: data.stream || data.error || e.data,
          container: data.container || '',
          containerId: data.containerId || '',
          eof: !!data.eof,
          error: !!data.error,
        };
        setLines((prev) => {
          const next = [...prev, entry];
          return next.length > MAX_LINES ? next.slice(next.length - MAX_LINES) : next;
        });
      } catch {
        setLines((prev) => {
          const next = [...prev, { text: e.data, container: '', containerId: '', eof: false, error: false }];
          return next.length > MAX_LINES ? next.slice(next.length - MAX_LINES) : next;
        });
      }
    };

    return () => {
      ws.close();
    };
  }, [containerIds]);

  useEffect(() => {
    if (autoScroll && containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
  }, [lines, autoScroll]);

  const clearLogs = useCallback(() => {
    setLines([]);
  }, []);

  const filtered = filter
    ? lines.filter((l) => l.text.toLowerCase().includes(filter.toLowerCase()))
    : lines;

  // Build color map from observed container names
  const colorMap = {};
  let colorIdx = 0;

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
        flexWrap: 'wrap',
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

        {/* Container legend */}
        {containerIds.map((cid, i) => {
          const color = colorForName(cid, i);
          return (
            <span
              key={cid}
              style={{
                fontSize: '0.7rem',
                padding: '1px 6px',
                borderRadius: '4px',
                background: color + '22',
                border: `1px solid ${color}55`,
                color: color,
              }}
            >
              {cid.length > 12 ? cid.substring(0, 12) : cid}
            </span>
          );
        })}

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
        <button className="btn-sm" onClick={onClose}>Close</button>
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
        {filtered.map((entry, i) => {
          // Assign color from map (stable across renders)
          const cid = entry.containerId || '';
          if (cid && !(cid in colorMap)) {
            colorMap[cid] = colorForName(cid, colorIdx++);
          }
          const badgeColor = cid ? colorMap[cid] : 'var(--text-muted)';
          const match = showTimestamps ? TIMESTAMP_RE.exec(entry.text) : null;

          return (
            <div
              key={i}
              style={{
                whiteSpace: 'pre-wrap',
                wordBreak: 'break-all',
                padding: '1px 0',
                opacity: entry.eof ? 0.5 : 1,
              }}
            >
              {/* Container name badge */}
              {entry.container && (
                <span style={{
                  display: 'inline-block',
                  fontSize: '0.65rem',
                  fontWeight: 600,
                  padding: '0px 5px',
                  marginRight: '6px',
                  borderRadius: '3px',
                  background: badgeColor + '33',
                  color: badgeColor,
                  border: `1px solid ${badgeColor}66`,
                  verticalAlign: 'middle',
                }}>
                  {entry.container}
                </span>
              )}
              {match ? (
                <>
                  <span style={{ color: 'var(--text-muted)', opacity: 0.6 }}>
                    {match[1] + ' '}
                  </span>
                  <span>{entry.text.slice(match[0].length)}</span>
                </>
              ) : (
                <span>{entry.text}</span>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
