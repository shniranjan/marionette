import { useState, useEffect, useCallback } from 'react';
import { api } from '../api/client';
import Modal from '../components/Modal';
import Spinner from '../components/Spinner';
import { useToast } from '../components/Toast';

export default function Templates() {
  const toast = useToast();
  const [templates, setTemplates] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [showNew, setShowNew] = useState(false);
  const [showDelete, setShowDelete] = useState(null);
  const [saving, setSaving] = useState(false);
  const [deploying, setDeploying] = useState({});

  // New form
  const [newName, setNewName] = useState('');
  const [newDescription, setNewDescription] = useState('');
  const [newImage, setNewImage] = useState('');
  const [newEnvVars, setNewEnvVars] = useState('{}');
  const [newPorts, setNewPorts] = useState('[]');
  const [newVolumes, setNewVolumes] = useState('[]');
  const [newRestartPolicy, setNewRestartPolicy] = useState('unless-stopped');
  const [newLabels, setNewLabels] = useState('{}');

  const load = useCallback(async () => {
    try {
      const data = await api.get('/api/templates');
      setTemplates(Array.isArray(data) ? data : []);
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

  const resetForm = () => {
    setNewName('');
    setNewDescription('');
    setNewImage('');
    setNewEnvVars('{}');
    setNewPorts('[]');
    setNewVolumes('[]');
    setNewRestartPolicy('unless-stopped');
    setNewLabels('{}');
  };

  const handleCreate = async () => {
    if (!newName.trim() || !newImage.trim()) return;
    setSaving(true);
    try {
      let portsParsed = '[]', envVarsParsed = '{}', volumesParsed = '[]', labelsParsed = '{}';
      try { portsParsed = JSON.stringify(JSON.parse(newPorts)); } catch {}
      try { envVarsParsed = JSON.stringify(JSON.parse(newEnvVars)); } catch {}
      try { volumesParsed = JSON.stringify(JSON.parse(newVolumes)); } catch {}
      try { labelsParsed = JSON.stringify(JSON.parse(newLabels)); } catch {}

      await api.post('/api/templates', {
        name: newName.trim(),
        description: newDescription.trim(),
        image: newImage.trim(),
        ports: portsParsed,
        envVars: envVarsParsed,
        volumes: volumesParsed,
        restartPolicy: newRestartPolicy,
        labels: labelsParsed,
      });
      resetForm();
      setShowNew(false);
      toast.success('Template created');
      load();
    } catch (err) {
      toast.error(err.message || 'Failed to create template');
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (id) => {
    try {
      await api.delete(`/api/templates/${id}`);
      toast.success('Template deleted');
      setShowDelete(null);
      load();
    } catch (err) {
      toast.error(err.message || 'Failed to delete template');
    }
  };

  const handleDeploy = async (id) => {
    setDeploying(prev => ({ ...prev, [id]: true }));
    try {
      const result = await api.post(`/api/templates/${id}/deploy`);
      toast.success(`Container ${result.containerName || result.containerId} deployed`);
    } catch (err) {
      toast.error(err.message || 'Failed to deploy template');
    } finally {
      setDeploying(prev => ({ ...prev, [id]: false }));
    }
  };

  if (loading) return <div className="loading-center"><Spinner size="lg" /></div>;

  return (
    <div>
      <div className="section-header">
        <div>
          <h1>Templates</h1>
          <div className="text-secondary" style={{ fontSize: '0.85rem' }}>
            Reusable container configurations
          </div>
        </div>
        <button className="btn btn-primary" onClick={() => { resetForm(); setShowNew(true); }}>
          + New Template
        </button>
      </div>

      {error && <div className="text-danger" style={{ marginBottom: '16px' }}>Error: {error}</div>}

      {templates.length === 0 ? (
        <div className="card text-center text-secondary" style={{ padding: '60px 20px' }}>
          <div style={{ fontSize: '3rem', marginBottom: '16px' }}>📋</div>
          <div style={{ fontSize: '1.1rem', marginBottom: '8px' }}>No templates yet</div>
          <div>Save running container configurations as templates for quick redeployment.</div>
        </div>
      ) : (
        <div style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fill, minmax(320px, 1fr))',
          gap: '16px',
        }}>
          {templates.map((t) => {
            let portsList = [];
            let envKeys = [];
            try { portsList = JSON.parse(t.ports); } catch {}
            try { envKeys = Object.keys(JSON.parse(t.envVars)); } catch {}

            return (
              <div key={t.id} className="card" style={{ display: 'flex', flexDirection: 'column' }}>
                <div style={{ marginBottom: '12px' }}>
                  <h3 style={{ margin: '0 0 4px 0', fontSize: '1.05rem' }}>{t.name}</h3>
                  {t.description && (
                    <div className="text-secondary" style={{ fontSize: '0.8rem', marginBottom: '8px' }}>
                      {t.description}
                    </div>
                  )}
                  <div style={{ display: 'flex', flexWrap: 'wrap', gap: '6px', fontSize: '0.75rem' }}>
                    <span className="badge" style={{ background: 'var(--bg-tertiary)', padding: '2px 8px', borderRadius: '4px' }}>
                      🖼 {t.image}
                    </span>
                    <span className="badge" style={{ background: 'var(--bg-tertiary)', padding: '2px 8px', borderRadius: '4px' }}>
                      🔄 {t.restartPolicy}
                    </span>
                  </div>
                </div>

                <div style={{ flex: 1, fontSize: '0.8rem', marginBottom: '12px' }}>
                  {portsList.length > 0 && (
                    <div style={{ marginBottom: '4px' }}>
                      <span className="text-secondary">Ports: </span>
                      {portsList.map((p, i) => (
                        <code key={i} style={{ marginRight: '4px' }}>
                          {p.hostPort}:{p.containerPort}
                        </code>
                      ))}
                    </div>
                  )}
                  {envKeys.length > 0 && (
                    <div>
                      <span className="text-secondary">Env: </span>
                      {envKeys.slice(0, 5).join(', ')}
                      {envKeys.length > 5 && ` +${envKeys.length - 5} more`}
                    </div>
                  )}
                </div>

                <div className="text-secondary" style={{ fontSize: '0.7rem', marginBottom: '12px' }}>
                  Created {new Date(t.createdAt).toLocaleDateString()}
                </div>

                <div style={{ display: 'flex', gap: '8px' }}>
                  <button
                    className="btn btn-primary"
                    style={{ flex: 1, fontSize: '0.8rem' }}
                    onClick={() => handleDeploy(t.id)}
                    disabled={deploying[t.id]}
                  >
                    {deploying[t.id] ? 'Deploying...' : '▶ Deploy'}
                  </button>
                  <button
                    className="btn btn-danger"
                    style={{ fontSize: '0.8rem', padding: '6px 12px' }}
                    onClick={() => setShowDelete(t.id)}
                  >
                    🗑
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* New Template Modal */}
      {showNew && (
        <Modal title="New Template" onClose={() => setShowNew(false)}>
          <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
            <div>
              <label className="text-secondary" style={{ fontSize: '0.8rem', display: 'block', marginBottom: '4px' }}>Name *</label>
              <input className="input" value={newName} onChange={e => setNewName(e.target.value)} placeholder="my-template" />
            </div>
            <div>
              <label className="text-secondary" style={{ fontSize: '0.8rem', display: 'block', marginBottom: '4px' }}>Description</label>
              <input className="input" value={newDescription} onChange={e => setNewDescription(e.target.value)} placeholder="Optional description" />
            </div>
            <div>
              <label className="text-secondary" style={{ fontSize: '0.8rem', display: 'block', marginBottom: '4px' }}>Image *</label>
              <input className="input" value={newImage} onChange={e => setNewImage(e.target.value)} placeholder="nginx:latest" />
            </div>
            <div>
              <label className="text-secondary" style={{ fontSize: '0.8rem', display: 'block', marginBottom: '4px' }}>Ports (JSON)</label>
              <textarea className="input" rows={3} value={newPorts} onChange={e => setNewPorts(e.target.value)}
                placeholder='[{"containerPort":80,"hostPort":8080}]' style={{ fontFamily: 'monospace', fontSize: '0.8rem' }} />
            </div>
            <div>
              <label className="text-secondary" style={{ fontSize: '0.8rem', display: 'block', marginBottom: '4px' }}>Env Vars (JSON)</label>
              <textarea className="input" rows={3} value={newEnvVars} onChange={e => setNewEnvVars(e.target.value)}
                placeholder='{"KEY":"value"}' style={{ fontFamily: 'monospace', fontSize: '0.8rem' }} />
            </div>
            <div>
              <label className="text-secondary" style={{ fontSize: '0.8rem', display: 'block', marginBottom: '4px' }}>Volumes (JSON)</label>
              <textarea className="input" rows={3} value={newVolumes} onChange={e => setNewVolumes(e.target.value)}
                placeholder='[{"source":"/host/path","destination":"/container/path","mode":"rw"}]' style={{ fontFamily: 'monospace', fontSize: '0.8rem' }} />
            </div>
            <div>
              <label className="text-secondary" style={{ fontSize: '0.8rem', display: 'block', marginBottom: '4px' }}>Restart Policy</label>
              <select className="input" value={newRestartPolicy} onChange={e => setNewRestartPolicy(e.target.value)}>
                <option value="unless-stopped">unless-stopped</option>
                <option value="always">always</option>
                <option value="on-failure">on-failure</option>
                <option value="no">no</option>
              </select>
            </div>
            <div>
              <label className="text-secondary" style={{ fontSize: '0.8rem', display: 'block', marginBottom: '4px' }}>Labels (JSON)</label>
              <textarea className="input" rows={2} value={newLabels} onChange={e => setNewLabels(e.target.value)}
                placeholder='{"key":"value"}' style={{ fontFamily: 'monospace', fontSize: '0.8rem' }} />
            </div>
            <button className="btn btn-primary" onClick={handleCreate} disabled={saving}>
              {saving ? 'Creating...' : 'Create Template'}
            </button>
          </div>
        </Modal>
      )}

      {/* Delete Confirmation */}
      {showDelete && (
        <Modal title="Delete Template" onClose={() => setShowDelete(null)}>
          <p>Are you sure you want to delete this template?</p>
          <div style={{ display: 'flex', gap: '8px', marginTop: '12px' }}>
            <button className="btn btn-danger" onClick={() => handleDelete(showDelete)}>Delete</button>
            <button className="btn" onClick={() => setShowDelete(null)}>Cancel</button>
          </div>
        </Modal>
      )}
    </div>
  );
}
