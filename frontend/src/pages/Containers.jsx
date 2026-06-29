import { useState, useEffect, useCallback, useMemo } from 'react';
import { api } from '../api/client';
import StatusBadge from '../components/StatusBadge';
import Spinner from '../components/Spinner';
import ListToolbar, { useSelection } from '../components/ListToolbar';
import useFilters from '../hooks/useFilters';
import SortableTable from '../components/SortableTable';
import FilterBar from '../components/FilterBar';

function renderPorts(ports) {
  if (!ports || !Array.isArray(ports) || ports.length === 0) {
    return <span className="text-secondary">—</span>;
  }
  return ports.map((p, i) => (
    <span key={i} className="mono" style={{ fontSize: '0.7rem', marginRight: '6px' }}>
      {p.publicPort ? `${p.publicPort}:${p.privatePort}` : p.privatePort}
    </span>
  ));
}

export default function Containers({ navigate }) {
  const [containers, setContainers] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);

  const load = useCallback(async () => {
    try {
      const data = await api.get('/api/containers?includeHealth=true');
      let items = Array.isArray(data) ? data : (data?.containers || []);
      // Normalize PascalCase to camelCase
      items = items.map((c) => ({
        id: c.id || c.Id,
        name: (c.name || c.Name || '').replace(/^\//, ''),
        image: c.image || c.Image || '',
        state: c.state || c.State || '',
        status: c.status || c.Status || '',
        ports: c.ports || c.Ports || [],
        health: c.health || null,
      }));
      setContainers(items);
      setError(null);
    } catch (err) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  const {
    filtered,
    searchQuery,
    setSearchQuery,
    stateFilter,
    setStateFilter,
    sortKey,
    setSortKey,
    sortDir,
    setSortDir,
  } = useFilters(containers, {
    searchFields: ['name', 'image'],
    stateField: 'state',
    stateMap: {
      running: ['running'],
      stopped: ['exited', 'stopped'],
      paused: ['paused'],
    },
  });

  // Filtered IDs for selection (derived from filtered data)
  const filteredIds = useMemo(() => filtered.map((c) => c.id), [filtered]);

  const { selected, toggle, toggleAll, clear, allFilteredSelected } = useSelection(
    containers,
    'id',
    filteredIds,
  );

  const selectedItems = containers.filter((c) => selected.has(c.id));
  const hasRunning = selectedItems.some((c) => c.state === 'running');
  const hasStopped = selectedItems.some(
    (c) => c.state !== 'running' && c.state !== 'removing',
  );

  const handleAction = async (action) => {
    const ids = Array.from(selected);
    for (const id of ids) {
      try {
        await api.post(`/api/containers/${id}/${action}`);
      } catch (e) {
        /* continue */
      }
    }
    clear();
    load();
  };

  const handleRemove = async () => {
    const ids = Array.from(selected);
    if (!confirm(`Remove ${ids.length} container(s)?`)) return;
    for (const id of ids) {
      try {
        await api.delete(`/api/containers/${id}`);
      } catch (e) {
        alert('Error: ' + e.message);
      }
    }
    clear();
    load();
  };

  // ── Bulk "All" actions ────────────────────────────────────────
  const anyStopped = containers.some(
    (c) => c.state === 'exited' || c.state === 'stopped',
  );
  const anyRunning = containers.some((c) => c.state === 'running');

  const handleBatchAction = async (action) => {
    const allIds = containers.map((c) => c.id);
    if (allIds.length === 0) return;
    if (action !== 'start') {
      if (!confirm(`Are you sure you want to ${action} ALL ${allIds.length} containers?`)) return;
    }
    try {
      const result = await api.post('/api/containers/batch', {
        action,
        containerIds: allIds,
      });
      const s = result.success?.length || 0;
      const f = result.failed?.length || 0;
      const verb = action === 'start' ? 'Started' : action === 'stop' ? 'Stopped' : 'Restarted';
      let msg = `${verb} ${s} container${s !== 1 ? 's' : ''}`;
      if (f > 0) msg += `, ${f} failed`;
      alert(msg);
      load();
    } catch (e) {
      alert('Error: ' + e.message);
    }
  };

  const columns = [
    {
      key: 'name',
      label: 'Name',
      sortable: true,
      render: (v, row) => (
        <span
          className="mono"
          style={{ fontWeight: 500, color: 'var(--accent)', cursor: 'pointer' }}
          onClick={(e) => {
            e.stopPropagation();
            navigate('containerDetail', { id: row.id, name: row.name });
          }}
        >
          {v}
        </span>
      ),
    },
    {
      key: 'state',
      label: 'State',
      sortable: true,
      render: (v) => <StatusBadge state={v} />,
    },
    { key: 'image', label: 'Image', sortable: true },
    { key: 'status', label: 'Status', sortable: true },
    {
      key: 'health',
      label: 'Health',
      sortable: true,
      render: (v) => v ? <StatusBadge health={v} /> : '\u2014',
    },
    {
      key: 'ports',
      label: 'Ports',
      sortable: false,
      render: (ports) => renderPorts(ports),
    },
  ];

  const stateOptions = [
    {
      value: 'all',
      label: 'All',
      count: containers.length,
    },
    {
      value: 'running',
      label: 'Running',
      count: containers.filter((c) => c.state === 'running').length,
    },
    {
      value: 'stopped',
      label: 'Stopped',
      count: containers.filter(
        (c) => c.state === 'exited' || c.state === 'stopped',
      ).length,
    },
    {
      value: 'paused',
      label: 'Paused',
      count: containers.filter((c) => c.state === 'paused').length,
    },
  ];

  if (loading) return <div className="loading-center"><Spinner size="lg" /></div>;

  return (
    <div>
      <div className="section-header">
        <h1>Containers ({containers.length})</h1>
        <div className="section-header-actions">
          {anyStopped && (
            <button onClick={() => handleBatchAction('start')} title="Start all stopped containers">
              ▶ Start All
            </button>
          )}
          {anyRunning && (
            <button onClick={() => handleBatchAction('stop')} title="Stop all running containers">
              ⏹ Stop All
            </button>
          )}
          {anyRunning && (
            <button onClick={() => handleBatchAction('restart')} title="Restart all running containers">
              🔄 Restart All
            </button>
          )}
          <button onClick={load}>🔄 Refresh</button>
        </div>
      </div>

      {error && <div className="text-danger mb-16">Error: {error}</div>}

      <FilterBar
        searchQuery={searchQuery}
        onSearchChange={setSearchQuery}
        searchPlaceholder="Search containers..."
        stateFilter={stateFilter}
        onStateFilterChange={setStateFilter}
        stateOptions={stateOptions}
        filteredCount={filtered.length}
        totalCount={containers.length}
      />

      <ListToolbar
        selected={selected}
        total={containers.length}
        onClear={clear}
        filteredIds={filteredIds}
        actions={[
          { label: '▶ Start', onClick: () => handleAction('start'), disabled: !hasStopped },
          { label: '⏹ Stop', onClick: () => handleAction('stop'), disabled: !hasRunning },
          { label: '🔄 Restart', onClick: () => handleAction('restart'), disabled: !hasRunning },
          { label: '🗑 Remove', onClick: handleRemove, variant: 'danger' },
        ]}
      />

      <SortableTable
        data={filtered}
        columns={columns}
        keyField="id"
        onRowClick={(row) => navigate('containerDetail', { id: row.id, name: row.name })}
        selected={selected}
        onToggle={toggle}
        onToggleAll={toggleAll}
        allSelected={allFilteredSelected || false}
        emptyMessage="No containers"
      />
    </div>
  );
}
