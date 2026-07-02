import { useState, useEffect, useCallback, useMemo } from 'react';
import { api } from '../api/client';
import Modal from '../components/Modal';
import Spinner from '../components/Spinner';
import ListToolbar, { useSelection } from '../components/ListToolbar';
import useFilters from '../hooks/useFilters';
import FilterBar from '../components/FilterBar';
import SortableTable from '../components/SortableTable';

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

  // Reload when endpoint changes (EndpointSwitcher dispatches 'endpoint:changed')
  useEffect(() => {
    const handler = () => load();
    window.addEventListener('endpoint:changed', handler);
    return () => window.removeEventListener('endpoint:changed', handler);
  }, [load]);

  const { filtered, searchQuery, setSearchQuery } = useFilters(networks, { searchFields: ['name', 'driver'] });

  const filteredIds = useMemo(() => filtered.map(net => net.id), [filtered]);

  const { selected, toggle, toggleAll, clear, allFilteredSelected } = useSelection(networks, 'id', filteredIds);

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

  const columns = [
    { key: 'name', label: 'Name', sortable: true },
    { key: 'driver', label: 'Driver', sortable: true },
    { key: 'scope', label: 'Scope', sortable: true },
    {
      key: 'containers',
      label: 'Containers',
      sortable: false,
      render: (containers) => {
        if (!containers || Object.keys(containers).length === 0) return <span className="text-secondary">—</span>;
        const names = Object.values(containers).map(c => c.name).join(', ');
        const count = Object.keys(containers).length;
        return <span title={names}>{count} — {names.substring(0, 50)}{names.length > 50 ? '…' : ''}</span>;
      },
    },
  ];

  if (loading) return <div className="loading-center"><Spinner size="lg" /></div>;

  return (
    <div>
      <div className="section-header">
        <h1>Networks ({filtered.length}{filtered.length !== networks.length ? ` / ${networks.length}` : ''})</h1>
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
        filteredIds={filteredIds}
        onClear={clear}
        actions={[
          { label: '🗑 Remove', onClick: handleRemove, variant: 'danger' },
        ]}
      />

      <FilterBar
        searchQuery={searchQuery}
        onSearchChange={setSearchQuery}
        searchPlaceholder="Search networks..."
        filteredCount={filtered.length}
        totalCount={networks.length}
      />

      <div style={{ overflowX: 'auto' }}>
        <SortableTable
          data={filtered}
          columns={columns}
          keyField="id"
          selected={selected}
          onToggle={toggle}
          onToggleAll={toggleAll}
          allSelected={allFilteredSelected || false}
          emptyMessage="No networks"
        />
      </div>

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
