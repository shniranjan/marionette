import { useState, useEffect, useCallback } from 'react';
import { api } from '../api/client';
import Modal from '../components/Modal';
import Spinner from '../components/Spinner';
import SortableTable from '../components/SortableTable';
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
  const [showEdit, setShowEdit] = useState(null);
  const [editName, setEditName] = useState('');
  const [editDescription, setEditDescription] = useState('');
  const [editImage, setEditImage] = useState('');
  const [editEnvVars, setEditEnvVars] = useState('{}');
  const [editPorts, setEditPorts] = useState('[]');
  const [editVolumes, setEditVolumes] = useState('[]');
  const [editRestartPolicy, setEditRestartPolicy] = useState('unless-stopped');
  const [editLabels, setEditLabels] = useState('{}');
  const [editSaving, setEditSaving] = useState(false);

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

  const handleEditClick = (t) => {
    setEditName(t.name || '');
    setEditDescription(t.description || '');
    setEditImage(t.image || '');
    setEditEnvVars(typeof t.envVars === 'string' ? t.envVars : JSON.stringify(t.envVars || {}));
    setEditPorts(typeof t.ports === 'string' ? t.ports : JSON.stringify(t.ports || []));
    setEditVolumes(typeof t.volumes === 'string' ? t.volumes : JSON.stringify(t.volumes || []));
    setEditRestartPolicy(t.restartPolicy || 'unless-stopped');
    setEditLabels(typeof t.labels === 'string' ? t.labels : JSON.stringify(t.labels || {}));
    setShowEdit(t.id);
  };

  const handleEditSave = async () => {
    if (!editName.trim() || !editImage.trim()) return;
    setEditSaving(true);
    try {
      let portsParsed = '[]', envVarsParsed = '{}', volumesParsed = '[]', labelsParsed = '{}';
      try { portsParsed = JSON.stringify(JSON.parse(editPorts)); } catch {}
      try { envVarsParsed = JSON.stringify(JSON.parse(editEnvVars)); } catch {}
      try { volumesParsed = JSON.stringify(JSON.parse(editVolumes)); } catch {}
      try { labelsParsed = JSON.stringify(JSON.parse(editLabels)); } catch {}

      await api.put(`/api/templates/${showEdit}`, {
        name: editName.trim(),
        description: editDescription.trim(),
        image: editImage.trim(),
        ports: portsParsed,
        envVars: envVarsParsed,
        volumes: volumesParsed,
        restartPolicy: editRestartPolicy,
        labels: labelsParsed,
      });
      setShowEdit(null);
      toast.success('Template updated');
      load();
    } catch (err) {
      toast.error(err.message || 'Failed to update template');
    } finally {
      setEditSaving(false);
    }
  };

  const columns = [
    { key: 'name', label: 'Name', sortable: true },
    { key: 'image', label: 'Image', sortable: true },
    { key: 'restartPolicy', label: 'Restart Policy', sortable: true },
    {
      key: 'ports',
      label: 'Ports',
      sortable: false,
      render: (ports) => {
        let list = [];
        try { list = JSON.parse(ports); } catch {}
        if (!list || !list.length) return <span className="text-secondary">—</span>;
        return list.map(p => `${p.hostPort || p.containerPort}:${p.containerPort}`).join(', ');
      },
      maxWidth: '160px',
    },
    {
      key: 'envVars',
      label: 'Env Vars',
      sortable: false,
      render: (envVars) => {
        try { return Object.keys(JSON.parse(envVars)).length; } catch { return 0; }
      },
      maxWidth: '80px',
    },
    {
      key: 'createdAt',
      label: 'Created',
      sortable: true,
      render: (v) => v ? new Date(v).toLocaleDateString() : '—',
    },
    {
      key: 'actions',
      label: 'Actions',
      sortable: false,
      render: (_, row) => (
        <div className="btn-group" style={{ gap: '4px' }}>
          <button
            className="btn-primary"
            style={{ fontSize: '0.7rem', padding: '2px 8px' }}
            onClick={(e) => { e.stopPropagation(); handleDeploy(row.id); }}
            disabled={deploying[row.id]}
          >
            {deploying[row.id] ? '…' : '▶ Deploy'}
          </button>
          <button
            style={{ fontSize: '0.7rem', padding: '2px 8px' }}
            onClick={(e) => { e.stopPropagation(); handleEditClick(row); }}
          >
            ✏ Edit
          </button>
          <button
            className="btn-danger"
            style={{ fontSize: '0.7rem', padding: '2px 8px' }}
            onClick={(e) => { e.stopPropagation(); setShowDelete(row.id); }}
          >
            🗑 Delete
          </button>
        </div>
      ),
    },
  ];

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

      <div style={{ overflowX: 'auto' }}>
        <SortableTable
          data={templates}
          columns={columns}
          keyField="id"
          emptyMessage="No templates yet"
        />
      </div>

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

      {/* Edit Template Modal */}
      {showEdit && (
        <Modal title="Edit Template" onClose={() => setShowEdit(null)}>
          <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
            <div>
              <label className="text-secondary" style={{ fontSize: '0.8rem', display: 'block', marginBottom: '4px' }}>Name *</label>
              <input className="input" value={editName} onChange={e => setEditName(e.target.value)} placeholder="my-template" />
            </div>
            <div>
              <label className="text-secondary" style={{ fontSize: '0.8rem', display: 'block', marginBottom: '4px' }}>Description</label>
              <input className="input" value={editDescription} onChange={e => setEditDescription(e.target.value)} placeholder="Optional description" />
            </div>
            <div>
              <label className="text-secondary" style={{ fontSize: '0.8rem', display: 'block', marginBottom: '4px' }}>Image *</label>
              <input className="input" value={editImage} onChange={e => setEditImage(e.target.value)} placeholder="nginx:latest" />
            </div>
            <div>
              <label className="text-secondary" style={{ fontSize: '0.8rem', display: 'block', marginBottom: '4px' }}>Ports (JSON)</label>
              <textarea className="input" rows={3} value={editPorts} onChange={e => setEditPorts(e.target.value)}
                placeholder='[{"containerPort":80,"hostPort":8080}]' style={{ fontFamily: 'monospace', fontSize: '0.8rem' }} />
            </div>
            <div>
              <label className="text-secondary" style={{ fontSize: '0.8rem', display: 'block', marginBottom: '4px' }}>Env Vars (JSON)</label>
              <textarea className="input" rows={3} value={editEnvVars} onChange={e => setEditEnvVars(e.target.value)}
                placeholder='{"KEY":"value"}' style={{ fontFamily: 'monospace', fontSize: '0.8rem' }} />
            </div>
            <div>
              <label className="text-secondary" style={{ fontSize: '0.8rem', display: 'block', marginBottom: '4px' }}>Volumes (JSON)</label>
              <textarea className="input" rows={3} value={editVolumes} onChange={e => setEditVolumes(e.target.value)}
                placeholder='[{"source":"/host/path","destination":"/container/path","mode":"rw"}]' style={{ fontFamily: 'monospace', fontSize: '0.8rem' }} />
            </div>
            <div>
              <label className="text-secondary" style={{ fontSize: '0.8rem', display: 'block', marginBottom: '4px' }}>Restart Policy</label>
              <select className="input" value={editRestartPolicy} onChange={e => setEditRestartPolicy(e.target.value)}>
                <option value="unless-stopped">unless-stopped</option>
                <option value="always">always</option>
                <option value="on-failure">on-failure</option>
                <option value="no">no</option>
              </select>
            </div>
            <div>
              <label className="text-secondary" style={{ fontSize: '0.8rem', display: 'block', marginBottom: '4px' }}>Labels (JSON)</label>
              <textarea className="input" rows={2} value={editLabels} onChange={e => setEditLabels(e.target.value)}
                placeholder='{"key":"value"}' style={{ fontFamily: 'monospace', fontSize: '0.8rem' }} />
            </div>
            <button className="btn btn-primary" onClick={handleEditSave} disabled={editSaving}>
              {editSaving ? 'Saving...' : 'Save Changes'}
            </button>
          </div>
        </Modal>
      )}
    </div>
  );
}
