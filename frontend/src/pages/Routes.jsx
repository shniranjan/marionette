import { useState, useEffect, useCallback } from 'react';
import { api } from '../api/client';
import Modal from '../components/Modal';
import Spinner from '../components/Spinner';
import { useToast } from '../components/Toast';

const AUTH_BADGE = {
  none: { bg: 'var(--text-secondary)', label: 'none' },
  key: { bg: 'var(--blue)', label: 'key' },
  basic: { bg: 'var(--green)', label: 'basic' },
};

export default function Routes() {
  const toast = useToast();
  const [routes, setRoutes] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [showAdd, setShowAdd] = useState(false);
  const [showEdit, setShowEdit] = useState(null);
  const [showDelete, setShowDelete] = useState(null);
  const [showAccess, setShowAccess] = useState(null);
  const [saving, setSaving] = useState(false);

  // Access modal state
  const [users, setUsers] = useState([]);
  const [accessUserIds, setAccessUserIds] = useState([]);
  const [accessLoading, setAccessLoading] = useState(false);
  const [accessToggling, setAccessToggling] = useState({});

  // Add form
  const [newPath, setNewPath] = useState('');
  const [newTarget, setNewTarget] = useState('');
  const [newAuthMode, setNewAuthMode] = useState('none');
  const [newAuthValue, setNewAuthValue] = useState('');
  const [newTls, setNewTls] = useState(false);

  // Edit form
  const [editPath, setEditPath] = useState('');
  const [editTarget, setEditTarget] = useState('');
  const [editAuthMode, setEditAuthMode] = useState('none');
  const [editAuthValue, setEditAuthValue] = useState('');
  const [editTls, setEditTls] = useState(false);
  const [editActive, setEditActive] = useState(true);

  const load = useCallback(async () => {
    try {
      const data = await api.get('/api/routes');
      setRoutes(Array.isArray(data) ? data : []);
      setError(null);
    } catch (err) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  const handleAdd = async () => {
    if (!newPath.trim() || !newTarget.trim()) return;
    if (!newPath.trim().startsWith('/')) {
      toast('Path must start with /', 'error');
      return;
    }
    setSaving(true);
    try {
      const body = {
        path: newPath.trim(),
        target: newTarget.trim(),
        authMode: newAuthMode,
        tls: newTls,
      };
      if (newAuthMode !== 'none' && newAuthValue.trim()) {
        body.authValue = newAuthValue.trim();
      }
      await api.post('/api/routes', body);
      setShowAdd(false);
      setNewPath('');
      setNewTarget('');
      setNewAuthMode('none');
      setNewAuthValue('');
      setNewTls(false);
      toast('Route added', 'success');
      load();
    } catch (err) {
      toast('Error: ' + err.message, 'error');
    } finally {
      setSaving(false);
    }
  };

  const handleEdit = async () => {
    if (!editPath.trim()) return;
    if (!editPath.trim().startsWith('/')) {
      toast('Path must start with /', 'error');
      return;
    }
    setSaving(true);
    try {
      const id = showEdit.id;
      const body = {
        path: editPath.trim(),
        target: editTarget.trim() || undefined,
        authMode: editAuthMode,
        tls: editTls,
        active: editActive,
      };
      if (editAuthMode !== 'none' && editAuthValue.trim()) {
        body.authValue = editAuthValue.trim();
      }
      await api.patch(`/api/routes/${id}`, body);
      setShowEdit(null);
      toast('Route updated', 'success');
      load();
    } catch (err) {
      toast('Error: ' + err.message, 'error');
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async () => {
    if (!showDelete) return;
    try {
      await api.delete(`/api/routes/${showDelete.id}`);
      setShowDelete(null);
      toast('Route deleted', 'success');
      load();
    } catch (err) {
      toast('Error: ' + err.message, 'error');
    }
  };

  const openEdit = (route) => {
    setShowEdit(route);
    setEditPath(route.path || '');
    setEditTarget(route.target || '');
    setEditAuthMode(route.authMode || 'none');
    setEditAuthValue('');
    setEditTls(!!route.tls);
    setEditActive(!!route.active);
  };

  const openAccess = async (route) => {
    setShowAccess(route);
    setAccessLoading(true);
    try {
      const [usersData, accessData] = await Promise.all([
        api.get('/api/users'),
        api.get(`/api/routes/${route.id}/access`),
      ]);
      setUsers(Array.isArray(usersData) ? usersData : []);
      setAccessUserIds(Array.isArray(accessData) ? accessData : []);
    } catch (err) {
      toast('Error loading access: ' + err.message, 'error');
      setShowAccess(null);
    } finally {
      setAccessLoading(false);
    }
  };

  const toggleAccess = async (userId, grant) => {
    if (!showAccess) return;
    const routeId = showAccess.id;
    setAccessToggling(prev => ({ ...prev, [userId]: true }));
    try {
      if (grant) {
        await api.post(`/api/routes/${routeId}/access`, { userId });
      } else {
        await api.delete(`/api/routes/${routeId}/access/${userId}`);
      }
      setAccessUserIds(prev =>
        grant ? [...prev, userId] : prev.filter(id => id !== userId)
      );
    } catch (err) {
      toast('Error: ' + err.message, 'error');
    } finally {
      setAccessToggling(prev => ({ ...prev, [userId]: false }));
    }
  };

  if (loading) return <div className="loading-center"><Spinner size="lg" /></div>;

  return (
    <div>
      <div className="section-header">
        <h1>Routes ({routes.length})</h1>
        <div className="btn-group">
          <button className="btn-primary" onClick={() => setShowAdd(true)}>+ Add Route</button>
          <button onClick={load}>🔄 Refresh</button>
        </div>
      </div>

      {error && <div className="text-danger mb-16">Error: {error}</div>}

      {routes.length === 0 ? (
        <div className="text-secondary" style={{ padding: '24px', textAlign: 'center' }}>
          No routes configured. Add a route to start proxying traffic.
        </div>
      ) : (
        <table>
          <thead>
            <tr>
              <th>Path</th>
              <th>Target</th>
              <th>Auth</th>
              <th>TLS</th>
              <th>Active</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            {routes.map((route) => {
              const authInfo = AUTH_BADGE[route.authMode] || AUTH_BADGE.none;
              const hasAuthValue = route.authMode && route.authMode !== 'none' && route.authValue;
              return (
                <tr key={route.id}>
                  <td style={{ fontWeight: 600 }}>{route.path}</td>
                  <td>
                    <code style={{
                      fontSize: '0.75rem',
                      padding: '2px 6px',
                      background: 'var(--bg-tertiary)',
                      borderRadius: '4px',
                    }}>
                      {route.target}
                    </code>
                  </td>
                  <td>
                    <span style={{
                      display: 'inline-flex',
                      alignItems: 'center',
                      gap: '4px',
                      padding: '1px 8px',
                      borderRadius: '10px',
                      background: authInfo.bg,
                      color: '#fff',
                      fontSize: '0.7rem',
                      fontWeight: 600,
                    }}>
                      {authInfo.label}
                      {hasAuthValue && ' ••••'}
                    </span>
                  </td>
                  <td>
                    {route.tls ? (
                      <span style={{ color: 'var(--green)', fontWeight: 600 }}>✓</span>
                    ) : (
                      <span className="text-secondary">—</span>
                    )}
                  </td>
                  <td>
                    <span style={{
                      width: '8px',
                      height: '8px',
                      borderRadius: '50%',
                      background: route.active ? 'var(--green)' : 'var(--red)',
                      display: 'inline-block',
                    }} />
                  </td>
                  <td>
                    <div className="btn-group">
                      <button className="btn-sm" onClick={() => openEdit(route)}>
                        ✏️ Edit
                      </button>
                      <button className="btn-sm" onClick={() => openAccess(route)}>
                        🔑 Access
                      </button>
                      <button
                        className="btn-danger btn-sm"
                        onClick={() => setShowDelete(route)}
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
      )}

      {/* Add Modal */}
      {showAdd && (
        <Modal
          title="Add Route"
          onClose={() => setShowAdd(false)}
          footer={
            <>
              <button onClick={() => setShowAdd(false)}>Cancel</button>
              <button
                className="btn-primary"
                onClick={handleAdd}
                disabled={saving || !newPath.trim() || !newTarget.trim()}
              >
                {saving ? 'Adding...' : 'Add'}
              </button>
            </>
          }
        >
          <div style={{ display: 'grid', gap: '12px' }}>
            <div>
              <label style={{ display: 'block', marginBottom: '4px', fontWeight: 500 }}>Path</label>
              <input
                type="text"
                value={newPath}
                onChange={e => setNewPath(e.target.value)}
                placeholder="/api/my-service"
                style={{ width: '100%' }}
                autoFocus
              />
              <div style={{ fontSize: '0.7rem', color: 'var(--text-secondary)', marginTop: '4px' }}>
                Must start with /
              </div>
            </div>
            <div>
              <label style={{ display: 'block', marginBottom: '4px', fontWeight: 500 }}>Target</label>
              <input
                type="text"
                value={newTarget}
                onChange={e => setNewTarget(e.target.value)}
                placeholder="host:port or service:port"
                style={{ width: '100%' }}
              />
            </div>
            <div>
              <label style={{ display: 'block', marginBottom: '4px', fontWeight: 500 }}>Auth Mode</label>
              <select
                value={newAuthMode}
                onChange={e => setNewAuthMode(e.target.value)}
                style={{ width: '100%' }}
              >
                <option value="none">None</option>
                <option value="key">Key</option>
                <option value="basic">Basic</option>
              </select>
            </div>
            {newAuthMode !== 'none' && (
              <div>
                <label style={{ display: 'block', marginBottom: '4px', fontWeight: 500 }}>Auth Value</label>
                <input
                  type="password"
                  value={newAuthValue}
                  onChange={e => setNewAuthValue(e.target.value)}
                  placeholder="Key or password"
                  style={{ width: '100%' }}
                />
              </div>
            )}
            <div>
              <label style={{ display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer' }}>
                <input
                  type="checkbox"
                  checked={newTls}
                  onChange={e => setNewTls(e.target.checked)}
                />
                <span style={{ fontWeight: 500 }}>TLS</span>
              </label>
            </div>
          </div>
        </Modal>
      )}

      {/* Edit Modal */}
      {showEdit && (
        <Modal
          title={`Edit: ${showEdit.path}`}
          onClose={() => setShowEdit(null)}
          footer={
            <>
              <button onClick={() => setShowEdit(null)}>Cancel</button>
              <button
                className="btn-primary"
                onClick={handleEdit}
                disabled={saving || !editPath.trim()}
              >
                {saving ? 'Saving...' : 'Save'}
              </button>
            </>
          }
        >
          <div style={{ display: 'grid', gap: '12px' }}>
            <div>
              <label style={{ display: 'block', marginBottom: '4px', fontWeight: 500 }}>Path</label>
              <input
                type="text"
                value={editPath}
                onChange={e => setEditPath(e.target.value)}
                style={{ width: '100%' }}
                autoFocus
              />
            </div>
            <div>
              <label style={{ display: 'block', marginBottom: '4px', fontWeight: 500 }}>Target</label>
              <input
                type="text"
                value={editTarget}
                onChange={e => setEditTarget(e.target.value)}
                placeholder="Leave blank to keep unchanged"
                style={{ width: '100%' }}
              />
            </div>
            <div>
              <label style={{ display: 'block', marginBottom: '4px', fontWeight: 500 }}>Auth Mode</label>
              <select
                value={editAuthMode}
                onChange={e => setEditAuthMode(e.target.value)}
                style={{ width: '100%' }}
              >
                <option value="none">None</option>
                <option value="key">Key</option>
                <option value="basic">Basic</option>
              </select>
            </div>
            {editAuthMode !== 'none' && (
              <div>
                <label style={{ display: 'block', marginBottom: '4px', fontWeight: 500 }}>Auth Value</label>
                <input
                  type="password"
                  value={editAuthValue}
                  onChange={e => setEditAuthValue(e.target.value)}
                  placeholder="Leave blank to keep unchanged"
                  style={{ width: '100%' }}
                />
                <div style={{ fontSize: '0.7rem', color: 'var(--text-secondary)', marginTop: '4px' }}>
                  Leave blank to keep current auth value
                </div>
              </div>
            )}
            <div>
              <label style={{ display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer' }}>
                <input
                  type="checkbox"
                  checked={editTls}
                  onChange={e => setEditTls(e.target.checked)}
                />
                <span style={{ fontWeight: 500 }}>TLS</span>
              </label>
            </div>
            <div>
              <label style={{ display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer' }}>
                <input
                  type="checkbox"
                  checked={editActive}
                  onChange={e => setEditActive(e.target.checked)}
                />
                <span style={{ fontWeight: 500 }}>Active</span>
              </label>
            </div>
          </div>
        </Modal>
      )}

      {/* Delete Confirmation */}
      {showDelete && (
        <Modal
          title="Delete Route"
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
            <p>Are you sure you want to delete <strong>{showDelete.path}</strong>?</p>
          </div>
        </Modal>
      )}

      {/* Access Modal */}
      {showAccess && (
        <Modal
          title={`Access: ${showAccess.path}`}
          onClose={() => { setShowAccess(null); setUsers([]); setAccessUserIds([]); }}
          size="large"
          footer={
            <button onClick={() => { setShowAccess(null); setUsers([]); setAccessUserIds([]); }}>
              Close
            </button>
          }
        >
          {accessLoading ? (
            <div style={{ textAlign: 'center', padding: '24px' }}>
              <Spinner size="lg" />
            </div>
          ) : users.length === 0 ? (
            <div className="text-secondary" style={{ padding: '24px', textAlign: 'center' }}>
              No users found.
            </div>
          ) : (
            <table>
              <thead>
                <tr>
                  <th>User</th>
                  <th>Role</th>
                  <th>Access</th>
                </tr>
              </thead>
              <tbody>
                {users.map((user) => {
                  const hasAccess = accessUserIds.includes(user.id);
                  const isActive = user.active;
                  const toggling = accessToggling[user.id];
                  return (
                    <tr key={user.id} style={!isActive ? { opacity: 0.5 } : undefined}>
                      <td style={{ fontWeight: isActive ? 500 : 400 }}>
                        {user.name || user.id}
                        {!isActive && (
                          <span style={{
                            marginLeft: '8px',
                            fontSize: '0.65rem',
                            padding: '1px 6px',
                            borderRadius: '8px',
                            background: 'var(--bg-tertiary)',
                            color: 'var(--text-secondary)',
                          }}>
                            inactive
                          </span>
                        )}
                      </td>
                      <td style={{ color: 'var(--text-secondary)', fontSize: '0.8rem' }}>
                        {user.role || '—'}
                      </td>
                      <td>
                        <label style={{
                          display: 'flex',
                          alignItems: 'center',
                          gap: '8px',
                          cursor: isActive ? 'pointer' : 'default',
                        }}>
                          <input
                            type="checkbox"
                            checked={hasAccess}
                            disabled={!isActive || toggling}
                            onChange={e => toggleAccess(user.id, e.target.checked)}
                          />
                          {toggling ? '...' : (hasAccess ? 'Granted' : 'Denied')}
                        </label>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          )}
        </Modal>
      )}
    </div>
  );
}
