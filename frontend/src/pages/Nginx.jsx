import { useState, useEffect, useCallback } from 'react';
import { api } from '../api/client';
import { useToast } from '../components/Toast';
import Modal from '../components/Modal';
import Spinner from '../components/Spinner';

/* ── helpers ─────────────────────────────────────────────── */

function formatTime(ts) {
  if (!ts) return '—';
  try {
    return new Date(ts).toLocaleString();
  } catch {
    return ts;
  }
}

function statusDot(ok) {
  return (
    <span style={{
      display: 'inline-block',
      width: '8px',
      height: '8px',
      borderRadius: '50%',
      marginRight: '6px',
      background: ok ? 'var(--green)' : 'var(--red)',
      boxShadow: ok ? '0 0 6px var(--green)' : '0 0 6px var(--red)',
    }} />
  );
}

/* ── main page ───────────────────────────────────────────── */

export default function Nginx() {
  const toast = useToast();

  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [status, setStatus] = useState(null);
  const [actionLoading, setActionLoading] = useState(false);

  /* modals */
  const [configModal, setConfigModal] = useState(null);
  const [testModal, setTestModal] = useState(null);
  const [reloadModal, setReloadModal] = useState(null);

  /* ── data loading ── */

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await api.get('/nginx/upstreams');
      setStatus(data);
    } catch (err) {
      setError(err.message);
      setStatus(null);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  /* ── actions ── */

  const doAction = useCallback(async (actionFn, okMsg, onSuccess) => {
    setActionLoading(true);
    try {
      const result = await actionFn();
      if (okMsg) toast(okMsg, 'success');
      if (onSuccess) onSuccess(result);
      await load();
    } catch (err) {
      toast(`Error: ${err.message}`, 'error');
    } finally {
      setActionLoading(false);
    }
  }, [toast, load]);

  const handleRegenerate = () => {
    doAction(
      () => api.post('/nginx/regenerate'),
      'Configuration regenerated'
    );
  };

  const handleTest = async () => {
    setActionLoading(true);
    try {
      const result = await api.post('/nginx/test');
      setTestModal(result);
    } catch (err) {
      toast(`Error: ${err.message}`, 'error');
    } finally {
      setActionLoading(false);
    }
  };

  const handleReload = async () => {
    setActionLoading(true);
    try {
      const result = await api.post('/nginx/reload');
      setReloadModal(result);
      await load();
    } catch (err) {
      toast(`Error: ${err.message}`, 'error');
    } finally {
      setActionLoading(false);
    }
  };

  const viewConfig = async (upstream) => {
    try {
      const text = await api.get('/nginx/config');
      setConfigModal({ title: `Config — ${upstream.domain || 'all'}`, content: text });
    } catch (err) {
      toast(`Error: ${err.message}`, 'error');
    }
  };

  /* ── render ── */

  if (loading) {
    return <div className="loading-center"><Spinner size="lg" /></div>;
  }

  const upstreams = status?.upstreams || [];
  const configValid = status?.config_valid;
  const lastGenerated = status?.last_generated;
  const nginxRunning = status?.nginx_running;

  return (
    <div>
      {/* Header */}
      <div className="section-header">
        <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
          <h1 style={{ marginBottom: 0 }}>⚖️ Load Balancer</h1>
          {status && (
            <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
              <span style={{
                display: 'inline-flex',
                alignItems: 'center',
                padding: '2px 10px',
                borderRadius: '12px',
                fontSize: '0.75rem',
                fontWeight: 600,
                background: configValid ? 'var(--green-dim)' : 'var(--red-dim)',
                color: configValid ? 'var(--green)' : 'var(--red)',
              }}>
                {statusDot(configValid)}
                {configValid ? 'Config Valid' : 'Config Invalid'}
              </span>
              {nginxRunning !== undefined && (
                <span style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  padding: '2px 10px',
                  borderRadius: '12px',
                  fontSize: '0.75rem',
                  fontWeight: 600,
                  background: nginxRunning ? 'var(--green-dim)' : 'var(--red-dim)',
                  color: nginxRunning ? 'var(--green)' : 'var(--red)',
                }}>
                  {statusDot(nginxRunning)}
                  Nginx {nginxRunning ? 'Running' : 'Stopped'}
                </span>
              )}
              {lastGenerated && (
                <span className="text-secondary" style={{ fontSize: '0.75rem' }}>
                  Last generated: {formatTime(lastGenerated)}
                </span>
              )}
            </div>
          )}
        </div>
        <div className="btn-group">
          <button onClick={load} className="btn-sm" disabled={actionLoading}>
            🔄 Refresh
          </button>
          <button
            className="btn-primary btn-sm"
            onClick={handleRegenerate}
            disabled={actionLoading}
          >
            {actionLoading ? '⏳' : '🔧'} Regenerate Config
          </button>
          <button
            className="btn-sm"
            onClick={handleTest}
            disabled={actionLoading}
          >
            {actionLoading ? '⏳' : '✓'} Test Config
          </button>
          <button
            className="btn-success btn-sm"
            onClick={handleReload}
            disabled={actionLoading}
          >
            {actionLoading ? '⏳' : '↻'} Reload Nginx
          </button>
        </div>
      </div>

      {error && <div className="text-danger mb-16">Error: {error}</div>}

      {/* Empty state */}
      {upstreams.length === 0 ? (
        <div className="card" style={{ textAlign: 'center', padding: '48px' }}>
          <div style={{ fontSize: '2.5rem', marginBottom: '12px' }}>⚖️</div>
          <h2 style={{ marginBottom: '8px' }}>No Load Balancer Upstreams Found</h2>
          <p className="text-secondary" style={{ marginBottom: '24px', maxWidth: '600px', marginLeft: 'auto', marginRight: 'auto', lineHeight: 1.6 }}>
            No containers with <code>marionette.lb.*</code> labels were discovered.
            Add labels to your containers to automatically configure Nginx reverse proxy upstreams.
          </p>
          <div style={{
            display: 'inline-block',
            textAlign: 'left',
            background: 'var(--bg-tertiary)',
            border: '1px solid var(--border)',
            borderRadius: '8px',
            padding: '16px 20px',
            maxWidth: '600px',
          }}>
            <div className="text-secondary" style={{ fontSize: '0.75rem', marginBottom: '8px', textTransform: 'uppercase', letterSpacing: '0.05em' }}>
              Example docker-compose.yml labels
            </div>
            <pre className="log-output" style={{ fontSize: '0.8rem', margin: 0 }}>{`services:
  myapp:
    image: myapp:latest
    labels:
      marionette.lb.enable: "true"
      marionette.lb.domain: "app.example.com"
      marionette.lb.port: "3000"
      marionette.lb.ssl: "true"
      marionette.lb.path: "/api"   # optional path prefix
      marionette.lb.weight: "1"    # optional, default 1`}</pre>
          </div>
        </div>
      ) : (
        <div>
          {/* Managed Domains */}
          {upstreams.map((upstream, idx) => (
            <div key={upstream.domain || idx} className="card mb-16">
              {/* Domain header */}
              <div style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                marginBottom: '12px',
              }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                  <h2 style={{ marginBottom: 0, fontSize: '1.1rem' }}>
                    {upstream.domain || 'unknown'}
                  </h2>
                  {upstream.ssl ? (
                    <span style={{
                      display: 'inline-block',
                      padding: '1px 8px',
                      borderRadius: '10px',
                      fontSize: '0.7rem',
                      fontWeight: 600,
                      background: 'var(--green-dim)',
                      color: 'var(--green)',
                    }}>🔒 SSL</span>
                  ) : (
                    <span style={{
                      display: 'inline-block',
                      padding: '1px 8px',
                      borderRadius: '10px',
                      fontSize: '0.7rem',
                      fontWeight: 600,
                      background: 'var(--bg-tertiary)',
                      color: 'var(--text-secondary)',
                    }}>🔓 HTTP</span>
                  )}
                  {upstream.path && (
                    <span style={{
                      display: 'inline-block',
                      padding: '1px 8px',
                      borderRadius: '10px',
                      fontSize: '0.7rem',
                      fontWeight: 600,
                      background: 'var(--bg-tertiary)',
                      color: 'var(--accent)',
                      fontFamily: 'monospace',
                    }}>
                      {upstream.path}
                    </span>
                  )}
                </div>
                <div className="btn-group">
                  {upstream.config_path && (
                    <span className="text-secondary" style={{ fontSize: '0.7rem', alignSelf: 'center', marginRight: '4px' }}>
                      {upstream.config_path}
                    </span>
                  )}
                  <button className="btn-sm" onClick={() => viewConfig(upstream)}>
                    📄 View Config
                  </button>
                </div>
              </div>

              {/* Servers table */}
              {(upstream.servers && upstream.servers.length > 0) ? (
                <div className="table-wrapper">
                  <table>
                    <thead>
                      <tr>
                        <th>Container</th>
                        <th>Host:Port</th>
                        <th>Endpoint</th>
                        <th>Weight</th>
                        <th>Health</th>
                      </tr>
                    </thead>
                    <tbody>
                      {upstream.servers.map((srv, si) => (
                        <tr key={srv.container_id || si}>
                          <td>
                            <span style={{ fontWeight: 500, color: 'var(--text-primary)' }}>
                              {srv.container_name || srv.container_id?.substring(0, 12) || '—'}
                            </span>
                          </td>
                          <td className="mono" style={{ fontSize: '0.8rem' }}>
                            {srv.host || '—'}:{srv.port || '—'}
                          </td>
                          <td>
                            <span className="text-secondary" style={{ fontSize: '0.75rem' }}>
                              {srv.endpoint_id || 'local'}
                            </span>
                          </td>
                          <td className="mono" style={{ fontSize: '0.8rem' }}>
                            {srv.weight ?? 1}
                          </td>
                          <td>
                            <span style={{ display: 'inline-flex', alignItems: 'center', gap: '4px' }}>
                              {statusDot(srv.healthy)}
                              <span style={{
                                fontSize: '0.75rem',
                                color: srv.healthy ? 'var(--green)' : 'var(--red)',
                                fontWeight: 500,
                              }}>
                                {srv.healthy ? 'Healthy' : 'Unhealthy'}
                              </span>
                            </span>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              ) : (
                <div className="text-secondary" style={{ padding: '12px 0', fontSize: '0.85rem' }}>
                  No servers configured for this upstream.
                </div>
              )}
            </div>
          ))}
        </div>
      )}

      {/* ── Test Config Modal ── */}
      {testModal && (
        <Modal title="nginx -t Output" onClose={() => setTestModal(null)}>
          <div style={{ marginBottom: '12px' }}>
            <span style={{
              display: 'inline-flex',
              alignItems: 'center',
              padding: '4px 12px',
              borderRadius: '12px',
              fontSize: '0.8rem',
              fontWeight: 600,
              background: testModal.valid ? 'var(--green-dim)' : 'var(--red-dim)',
              color: testModal.valid ? 'var(--green)' : 'var(--red)',
            }}>
              {statusDot(testModal.valid)}
              {testModal.valid ? 'Configuration OK' : 'Configuration Error'}
            </span>
          </div>
          <pre style={{
            background: 'var(--bg-tertiary)',
            border: '1px solid var(--border)',
            borderRadius: '6px',
            padding: '12px',
            fontFamily: "'JetBrains Mono', monospace",
            fontSize: '0.8rem',
            color: testModal.valid ? 'var(--green)' : 'var(--red)',
            whiteSpace: 'pre-wrap',
            wordBreak: 'break-all',
            maxHeight: '400px',
            overflow: 'auto',
            margin: 0,
          }}>
            {testModal.output || '(no output)'}
          </pre>
        </Modal>
      )}

      {/* ── Reload Result Modal ── */}
      {reloadModal && (
        <Modal title="nginx -s reload" onClose={() => setReloadModal(null)}>
          <div style={{ marginBottom: '12px' }}>
            <span style={{
              display: 'inline-flex',
              alignItems: 'center',
              padding: '4px 12px',
              borderRadius: '12px',
              fontSize: '0.8rem',
              fontWeight: 600,
              background: reloadModal.success ? 'var(--green-dim)' : 'var(--red-dim)',
              color: reloadModal.success ? 'var(--green)' : 'var(--red)',
            }}>
              {statusDot(reloadModal.success)}
              {reloadModal.success ? 'Reload Successful' : 'Reload Failed'}
            </span>
          </div>
          {reloadModal.output && (
            <pre style={{
              background: 'var(--bg-tertiary)',
              border: '1px solid var(--border)',
              borderRadius: '6px',
              padding: '12px',
              fontFamily: "'JetBrains Mono', monospace",
              fontSize: '0.8rem',
              color: reloadModal.success ? 'var(--green)' : 'var(--red)',
              whiteSpace: 'pre-wrap',
              wordBreak: 'break-all',
              maxHeight: '300px',
              overflow: 'auto',
              margin: 0,
            }}>
              {reloadModal.output}
            </pre>
          )}
        </Modal>
      )}

      {/* ── View Config Modal ── */}
      {configModal && (
        <Modal title={configModal.title} onClose={() => setConfigModal(null)}>
          <pre style={{
            background: 'var(--bg-tertiary)',
            border: '1px solid var(--border)',
            borderRadius: '6px',
            padding: '12px',
            fontFamily: "'JetBrains Mono', monospace",
            fontSize: '0.75rem',
            color: 'var(--text-primary)',
            whiteSpace: 'pre-wrap',
            wordBreak: 'break-all',
            maxHeight: '60vh',
            overflow: 'auto',
            margin: 0,
          }}>
            {configModal.content || '(empty)'}
          </pre>
        </Modal>
      )}
    </div>
  );
}
