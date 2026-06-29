import { useState, useEffect, useCallback, useMemo } from 'react';
import { api } from '../api/client';
import Modal from '../components/Modal';
import YamlEditor from '../components/YamlEditor';
import Spinner from '../components/Spinner';
import ListToolbar, { useSelection } from '../components/ListToolbar';
import useFilters from '../hooks/useFilters';
import FilterBar from '../components/FilterBar';

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

  // ── Validate + Diff state ────────────────────────────────────
  const [validateResult, setValidateResult] = useState(null); // { valid, rendered, errors }
  const [validating, setValidating] = useState(false);
  const [showDiff, setShowDiff] = useState(false);
  const [originalYml, setOriginalYml] = useState('');

  // ── Env editor state ────────────────────────────────────────
  const [envVars, setEnvVars] = useState([]);       // [{key, value}, ...]
  const [showEnv, setShowEnv] = useState(false);

  const load = useCallback(async () => {
    try {
      const data = await api.get('/api/stacks');
      const raw = Array.isArray(data) ? data : (data?.stacks || []);
      // Normalize: ensure lowercase status field for useFilters
      const normalized = raw.map(s => ({
        ...s,
        status: (s.Status || s.status || 'unknown').toLowerCase(),
      }));
      setStacks(normalized);
      setError(null);
    } catch (err) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  const { filtered, searchQuery, setSearchQuery, stateFilter, setStateFilter } = useFilters(stacks, {
    searchFields: ['name'],
    stateField: 'status',
    stateMap: { running: ['running'], stopped: ['stopped', 'exited'] },
  });

  const filteredIds = useMemo(() => filtered.map(s => s.id || s.Name || s.name), [filtered]);

  const { selected, toggle, clear } = useSelection(stacks, 'id', filteredIds);

  const stateOptions = [
    { value: 'running', label: 'Running' },
    { value: 'stopped', label: 'Stopped' },
  ];

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
      setOriginalYml(data.content || '');
      setValidateResult(null);
      setShowDiff(false);
      // Load env vars
      try {
        const envData = await api.get(`/api/stacks/${name}/env`);
        const vars = envData.variables || {};
        setEnvVars(Object.entries(vars).map(([key, value]) => ({ key, value })));
      } catch {
        setEnvVars([]);
      }
      setShowEnv(false);
      setShowEdit(name);
    } catch (err) {
      alert('Error: ' + err.message);
    }
  };

  const handleSave = async (deployAfter = false) => {
    if (!showEdit) return;
    setEditSaving(true);
    try {
      await api.put(`/api/stacks/${showEdit}`, { content: editYml });
      // Save env vars
      const varObj = {};
      envVars.forEach(({ key, value }) => { if (key.trim()) varObj[key.trim()] = value; });
      await api.put(`/api/stacks/${showEdit}/env`, { variables: varObj });
      setShowEdit(null);
      load();
      if (deployAfter) {
        setTimeout(() => handleAction(showEdit, 'deploy'), 300);
      }
    } catch (err) {
      alert('Error: ' + err.message);
    } finally {
      setEditSaving(false);
    }
  };

  const handleValidate = async () => {
    if (!showEdit) return;
    setValidating(true);
    setValidateResult(null);
    try {
      const data = await api.post(`/api/stacks/${showEdit}/validate`, { content: editYml });
      setValidateResult(data);
    } catch (err) {
      setValidateResult({ valid: false, errors: [err.message], name: showEdit });
    } finally {
      setValidating(false);
    }
  };

  // ── Simple line diff ─────────────────────────────────────────
  const diffLines = useMemo(() => {
    if (!showDiff || !originalYml) return null;
    const orig = originalYml.split('\n');
    const mod = editYml.split('\n');
    const result = [];
    const maxLen = Math.max(orig.length, mod.length);
    // Build a set of lines from original for quick lookup
    const origSet = new Set(orig);
    const modSet = new Set(mod);
    for (let i = 0; i < maxLen; i++) {
      const oLine = i < orig.length ? orig[i] : null;
      const mLine = i < mod.length ? mod[i] : null;
      if (oLine === mLine) {
        result.push({ type: 'same', orig: oLine, mod: mLine, num: i + 1 });
      } else if (oLine === null) {
        result.push({ type: 'added', orig: null, mod: mLine, num: i + 1 });
      } else if (mLine === null) {
        result.push({ type: 'removed', orig: oLine, mod: null, num: i + 1 });
      } else if (modSet.has(oLine) || origSet.has(mLine)) {
        // Line exists somewhere else — treat as changed
        result.push({ type: 'changed', orig: oLine, mod: mLine, num: i + 1 });
      } else {
        result.push({ type: 'changed', orig: oLine, mod: mLine, num: i + 1 });
      }
    }
    return result;
  }, [showDiff, originalYml, editYml]);

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

  const handleDeleteSelected = async () => {
    const names = Array.from(selected);
    if (!confirm(`Delete ${names.length} stack(s)?`)) return;
    for (const name of names) {
      try { await api.delete(`/api/stacks/${name}`); } catch (e) { alert('Error: ' + e.message); }
    }
    clear();
    load();
  };

  if (loading) return <div className="loading-center"><Spinner size="lg" /></div>;

  return (
    <div>
      <div className="section-header">
        <h1>Stacks ({filtered.length}{filtered.length !== stacks.length ? ` / ${stacks.length}` : ''})</h1>
        <div className="btn-group">
          <button className="btn-primary" onClick={() => setShowCreate(true)}>+ New Stack</button>
          <button onClick={load}>🔄 Refresh</button>
        </div>
      </div>

      {error && <div className="text-danger mb-16">Error: {error}</div>}

      <ListToolbar
        selected={selected}
        total={stacks.length}
        filteredIds={filteredIds}
        onClear={clear}
        actions={[
          { label: '🗑 Delete', onClick: handleDeleteSelected, variant: 'danger' },
        ]}
      />

      <FilterBar
        searchQuery={searchQuery}
        onSearchChange={setSearchQuery}
        searchPlaceholder="Search stacks..."
        stateFilter={stateFilter}
        onStateFilterChange={setStateFilter}
        stateOptions={stateOptions}
        filteredCount={filtered.length}
        totalCount={stacks.length}
      />

      {filtered.length === 0 ? (
        <div className="text-secondary" style={{ padding: '24px', textAlign: 'center' }}>
          No stacks. Create one to get started.
        </div>
      ) : (
        <div style={{ display: 'grid', gap: '12px' }}>
          {filtered.map((stack) => {
            const name = stack.Name || stack.name;
            const svcCount = stack.Services || stack.serviceCount || 0;
            const status = (stack.Status || stack.status || 'unknown').toLowerCase();
            const isRunning = status === 'running';
            const selKey = stack.id || name;
            const isSel = selected.has(selKey);
            return (
              <div key={name} className="card" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                  <input
                    type="checkbox"
                    checked={isSel}
                    onChange={() => toggle(stack)}
                    style={{ width: '16px', height: '16px', cursor: 'pointer' }}
                  />
                  <div>
                    <div style={{ fontWeight: 600, fontSize: '1rem' }}>{name}</div>
                    <div className="text-secondary" style={{ fontSize: '0.75rem', marginTop: '4px' }}>
                      Services: {svcCount} &nbsp;|&nbsp;
                      <StatusBadge status={status} />
                    </div>
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
          size="large"
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
          <div style={{ display: 'flex', flexDirection: 'column', flex: 1, minHeight: 0 }}>
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
            <YamlEditor value={yml} onChange={setYml} fill />
          </div>
        </Modal>
      )}

      {showEdit && (
        <Modal
          title={`Edit: ${showEdit}`}
          size="large"
          onClose={() => setShowEdit(null)}
          footer={
            <>
              <button onClick={() => setShowEdit(null)}>Cancel</button>
              <button
                onClick={handleValidate}
                disabled={validating}
                title="Validate compose config"
                style={{ background: validateResult?.valid ? '#1a3a1a' : undefined, color: validateResult?.valid ? '#4caf50' : undefined }}
              >
                {validating ? '⏳ Validating...' : '✓ Validate'}
              </button>
              <button className="btn-primary" onClick={() => handleSave(true)} disabled={editSaving}>
                {editSaving ? 'Saving...' : 'Save & Deploy'}
              </button>
              <button onClick={() => handleSave(false)} disabled={editSaving}>
                {editSaving ? 'Saving...' : 'Save Only'}
              </button>
            </>
          }
        >
          <div style={{ marginBottom: '8px', display: 'flex', gap: '10px', alignItems: 'center' }}>
            <label style={{ display: 'flex', alignItems: 'center', gap: '6px', cursor: 'pointer', fontSize: '0.85rem', userSelect: 'none' }}>
              <input
                type="checkbox"
                checked={showDiff}
                onChange={(e) => setShowDiff(e.target.checked)}
                style={{ width: '14px', height: '14px', cursor: 'pointer' }}
              />
              Show Diff
            </label>
          </div>

          {/* Validation result */}
          {validateResult && (
            <div style={{
              marginBottom: '12px',
              padding: '10px 14px',
              borderRadius: '6px',
              background: validateResult.valid ? '#0d2818' : '#3d1010',
              border: `1px solid ${validateResult.valid ? '#2d7a3a' : '#8b2020'}`,
              fontSize: '0.85rem',
            }}>
              {validateResult.valid ? (
                <div style={{ color: '#4caf50', fontWeight: 600 }}>
                  ✅ Config valid
                </div>
              ) : (
                <div>
                  <div style={{ color: '#f44336', fontWeight: 600, marginBottom: '6px' }}>
                    ❌ Validation errors:
                  </div>
                  {(validateResult.errors || []).map((err, i) => (
                    <div key={i} style={{ color: '#ffab91', fontSize: '0.8rem', marginBottom: '2px', fontFamily: 'monospace' }}>
                      {err}
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}

          {showDiff && diffLines ? (
            <div style={{
              border: '1px solid #333',
              borderRadius: '4px',
              overflow: 'auto',
              maxHeight: '400px',
              marginBottom: '12px',
              fontFamily: 'monospace',
              fontSize: '0.8rem',
              lineHeight: '1.5',
            }}>
              <table style={{ width: '100%', borderCollapse: 'collapse' }}>
                <tbody>
                  {diffLines.map((line, i) => {
                    let bg = 'transparent';
                    let prefix = ' ';
                    if (line.type === 'added') { bg = '#0d3320'; prefix = '+'; }
                    else if (line.type === 'removed') { bg = '#3d1010'; prefix = '-'; }
                    else if (line.type === 'changed') { bg = '#2a2a10'; prefix = '~'; }
                    return (
                      <tr key={i} style={{ background: bg }}>
                        <td style={{ width: '40px', textAlign: 'right', padding: '0 8px', color: '#555', borderRight: '1px solid #333' }}>
                          {line.num}
                        </td>
                        <td style={{ width: '20px', textAlign: 'center', color: line.type === 'added' ? '#4caf50' : line.type === 'removed' ? '#f44336' : line.type === 'changed' ? '#ffc107' : '#555' }}>
                          {prefix}
                        </td>
                        <td style={{ padding: '0 8px', color: '#ccc', whiteSpace: 'pre', textDecoration: line.type === 'removed' ? 'line-through' : undefined }}>
                          {line.type === 'removed' ? line.orig : line.mod}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          ) : (
            <YamlEditor value={editYml} onChange={setEditYml} fill />
          )}

          {/* Env editor */}
          <details open={showEnv} onToggle={(e) => setShowEnv(e.target.open)} style={{ marginTop: '20px' }}>
            <summary style={{ cursor: 'pointer', fontWeight: 600, fontSize: '0.95rem', padding: '4px 0' }}>
              🔧 Environment Variables ({envVars.length})
            </summary>
            <div style={{ marginTop: '12px' }}>
              <table role="grid" style={{ width: '100%' }}>
                <thead>
                  <tr>
                    <th style={{ width: '35%' }}>Key</th>
                    <th style={{ width: '50%' }}>Value</th>
                    <th style={{ width: '15%' }}></th>
                  </tr>
                </thead>
                <tbody>
                  {envVars.map((row, i) => (
                    <tr key={i}>
                      <td>
                        <input
                          type="text"
                          value={row.key}
                          onChange={(e) => {
                            const next = [...envVars];
                            next[i] = { ...next[i], key: e.target.value };
                            setEnvVars(next);
                          }}
                          placeholder="KEY"
                          style={{ width: '100%', boxSizing: 'border-box' }}
                        />
                      </td>
                      <td>
                        <input
                          type="text"
                          value={row.value}
                          onChange={(e) => {
                            const next = [...envVars];
                            next[i] = { ...next[i], value: e.target.value };
                            setEnvVars(next);
                          }}
                          placeholder="value"
                          style={{ width: '100%', boxSizing: 'border-box' }}
                        />
                      </td>
                      <td style={{ textAlign: 'center' }}>
                        <button
                          className="btn-sm btn-danger"
                          onClick={() => setEnvVars(envVars.filter((_, j) => j !== i))}
                          title="Remove"
                        >
                          ✕
                        </button>
                      </td>
                    </tr>
                  ))}
                  {envVars.length === 0 && (
                    <tr>
                      <td colSpan={3} style={{ textAlign: 'center', color: '#888', padding: '16px' }}>
                        No environment variables set.
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
              <button
                className="btn-sm"
                onClick={() => setEnvVars([...envVars, { key: '', value: '' }])}
                style={{ marginTop: '8px' }}
              >
                + Add Variable
              </button>
            </div>
          </details>
        </Modal>
      )}
    </div>
  );
}
