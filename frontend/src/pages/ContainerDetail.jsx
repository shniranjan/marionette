import { useState, useEffect, useCallback } from 'react';
import { api } from '../api/client';
import LogViewer from '../components/LogViewer';
import StatsPanel from '../components/StatsPanel';
import JsonTree from '../components/JsonTree';
import SecretMask from '../components/SecretMask';
import ActionBar from '../components/ActionBar';
import Spinner from '../components/Spinner';
import StatusBadge from '../components/StatusBadge';
import Modal from '../components/Modal';
import { useToast } from '../components/Toast';

const TABS = ['info', 'logs', 'stats', 'env', 'mounts', 'network'];

export default function ContainerDetail({ id, name, navigate }) {
  const toast = useToast();
  const [inspect, setInspect] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [tab, setTab] = useState(() => {
    const h = window.location.hash?.replace('#', '');
    return TABS.includes(h) ? h : 'info';
  });
  const [showSaveTemplate, setShowSaveTemplate] = useState(false);
  const [templateName, setTemplateName] = useState('');
  const [templateDesc, setTemplateDesc] = useState('');
  const [saving, setSaving] = useState(false);

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

  const handleSaveAsTemplate = async () => {
    if (!templateName.trim()) return;
    setSaving(true);
    try {
      // Convert env array ["KEY=VAL", ...] to object
      const envObj = {};
      if (Array.isArray(inspect.env)) {
        inspect.env.forEach((e) => {
          const idx = e.indexOf('=');
          if (idx > 0) envObj[e.substring(0, idx)] = e.substring(idx + 1);
        });
      }

      // Map ports from inspect format to template format
      const portsArr = (inspect.ports || []).map((p) => ({
        containerPort: p.privatePort || p.containerPort || 0,
        hostPort: p.publicPort || p.hostPort || 0,
      }));

      // Map mounts to template volumes format
      const volsArr = (inspect.mounts || []).map((m) => ({
        source: m.source || '',
        destination: m.destination || '',
        mode: m.mode || 'rw',
      }));

      await api.post('/api/templates', {
        name: templateName.trim(),
        description: templateDesc.trim(),
        image: inspect.image || '',
        ports: JSON.stringify(portsArr),
        envVars: JSON.stringify(envObj),
        volumes: JSON.stringify(volsArr),
        restartPolicy: inspect.restartPolicy || 'unless-stopped',
        labels: JSON.stringify(inspect.labels || {}),
      });
      toast.success('Template saved');
      setShowSaveTemplate(false);
      setTemplateName('');
      setTemplateDesc('');
    } catch (err) {
      toast.error(err.message || 'Failed to save template');
    } finally {
      setSaving(false);
    }
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
        <button
          className="btn"
          onClick={() => {
            setTemplateName(displayName || '');
            setTemplateDesc('');
            setShowSaveTemplate(true);
          }}
          style={{ marginLeft: '8px' }}
          title="Save as Template"
        >
          💾 Save as Template
        </button>
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

      {/* Save as Template Modal */}
      {showSaveTemplate && (
        <Modal title="Save as Template" onClose={() => setShowSaveTemplate(false)}>
          <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
            <div>
              <label className="text-secondary" style={{ fontSize: '0.8rem', display: 'block', marginBottom: '4px' }}>Template Name *</label>
              <input className="input" value={templateName} onChange={e => setTemplateName(e.target.value)} placeholder="my-template" />
            </div>
            <div>
              <label className="text-secondary" style={{ fontSize: '0.8rem', display: 'block', marginBottom: '4px' }}>Description</label>
              <input className="input" value={templateDesc} onChange={e => setTemplateDesc(e.target.value)} placeholder="Optional description" />
            </div>
            <div className="text-secondary" style={{ fontSize: '0.75rem' }}>
              Image: <code>{inspect?.image}</code> &nbsp;|&nbsp;
              Restart: <code>{inspect?.restartPolicy || 'unless-stopped'}</code>
            </div>
            <button className="btn btn-primary" onClick={handleSaveAsTemplate} disabled={saving}>
              {saving ? 'Saving...' : 'Save Template'}
            </button>
          </div>
        </Modal>
      )}
    </div>
  );
}
