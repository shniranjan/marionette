import { useState, useEffect, useCallback, useMemo } from 'react';
import { api } from '../api/client';
import Modal from '../components/Modal';
import JsonTree from '../components/JsonTree';
import Spinner from '../components/Spinner';
import ListToolbar, { useSelection } from '../components/ListToolbar';
import SortableTable from '../components/SortableTable';
import FilterBar from '../components/FilterBar';
import useFilters from '../hooks/useFilters';

const COLUMNS = [
  { key: 'name', label: 'Name', sortable: true },
  { key: 'driver', label: 'Driver', sortable: true },
  { key: 'mountpoint', label: 'Mountpoint', sortable: true },
];

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

  const { filtered, searchQuery, setSearchQuery } = useFilters(volumes, {
    searchFields: ['name', 'driver'],
  });

  const filteredIds = useMemo(() => filtered.map((v) => v.name), [filtered]);

  const { selected, toggle, toggleAll, selectAll, clear, allFilteredSelected } =
    useSelection(volumes, 'name', filteredIds);

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

      <FilterBar
        searchQuery={searchQuery}
        onSearchChange={setSearchQuery}
        searchPlaceholder="Filter volumes..."
        filteredCount={filtered.length}
        totalCount={volumes.length}
      />

      <ListToolbar
        selected={selected}
        total={volumes.length}
        filteredIds={filteredIds}
        onClear={clear}
        actions={[
          { label: '🗑 Remove', onClick: handleRemove, variant: 'danger' },
        ]}
      />

      <SortableTable
        data={filtered}
        columns={COLUMNS}
        keyField="name"
        onRowClick={(row) => handleInspect(row.name)}
        selected={selected}
        onToggle={toggle}
        onToggleAll={toggleAll}
        allSelected={allFilteredSelected}
        emptyMessage="No volumes"
      />

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
