import { useState, useEffect, useCallback, useMemo } from 'react';
import { api } from '../api/client';
import Modal from '../components/Modal';
import JsonTree from '../components/JsonTree';
import Spinner from '../components/Spinner';
import ListToolbar, { useSelection } from '../components/ListToolbar';
import useFilters from '../hooks/useFilters';
import FilterBar from '../components/FilterBar';

export default function Images() {
  const [images, setImages] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [showPull, setShowPull] = useState(false);
  const [pullImage, setPullImage] = useState('');
  const [pulling, setPulling] = useState(false);
  const [pullError, setPullError] = useState('');
  const [inspectData, setInspectData] = useState(null);

  const load = useCallback(async () => {
    try {
      const data = await api.get('/api/images');
      setImages(Array.isArray(data) ? data : (data?.images || []));
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

  const { filtered, searchQuery, setSearchQuery } = useFilters(images, { searchFields: ['repoTags'] });

  const filteredIds = useMemo(() => filtered.map(img => img.id), [filtered]);

  const { selected, toggle, selectAll, clear } = useSelection(images, 'id', filteredIds);

  const handlePull = async () => {
    if (!pullImage.trim()) return;
    setPulling(true);
    setPullError('');
    try {
      await api.post('/api/images/pull', { image: pullImage.trim() });
      setShowPull(false);
      setPullImage('');
      load();
    } catch (err) {
      setPullError(err.message);
    } finally {
      setPulling(false);
    }
  };

  const handleRemove = async () => {
    const ids = Array.from(selected);
    if (!confirm(`Remove ${ids.length} image(s)?`)) return;
    for (const id of ids) {
      try { await api.delete(`/api/images/${id}`); } catch (e) { alert('Error: ' + e.message); }
    }
    clear();
    load();
  };

  const handleInspect = async (id) => {
    try {
      const data = await api.get(`/api/images/${id}`);
      setInspectData(data);
    } catch (err) {
      alert('Error: ' + err.message);
    }
  };

  if (loading) return <div className="loading-center"><Spinner size="lg" /></div>;

  return (
    <div>
      <div className="section-header">
        <h1>Images ({filtered.length}{filtered.length !== images.length ? ` / ${images.length}` : ''})</h1>
        <div className="btn-group">
          <button className="btn-primary" onClick={() => setShowPull(true)}>⬇ Pull Image</button>
          <button onClick={load}>🔄 Refresh</button>
        </div>
      </div>

      {error && <div className="text-danger mb-16">Error: {error}</div>}

      <ListToolbar
        selected={selected}
        total={images.length}
        filteredIds={filteredIds}
        onClear={clear}
        actions={[
          { label: '🗑 Remove', onClick: handleRemove, variant: 'danger' },
        ]}
      />

      <FilterBar
        searchQuery={searchQuery}
        onSearchChange={setSearchQuery}
        searchPlaceholder="Search images..."
        filteredCount={filtered.length}
        totalCount={images.length}
      />

      {filtered.length === 0 ? (
        <div className="text-secondary" style={{ padding: '24px', textAlign: 'center' }}>No images</div>
      ) : (
        <div style={{ display: 'grid', gap: '10px' }}>
          {filtered.map((img) => {
            const tags = img.repoTags || [];
            const primaryTag = tags.length > 0 ? tags[0] : null;
            const { name, tag } = primaryTag ? splitImageTag(primaryTag) : { name: '<none>', tag: '' };
            const isSel = selected.has(img.id);
            return (
              <div key={img.id}
                onClick={() => toggle(img)}
                style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center',
                  cursor: 'pointer',
                  padding: '10px 14px',
                  borderRadius: '8px',
                  background: isSel ? 'var(--bg-secondary, #1e293b)' : 'var(--card-bg, #0f172a)',
                  border: isSel ? '2px solid var(--accent, #3b82f6)' : '2px solid var(--card-border, #1e293b)',
                  transition: 'border-color 0.15s, background 0.15s',
                }}
              >
                <div style={{ display: 'flex', alignItems: 'center', gap: '14px', minWidth: 0, flex: 1 }}>
                  <div style={{
                    width: '22px', height: '22px', minWidth: '22px',
                    borderRadius: '4px',
                    background: isSel ? 'var(--accent, #3b82f6)' : 'transparent',
                    border: isSel ? 'none' : '2px solid var(--card-border, #475569)',
                    display: 'flex', alignItems: 'center', justifyContent: 'center',
                    color: '#fff', fontSize: '0.75rem', fontWeight: 700,
                    transition: 'background 0.15s, border-color 0.15s',
                  }}>
                    {isSel ? '✓' : ''}
                  </div>
                  <div style={{ minWidth: 0, flex: 1 }}>
                    <div style={{ display: 'flex', alignItems: 'baseline', gap: '8px', marginBottom: '3px' }}>
                      <span style={{ fontWeight: 600, fontSize: '0.9rem', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                        {name}
                      </span>
                      {tag && (
                        <span style={{
                          fontSize: '0.65rem', fontWeight: 600, padding: '1px 7px',
                          borderRadius: '10px', whiteSpace: 'nowrap',
                          background: tag === 'latest' ? '#1e3a5f' : '#2a1a3f',
                          color: tag === 'latest' ? '#60a5fa' : '#a78bfa',
                          border: `1px solid ${tag === 'latest' ? '#3b82f6' : '#7c3aed'}33`,
                        }}>
                          {tag}
                        </span>
                      )}
                    </div>
                    <div style={{ fontSize: '0.72rem', color: 'var(--pico-muted-color)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                      ID: {img.id?.substring(0, 12)} &nbsp;|&nbsp;
                      Size: {formatSize(img.size)} &nbsp;|&nbsp;
                      Created: {img.created ? new Date(img.created * 1000).toLocaleDateString() : '—'}
                      {tags.length > 1 && <span> &nbsp;|&nbsp; {tags.slice(1).join(', ')}</span>}
                    </div>
                  </div>
                </div>
                <button
                  className="btn-sm"
                  onClick={(e) => { e.stopPropagation(); handleInspect(img.id); }}
                  style={{ marginLeft: '12px', flexShrink: 0 }}
                >
                  Inspect
                </button>
              </div>
            );
          })}
        </div>
      )}

      {showPull && (
        <Modal
          title="Pull Image"
          onClose={() => { setShowPull(false); setPullError(''); setPullImage(''); }}
          footer={
            <>
              <button onClick={() => { setShowPull(false); setPullError(''); }}>Cancel</button>
              <button className="btn-primary" onClick={handlePull} disabled={pulling || !pullImage.trim()}>
                {pulling ? 'Pulling...' : 'Pull'}
              </button>
            </>
          }
        >
          <div>
            <label style={{ display: 'block', marginBottom: '6px', fontWeight: 500 }}>
              Image name (e.g., nginx:latest)
            </label>
            <input
              type="text"
              value={pullImage}
              onChange={(e) => setPullImage(e.target.value)}
              placeholder="nginx:latest"
              style={{ width: '100%' }}
              autoFocus
              onKeyDown={(e) => e.key === 'Enter' && handlePull()}
            />
            {pullError && (
              <div style={{ color: 'var(--red)', fontSize: '0.8rem', marginTop: '8px' }}>{pullError}</div>
            )}
          </div>
        </Modal>
      )}

      {inspectData && (
        <Modal title="Image Inspect" onClose={() => setInspectData(null)}>
          <JsonTree data={inspectData} />
        </Modal>
      )}
    </div>
  );
}

function formatSize(bytes) {
  if (!bytes) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

/** Split "nginx:latest" into { name: "nginx", tag: "latest" } */
function splitImageTag(full) {
  if (!full || full === '<none>') return { name: '<none>', tag: '' };
  const lastColon = full.lastIndexOf(':');
  if (lastColon === -1) return { name: full, tag: 'latest' };
  // Check if the colon is part of a port (e.g., registry:5000/image)
  const afterColon = full.substring(lastColon + 1);
  if (/^\d+$/.test(afterColon) || afterColon.includes('/')) {
    return { name: full, tag: 'latest' };
  }
  return { name: full.substring(0, lastColon), tag: afterColon };
}
