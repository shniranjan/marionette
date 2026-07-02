import { useState, useEffect, useCallback } from 'react';
import { api } from '../api/client';
import Modal from '../components/Modal';
import SetupScriptGenerator from '../components/SetupScriptGenerator';
import Spinner from '../components/Spinner';
import { useToast } from '../components/Toast';

function maskConnection(conn) {
  if (!conn) return '';
  // Mask user:pass@host:port to host:port
  const cleaned = conn.replace(/^[a-z]+:\/\/[^@]+@/, (m) => {
    const proto = m.split('://')[0];
    return `${proto}://••••@`;
  });
  return cleaned;
}

const STATUS_STYLES = {
  connected: { dot: 'var(--green)', label: 'Connected' },
  disconnected: { dot: 'var(--red)', label: 'Disconnected' },
  error: { dot: 'var(--yellow)', label: 'Error' },
};

export default function Endpoints() {
  const toast = useToast();
  const [endpoints, setEndpoints] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [showAdd, setShowAdd] = useState(false);
  const [showEdit, setShowEdit] = useState(null);
  const [showDelete, setShowDelete] = useState(null);
  const [showGenerator, setShowGenerator] = useState(false);
  const [testing, setTesting] = useState({});
  const [testResults, setTestResults] = useState({});
  const [saving, setSaving] = useState(false);

  // Add form
  const [newName, setNewName] = useState('');
  const [newConnection, setNewConnection] = useState('');
  const [newTags, setNewTags] = useState('');
  const [newCertPath, setNewCertPath] = useState('');
  const [newStacksDir, setNewStacksDir] = useState('');

  // Edit form
  const [editName, setEditName] = useState('');
  const [editConnection, setEditConnection] = useState('');
  const [editTags, setEditTags] = useState('');
  const [editCertPath, setEditCertPath] = useState('');
  const [editStacksDir, setEditStacksDir] = useState('');

  const load = useCallback(async () => {
    try {
      const data = await api.get('/api/endpoints');
      setEndpoints(Array.isArray(data) ? data : (data?.endpoints || []));
      setError(null);
    } catch (err) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  // Reload when endpoint changes (EndpointSwitcher dispatches 'endpoint:changed')
  useEffect(() => {
    const handler = () => load();
    window.addEventListener('endpoint:changed', handler);
    return () => window.removeEventListener('endpoint:changed', handler);
  }, [load]);

  const handleAdd = async () => {
    if (!newName.trim() || !newConnection.trim()) return;
    setSaving(true);
    try {
      await api.post('/api/endpoints', {
        name: newName.trim(),
        connection: newConnection.trim(),
        tags: newTags.split(',').map(t => t.trim()).filter(Boolean),
        certPath: newCertPath.trim() || undefined,
        stacksDir: newStacksDir.trim() || undefined,
      });
      setShowAdd(false);
      setNewName('');
      setNewConnection('');
      setNewTags('');
      setNewCertPath('');
      setNewStacksDir('');
      toast('Endpoint added', 'success');
      load();
    } catch (err) {
      toast('Error: ' + err.message, 'error');
    } finally {
      setSaving(false);
    }
  };

  const handleEdit = async () => {
    if (!editName.trim()) return;
    setSaving(true);
    try {
      const id = showEdit.id || showEdit.Id;
      await api.patch(`/api/endpoints/${id}`, {
        name: editName.trim(),
        connection: editConnection.trim() || undefined,
        tags: editTags.split(',').map(t => t.trim()).filter(Boolean),
        certPath: editCertPath.trim() || null,
        stacksDir: editStacksDir.trim() || null,
      });
      setShowEdit(null);
      toast('Endpoint updated', 'success');
      load();
    } catch (err) {
      toast('Error: ' + err.message, 'error');
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async () => {
    if (!showDelete) return;
    const id = showDelete.id || showDelete.Id;
    const name = showDelete.name || showDelete.Name;
    if ((name || '').toLowerCase() === 'local') {
      toast('Cannot delete the local endpoint', 'error');
      setShowDelete(null);
      return;
    }
    try {
      await api.delete(`/api/endpoints/${id}`);
      setShowDelete(null);
      toast('Endpoint deleted', 'success');
      load();
    } catch (err) {
      toast('Error: ' + err.message, 'error');
    }
  };

  const handleTest = async (ep) => {
    const id = ep.id || ep.Id;
    setTesting(prev => ({ ...prev, [id]: true }));
    try {
      const result = await api.post(`/api/endpoints/${id}/test`);
      setTestResults(prev => ({ ...prev, [id]: result }));
      const ok = result?.status === 'connected' || result?.success;
      toast(ok ? 'Connection successful' : `Test failed: ${result?.error || 'unknown'}`, ok ? 'success' : 'error');
    } catch (err) {
      setTestResults(prev => ({ ...prev, [id]: { status: 'error', error: err.message } }));
      toast('Test failed: ' + err.message, 'error');
    } finally {
      setTesting(prev => ({ ...prev, [id]: false }));
    }
  };

  const handleReconnect = async (ep) => {
    const id = ep.id || ep.Id;
    try {
      await api.post(`/api/endpoints/${id}/reconnect`);
      toast('Reconnected', 'success');
      load();
    } catch (err) {
      toast('Error: ' + err.message, 'error');
    }
  };

  const openEdit = (ep) => {
    setShowEdit(ep);
    setEditName(ep.name || ep.Name || '');
    setEditConnection(ep.connection || ep.Connection || '');
    setEditTags((ep.tags || ep.Tags || []).join(', '));
    setEditCertPath(ep.certPath || ep.CertPath || '');
    setEditStacksDir(ep.stacksDir || ep.StacksDir || '');
  };

  if (loading) return <div className="loading-center"><Spinner size="lg" /></div>;

  return (
    <div>
      <div className="section-header">
        <h1>Endpoints ({endpoints.length})</h1>
        <div className="btn-group">
          <button className="btn-primary" onClick={() => setShowAdd(true)}>+ Add Endpoint</button>
          <button onClick={() => setShowGenerator(true)}>🔧 Setup Script</button>
          <button onClick={load}>🔄 Refresh</button>
        </div>
      </div>

      {error && <div className="text-danger mb-16">Error: {error}</div>}

      {endpoints.length === 0 ? (
        <div className="text-secondary" style={{ padding: '24px', textAlign: 'center' }}>
          No remote endpoints configured. All operations run on the local Docker host.
        </div>
      ) : (
        <div style={{ overflowX: 'auto' }}>
          <table>
            <thead>
              <tr>
                <th>Name</th>
                <th>Connection</th>
                <th>Status</th>
                <th>Containers</th>
                <th>Tags</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {endpoints.map((ep) => {
                const id = ep.id || ep.Id;
                const name = ep.name || ep.Name || '—';
                const conn = ep.connection || ep.Connection || '';
                const status = (ep.status || ep.Status || 'unknown').toLowerCase();
                const containers = ep.container_count ?? ep.ContainerCount ?? '—';
                const tags = ep.tags || ep.Tags || [];
                const st = STATUS_STYLES[status] || { dot: 'var(--text-secondary)', label: 'Unknown' };
                const isLocal = name.toLowerCase() === 'local';
                const tr = testResults[id];

                return (
                  <tr key={id}>
                    <td style={{ fontWeight: 600 }}>{name}</td>
                    <td>
                      <code style={{
                        fontSize: '0.75rem',
                        padding: '2px 6px',
                        background: 'var(--bg-tertiary)',
                        borderRadius: '4px',
                      }}>
                        {maskConnection(conn)}
                      </code>
                    </td>
                    <td>
                      <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                        <span style={{
                          width: '8px', height: '8px', borderRadius: '50%',
                          background: st.dot,
                          display: 'inline-block',
                        }} />
                        <span style={{ fontSize: '0.8rem' }}>{st.label}</span>
                        {tr && (
                          <span style={{ fontSize: '0.7rem', color: 'var(--text-secondary)' }}>
                            ({tr.latency_ms != null ? `${tr.latency_ms}ms` : tr.error || tr.status})
                          </span>
                        )}
                      </div>
                    </td>
                    <td>{containers}</td>
                    <td>
                      {tags.length > 0 ? tags.map((t, i) => (
                        <span key={i} style={{
                          display: 'inline-block',
                          padding: '1px 8px',
                          margin: '2px',
                          borderRadius: '10px',
                          background: 'var(--bg-tertiary)',
                          fontSize: '0.7rem',
                          color: 'var(--text-secondary)',
                          border: '1px solid var(--border)',
                        }}>
                          {t}
                        </span>
                      )) : <span className="text-secondary">—</span>}
                    </td>
                    <td>
                      <div className="btn-group">
                        <button
                          className="btn-sm"
                          onClick={() => handleTest(ep)}
                          disabled={testing[id]}
                        >
                          {testing[id] ? '...' : '🔍 Test'}
                        </button>
                        {status === 'disconnected' && (
                          <button className="btn-sm btn-warning" onClick={() => handleReconnect(ep)}>
                            🔗 Reconnect
                          </button>
                        )}
                        <button className="btn-sm" onClick={() => openEdit(ep)}>
                          ✏️ Edit
                        </button>
                        <button
                          className="btn-danger btn-sm"
                          onClick={() => setShowDelete(ep)}
                          disabled={isLocal}
                          title={isLocal ? 'Cannot delete local endpoint' : 'Delete endpoint'}
                        >
                          🗑
                        </button>
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}

      {/* Add Modal */}
      {showAdd && (
        <Modal
          title="Add Endpoint"
          onClose={() => setShowAdd(false)}
          footer={
            <>
              <button onClick={() => setShowAdd(false)}>Cancel</button>
              <button
                className="btn-primary"
                onClick={handleAdd}
                disabled={saving || !newName.trim() || !newConnection.trim()}
              >
                {saving ? 'Adding...' : 'Add'}
              </button>
            </>
          }
        >
          <div style={{ display: 'grid', gap: '12px' }}>
            <div>
              <label style={{ display: 'block', marginBottom: '4px', fontWeight: 500 }}>Name</label>
              <input
                type="text"
                value={newName}
                onChange={e => setNewName(e.target.value)}
                placeholder="e.g. prod-hq-1"
                style={{ width: '100%' }}
                autoFocus
              />
            </div>
            <div>
              <label style={{ display: 'block', marginBottom: '4px', fontWeight: 500 }}>Connection String</label>
              <input
                type="text"
                value={newConnection}
                onChange={e => setNewConnection(e.target.value)}
                placeholder="tcp://user:pass@host:2376 or ssh://host"
                style={{ width: '100%' }}
              />
              <div style={{ fontSize: '0.7rem', color: 'var(--text-secondary)', marginTop: '4px' }}>
                TCP connections with credentials will be masked in the UI
              </div>
            </div>
            <div>
              <label style={{ display: 'block', marginBottom: '4px', fontWeight: 500 }}>Tags (comma separated)</label>
              <input
                type="text"
                value={newTags}
                onChange={e => setNewTags(e.target.value)}
                placeholder="production, us-east-1"
                style={{ width: '100%' }}
              />
            </div>
            <div>
              <label style={{ display: 'block', marginBottom: '4px', fontWeight: 500 }}>TLS Cert Path (optional)</label>
              <input
                type="text"
                value={newCertPath}
                onChange={e => setNewCertPath(e.target.value)}
                placeholder="/app/certs/llmdebian"
                style={{ width: '100%' }}
              />
              <div style={{ fontSize: '0.7rem', color: 'var(--text-secondary)', marginTop: '4px' }}>
                Directory containing ca.pem, cert.pem, key.pem for https:// endpoints
              </div>
            </div>
            <div>
              <label style={{ display: 'block', marginBottom: '4px', fontWeight: 500 }}>Compose Stacks Directory (optional)</label>
              <input
                type="text"
                value={newStacksDir}
                onChange={e => setNewStacksDir(e.target.value)}
                placeholder="/home/user/docker-compose"
                style={{ width: '100%' }}
              />
              <div style={{ fontSize: '0.7rem', color: 'var(--text-secondary)', marginTop: '4px' }}>
                Host path where compose files are stored (for migration). Leave empty if not using compose migration.
              </div>
            </div>
          </div>
        </Modal>
      )}

      {/* Edit Modal */}
      {showEdit && (
        <Modal
          title={`Edit: ${showEdit.name || showEdit.Name}`}
          onClose={() => setShowEdit(null)}
          footer={
            <>
              <button onClick={() => setShowEdit(null)}>Cancel</button>
              <button
                className="btn-primary"
                onClick={handleEdit}
                disabled={saving || !editName.trim()}
              >
                {saving ? 'Saving...' : 'Save'}
              </button>
            </>
          }
        >
          <div style={{ display: 'grid', gap: '12px' }}>
            <div>
              <label style={{ display: 'block', marginBottom: '4px', fontWeight: 500 }}>Name</label>
              <input
                type="text"
                value={editName}
                onChange={e => setEditName(e.target.value)}
                style={{ width: '100%' }}
                autoFocus
              />
            </div>
            <div>
              <label style={{ display: 'block', marginBottom: '4px', fontWeight: 500 }}>Connection String</label>
              <input
                type="text"
                value={editConnection}
                onChange={e => setEditConnection(e.target.value)}
                placeholder="Leave blank to keep unchanged"
                style={{ width: '100%' }}
              />
              <div style={{ fontSize: '0.7rem', color: 'var(--text-secondary)', marginTop: '4px' }}>
                Leave blank to keep current connection string
              </div>
            </div>
            <div>
              <label style={{ display: 'block', marginBottom: '4px', fontWeight: 500 }}>Tags</label>
              <input
                type="text"
                value={editTags}
                onChange={e => setEditTags(e.target.value)}
                placeholder="production, us-east-1"
                style={{ width: '100%' }}
              />
            </div>
            <div>
              <label style={{ display: 'block', marginBottom: '4px', fontWeight: 500 }}>TLS Cert Path</label>
              <input
                type="text"
                value={editCertPath}
                onChange={e => setEditCertPath(e.target.value)}
                placeholder="/app/certs/endpoint-name"
                style={{ width: '100%' }}
              />
              <div style={{ fontSize: '0.7rem', color: 'var(--text-secondary)', marginTop: '4px' }}>
                Leave blank to use DOCKER_CERT_PATH env var
              </div>
            </div>
            <div>
              <label style={{ display: 'block', marginBottom: '4px', fontWeight: 500 }}>Compose Stacks Directory</label>
              <input
                type="text"
                value={editStacksDir}
                onChange={e => setEditStacksDir(e.target.value)}
                placeholder="/home/user/docker-compose"
                style={{ width: '100%' }}
              />
              <div style={{ fontSize: '0.7rem', color: 'var(--text-secondary)', marginTop: '4px' }}>
                Host path where compose files are stored (for migration). Leave empty to clear.
              </div>
            </div>
          </div>
        </Modal>
      )}

      {/* Delete Confirmation */}
      {showDelete && (
        <Modal
          title="Delete Endpoint"
          onClose={() => setShowDelete(null)}
          footer={
            <>
              <button onClick={() => setShowDelete(null)}>Cancel</button>
              <button className="btn-danger" onClick={handleDelete}>
                Delete
              </button>
            </>
          }
        >
          <div>
            <p>Are you sure you want to delete <strong>{showDelete.name || showDelete.Name}</strong>?</p>
            {(showDelete.name || showDelete.Name || '').toLowerCase() === 'local' && (
              <div className="text-danger" style={{ marginTop: '8px' }}>Cannot delete the local endpoint.</div>
            )}
          </div>
        </Modal>
      )}

      {/* Setup Script Generator */}
      {showGenerator && (
        <SetupScriptGenerator onClose={() => setShowGenerator(false)} />
      )}
    </div>
  );
}
