import { useState, useEffect, useCallback } from 'react';
import { api } from '../api/client';
import LogViewer from '../components/LogViewer';
import StatsPanel from '../components/StatsPanel';
import JsonTree from '../components/JsonTree';
import SecretMask from '../components/SecretMask';
import ActionBar from '../components/ActionBar';
import Spinner from '../components/Spinner';
import StatusBadge from '../components/StatusBadge';

const TABS = ['info', 'logs', 'stats', 'env', 'mounts', 'network'];

export default function ContainerDetail({ id, name, navigate }) {
  const [inspect, setInspect] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [tab, setTab] = useState(() => {
    const h = window.location.hash?.replace('#', '');
    return TABS.includes(h) ? h : 'info';
  });

  const load = useCallback(async () => {
    if (!id) return;
    try {
      const data = await api.get(`/api/containers/${id}`);
      setInspect(data);
      setError(null);
    } catch (err) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  }, [id]);

  useEffect(() => { load(); }, [load]);

  const switchTab = (t) => {
    setTab(t);
    window.location.hash = t;
  };

  if (loading) return <div className="loading-center"><Spinner size="lg" /></div>;
  if (error) return <div className="text-danger">Error: {error}</div>;
  if (!inspect) return <div className="text-secondary">No data</div>;

  const displayName = (name || inspect.name || id || '').replace(/^\//, '');
  const state = inspect.state || 'unknown';

  return (
    <div>
      {/* Header */}
      <div className="section-header">
        <div>
          <button
            onClick={() => navigate('containers')}
            style={{ border: 'none', background: 'none', color: 'var(--accent)', cursor: 'pointer', padding: '0', marginRight: '8px', fontSize: '1rem' }}
          >
            ← Back
          </button>
          <h1 style={{ display: 'inline' }}>{displayName}</h1>
          <span style={{ marginLeft: '12px' }}>
            <StatusBadge state={state} />
          </span>
        </div>
        <ActionBar containerId={id} state={state} onAction={load} />
      </div>

      <div style={{ color: 'var(--text-secondary)', fontSize: '0.8rem', marginBottom: '16px' }}>
        ID: <code>{id?.substring(0, 12)}</code> &nbsp;|&nbsp;
        Image: <code>{inspect.image}</code> &nbsp;|&nbsp;
        Created: <code>{inspect.created ? new Date(inspect.created).toLocaleString() : '—'}</code>
      </div>

      {/* Tabs */}
      <div className="tabs">
        {TABS.map((t) => (
          <button
            key={t}
            className={`tab ${tab === t ? 'active' : ''}`}
            onClick={() => switchTab(t)}
          >
            {t.charAt(0).toUpperCase() + t.slice(1)}
          </button>
        ))}
      </div>

      {/* Tab Content */}
      <div>
        {tab === 'info' && (
          <div className="card">
            <JsonTree data={inspect} />
          </div>
        )}
        {tab === 'logs' && (
          <div style={{ height: 'calc(100vh - 280px)' }}>
            <LogViewer containerId={id} />
          </div>
        )}
        {tab === 'stats' && <StatsPanel containerId={id} />}
        {tab === 'env' && (
          <div className="card">
            <h2>Environment Variables</h2>
            {inspect.env && inspect.env.length > 0 ? (
              inspect.env.map((env, i) => {
                const [label, ...rest] = env.split('=');
                return (
                  <SecretMask key={i} label={label} value={rest.join('=')} />
                );
              }).reduce((acc, item, i) => {
                if (i > 0) acc.push(<div key={`s${i}`} style={{ height: '8px' }} />);
                acc.push(item);
                return acc;
              }, [])
            ) : (
              <div className="text-secondary">No environment variables</div>
            )}
          </div>
        )}
        {tab === 'mounts' && (
          <div className="card">
            <h2>Mounts</h2>
            {inspect.mounts && inspect.mounts.length > 0 ? (
              <table>
                <thead>
                  <tr>
                    <th>Type</th><th>Source</th><th>Destination</th><th>Mode</th>
                  </tr>
                </thead>
                <tbody>
                  {inspect.mounts.map((m, i) => (
                    <tr key={i}>
                      <td>{m.type}</td>
                      <td className="mono">{m.source}</td>
                      <td className="mono">{m.destination}</td>
                      <td>{m.mode || '—'}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            ) : (
              <div className="text-secondary">No mounts</div>
            )}
          </div>
        )}
        {tab === 'network' && (
          <div className="card">
            <h2>Network</h2>
            {inspect.networks && inspect.networks.length > 0 ? (
              inspect.networks.map((net) => (
                <div key={net.name} className="card mb-16" style={{ background: 'var(--bg-tertiary)' }}>
                  <h3>{net.name}</h3>
                  <div style={{ fontSize: '0.85rem', display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '4px' }}>
                    <span className="text-secondary">IP Address</span>
                    <span className="mono">{net.ipAddress || '—'}</span>
                    <span className="text-secondary">Gateway</span>
                    <span className="mono">{net.gateway || '—'}</span>
                  </div>
                </div>
              ))
            ) : (
              <div className="text-secondary">No network info</div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
