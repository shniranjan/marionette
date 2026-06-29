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
        <div style={{ display: 'grid', gap: '12px' }}>
          {filtered.map((img) => {
            const tags = img.repoTags || [];
            const isSel = selected.has(img.id);
            return (
              <div key={img.id}
                className="card"
                onClick={() => handleInspect(img.id)}
                style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', cursor: 'pointer' }}
              >
                <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                  <input
                    type="checkbox"
                    checked={isSel}
                    onChange={(e) => { e.stopPropagation(); toggle(img); }}
                    style={{ width: '16px', height: '16px', cursor: 'pointer' }}
                  />
                  <div>
                    <div className="mono" style={{ fontWeight: 600 }}>
                      {tags.length > 0 ? tags[0].replace(/^<none>:<none>$/, '<none>') : img.id?.substring(0, 12)}
                    </div>
                    <div style={{ fontSize: '0.75rem', color: 'var(--pico-muted-color)', marginTop: '4px' }}>
                      ID: {img.id?.substring(0, 12)} &nbsp;|&nbsp;
                      Size: {formatSize(img.size)} &nbsp;|&nbsp;
                      Created: {img.created ? new Date(img.created * 1000).toLocaleDateString() : '—'}
                      {tags.length > 1 && <span> &nbsp;|&nbsp; {tags.slice(1).join(', ')}</span>}
                    </div>
                  </div>
                </div>
                <span style={{ fontSize: '0.75rem', color: 'var(--pico-muted-color)' }}>Click to inspect</span>
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
