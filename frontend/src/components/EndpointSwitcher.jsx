import { useState, useEffect, useCallback } from 'react';
import { api } from '../api/client';

const STORAGE_KEY = 'marionette-endpoint';

export function getCurrentEndpoint() {
  const params = new URLSearchParams(window.location.search);
  return params.get('endpoint') || localStorage.getItem(STORAGE_KEY) || 'local';
}

export function setEndpoint(id) {
  if (id === 'local') {
    localStorage.removeItem(STORAGE_KEY);
  } else {
    localStorage.setItem(STORAGE_KEY, id);
  }
  const params = new URLSearchParams(window.location.search);
  params.set('endpoint', id);
  window.history.replaceState(null, '', `${window.location.pathname}?${params.toString()}`);
  window.dispatchEvent(new CustomEvent('endpoint:changed', { detail: id }));
}

export default function EndpointSwitcher({ currentEndpoint, onEndpointChange }) {
  const [endpoints, setEndpoints] = useState([]);
  const [open, setOpen] = useState(false);
  const [loading, setLoading] = useState(false);

  const load = useCallback(async () => {
    try {
      const data = await api.get('/api/endpoints');
      setEndpoints(Array.isArray(data) ? data : (data?.endpoints || []));
    } catch {
      // Silently fail — endpoints may not be available yet
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  useEffect(() => {
    const handler = () => load();
    window.addEventListener('endpoint:changed', handler);
    return () => window.removeEventListener('endpoint:changed', handler);
  }, [load]);

  const current = endpoints.find(e => (e.id || e.Id) === currentEndpoint) || {
    id: 'local', name: 'Local', status: 'connected',
  };

  const statusColor = (s) => {
    switch ((s || '').toLowerCase()) {
      case 'connected': return 'var(--green)';
      case 'disconnected': return 'var(--red)';
      default: return 'var(--yellow)';
    }
  };

  const handleSelect = (id) => {
    setEndpoint(id);
    setOpen(false);
    if (onEndpointChange) onEndpointChange(id);
  };

  return (
    <div style={{ position: 'relative' }}>
      <button
        onClick={() => { setOpen(!open); if (!open) load(); }}
        style={{
          width: '100%',
          padding: '8px 12px',
          background: 'var(--bg-tertiary)',
          border: '1px solid var(--border)',
          borderRadius: '6px',
          color: 'var(--text-primary)',
          cursor: 'pointer',
          textAlign: 'left',
          fontSize: '0.8rem',
          display: 'flex',
          alignItems: 'center',
          gap: '8px',
          fontWeight: 500,
        }}
      >
        <span style={{
          width: '8px', height: '8px', borderRadius: '50%',
          background: statusColor(current.status || current.Status),
          flexShrink: 0,
        }} />
        <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
          {current.name || current.Name || 'Local'}
        </span>
        <span style={{ fontSize: '0.7rem', color: 'var(--text-secondary)' }}>▾</span>
      </button>

      {open && (
        <>
          <div
            style={{ position: 'fixed', inset: 0, zIndex: 99 }}
            onClick={() => setOpen(false)}
          />
          <div style={{
            position: 'absolute',
            top: '100%',
            left: 0,
            right: 0,
            marginTop: '4px',
            background: 'var(--bg-secondary)',
            border: '1px solid var(--border)',
            borderRadius: '8px',
            boxShadow: 'var(--card-shadow)',
            zIndex: 100,
            maxHeight: '280px',
            overflowY: 'auto',
          }}>
            {/* Local endpoint */}
            <div
              onClick={() => handleSelect('local')}
              style={{
                padding: '10px 12px',
                cursor: 'pointer',
                display: 'flex',
                alignItems: 'center',
                gap: '8px',
                borderBottom: '1px solid var(--border)',
                background: currentEndpoint === 'local' ? 'var(--bg-tertiary)' : 'transparent',
                color: currentEndpoint === 'local' ? 'var(--accent)' : 'var(--text-primary)',
              }}
            >
              <span style={{
                width: '8px', height: '8px', borderRadius: '50%',
                background: 'var(--green)',
              }} />
              <span style={{ fontWeight: currentEndpoint === 'local' ? 600 : 400 }}>Local</span>
            </div>

            {endpoints.map((ep) => {
              const id = ep.id || ep.Id;
              const name = ep.name || ep.Name || id;
              const s = (ep.status || ep.Status || '').toLowerCase();
              const containers = ep.container_count ?? ep.ContainerCount ?? '—';
              const active = currentEndpoint === id;
              return (
                <div
                  key={id}
                  onClick={() => handleSelect(id)}
                  style={{
                    padding: '10px 12px',
                    cursor: 'pointer',
                    display: 'flex',
                    alignItems: 'center',
                    gap: '8px',
                    borderBottom: '1px solid var(--border)',
                    background: active ? 'var(--bg-tertiary)' : 'transparent',
                    color: active ? 'var(--accent)' : 'var(--text-primary)',
                  }}
                >
                  <span style={{
                    width: '8px', height: '8px', borderRadius: '50%',
                    background: statusColor(s),
                    flexShrink: 0,
                  }} />
                  <span style={{ flex: 1, fontWeight: active ? 600 : 400 }}>{name}</span>
                  <span style={{ fontSize: '0.7rem', color: 'var(--text-secondary)' }}>
                    {containers} 📦
                  </span>
                </div>
              );
            })}

            {endpoints.length === 0 && (
              <div style={{ padding: '16px', textAlign: 'center', color: 'var(--text-secondary)', fontSize: '0.8rem' }}>
                No endpoints configured
              </div>
            )}
          </div>
        </>
      )}
    </div>
  );
}
