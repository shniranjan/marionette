import { useState, useEffect, useCallback } from 'react';
import { api } from '../api/client';
import Modal from '../components/Modal';
import YamlEditor from '../components/YamlEditor';
import Spinner from '../components/Spinner';

const DEFAULT_YML = `# docker-compose.yml
services:
  web:
    image: nginx:latest
    ports:
      - "8080:80"
`;

function StatusBadge({ status }) {
  const s = (status || 'unknown').toLowerCase();
  const colors = {
    running:  { bg: '#1a3a1a', fg: '#4caf50', label: 'Running' },
    stopped:  { bg: '#2a2a2a', fg: '#999',    label: 'Stopped' },
    unknown:  { bg: '#2a2a15', fg: '#ccc',    label: 'Unknown' },
  };
  const c = colors[s] || colors.unknown;
  return (
    <span style={{
      display: 'inline-block',
      padding: '1px 10px',
      borderRadius: '10px',
      fontSize: '0.7rem',
      fontWeight: 600,
      background: c.bg,
      color: c.fg,
      border: `1px solid ${c.fg}`,
    }}>
      {c.label}
    </span>
  );
}

export default function Stacks() {
  const [stacks, setStacks] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [showCreate, setShowCreate] = useState(false);
  const [yml, setYml] = useState(DEFAULT_YML);
  const [stackName, setStackName] = useState('');
  const [creating, setCreating] = useState(false);

  const [showEdit, setShowEdit] = useState(null);
  const [editYml, setEditYml] = useState('');
  const [editSaving, setEditSaving] = useState(false);

  const load = useCallback(async () => {
    try {
      const data = await api.get('/api/stacks');
      setStacks(Array.isArray(data) ? data : (data?.stacks || []));
      setError(null);
    } catch (err) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  const handleCreate = async () => {
    if (!stackName.trim()) return;
    setCreating(true);
    try {
      await api.post('/api/stacks', { name: stackName.trim(), compose: yml });
      setShowCreate(false);
      setStackName('');
      setYml(DEFAULT_YML);
      load();
    } catch (err) {
      alert('Error: ' + err.message);
    } finally {
      setCreating(false);
    }
  };

  const handleEdit = async (name) => {
    try {
      const data = await api.get(`/api/stacks/${name}`);
      setEditYml(data.content || '');
      setShowEdit(name);
    } catch (err) {
      alert('Error: ' + err.message);
    }
  };

  const handleSave = async () => {
    if (!showEdit) return;
    setEditSaving(true);
    try {
      await api.put(`/api/stacks/${showEdit}`, { content: editYml });
      setShowEdit(null);
      load();
    } catch (err) {
      alert('Error: ' + err.message);
    } finally {
      setEditSaving(false);
    }
  };

  const handleAction = async (name, action) => {
    try {
      await api.post(`/api/stacks/${name}/${action}`);
      load();
    } catch (err) {
      alert(`Error: ${err.message}`);
    }
  };

  const handleDelete = async (name) => {
    if (!confirm(`Delete stack "${name}"?`)) return;
    try {
      await api.delete(`/api/stacks/${name}`);
      load();
    } catch (err) {
      alert('Error: ' + err.message);
    }
  };

  if (loading) return <div className="loading-center"><Spinner size="lg" /></div>;

  return (
    <div>
      <div className="section-header">
        <h1>Stacks ({stacks.length})</h1>
        <div className="btn-group">
          <button className="btn-primary" onClick={() => setShowCreate(true)}>+ New Stack</button>
          <button onClick={load}>🔄 Refresh</button>
        </div>
      </div>

      {error && <div className="text-danger mb-16">Error: {error}</div>}

      {stacks.length === 0 ? (
        <div className="text-secondary" style={{ padding: '24px', textAlign: 'center' }}>
          No stacks. Create one to get started.
        </div>
      ) : (
        <div style={{ display: 'grid', gap: '12px' }}>
          {stacks.map((stack) => {
            const name = stack.Name || stack.name;
            const svcCount = stack.Services || stack.serviceCount || 0;
            const status = (stack.Status || stack.status || 'unknown').toLowerCase();
            const isRunning = status === 'running';
            return (
              <div key={name} className="card" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <div>
                  <div style={{ fontWeight: 600, fontSize: '1rem' }}>{name}</div>
                  <div className="text-secondary" style={{ fontSize: '0.75rem', marginTop: '4px' }}>
                    Services: {svcCount} &nbsp;|&nbsp;
                    <StatusBadge status={status} />
                  </div>
                </div>
                <div className="btn-group">
                  <button className="btn-sm" onClick={() => handleEdit(name)}>✏ Edit</button>
                  {!isRunning && (
                    <button className="btn-success btn-sm" onClick={() => handleAction(name, 'deploy')}>▶ Deploy</button>
                  )}
                  {isRunning && (
                    <>
                      <button className="btn-warning btn-sm" onClick={() => handleAction(name, 'stop')}>⏹ Stop</button>
                      <button className="btn-sm" onClick={() => handleAction(name, 'down')}>⬇ Down</button>
                    </>
                  )}
                  <button className="btn-danger btn-sm" onClick={() => handleDelete(name)}>🗑</button>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {showCreate && (
        <Modal
          title="Create New Stack"
          onClose={() => setShowCreate(false)}
          footer={
            <>
              <button onClick={() => setShowCreate(false)}>Cancel</button>
              <button className="btn-primary" onClick={handleCreate} disabled={creating || !stackName.trim()}>
                {creating ? 'Creating...' : 'Create Stack'}
              </button>
            </>
          }
        >
          <div>
            <label style={{ display: 'block', marginBottom: '6px', fontWeight: 500 }}>Stack Name</label>
            <input
              type="text"
              value={stackName}
              onChange={(e) => setStackName(e.target.value)}
              placeholder="my-stack"
              style={{ width: '100%', marginBottom: '16px' }}
              autoFocus
            />
            <label style={{ display: 'block', marginBottom: '6px', fontWeight: 500 }}>
              docker-compose.yml
            </label>
            <YamlEditor value={yml} onChange={setYml} />
          </div>
        </Modal>
      )}

      {showEdit && (
        <Modal
          title={`Edit: ${showEdit}`}
          onClose={() => setShowEdit(null)}
          footer={
            <>
              <button onClick={() => setShowEdit(null)}>Cancel</button>
              <button className="btn-primary" onClick={handleSave} disabled={editSaving}>
                {editSaving ? 'Saving...' : 'Save Changes'}
              </button>
            </>
          }
        >
          <YamlEditor value={editYml} onChange={setEditYml} />
        </Modal>
      )}
    </div>
  );
}
