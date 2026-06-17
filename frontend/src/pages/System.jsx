import { useState, useEffect, useCallback, useRef } from 'react';
import { api } from '../api/client';
import JsonTree from '../components/JsonTree';
import Spinner from '../components/Spinner';

export default function System() {
  const [info, setInfo] = useState(null);
  const [version, setVersion] = useState(null);
  const [audit, setAudit] = useState([]);
  const [events, setEvents] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [pruning, setPruning] = useState(false);
  const [pruneResult, setPruneResult] = useState(null);
  const eventsRef = useRef(null);
  const [eventsConnected, setEventsConnected] = useState(false);

  const load = useCallback(async () => {
    try {
      const [infoData, versionData, auditData] = await Promise.all([
        api.get('/api/system'),
        api.get('/api/system/version'),
        api.get('/api/system/audit'),
      ]);
      setInfo(infoData);
      setVersion(versionData);
      setAudit(Array.isArray(auditData) ? auditData : []);
      setError(null);
    } catch (err) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  // SSE events
  useEffect(() => {
    const es = new EventSource('/api/system/events');
    eventsRef.current = es;
    es.onopen = () => setEventsConnected(true);
    es.onerror = () => setEventsConnected(false);
    es.onmessage = (e) => {
      try {
        const data = JSON.parse(e.data);
        setEvents((prev) => {
          const next = [data, ...prev];
          return next.length > 100 ? next.slice(0, 100) : next;
        });
      } catch {
        // ignore parse errors
      }
    };
    return () => es.close();
  }, []);

  const handlePrune = async () => {
    if (!confirm('Prune all unused Docker objects (containers, images, volumes, networks)?')) return;
    setPruning(true);
    try {
      const result = await api.post('/api/system/prune');
      setPruneResult(result);
    } catch (err) {
      alert('Error: ' + err.message);
    } finally {
      setPruning(false);
    }
  };

  if (loading) return <div className="loading-center"><Spinner size="lg" /></div>;

  return (
    <div>
      <h1>System</h1>
      {error && <div className="text-danger mb-16">Error: {error}</div>}

      {/* Docker Info */}
      {info && (
        <div className="card mb-16">
          <h2>Docker Info</h2>
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: '8px', fontSize: '0.85rem' }}>
            <InfoItem label="Containers" value={info.containers} />
            <InfoItem label="Running" value={info.containersRunning} />
            <InfoItem label="Paused" value={info.containersPaused} />
            <InfoItem label="Stopped" value={info.containersStopped} />
            <InfoItem label="Images" value={info.images} />
            <InfoItem label="OS" value={`${info.os || ''} ${info.architecture || ''}`} />
            <InfoItem label="CPUs" value={info.cpuCount} />
            <InfoItem label="Memory" value={info.memoryBytes ? formatBytes(info.memoryBytes) : '—'} />
            <InfoItem label="Storage Driver" value={info.driver} />
          </div>
        </div>
      )}

      {/* Version */}
      {version && (
        <div className="card mb-16">
          <h2>Version</h2>
          <div style={{ fontSize: '0.85rem' }}>
            <div><span className="text-secondary">Version: </span> {version.Version || version.ServerVersion}</div>
            <div><span className="text-secondary">API Version: </span> {version.ApiVersion}</div>
            <div><span className="text-secondary">Go Version: </span> {version.GoVersion}</div>
            <div><span className="text-secondary">Git Commit: </span> {version.GitCommit}</div>
            <div><span className="text-secondary">Built: </span> {version.BuildTime}</div>
          </div>
        </div>
      )}

      {/* Prune */}
      <div className="card mb-16">
        <h2>Maintenance</h2>
        <p className="text-secondary" style={{ fontSize: '0.85rem', marginBottom: '12px' }}>
          Remove unused containers, images, volumes, and networks to free up disk space.
        </p>
        <button className="btn-danger" onClick={handlePrune} disabled={pruning}>
          {pruning ? 'Pruning...' : '🗑 Prune System'}
        </button>
        {pruneResult && (
          <div className="card mb-16" style={{ marginTop: '12px', background: 'var(--bg-tertiary)' }}>
            <div style={{ fontSize: '0.85rem' }}>
              <div>Containers removed: {pruneResult.ContainersDeleted?.length || 0}</div>
              <div>Images removed: {pruneResult.ImagesDeleted?.length || 0}</div>
              <div>Volumes removed: {pruneResult.VolumesDeleted?.length || 0}</div>
              <div>Space reclaimed: {pruneResult.SpaceReclaimed ? formatBytes(pruneResult.SpaceReclaimed) : '0 B'}</div>
            </div>
          </div>
        )}
      </div>

      {/* Audit Log */}
      <div className="card mb-16">
        <h2>Audit Log</h2>
        {audit.length === 0 ? (
          <div className="text-secondary">No audit entries</div>
        ) : (
          <div className="table-wrapper">
            <table>
              <thead>
                <tr>
                  <th>Time</th>
                  <th>Action</th>
                  <th>Target</th>
                  <th>User</th>
                </tr>
              </thead>
              <tbody>
                {audit.map((entry, i) => (
                  <tr key={i}>
                    <td className="mono" style={{ fontSize: '0.75rem' }}>
                      {entry.timestamp ? new Date(entry.timestamp).toLocaleString() : '—'}
                    </td>
                    <td>{entry.action || '—'}</td>
                    <td className="mono" style={{ fontSize: '0.75rem' }}>{entry.target || '—'}</td>
                    <td>{entry.user || '—'}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Live Events */}
      <div className="card">
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '12px' }}>
          <h2 style={{ margin: 0 }}>Live Events</h2>
          <div style={{
            width: '8px', height: '8px', borderRadius: '50%',
            background: eventsConnected ? 'var(--green)' : 'var(--red)',
          }} />
          <span style={{ fontSize: '0.75rem', color: 'var(--text-secondary)' }}>
            {eventsConnected ? 'Connected' : 'Disconnected'}
          </span>
        </div>
        <div style={{ maxHeight: '400px', overflow: 'auto', background: 'var(--bg-tertiary)', borderRadius: '6px', padding: '12px' }}>
          {events.length === 0 ? (
            <div className="text-secondary" style={{ textAlign: 'center', padding: '24px' }}>
              Waiting for events...
            </div>
          ) : (
            events.map((evt, i) => (
              <div key={i} className="log-output" style={{ marginBottom: '4px', fontSize: '0.75rem' }}>
                <span className="text-secondary">
                  {evt.time ? new Date(evt.time * 1000).toLocaleTimeString() : '—'}
                </span>
                {' '}
                <span style={{ color: 'var(--accent)' }}>{evt.Type}</span>
                {' '}
                <span>{evt.Action}</span>
                {' '}
                <span className="text-secondary">{evt.Actor?.ID?.substring(0, 12)}</span>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}

function InfoItem({ label, value }) {
  return (
    <div>
      <div className="text-secondary" style={{ fontSize: '0.75rem' }}>{label}</div>
      <div className="mono">{value ?? '—'}</div>
    </div>
  );
}

function formatBytes(bytes) {
  if (!bytes || bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}
