import { useState, useEffect, useCallback } from 'react';
import { api } from '../api/client';
import ContainerTable from '../components/ContainerTable';
import StatusBadge from '../components/StatusBadge';
import ActionBar from '../components/ActionBar';
import Spinner from '../components/Spinner';

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

  const onAction = useCallback(() => {
    load();
  }, [load]);

  const columns = [
    { key: 'name', label: 'Name', render: (v, row) => (
      <span style={{ fontWeight: 500, cursor: 'pointer', color: 'var(--accent)' }}
        onClick={(e) => { e.stopPropagation(); navigate('containerDetail', { id: row.id, name: row.name }); }}>
        {(v || '').replace(/^\//, '')}
      </span>
    )},
    { key: 'image', label: 'Image' },
    { key: 'state', label: 'State', render: (v) => <StatusBadge state={v} /> },
    { key: 'status', label: 'Status' },
    { key: 'ports', label: 'Ports', render: (v) => {
      if (!v || !Array.isArray(v) || v.length === 0) return <span className="text-secondary">—</span>;
      return v.map((p, i) => (
        <span key={i} className="mono" style={{ fontSize: '0.7rem', marginRight: '6px' }}>
          {p.publicPort ? `${p.publicPort}:${p.privatePort}` : p.privatePort}
        </span>
      ));
    }},
    { key: 'actions', label: 'Actions', sortable: false, render: (_, row) => (
      <ActionBar containerId={row.id} state={row.state} onAction={onAction} />
    )},
  ];

  if (loading) return <div className="loading-center"><Spinner size="lg" /></div>;

  return (
    <div>
      <div className="section-header">
        <h1>Containers ({containers.length})</h1>
        <button onClick={load} className="btn-sm">🔄 Refresh</button>
      </div>
      {error && <div className="text-danger mb-16">Error: {error}</div>}
      <ContainerTable
        columns={columns}
        data={containers}
        onRowClick={(row) => navigate('containerDetail', { id: row.id, name: row.name })}
      />
    </div>
  );
}
