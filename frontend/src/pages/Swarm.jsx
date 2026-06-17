import { useState, useEffect, useCallback } from 'react';
import { api } from '../api/client';
import { useToast } from '../components/Toast';
import Modal from '../components/Modal';
import Spinner from '../components/Spinner';
import StatusBadge from '../components/StatusBadge';
import SwarmVisualizer from '../components/SwarmVisualizer';

/* ── helpers ─────────────────────────────────────────────── */

function formatBytes(bytes) {
  if (!bytes || bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}

function roleBadge(role) {
  const isManager = (role || '').toLowerCase() === 'manager';
  return (
    <span style={{
      display: 'inline-block',
      padding: '2px 10px',
      borderRadius: '12px',
      fontSize: '0.75rem',
      fontWeight: 600,
      color: isManager ? 'var(--accent)' : 'var(--text-secondary)',
      background: isManager ? 'var(--bg-tertiary)' : 'var(--bg-secondary)',
      border: `1px solid ${isManager ? 'var(--accent)' : 'var(--border)'}`,
      textTransform: 'capitalize',
    }}>
      {isManager ? '👑 ' : ''}{role || 'worker'}
    </span>
  );
}

function availabilityBadge(avail) {
  const a = (avail || '').toLowerCase();
  const colors = {
    active: { c: 'var(--green)', bg: 'var(--green-dim)' },
    pause: { c: 'var(--yellow)', bg: 'var(--yellow-dim)' },
    drain: { c: 'var(--red)', bg: 'var(--red-dim)' },
  };
  const s = colors[a] || { c: 'var(--text-secondary)', bg: 'var(--bg-tertiary)' };
  return (
    <span style={{
      display: 'inline-block',
      padding: '2px 8px',
      borderRadius: '10px',
      fontSize: '0.7rem',
      fontWeight: 600,
      color: s.c,
      background: s.bg,
      textTransform: 'capitalize',
    }}>
      {avail || 'active'}
    </span>
  );
}

/* ── main page ───────────────────────────────────────────── */

export default function Swarm({ navigate }) {
  const toast = useToast();
  const [tab, setTab] = useState('nodes');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);

  const [swarm, setSwarm] = useState(null);
  const [nodes, setNodes] = useState([]);
  const [services, setServices] = useState([]);
  const [tasks, setTasks] = useState([]);
  const [secrets, setSecrets] = useState([]);
  const [configs, setConfigs] = useState([]);

  /* ── modals ── */
  const [showInit, setShowInit] = useState(false);
  const [showJoin, setShowJoin] = useState(false);
  const [showCreateService, setShowCreateService] = useState(false);
  const [showServiceDetail, setShowServiceDetail] = useState(null);
  const [showCreateSecret, setShowCreateSecret] = useState(false);
  const [showCreateConfig, setShowCreateConfig] = useState(false);
  const [confirmAction, setConfirmAction] = useState(null);

  /* ── data loading ── */

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const [sw, nd, sv, se, cf] = await Promise.all([
        api.get('/swarm').catch(() => null),
        api.get('/swarm/nodes').catch(() => []),
        api.get('/swarm/services').catch(() => []),
        api.get('/swarm/secrets').catch(() => []),
        api.get('/swarm/configs').catch(() => []),
      ]);
      // Validate swarm: error responses from Docker (like "not a swarm manager")
      // come back as objects with .message, not actual swarm data with .ID
      const swData = (sw && sw.ID) ? sw : null;
      const ndData = Array.isArray(nd) ? nd : (Array.isArray(nd?.nodes) ? nd.nodes : []);
      const svData = Array.isArray(sv) ? sv : (Array.isArray(sv?.services) ? sv.services : []);
      const seData = Array.isArray(se) ? se : (Array.isArray(se?.secrets) ? se.secrets : []);
      const cfData = Array.isArray(cf) ? cf : (Array.isArray(cf?.configs) ? cf.configs : []);
      setSwarm(swData);
      setNodes(ndData);
      setServices(svData);
      setSecrets(seData);
      setConfigs(cfData);
      setError(null);

      if (svData.length > 0) {
        try {
          const allTasks = await api.get('/swarm/tasks');
          setTasks(Array.isArray(allTasks) ? allTasks : (Array.isArray(allTasks?.tasks) ? allTasks.tasks : []));
        } catch { /* tasks are optional */ }
      }
    } catch (err) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  /* ── actions ── */

  const doAction = useCallback(async (actionFn, okMsg) => {
    try {
      await actionFn();
      if (okMsg) toast(okMsg, 'success');
      await load();
      setConfirmAction(null);
    } catch (err) {
      toast(`Error: ${err.message}`, 'error');
    }
  }, [toast, load]);

  /* ── swarm init / join / leave ── */

  const handleInit = async (e) => {
    e.preventDefault();
    const fd = new FormData(e.target);
    await doAction(
      () => api.post('/swarm/init', {
        advertise_addr: fd.get('advertise_addr') || undefined,
        listen_addr: fd.get('listen_addr') || undefined,
        force_new_cluster: fd.get('force_new_cluster') === 'on',
      }),
      'Swarm initialized'
    );
    setShowInit(false);
  };

  const handleJoin = async (e) => {
    e.preventDefault();
    const fd = new FormData(e.target);
    await doAction(
      () => api.post('/swarm/join', {
        join_token: fd.get('join_token'),
        listen_addr: fd.get('listen_addr') || undefined,
        remote_addrs: fd.get('remote_addrs') ? fd.get('remote_addrs').split(',').map(s => s.trim()) : undefined,
      }),
      'Joined swarm'
    );
    setShowJoin(false);
  };

  const handleLeave = () => {
    setConfirmAction({
      title: 'Leave Swarm',
      message: 'Are you sure you want to leave the Swarm? This node will be removed from the cluster.',
      onConfirm: () => doAction(() => api.post('/swarm/leave', { force: true }), 'Left swarm'),
    });
  };

  /* ── node actions ── */

  const handleNodeAction = (node, action) => {
    const id = node.ID || node.id;
    const name = node.Description?.Hostname || node.hostname || id;
    setConfirmAction({
      title: `${action} Node`,
      message: `${action} "${name}"?`,
      onConfirm: async () => {
        try {
          if (action === 'Remove') {
            await api.delete(`/swarm/nodes/${id}`);
            toast(`Node ${name} removed`, 'success');
          } else if (action === 'Promote') {
            await api.patch(`/swarm/nodes/${id}`, { role: 'manager' });
            toast(`Node ${name} promoted`, 'success');
          } else if (action === 'Demote') {
            await api.patch(`/swarm/nodes/${id}`, { role: 'worker' });
            toast(`Node ${name} demoted`, 'success');
          } else if (action === 'Pause') {
            await api.patch(`/swarm/nodes/${id}`, { availability: 'pause' });
            toast(`Node ${name} paused`, 'success');
          } else if (action === 'Drain') {
            await api.patch(`/swarm/nodes/${id}`, { availability: 'drain' });
            toast(`Node ${name} drained`, 'success');
          } else if (action === 'Activate') {
            await api.patch(`/swarm/nodes/${id}`, { availability: 'active' });
            toast(`Node ${name} activated`, 'success');
          }
          await load();
        } catch (err) {
          toast(`Error: ${err.message}`, 'error');
        }
        setConfirmAction(null);
      },
    });
  };

  /* ── service actions ── */

  const handleCreateService = async (e) => {
    e.preventDefault();
    const fd = new FormData(e.target);
    const portsStr = fd.get('ports');
    const envStr = fd.get('env');
    const ports = portsStr ? portsStr.split(',').map(p => p.trim()).filter(Boolean) : [];
    const env = envStr ? envStr.split('\n').filter(Boolean).map(line => {
      const idx = line.indexOf('=');
      return idx > 0 ? `${line.slice(0, idx)}=${line.slice(idx + 1)}` : line;
    }) : [];

    await doAction(
      () => api.post('/swarm/services/create', {
        name: fd.get('name'),
        image: fd.get('image'),
        ports,
        replicas: parseInt(fd.get('replicas') || '1', 10),
        env,
      }),
      `Service "${fd.get('name')}" created`
    );
    setShowCreateService(false);
  };

  const handleServiceAction = (svc, action) => {
    const id = svc.ID || svc.id;
    const name = svc.Spec?.Name || svc.name || id;
    if (action === 'rollback') {
      setConfirmAction({
        title: 'Rollback Service',
        message: `Rollback "${name}" to the previous version?`,
        onConfirm: () => doAction(() => api.post(`/swarm/services/${id}/rollback`), `Service "${name}" rolling back`),
      });
    } else if (action === 'remove') {
      setConfirmAction({
        title: 'Remove Service',
        message: `Remove service "${name}"? This cannot be undone.`,
        onConfirm: () => doAction(() => api.delete(`/swarm/services/${id}`), `Service "${name}" removed`),
      });
    }
  };

  const handleScaleService = async (svc, replicas) => {
    const id = svc.ID || svc.id;
    const name = svc.Spec?.Name || svc.name || id;
    await doAction(
      () => api.patch(`/swarm/services/${id}`, { replicas }),
      `Service "${name}" scaled to ${replicas} replicas`
    );
  };

  const handleUpdateImage = async (svc, newImage) => {
    if (!newImage) return;
    const id = svc.ID || svc.id;
    const name = svc.Spec?.Name || svc.name || id;
    await doAction(
      () => api.patch(`/swarm/services/${id}`, { image: newImage }),
      `Service "${name}" image updated`
    );
  };

  /* ── service detail ── */

  const openServiceDetail = useCallback(async (svc) => {
    const id = svc.ID || svc.id;
    try {
      const [detail, svcTasks, logs] = await Promise.all([
        api.get(`/swarm/services/${id}`).catch(() => svc),
        api.get(`/swarm/tasks?service=${id}`).catch(() => []),
        api.get(`/swarm/services/${id}/logs`).catch(() => ''),
      ]);
      setShowServiceDetail({
        service: detail || svc,
        tasks: Array.isArray(svcTasks) ? svcTasks : (svcTasks?.tasks || []),
        logs: typeof logs === 'string' ? logs : '',
      });
    } catch {
      setShowServiceDetail({ service: svc, tasks: [], logs: '' });
    }
  }, []);

  /* ── secret / config actions ── */

  const handleCreateSecret = async (e) => {
    e.preventDefault();
    const fd = new FormData(e.target);
    await doAction(
      () => api.post('/swarm/secrets/create', { name: fd.get('name'), data: fd.get('data') }),
      `Secret "${fd.get('name')}" created`
    );
    setShowCreateSecret(false);
  };

  const handleCreateConfig = async (e) => {
    e.preventDefault();
    const fd = new FormData(e.target);
    await doAction(
      () => api.post('/swarm/configs/create', { name: fd.get('name'), data: fd.get('data') }),
      `Config "${fd.get('name')}" created`
    );
    setShowCreateConfig(false);
  };

  const handleDeleteSecret = (secret) => {
    const id = secret.ID || secret.id;
    const name = secret.Spec?.Name || secret.name || id;
    setConfirmAction({
      title: 'Delete Secret',
      message: `Delete secret "${name}"?`,
      onConfirm: () => doAction(() => api.delete(`/swarm/secrets/${id}`), `Secret "${name}" deleted`),
    });
  };

  const handleDeleteConfig = (config) => {
    const id = config.ID || config.id;
    const name = config.Spec?.Name || config.name || id;
    setConfirmAction({
      title: 'Delete Config',
      message: `Delete config "${name}"?`,
      onConfirm: () => doAction(() => api.delete(`/swarm/configs/${id}`), `Config "${name}" deleted`),
    });
  };

  /* ── render ── */

  if (loading) return <div className="loading-center"><Spinner size="lg" /></div>;

  const hasSwarm = !!swarm;
  const managerCount = nodes.filter(n => (n.Spec?.Role || n.role || '').toLowerCase() === 'manager').length;
  const workerCount = nodes.length - managerCount;

  return (
    <div>
      {/* Header */}
      <div className="section-header">
        <h1>
          🐝 Swarm
          {hasSwarm && (
            <span style={{ fontSize: '0.85rem', fontWeight: 400, color: 'var(--text-secondary)', marginLeft: '12px' }}>
              {nodes.length} nodes ({managerCount} managers, {workerCount} workers) · {services.length} services
            </span>
          )}
        </h1>
        <div className="btn-group">
          <button onClick={load} className="btn-sm">🔄 Refresh</button>
          {!hasSwarm ? (
            <>
              <button className="btn-primary btn-sm" onClick={() => setShowInit(true)}>⚡ Init Swarm</button>
              <button className="btn-sm" onClick={() => setShowJoin(true)}>🔗 Join Swarm</button>
            </>
          ) : (
            <button className="btn-danger btn-sm" onClick={handleLeave}>🚪 Leave Swarm</button>
          )}
        </div>
      </div>

      {error && <div className="text-danger mb-16">Error: {error}</div>}

      {/* Tabs */}
      <div className="tabs">
        {['nodes', 'services', 'secrets-configs', 'visualizer'].map((t) => (
          <button
            key={t}
            className={`tab${tab === t ? ' active' : ''}`}
            onClick={() => setTab(t)}
          >
            {t === 'nodes' && '🖥 Nodes'}
            {t === 'services' && '📦 Services'}
            {t === 'secrets-configs' && '🔐 Secrets & Configs'}
            {t === 'visualizer' && '🗺 Visualizer'}
          </button>
        ))}
      </div>

      {/* ── Nodes Tab ── */}
      {tab === 'nodes' && (
        <div>
          {nodes.length === 0 ? (
            <div className="card" style={{ textAlign: 'center', padding: '48px' }}>
              <div style={{ fontSize: '2rem', marginBottom: '8px' }}>🐝</div>
              <h3>No Nodes Found</h3>
              <p className="text-secondary">Initialize a Swarm or join one to see nodes.</p>
            </div>
          ) : (
            <div className="table-wrapper">
              <table>
                <thead>
                  <tr>
                    <th>Hostname</th>
                    <th>Role</th>
                    <th>Status</th>
                    <th>Availability</th>
                    <th>Engine</th>
                    <th>CPU / RAM</th>
                    <th>Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {nodes.map((node) => {
                    const id = node.ID || node.id;
                    const hostname = node.Description?.Hostname || node.hostname || '?';
                    const role = (node.Spec?.Role || node.role || 'worker').toLowerCase();
                    const status = node.Status?.State || node.status || 'unknown';
                    const availability = node.Spec?.Availability || node.availability || 'active';
                    const engine = node.Description?.Engine?.EngineVersion || node.engine_version || '—';
                    const cpu = node.Description?.Resources?.NanoCPUs
                      ? Math.round(node.Description.Resources.NanoCPUs / 1e9)
                      : (node.cpu_count || '—');
                    const mem = node.Description?.Resources?.MemoryBytes || node.memory_bytes || 0;

                    return (
                      <tr key={id}>
                        <td>
                          <span style={{ fontWeight: 500, color: 'var(--text-primary)' }}>
                            {hostname}
                          </span>
                        </td>
                        <td>{roleBadge(role)}</td>
                        <td><StatusBadge state={status} /></td>
                        <td>{availabilityBadge(availability)}</td>
                        <td className="mono" style={{ fontSize: '0.75rem' }}>{engine}</td>
                        <td className="mono" style={{ fontSize: '0.75rem' }}>
                          {cpu} CPU / {mem ? formatBytes(mem) : '—'}
                        </td>
                        <td>
                          <div className="btn-group">
                            {role === 'worker' ? (
                              <button className="btn-sm" onClick={() => handleNodeAction(node, 'Promote')}>⬆ Promote</button>
                            ) : (
                              <button className="btn-sm" onClick={() => handleNodeAction(node, 'Demote')}>⬇ Demote</button>
                            )}
                            {availability === 'active' ? (
                              <button className="btn-sm btn-warning" onClick={() => handleNodeAction(node, 'Pause')}>⏸ Pause</button>
                            ) : (
                              <button className="btn-sm btn-success" onClick={() => handleNodeAction(node, 'Activate')}>▶ Activate</button>
                            )}
                            <button className="btn-sm btn-danger" onClick={() => handleNodeAction(node, 'Drain')}>🚫 Drain</button>
                            <button className="btn-sm btn-danger" onClick={() => handleNodeAction(node, 'Remove')}>✕ Remove</button>
                          </div>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}
        </div>
      )}

      {/* ── Services Tab ── */}
      {tab === 'services' && (
        <div>
          <div style={{ marginBottom: '12px', display: 'flex', justifyContent: 'flex-end' }}>
            <button className="btn-primary" onClick={() => setShowCreateService(true)}>＋ Create Service</button>
          </div>
          {services.length === 0 ? (
            <div className="card" style={{ textAlign: 'center', padding: '48px' }}>
              <h3>No Services</h3>
              <p className="text-secondary">Create a service to get started.</p>
            </div>
          ) : (
            <div className="table-wrapper">
              <table>
                <thead>
                  <tr>
                    <th>Name</th>
                    <th>Image</th>
                    <th>Replicas</th>
                    <th>Ports</th>
                    <th>Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {services.map((svc) => {
                    const id = svc.ID || svc.id;
                    const name = svc.Spec?.Name || svc.name || '?';
                    const image = (svc.Spec?.TaskTemplate?.ContainerSpec?.Image || svc.image || '—').split('@')[0];
                    const mode = svc.Spec?.Mode || {};
                    const desired = mode.Replicated?.Replicas ?? (svc.replicas ?? 0);
                    const running = (svc.ServiceStatus?.RunningTasks || svc.running_tasks) ?? desired;
                    const ports = (svc.Spec?.EndpointSpec?.Ports || svc.ports || []).map(p =>
                      p.PublishedPort ? `${p.PublishedPort}:${p.TargetPort || p.PublishedPort}` : (p.TargetPort || '')
                    ).filter(Boolean);

                    return (
                      <tr key={id}>
                        <td>
                          <span
                            style={{ fontWeight: 500, color: 'var(--accent)', cursor: 'pointer' }}
                            onClick={() => openServiceDetail(svc)}
                          >
                            {name}
                          </span>
                        </td>
                        <td className="mono" style={{ fontSize: '0.75rem' }}>{image}</td>
                        <td>
                          <span style={{
                            color: running === desired ? 'var(--green)' : 'var(--yellow)',
                            fontWeight: 600,
                          }}>
                            {running}/{desired}
                          </span>
                        </td>
                        <td className="mono" style={{ fontSize: '0.7rem' }}>
                          {ports.length > 0 ? ports.join(', ') : <span className="text-secondary">—</span>}
                        </td>
                        <td>
                          <div className="btn-group">
                            <ScaleControl svc={svc} onScale={replicas => handleScaleService(svc, replicas)} />
                            <UpdateImageControl svc={svc} onUpdate={img => handleUpdateImage(svc, img)} />
                            <button className="btn-sm btn-warning" onClick={() => handleServiceAction(svc, 'rollback')}>↩ Rollback</button>
                            <button className="btn-sm btn-danger" onClick={() => handleServiceAction(svc, 'remove')}>✕ Remove</button>
                          </div>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}
        </div>
      )}

      {/* ── Secrets & Configs Tab ── */}
      {tab === 'secrets-configs' && (
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '24px' }}>
          {/* Secrets */}
          <div>
            <div className="section-header">
              <h2>🔑 Secrets</h2>
              <button className="btn-primary btn-sm" onClick={() => setShowCreateSecret(true)}>＋ Create</button>
            </div>
            {secrets.length === 0 ? (
              <div className="card" style={{ textAlign: 'center', padding: '32px' }}>
                <p className="text-secondary">No secrets</p>
              </div>
            ) : (
              <div className="table-wrapper">
                <table>
                  <thead>
                    <tr>
                      <th>Name</th>
                      <th>Created</th>
                      <th>Actions</th>
                    </tr>
                  </thead>
                  <tbody>
                    {secrets.map((s) => {
                      const id = s.ID || s.id;
                      const name = s.Spec?.Name || s.name || '?';
                      const created = s.CreatedAt || s.created_at || '—';
                      return (
                        <tr key={id}>
                          <td>
                            <span className="mono" style={{ fontSize: '0.8rem' }}>{name}</span>
                          </td>
                          <td className="text-secondary" style={{ fontSize: '0.75rem' }}>{new Date(created).toLocaleString()}</td>
                          <td>
                            <button className="btn-sm btn-danger" onClick={() => handleDeleteSecret(s)}>✕ Delete</button>
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            )}
          </div>

          {/* Configs */}
          <div>
            <div className="section-header">
              <h2>📋 Configs</h2>
              <button className="btn-primary btn-sm" onClick={() => setShowCreateConfig(true)}>＋ Create</button>
            </div>
            {configs.length === 0 ? (
              <div className="card" style={{ textAlign: 'center', padding: '32px' }}>
                <p className="text-secondary">No configs</p>
              </div>
            ) : (
              <div className="table-wrapper">
                <table>
                  <thead>
                    <tr>
                      <th>Name</th>
                      <th>Created</th>
                      <th>Actions</th>
                    </tr>
                  </thead>
                  <tbody>
                    {configs.map((c) => {
                      const id = c.ID || c.id;
                      const name = c.Spec?.Name || c.name || '?';
                      const created = c.CreatedAt || c.created_at || '—';
                      return (
                        <tr key={id}>
                          <td>
                            <span className="mono" style={{ fontSize: '0.8rem' }}>{name}</span>
                          </td>
                          <td className="text-secondary" style={{ fontSize: '0.75rem' }}>{new Date(created).toLocaleString()}</td>
                          <td>
                            <button className="btn-sm btn-danger" onClick={() => handleDeleteConfig(c)}>✕ Delete</button>
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            )}
          </div>
        </div>
      )}

      {/* ── Visualizer Tab ── */}
      {tab === 'visualizer' && (
        <SwarmVisualizer
          swarm={swarm}
          nodes={nodes}
          services={services}
          tasks={tasks}
        />
      )}

      {/* ── Modals ── */}

      {/* Init Swarm Modal */}
      {showInit && (
        <Modal title="⚡ Initialize Swarm" onClose={() => setShowInit(false)}
          footer={<>
            <button onClick={() => setShowInit(false)}>Cancel</button>
            <button className="btn-primary" type="submit" form="init-form">Initialize</button>
          </>}
        >
          <form id="init-form" onSubmit={handleInit}>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
              <label>
                <span className="text-secondary" style={{ display: 'block', marginBottom: '4px' }}>Advertise Address</span>
                <input name="advertise_addr" placeholder="e.g. 192.168.1.10:2377" style={{ width: '100%' }} />
              </label>
              <label>
                <span className="text-secondary" style={{ display: 'block', marginBottom: '4px' }}>Listen Address</span>
                <input name="listen_addr" placeholder="0.0.0.0:2377" style={{ width: '100%' }} />
              </label>
              <label style={{ display: 'flex', alignItems: 'center', gap: '8px', fontSize: '0.85rem' }}>
                <input type="checkbox" name="force_new_cluster" />
                Force new cluster
              </label>
            </div>
          </form>
        </Modal>
      )}

      {/* Join Swarm Modal */}
      {showJoin && (
        <Modal title="🔗 Join Swarm" onClose={() => setShowJoin(false)}
          footer={<>
            <button onClick={() => setShowJoin(false)}>Cancel</button>
            <button className="btn-primary" type="submit" form="join-form">Join</button>
          </>}
        >
          <form id="join-form" onSubmit={handleJoin}>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
              <label>
                <span className="text-secondary" style={{ display: 'block', marginBottom: '4px' }}>Join Token *</span>
                <input name="join_token" required placeholder="SWMTKN-1-..." style={{ width: '100%' }} />
              </label>
              <label>
                <span className="text-secondary" style={{ display: 'block', marginBottom: '4px' }}>Listen Address</span>
                <input name="listen_addr" placeholder="0.0.0.0:2377" style={{ width: '100%' }} />
              </label>
              <label>
                <span className="text-secondary" style={{ display: 'block', marginBottom: '4px' }}>Remote Addresses</span>
                <input name="remote_addrs" placeholder="192.168.1.10:2377, 192.168.1.11:2377" style={{ width: '100%' }} />
              </label>
            </div>
          </form>
        </Modal>
      )}

      {/* Create Service Modal */}
      {showCreateService && (
        <Modal title="＋ Create Service" onClose={() => setShowCreateService(false)}
          footer={<>
            <button onClick={() => setShowCreateService(false)}>Cancel</button>
            <button className="btn-primary" type="submit" form="create-service-form">Create</button>
          </>}
        >
          <form id="create-service-form" onSubmit={handleCreateService}>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
              <label>
                <span className="text-secondary" style={{ display: 'block', marginBottom: '4px' }}>Name *</span>
                <input name="name" required placeholder="my-service" style={{ width: '100%' }} />
              </label>
              <label>
                <span className="text-secondary" style={{ display: 'block', marginBottom: '4px' }}>Image *</span>
                <input name="image" required placeholder="nginx:latest" style={{ width: '100%' }} />
              </label>
              <label>
                <span className="text-secondary" style={{ display: 'block', marginBottom: '4px' }}>Ports (comma-separated)</span>
                <input name="ports" placeholder="80:80, 443:443" style={{ width: '100%' }} />
              </label>
              <label>
                <span className="text-secondary" style={{ display: 'block', marginBottom: '4px' }}>Replicas</span>
                <input name="replicas" type="number" defaultValue="1" min="0" style={{ width: '100%' }} />
              </label>
              <label>
                <span className="text-secondary" style={{ display: 'block', marginBottom: '4px' }}>Environment Variables (KEY=VALUE, one per line)</span>
                <textarea name="env" rows="4" placeholder="NODE_ENV=production&#10;DEBUG=false" style={{ width: '100%', resize: 'vertical' }} />
              </label>
            </div>
          </form>
        </Modal>
      )}

      {/* Service Detail Modal */}
      {showServiceDetail && (
        <Modal
          title={`📦 ${showServiceDetail.service.Spec?.Name || showServiceDetail.service.name || 'Service Detail'}`}
          onClose={() => setShowServiceDetail(null)}
          footer={<>
            <button onClick={() => setShowServiceDetail(null)}>Close</button>
            <button className="btn-warning" onClick={() => {
              handleServiceAction(showServiceDetail.service, 'rollback');
              setShowServiceDetail(null);
            }}>↩ Rollback</button>
            <button className="btn-danger" onClick={() => {
              handleServiceAction(showServiceDetail.service, 'remove');
              setShowServiceDetail(null);
            }}>✕ Remove</button>
          </>}
        >
          <div style={{ fontSize: '0.85rem' }}>
            <DetailRow label="Image" value={(showServiceDetail.service.Spec?.TaskTemplate?.ContainerSpec?.Image || showServiceDetail.service.image || '—').split('@')[0]} mono />
            <DetailRow label="Mode" value={showServiceDetail.service.Spec?.Mode?.Replicated ? 'Replicated' : (showServiceDetail.service.Spec?.Mode?.Global ? 'Global' : showServiceDetail.service.mode || '—')} />
            <DetailRow label="Replicas" value={
              (() => {
                const d = showServiceDetail.service.Spec?.Mode?.Replicated?.Replicas ?? showServiceDetail.service.replicas ?? 0;
                const r = showServiceDetail.service.ServiceStatus?.RunningTasks ?? showServiceDetail.service.running_tasks ?? d;
                return `${r} / ${d} running`;
              })()
            } />

            {/* Tasks */}
            <h4 style={{ marginTop: '16px', marginBottom: '8px' }}>Tasks</h4>
            {showServiceDetail.tasks.length === 0 ? (
              <p className="text-secondary">No tasks found</p>
            ) : (
              <div className="table-wrapper">
                <table style={{ fontSize: '0.75rem' }}>
                  <thead>
                    <tr>
                      <th>Task ID</th>
                      <th>Node</th>
                      <th>Status</th>
                      <th>Desired</th>
                    </tr>
                  </thead>
                  <tbody>
                    {showServiceDetail.tasks.map((t) => (
                      <tr key={t.ID || t.id}>
                        <td className="mono">{(t.ID || t.id || '').slice(0, 12)}</td>
                        <td className="mono">{t.NodeID || t.node || '—'}</td>
                        <td><StatusBadge state={t.Status?.State || t.status || 'pending'} /></td>
                        <td>{t.DesiredState || t.desired_state || '—'}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}

            {/* Logs */}
            <h4 style={{ marginTop: '16px', marginBottom: '8px' }}>Logs</h4>
            {showServiceDetail.logs ? (
              <pre className="log-output" style={{
                maxHeight: '200px',
                overflow: 'auto',
                background: 'var(--bg-primary)',
                padding: '12px',
                borderRadius: '6px',
                border: '1px solid var(--border)',
              }}>
                {showServiceDetail.logs}
              </pre>
            ) : (
              <p className="text-secondary">No logs available</p>
            )}
          </div>
        </Modal>
      )}

      {/* Create Secret Modal */}
      {showCreateSecret && (
        <Modal title="＋ Create Secret" onClose={() => setShowCreateSecret(false)}
          footer={<>
            <button onClick={() => setShowCreateSecret(false)}>Cancel</button>
            <button className="btn-primary" type="submit" form="create-secret-form">Create</button>
          </>}
        >
          <form id="create-secret-form" onSubmit={handleCreateSecret}>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
              <label>
                <span className="text-secondary" style={{ display: 'block', marginBottom: '4px' }}>Name *</span>
                <input name="name" required placeholder="my-secret" style={{ width: '100%' }} />
              </label>
              <label>
                <span className="text-secondary" style={{ display: 'block', marginBottom: '4px' }}>Data *</span>
                <textarea name="data" required rows="4" placeholder="Secret value..." style={{ width: '100%', resize: 'vertical' }} />
              </label>
            </div>
          </form>
        </Modal>
      )}

      {/* Create Config Modal */}
      {showCreateConfig && (
        <Modal title="＋ Create Config" onClose={() => setShowCreateConfig(false)}
          footer={<>
            <button onClick={() => setShowCreateConfig(false)}>Cancel</button>
            <button className="btn-primary" type="submit" form="create-config-form">Create</button>
          </>}
        >
          <form id="create-config-form" onSubmit={handleCreateConfig}>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
              <label>
                <span className="text-secondary" style={{ display: 'block', marginBottom: '4px' }}>Name *</span>
                <input name="name" required placeholder="my-config" style={{ width: '100%' }} />
              </label>
              <label>
                <span className="text-secondary" style={{ display: 'block', marginBottom: '4px' }}>Data *</span>
                <textarea name="data" required rows="4" placeholder="Config content..." style={{ width: '100%', resize: 'vertical' }} />
              </label>
            </div>
          </form>
        </Modal>
      )}

      {/* Confirm Action Modal */}
      {confirmAction && (
        <Modal title={confirmAction.title} onClose={() => setConfirmAction(null)}
          footer={<>
            <button onClick={() => setConfirmAction(null)}>Cancel</button>
            <button className="btn-danger" onClick={confirmAction.onConfirm}>Confirm</button>
          </>}
        >
          <p>{confirmAction.message}</p>
        </Modal>
      )}
    </div>
  );
}

/* ── sub-components ──────────────────────────────────────── */

function DetailRow({ label, value, mono }) {
  return (
    <div style={{ display: 'flex', marginBottom: '6px' }}>
      <span className="text-secondary" style={{ minWidth: '100px' }}>{label}</span>
      <span className={mono ? 'mono' : ''} style={{ fontSize: mono ? '0.75rem' : undefined }}>{value}</span>
    </div>
  );
}

function ScaleControl({ svc, onScale }) {
  const [open, setOpen] = useState(false);
  const current = svc.Spec?.Mode?.Replicated?.Replicas ?? (svc.replicas ?? 1);

  if (!open) {
    return <button className="btn-sm" onClick={() => setOpen(true)}>📏 Scale</button>;
  }

  return (
    <span style={{ display: 'inline-flex', gap: '4px', alignItems: 'center' }}>
      <input
        type="number"
        defaultValue={current}
        min="0"
        style={{ width: '50px', padding: '2px 6px', fontSize: '0.75rem' }}
        onKeyDown={(e) => {
          if (e.key === 'Enter') { onScale(parseInt(e.target.value, 10)); setOpen(false); }
          if (e.key === 'Escape') setOpen(false);
        }}
        autoFocus
      />
      <button className="btn-sm btn-success" onClick={(e) => {
        const input = e.target.parentElement.querySelector('input');
        onScale(parseInt(input.value, 10));
        setOpen(false);
      }}>✓</button>
      <button className="btn-sm" onClick={() => setOpen(false)}>✕</button>
    </span>
  );
}

function UpdateImageControl({ svc, onUpdate }) {
  const [open, setOpen] = useState(false);
  const current = (svc.Spec?.TaskTemplate?.ContainerSpec?.Image || svc.image || '').split('@')[0];

  if (!open) {
    return <button className="btn-sm" onClick={() => setOpen(true)}>🔄 Update</button>;
  }

  return (
    <span style={{ display: 'inline-flex', gap: '4px', alignItems: 'center' }}>
      <input
        defaultValue={current}
        style={{ width: '120px', padding: '2px 6px', fontSize: '0.75rem' }}
        onKeyDown={(e) => {
          if (e.key === 'Enter') { onUpdate(e.target.value); setOpen(false); }
          if (e.key === 'Escape') setOpen(false);
        }}
        autoFocus
        placeholder="image:tag"
      />
      <button className="btn-sm btn-success" onClick={(e) => {
        const input = e.target.parentElement.querySelector('input');
        onUpdate(input.value);
        setOpen(false);
      }}>✓</button>
      <button className="btn-sm" onClick={() => setOpen(false)}>✕</button>
    </span>
  );
}
