import { useState, useEffect, useCallback } from 'react';
import { api } from '../api/client';
import StatusBadge from '../components/StatusBadge';
import Spinner from '../components/Spinner';
import ListToolbar, { useSelection } from '../components/ListToolbar';

export default function Containers({ navigate }) {
  const [containers, setContainers] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);

  const load = useCallback(async () => {
    try {
      const data = await api.get('/api/containers');
      setContainers(Array.isArray(data) ? data : (data?.containers || []));
      setError(null);
    } catch (err) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  const { selected, toggle, clear } = useSelection(containers, 'id');

  const selectedItems = containers.filter(c => selected.has(c.id));
  const hasRunning = selectedItems.some(c => c.state === 'running');
  const hasStopped = selectedItems.some(c => c.state !== 'running' && c.state !== 'removing');
  const allRunning = selectedItems.length > 0 && selectedItems.every(c => c.state === 'running');
  const allStopped = selectedItems.length > 0 && selectedItems.every(c => c.state === 'exited' || c.state === 'stopped');

  const handleAction = async (action) => {
    const ids = Array.from(selected);
    for (const id of ids) {
      try { await api.post(`/api/containers/${id}/${action}`); } catch (e) { /* continue */ }
    }
    clear();
    load();
  };

  const handleRemove = async () => {
    const ids = Array.from(selected);
    if (!confirm(`Remove ${ids.length} container(s)?`)) return;
    for (const id of ids) {
      try { await api.delete(`/api/containers/${id}`); } catch (e) { alert('Error: ' + e.message); }
    }
    clear();
    load();
  };

  if (loading) return <div className="loading-center"><Spinner size="lg" /></div>;

  return (
    <div>
      <div className="section-header">
        <h1>Containers ({containers.length})</h1>
        <button onClick={load}>🔄 Refresh</button>
      </div>

      {error && <div className="text-danger mb-16">Error: {error}</div>}

      <ListToolbar
        selected={selected}
        total={containers.length}
        onClear={clear}
        actions={[
          { label: '▶ Start', onClick: () => handleAction('start'), disabled: !hasStopped },
          { label: '⏹ Stop', onClick: () => handleAction('stop'), disabled: !hasRunning },
          { label: '🔄 Restart', onClick: () => handleAction('restart'), disabled: !hasRunning },
          { label: '🗑 Remove', onClick: handleRemove, variant: 'danger' },
        ]}
      />

      {containers.length === 0 ? (
        <div className="text-secondary" style={{ padding: '24px', textAlign: 'center' }}>No containers</div>
      ) : (
        <table>
          <thead>
            <tr>
              <th style={{ width: '32px' }}></th>
              <th>Name</th>
              <th>Image</th>
              <th>State</th>
              <th>Status</th>
              <th>Ports</th>
            </tr>
          </thead>
          <tbody>
            {containers.map((row) => {
              const isSel = selected.has(row.id);
              const name = (row.name || '').replace(/^\//, '');
              return (
                <tr key={row.id}
                  onClick={() => navigate('containerDetail', { id: row.id, name: row.name })}
                  style={{ cursor: 'pointer' }}
                >
                  <td onClick={(e) => e.stopPropagation()}>
                    <input
                      type="checkbox"
                      checked={isSel}
                      onChange={() => toggle(row)}
                      style={{ width: '14px', height: '14px' }}
                    />
                  </td>
                  <td style={{ fontWeight: 500, color: 'var(--accent)' }}>{name}</td>
                  <td className="mono" style={{ fontSize: '0.8rem' }}>{row.image}</td>
                  <td><StatusBadge state={row.state} /></td>
                  <td className="text-secondary" style={{ fontSize: '0.8rem' }}>{row.status}</td>
                  <td>
                    {row.ports && Array.isArray(row.ports) && row.ports.length > 0
                      ? row.ports.map((p, i) => (
                          <span key={i} className="mono" style={{ fontSize: '0.7rem', marginRight: '6px' }}>
                            {p.publicPort ? `${p.publicPort}:${p.privatePort}` : p.privatePort}
                          </span>
                        ))
                      : <span className="text-secondary">—</span>
                    }
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
    </div>
  );
}
