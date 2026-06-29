import { useState, useEffect, useCallback, useMemo } from 'react';
import { api } from '../api/client';
import LogViewer from '../components/LogViewer';
import Terminal from '../components/Terminal';
import StatsPanel from '../components/StatsPanel';
import JsonTree from '../components/JsonTree';
import SecretMask from '../components/SecretMask';
import ActionBar from '../components/ActionBar';
import Spinner from '../components/Spinner';
import StatusBadge from '../components/StatusBadge';
import Modal from '../components/Modal';
import { useToast } from '../components/Toast';

const TABS = ['info', 'logs', 'stats', 'shell', 'env', 'mounts', 'network', 'labels'];

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

  // Labels editing state
  const [editLabels, setEditLabels] = useState(null);
  const [savingLabels, setSavingLabels] = useState(false);

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

  // Initialize edit labels when switching to labels tab
  useEffect(() => {
    if (tab === 'labels' && editLabels === null) {
      const labels = inspect?.labels || inspect?.Config?.Labels || {};
      setEditLabels({ ...labels });
    }
  }, [tab, inspect, editLabels]);

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

  // ── Labels editing ──────────────────────────────────────────
  const initLabels = useCallback(() => {
    const labels = inspect?.labels || inspect?.Config?.Labels || {};
    setEditLabels({ ...labels });
  }, [inspect]);

  const handleAddLabel = () => {
    setEditLabels((prev) => ({ ...(prev || {}), '': '' }));
  };

  const handleLabelChange = (oldKey, newKey, value) => {
    setEditLabels((prev) => {
      const next = { ...prev };
      delete next[oldKey];
      if (newKey.trim()) next[newKey.trim()] = value;
      return next;
    });
  };

  const handleRemoveLabel = (key) => {
    setEditLabels((prev) => {
      const next = { ...prev };
      delete next[key];
      return next;
    });
  };

  const handleSaveLabels = async () => {
    setSavingLabels(true);
    try {
      // Filter out empty keys
      const cleaned = {};
      Object.entries(editLabels || {}).forEach(([k, v]) => {
        if (k.trim()) cleaned[k.trim()] = v;
      });
      await api.patch(`/api/containers/${id}/labels`, { labels: cleaned });
      toast.success('Labels updated');
      load(); // refresh inspect data
    } catch (err) {
      toast.error(err.message || 'Failed to save labels');
    } finally {
      setSavingLabels(false);
    }
  };

  const labelEntries = editLabels ? Object.entries(editLabels) : [];

  // Build open-in-browser link if any common web port is exposed
  const webLink = useMemo(() => {
    const WEB_PORTS = new Set([80, 443, 8080, 3000, 8000, 8443]);
    const ports = inspect.ports || [];
    const web = ports.find((p) => WEB_PORTS.has(p.privatePort));
    if (!web) return null;
    const publicPort = web.publicPort || web.privatePort;
    const protocol = web.privatePort === 443 || web.privatePort === 8443 ? 'https' : 'http';
    return `${protocol}://${window.location.hostname}:${publicPort}`;
  }, [inspect]);

  if (loading) return <div className="loading-center"><Spinner size="lg" /></div>;
  if (error) return <div className="text-danger">Error: {error}</div>;
  if (!inspect) return <div className="text-secondary">No data</div>;

  const displayName = (name || inspect.name || id || '').replace(/^\//, '');
  const state = inspect.state || 'unknown';

  // Extract stack name from Docker Compose labels
  const composeProject = inspect?.labels?.['com.docker.compose.project'] ||
                          inspect?.Config?.Labels?.['com.docker.compose.project'];
  const composeService = inspect?.labels?.['com.docker.compose.service'] ||
                          inspect?.Config?.Labels?.['com.docker.compose.service'];

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
        <ActionBar containerId={id} state={state} onAction={load} onShell={() => switchTab('shell')} />
        {webLink && (
          <a
            href={webLink}
            target="_blank"
            rel="noopener noreferrer"
            className="btn"
            style={{ marginLeft: '8px', textDecoration: 'none' }}
            title={`Open ${webLink}`}
          >
            🌐 Open in Browser
          </a>
        )}
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
        {composeProject && (
          <span>
            &nbsp;|&nbsp;
            Stack: <button
              className="btn-sm outline"
              style={{ fontFamily: 'var(--pico-font-family-monospace)', fontSize: '0.75rem', padding: '1px 6px' }}
              onClick={() => navigate('stacks')}
              title={composeService ? `Service: ${composeService}` : ''}
            >
              📚 {composeProject}
            </button>
          </span>
        )}
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
        {tab === 'shell' && (
          <div style={{ height: 'calc(100vh - 260px)' }}>
            <Terminal
              containerId={id}
              containerName={displayName}
              onClose={() => switchTab('info')}
            />
          </div>
        )}
        {tab === 'labels' && (
          <div className="card">
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '12px' }}>
              <h2 style={{ margin: 0 }}>Labels</h2>
              <div style={{ display: 'flex', gap: '8px' }}>
                <button className="btn" onClick={handleAddLabel} disabled={savingLabels}>+ Add Label</button>
                <button className="btn btn-primary" onClick={handleSaveLabels} disabled={savingLabels}>
                  {savingLabels ? 'Saving...' : '💾 Save Labels'}
                </button>
              </div>
            </div>
            {labelEntries.length > 0 ? (
              <table>
                <thead>
                  <tr>
                    <th style={{ width: '40%' }}>Key</th>
                    <th style={{ width: '50%' }}>Value</th>
                    <th style={{ width: '10%' }}></th>
                  </tr>
                </thead>
                <tbody>
                  {labelEntries.map(([key, value]) => (
                    <tr key={key}>
                      <td>
                        <input
                          className="input"
                          value={key}
                          onChange={(e) => handleLabelChange(key, e.target.value, value)}
                          placeholder="key"
                          style={{ width: '100%' }}
                          disabled={savingLabels}
                        />
                      </td>
                      <td>
                        <input
                          className="input"
                          value={value}
                          onChange={(e) => handleLabelChange(key, key, e.target.value)}
                          placeholder="value"
                          style={{ width: '100%' }}
                          disabled={savingLabels}
                        />
                      </td>
                      <td>
                        <button
                          className="btn btn-sm"
                          onClick={() => handleRemoveLabel(key)}
                          disabled={savingLabels}
                          style={{ color: 'var(--danger)', padding: '2px 8px' }}
                          title="Remove label"
                        >
                          ✕
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            ) : (
              <div className="text-secondary" style={{ padding: '16px 0' }}>
                No labels. Click "+ Add Label" to add one.
              </div>
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
