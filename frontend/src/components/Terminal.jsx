import { useEffect, useRef, useState, useCallback } from 'react';
import { Terminal as XTerm } from 'xterm';
import { FitAddon } from 'xterm-addon-fit';
import 'xterm/css/xterm.css';
import { wsUrl, getKey } from '../api/client';

const RECONNECT_DELAY_MS = 3000;
const MAX_RECONNECT_ATTEMPTS = 5;

export default function Terminal({ containerId, containerName, onClose }) {
  const termRef = useRef(null);
  const wsRef = useRef(null);
  const reconnectCountRef = useRef(0);
  const reconnectTimerRef = useRef(null);
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState(null);

  const connect = useCallback(() => {
    // Clean up any existing connection
    if (wsRef.current) {
      wsRef.current.close();
      wsRef.current = null;
    }

    const url = wsUrl(`/api/containers/${containerId}/exec?cmd=bash`);
    let ws;
    try {
      ws = new WebSocket(url);
    } catch (err) {
      setError(`Failed to create WebSocket: ${err.message}`);
      scheduleReconnect();
      return;
    }

    wsRef.current = ws;

    ws.onopen = () => {
      setConnected(true);
      setError(null);
      reconnectCountRef.current = 0;
    };

    ws.onmessage = (event) => {
      if (termRef.current) {
        termRef.current.write(event.data);
      }
    };

    ws.onerror = () => {
      setConnected(false);
      setError('WebSocket error');
      scheduleReconnect();
    };

    ws.onclose = () => {
      setConnected(false);
      wsRef.current = null;
      if (termRef.current) {
        termRef.current.write('\r\n\x1b[33m[Connection closed]\x1b[0m\r\n');
      }
    };
  }, [containerId]);

  const scheduleReconnect = useCallback(() => {
    if (reconnectCountRef.current >= MAX_RECONNECT_ATTEMPTS) {
      setError('Max reconnection attempts reached');
      return;
    }
    reconnectCountRef.current++;
    reconnectTimerRef.current = setTimeout(() => {
      connect();
    }, RECONNECT_DELAY_MS);
  }, [connect]);

  // Initialize terminal
  useEffect(() => {
    const container = document.getElementById(`terminal-container-${containerId}`);
    if (!container) return;

    const term = new XTerm({
      cursorBlink: true,
      fontSize: 14,
      fontFamily: 'Menlo, Monaco, "Courier New", monospace',
      theme: {
        background: '#1e1e2e',
        foreground: '#cdd6f4',
        cursor: '#f5e0dc',
        selectionBackground: '#585b70',
        black: '#45475a',
        red: '#f38ba8',
        green: '#a6e3a1',
        yellow: '#f9e2af',
        blue: '#89b4fa',
        magenta: '#f5c2e7',
        cyan: '#94e2d5',
        white: '#bac2de',
        brightBlack: '#585b70',
        brightRed: '#f38ba8',
        brightGreen: '#a6e3a1',
        brightYellow: '#f9e2af',
        brightBlue: '#89b4fa',
        brightMagenta: '#f5c2e7',
        brightCyan: '#94e2d5',
        brightWhite: '#a6adc8',
      },
      allowTransparency: false,
    });

    const fitAddon = new FitAddon();
    fitAddon.activate(term);
    term.open(container);
    fitAddon.fit();

    // Resize observer to keep terminal fitted
    const resizeObserver = new ResizeObserver(() => {
      fitAddon.fit();
    });
    resizeObserver.observe(container);

    // Handle terminal resize → send to backend if supported
    const resizeDisposable = term.onResize(({ cols, rows }) => {
      // v1: skip resize control message
    });

    // Handle user input
    const dataDisposable = term.onData((data) => {
      if (wsRef.current && wsRef.current.readyState === WebSocket.OPEN) {
        wsRef.current.send(data);
      }
    });

    termRef.current = term;

    // Connect WebSocket
    connect();

    // Cleanup
    return () => {
      if (reconnectTimerRef.current) {
        clearTimeout(reconnectTimerRef.current);
      }
      resizeObserver.disconnect();
      resizeDisposable.dispose();
      dataDisposable.dispose();
      fitAddon.dispose();
      term.dispose();
      termRef.current = null;
      if (wsRef.current) {
        wsRef.current.close();
        wsRef.current = null;
      }
    };
  }, [containerId, connect]);

  const handleReconnect = () => {
    reconnectCountRef.current = 0;
    setError(null);
    connect();
  };

  const handleEnter = () => {
    if (wsRef.current && wsRef.current.readyState === WebSocket.OPEN) {
      wsRef.current.send('\r');
    }
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      {/* Header bar */}
      <div style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        padding: '8px 12px',
        background: 'var(--bg-tertiary)',
        borderBottom: '1px solid var(--border)',
        fontSize: '0.85rem',
      }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
          <span style={{ fontWeight: 600 }}>
            🖥 Shell: {containerName || containerId?.substring(0, 12)}
          </span>
          <span style={{
            display: 'inline-flex',
            alignItems: 'center',
            gap: '4px',
            fontSize: '0.75rem',
          }}>
            <span style={{
              width: '8px',
              height: '8px',
              borderRadius: '50%',
              background: connected ? '#a6e3a1' : '#f38ba8',
              display: 'inline-block',
            }} />
            {connected ? 'Connected' : 'Disconnected'}
          </span>
        </div>
        <div style={{ display: 'flex', gap: '8px' }}>
          {!connected && reconnectCountRef.current < MAX_RECONNECT_ATTEMPTS && (
            <button
              className="btn-sm"
              onClick={handleReconnect}
              title="Reconnect"
            >
              🔄 Reconnect
            </button>
          )}
          <button
            className="btn-sm"
            onClick={handleEnter}
            title="Send Enter"
          >
            ↵ Enter
          </button>
          <button
            className="btn-sm"
            onClick={onClose}
            title="Close Shell"
          >
            ✕
          </button>
        </div>
      </div>

      {/* Error banner */}
      {error && (
        <div style={{
          padding: '6px 12px',
          background: '#f38ba8',
          color: '#1e1e2e',
          fontSize: '0.8rem',
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
        }}>
          <span>{error}</span>
          <button
            className="btn-sm"
            onClick={handleReconnect}
            style={{ background: '#1e1e2e', color: '#cdd6f4', border: 'none' }}
          >
            Retry
          </button>
        </div>
      )}

      {/* Terminal container */}
      <div
        id={`terminal-container-${containerId}`}
        style={{
          flex: 1,
          overflow: 'hidden',
          background: '#1e1e2e',
        }}
      />

      {/* Footer hint */}
      <div style={{
        padding: '4px 12px',
        background: 'var(--bg-tertiary)',
        borderTop: '1px solid var(--border)',
        fontSize: '0.7rem',
        color: 'var(--text-secondary)',
        textAlign: 'center',
      }}>
        Type <code>exit</code> or press <code>Ctrl+D</code> to close the shell
      </div>
    </div>
  );
}
