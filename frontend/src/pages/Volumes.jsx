import { useState, useEffect, useCallback } from 'react';
import { api } from '../api/client';
import Modal from '../components/Modal';
import JsonTree from '../components/JsonTree';
import Spinner from '../components/Spinner';
import ListToolbar, { useSelection } from '../components/ListToolbar';

export default function Volumes() {
  const [volumes, setVolumes] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [showCreate, setShowCreate] = useState(false);
  const [newName, setNewName] = useState('');
  const [newDriver, setNewDriver] = useState('local');
  const [creating, setCreating] = useState(false);
  const [inspectData, setInspectData] = useState(null);

  const load = useCallback(async () => {
    try {
      const data = await api.get('/api/volumes');
      setVolumes(Array.isArray(data) ? data : (data?.volumes || []));
      setError(null);
    } catch (err) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  const { selected, toggle, selectAll, clear } = useSelection(volumes, 'name');

  const handleCreate = async () => {
    if (!newName.trim()) return;
    setCreating(true);
    try {
      await api.post('/api/volumes', { name: newName.trim(), driver: newDriver || 'local' });
      setShowCreate(false);
      setNewName('');
      load();
    } catch (err) {
      alert('Error: ' + err.message);
    } finally {
      setCreating(false);
    }
  };

  const handleRemove = async () => {
    const names = Array.from(selected);
    if (!confirm(`Remove ${names.length} volume(s)?`)) return;
    for (const name of names) {
      try { await api.delete(`/api/volumes/${name}`); } catch (e) { alert('Error: ' + e.message); }
    }
    clear();
    load();
  };

  const handlePrune = async () => {
    if (!confirm('Remove all unused volumes?')) return;
    try {
      const result = await api.post('/api/volumes/prune');
      alert(`Pruned volumes. Reclaimed: ${result?.spaceReclaimed || 'unknown'}`);
      load();
    } catch (err) {
      alert('Error: ' + err.message);
    }
  };

  const handleInspect = async (name) => {
    try {
      const data = await api.get(`/api/volumes/${name}/inspect`);
      setInspectData(data);
    } catch (err) {
      alert('Error: ' + err.message);
    }
  };

  if (loading) return <div className="loading-center"><Spinner size="lg" /></div>;

  return (
    <div>
      <div className="section-header">
        <h1>Volumes ({volumes.length})</h1>
        <div className="btn-group">
          <button className="btn-primary" onClick={() => setShowCreate(true)}>+ Create</button>
          <button className="btn-danger" onClick={handlePrune}>🗑 Prune</button>
          <button onClick={load}>🔄 Refresh</button>
        </div>
      </div>

      {error && <div className="text-danger mb-16">Error: {error}</div>}

      <ListToolbar
        selected={selected}
        total={volumes.length}
        onClear={clear}
        actions={[
          { label: '🗑 Remove', onClick: handleRemove, variant: 'danger' },
        ]}
      />

      {volumes.length === 0 ? (
        <div className="text-secondary" style={{ padding: '24px', textAlign: 'center' }}>No volumes</div>
      ) : (
        <table>
          <thead>
            <tr>
              <th style={{ width: '32px' }}></th>
              <th>Name</th>
              <th>Driver</th>
              <th>Mountpoint</th>
            </tr>
          </thead>
          <tbody>
            {volumes.map((v) => {
              const isSel = selected.has(v.name);
              return (
                <tr key={v.name} onClick={() => handleInspect(v.name)} style={{ cursor: 'pointer' }}>
                  <td onClick={(e) => e.stopPropagation()}>
                    <input
                      type="checkbox"
                      checked={isSel}
                      onChange={() => toggle(v)}
                      style={{ width: '14px', height: '14px' }}
                    />
                  </td>
                  <td className="mono">{v.name}</td>
                  <td>{v.driver}</td>
                  <td className="mono" style={{ fontSize: '0.75rem' }}>{v.mountpoint}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}

      {showCreate && (
        <Modal
          title="Create Volume"
          onClose={() => setShowCreate(false)}
          footer={
            <>
              <button onClick={() => setShowCreate(false)}>Cancel</button>
              <button className="btn-primary" onClick={handleCreate} disabled={creating || !newName.trim()}>
                {creating ? 'Creating...' : 'Create'}
              </button>
            </>
          }
        >
          <div>
            <label style={{ display: 'block', marginBottom: '6px', fontWeight: 500 }}>Name</label>
            <input
              type="text"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              placeholder="my-volume"
              style={{ width: '100%', marginBottom: '12px' }}
              autoFocus
            />
            <label style={{ display: 'block', marginBottom: '6px', fontWeight: 500 }}>Driver</label>
            <input
              type="text"
              value={newDriver}
              onChange={(e) => setNewDriver(e.target.value)}
              placeholder="local"
              style={{ width: '100%' }}
            />
          </div>
        </Modal>
      )}

      {inspectData && (
        <Modal title="Volume Inspect" onClose={() => setInspectData(null)}>
          <JsonTree data={inspectData} />
        </Modal>
      )}
    </div>
  );
}
