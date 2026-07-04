import { useState, useEffect, useRef, useCallback } from 'react';
import { wsUrl } from '../api/client';

export default function RelayConsole({ relayHost, onClose }) {
  const [lines, setLines] = useState([]);
  const [connected, setConnected] = useState(false);
  const [input, setInput] = useState('');
  const wsRef = useRef(null);
  const outputRef = useRef(null);

  const connect = useCallback(() => {
    const url = wsUrl(`/api/relay/stream/${relayHost}`);
    const ws = new WebSocket(url);
    wsRef.current = ws;

    ws.onopen = () => {
      setConnected(true);
      setLines(prev => [...prev, { type: 'system', text: `Connected to relay: ${relayHost}`, time: new Date().toLocaleTimeString() }]);
    };

    ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data);
        if (msg.error) {
          setLines(prev => [...prev, { type: 'error', text: msg.error, time: new Date().toLocaleTimeString() }]);
        } else if (msg.type === 'event') {
          const subtype = msg.subtype || 'event';
          const payload = msg.payload || {};
          // Format based on subtype
          if (subtype === 'docker.logs' || subtype === 'compose.output') {
            const stream = payload.stream || 'stdout';
            const line = payload.line || JSON.stringify(payload);
            setLines(prev => [...prev, { type: stream === 'stderr' ? 'stderr' : 'stdout', text: line, time: new Date().toLocaleTimeString() }]);
          } else if (subtype === 'docker.exec.stdout') {
            setLines(prev => [...prev, { type: 'stdout', text: payload.data || '', time: new Date().toLocaleTimeString() }]);
          } else if (subtype === 'docker.exec.stderr') {
            setLines(prev => [...prev, { type: 'stderr', text: payload.data || '', time: new Date().toLocaleTimeString() }]);
          } else if (subtype === 'docker.stats') {
            const cpu = payload.cpu_percent?.toFixed(1) || '0';
            const mem = ((payload.memory_usage_bytes || 0) / 1024 / 1024).toFixed(1);
            setLines(prev => [...prev, { type: 'stats', text: `${payload.container || ''}: CPU ${cpu}% MEM ${mem}MB`, time: new Date().toLocaleTimeString() }]);
          } else if (subtype === 'image.pull') {
            setLines(prev => [...prev, { type: 'stdout', text: `${payload.status || ''} ${payload.id || ''}`, time: new Date().toLocaleTimeString() }]);
          } else {
            setLines(prev => [...prev, { type: 'stdout', text: JSON.stringify(payload), time: new Date().toLocaleTimeString() }]);
          }
        } else if (msg.type === 'response') {
          setLines(prev => [...prev, { type: 'system', text: `OK: ${msg.subtype} ${JSON.stringify(msg.payload).substring(0, 200)}`, time: new Date().toLocaleTimeString() }]);
        }
      } catch (e) {
        setLines(prev => [...prev, { type: 'stderr', text: event.data, time: new Date().toLocaleTimeString() }]);
      }
    };

    ws.onclose = () => {
      setConnected(false);
      setLines(prev => [...prev, { type: 'system', text: 'Disconnected', time: new Date().toLocaleTimeString() }]);
    };

    ws.onerror = () => {
      setConnected(false);
    };
  }, [relayHost]);

  useEffect(() => {
    connect();
    return () => {
      if (wsRef.current) wsRef.current.close();
    };
  }, [connect]);

  // Auto-scroll to bottom
  useEffect(() => {
    if (outputRef.current) {
      outputRef.current.scrollTop = outputRef.current.scrollHeight;
    }
  }, [lines]);

  const sendCommand = (cmd) => {
    if (!wsRef.current || wsRef.current.readyState !== WebSocket.OPEN) return;
    try {
      const parsed = JSON.parse(cmd);
      // Add an id if not present
      if (!parsed.id) parsed.id = crypto.randomUUID();
      wsRef.current.send(JSON.stringify(parsed));
      setLines(prev => [...prev, { type: 'command', text: `> ${cmd.substring(0, 100)}`, time: new Date().toLocaleTimeString() }]);
    } catch (e) {
      setLines(prev => [...prev, { type: 'error', text: `Invalid JSON: ${e.message}`, time: new Date().toLocaleTimeString() }]);
    }
  };

  const handleSubmit = (e) => {
    e.preventDefault();
    if (!input.trim()) return;
    sendCommand(input.trim());
    setInput('');
  };

  // Quick command buttons
  const quickCommands = [
    { label: 'PS', cmd: '{"subtype":"docker.ps","payload":{"all":true}}' },
    { label: 'Host', cmd: '{"subtype":"host.info","payload":{}}' },
    { label: 'Logs', cmd: '{"subtype":"docker.logs","payload":{"container":"marionette","tail":20}}' },
    { label: 'Stats', cmd: '{"subtype":"docker.stats","payload":{"one_shot":true}}' },
    { label: 'Compose', cmd: '{"subtype":"compose.config","payload":{"project_dir":"/stacks/marionette"}}' },
  ];

  const colorMap = {
    system: '#89b4fa',
    command: '#f9e2af',
    stdout: '#cdd6f4',
    stderr: '#f38ba8',
    error: '#f38ba8',
    stats: '#a6e3a1',
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      {/* Header */}
      <div style={{
        display: 'flex', alignItems: 'center', justifyContent: 'space-between',
        padding: '8px 12px', background: 'var(--bg-tertiary)',
        borderBottom: '1px solid var(--border)', fontSize: '0.85rem',
      }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
          <span style={{ fontWeight: 600 }}>📡 Relay: {relayHost}</span>
          <span style={{
            display: 'inline-flex', alignItems: 'center', gap: '4px', fontSize: '0.75rem',
          }}>
            <span style={{
              width: '8px', height: '8px', borderRadius: '50%',
              background: connected ? '#a6e3a1' : '#f38ba8', display: 'inline-block',
            }} />
            {connected ? 'Connected' : 'Disconnected'}
          </span>
        </div>
        <button className="btn-sm" onClick={onClose}>✕</button>
      </div>

      {/* Output area */}
      <div ref={outputRef} style={{
        flex: 1, overflow: 'auto', background: '#1e1e2e',
        padding: '8px', fontFamily: 'Menlo, Monaco, monospace', fontSize: '0.8rem',
        lineHeight: '1.5',
      }}>
        {lines.map((line, i) => (
          <div key={i} style={{ color: colorMap[line.type] || '#cdd6f4', whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}>
            <span style={{ color: '#585b70', marginRight: '8px', fontSize: '0.7rem' }}>{line.time}</span>
            {line.text}
          </div>
        ))}
        {lines.length === 0 && (
          <div style={{ color: '#585b70', fontStyle: 'italic' }}>
            Connected. Use quick commands or type raw relay JSON below.
            Example: {'{'}"subtype":"host.info","payload":{}{'}'}
          </div>
        )}
      </div>

      {/* Quick commands */}
      <div style={{
        display: 'flex', gap: '4px', padding: '6px 8px',
        background: 'var(--bg-tertiary)', borderTop: '1px solid var(--border)',
        flexWrap: 'wrap',
      }}>
        {quickCommands.map((qc) => (
          <button key={qc.label} className="btn-sm" onClick={() => sendCommand(qc.cmd)}
            style={{ fontSize: '0.7rem' }}>
            {qc.label}
          </button>
        ))}
      </div>

      {/* Input */}
      <form onSubmit={handleSubmit} style={{
        display: 'flex', padding: '6px 8px',
        background: 'var(--bg-tertiary)', borderTop: '1px solid var(--border)',
      }}>
        <span style={{
          color: '#a6e3a1', fontFamily: 'monospace', fontSize: '0.8rem',
          lineHeight: '32px', marginRight: '6px',
        }}>$</span>
        <input
          type="text"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder='{"subtype":"host.info","payload":{}}'
          style={{
            flex: 1, background: '#1e1e2e', color: '#cdd6f4', border: '1px solid var(--border)',
            borderRadius: '4px', padding: '4px 8px', fontFamily: 'monospace', fontSize: '0.8rem',
          }}
        />
      </form>
    </div>
  );
}
