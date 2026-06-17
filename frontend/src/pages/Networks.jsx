import { useState, useEffect, useCallback } from 'react';
import { api } from '../api/client';
import Modal from '../components/Modal';
import Spinner from '../components/Spinner';
import ListToolbar, { useSelection } from '../components/ListToolbar';

export default function Networks() {
  const [networks, setNetworks] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [showCreate, setShowCreate] = useState(false);
  const [newName, setNewName] = useState('');
  const [newDriver, setNewDriver] = useState('bridge');
  const [creating, setCreating] = useState(false);
  const [showConnect, setShowConnect] = useState(null);
  const [connectContainer, setConnectContainer] = useState('');

  const load = useCallback(async () => {
    try {
      const data = await api.get('/api/networks');
      setNetworks(Array.isArray(data) ? data : (data?.networks || []));
      setError(null);
    } catch (err) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  const { selected, toggle, clear } = useSelection(networks, 'id');

  const handleCreate = async () => {
    if (!newName.trim()) return;
    setCreating(true);
    try {
      await api.post('/api/networks', { name: newName.trim(), driver: newDriver || 'bridge' });
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
    const ids = Array.from(selected);
    if (!confirm(`Remove ${ids.length} network(s)?`)) return;
    for (const id of ids) {
      try { await api.delete(`/api/networks/${id}`); } catch (e) { alert('Error: ' + e.message); }
    }
    clear();
    load();
  };

  const handlePrune = async () => {
    if (!confirm('Remove all unused networks?')) return;
    try {
      await api.post('/api/networks/prune');
      load();
    } catch (err) {
      alert('Error: ' + err.message);
    }
  };

  const handleConnect = async () => {
    if (!showConnect || !connectContainer.trim()) return;
    try {
      await api.post(`/api/networks/${showConnect.id}/connect`, { container: connectContainer.trim() });
      setShowConnect(null);
      setConnectContainer('');
      load();
    } catch (err) {
      alert('Error: ' + err.message);
    }
  };

  const handleDisconnect = async (networkId, containerId) => {
    if (!confirm('Disconnect this container?')) return;
    try {
      await api.post(`/api/networks/${networkId}/disconnect`, { container: containerId });
      load();
    } catch (err) {
      alert('Error: ' + err.message);
    }
  };

  if (loading) return <div className="loading-center"><Spinner size="lg" /></div>;

  return (
    <div>
      <div className="section-header">
        <h1>Networks ({networks.length})</h1>
        <div className="btn-group">
          <button className="btn-primary" onClick={() => setShowCreate(true)}>+ Create</button>
          <button className="btn-danger" onClick={handlePrune}>🗑 Prune</button>
          <button onClick={load}>🔄 Refresh</button>
        </div>
      </div>

      {error && <div className="text-danger mb-16">Error: {error}</div>}

      <ListToolbar
        selected={selected}
        total={networks.length}
        onClear={clear}
        actions={[
          { label: '🗑 Remove', onClick: handleRemove, variant: 'danger' },
        ]}
      />

      {networks.length === 0 ? (
        <div className="text-secondary" style={{ padding: '24px', textAlign: 'center' }}>No networks</div>
      ) : (
        <div style={{ display: 'grid', gap: '12px' }}>
          {networks.map((net) => {
            const isSel = selected.has(net.id);
            return (
              <div key={net.id} className="card">
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
                  <div style={{ display: 'flex', alignItems: 'flex-start', gap: '10px' }}>
                    <input
                      type="checkbox"
                      checked={isSel}
                      onChange={() => toggle(net)}
                      style={{ width: '16px', height: '16px', marginTop: '2px', cursor: 'pointer' }}
                    />
                    <div>
                      <div style={{ fontWeight: 600 }}>{net.name}</div>
                      <div className="text-secondary" style={{ fontSize: '0.75rem', marginTop: '4px' }}>
                        ID: {net.id?.substring(0, 12)} &nbsp;|&nbsp;
                        Driver: {net.driver} &nbsp;|&nbsp;
                        Scope: {net.scope}
                        {net.ipam?.config?.length > 0 && (
                          <span> &nbsp;|&nbsp; Subnet: {net.ipam.config[0].subnet}</span>
                        )}
                      </div>
                      {net.containers && Object.keys(net.containers).length > 0 && (
                        <div style={{ marginTop: '8px' }}>
                          <div className="text-secondary" style={{ fontSize: '0.75rem', marginBottom: '4px' }}>
                            Containers ({Object.keys(net.containers).length}):
                          </div>
                          {Object.entries(net.containers).map(([cid, info]) => (
                            <div key={cid} style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '2px' }}>
                              <span className="mono" style={{ fontSize: '0.75rem' }}>{info.name}</span>
                              <span className="mono" style={{ fontSize: '0.7rem', color: 'var(--pico-muted-color)' }}>
                                ({info.ipv4Address})
                              </span>
                              <button
                                className="btn-sm"
                                style={{ fontSize: '0.65rem', padding: '1px 6px' }}
                                onClick={() => handleDisconnect(net.id, cid)}
                              >
                                Disconnect
                              </button>
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  </div>
                  <button className="btn-sm" onClick={() => setShowConnect(net)}>🔗 Connect</button>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {showCreate && (
        <Modal
          title="Create Network"
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
              placeholder="my-network"
              style={{ width: '100%', marginBottom: '12px' }}
              autoFocus
            />
            <label style={{ display: 'block', marginBottom: '6px', fontWeight: 500 }}>Driver</label>
            <select value={newDriver} onChange={(e) => setNewDriver(e.target.value)} style={{ width: '100%' }}>
              <option value="bridge">bridge</option>
              <option value="overlay">overlay</option>
              <option value="host">host</option>
              <option value="none">none</option>
            </select>
          </div>
        </Modal>
      )}

      {showConnect && (
        <Modal
          title={`Connect to ${showConnect.name}`}
          onClose={() => { setShowConnect(null); setConnectContainer(''); }}
          footer={
            <>
              <button onClick={() => { setShowConnect(null); setConnectContainer(''); }}>Cancel</button>
              <button className="btn-primary" onClick={handleConnect} disabled={!connectContainer.trim()}>
                Connect
              </button>
            </>
          }
        >
          <div>
            <label style={{ display: 'block', marginBottom: '6px', fontWeight: 500 }}>Container ID or Name</label>
            <input
              type="text"
              value={connectContainer}
              onChange={(e) => setConnectContainer(e.target.value)}
              placeholder="my-container"
              style={{ width: '100%' }}
              autoFocus
              onKeyDown={(e) => e.key === 'Enter' && handleConnect()}
            />
          </div>
        </Modal>
      )}
    </div>
  );
}
