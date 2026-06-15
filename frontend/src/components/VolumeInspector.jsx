import { useState, useCallback } from 'react';
import { api } from '../api/client';
import Modal from './Modal';
import SecretMask from './SecretMask';

function bytesToSize(bytes) {
  if (!bytes && bytes !== 0) return 'unknown';
  if (bytes === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return `${(bytes / Math.pow(1024, i)).toFixed(i > 0 ? 1 : 0)} ${units[i]}`;
}

export default function VolumeInspector({ volumeName, onClose }) {
  const [data, setData] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [revealedOpts, setRevealedOpts] = useState(false);
  const [revealConfirm, setRevealConfirm] = useState(false);

  useState(() => {
    (async () => {
      try {
        const result = await api.get(`/api/volumes/${volumeName}/inspect`);
        setData(result);
      } catch (err) {
        setError(err.message);
      } finally {
        setLoading(false);
      }
    })();
  }, [volumeName]);

  const handleRevealOptions = useCallback(() => {
    if (!revealConfirm) {
      setRevealConfirm(true);
      return;
    }
    setRevealedOpts(true);
    setRevealConfirm(false);
    // Audit log entry (client-side)
    const entry = {
      timestamp: new Date().toISOString(),
      action: 'reveal_volume_options',
      volume: volumeName,
    };
    const audit = JSON.parse(localStorage.getItem('marionette-audit-log') || '[]');
    audit.push(entry);
    localStorage.setItem('marionette-audit-log', JSON.stringify(audit));
  }, [revealConfirm, volumeName]);

  const sanitizeOptions = (opts) => {
    if (!opts || typeof opts !== 'object' || revealedOpts) return opts;
    const sensitive = ['password', 'secret', 'key', 'token', 'pass', 'auth'];
    const sanitized = {};
    for (const [k, v] of Object.entries(opts)) {
      const lower = k.toLowerCase();
      const isSensitive = sensitive.some(s => lower.includes(s));
      sanitized[k] = isSensitive ? '••••••••' : v;
    }
    return sanitized;
  };

  const categoryInfo = (driver) => {
    const d = (driver || '').toLowerCase();
    if (d === 'local') return { cat: 'Local Bind Mount', advice: 'Use scp or rsync-over-ssh for cross-host' };
    if (d.includes('nfs') || d.includes('efs')) return { cat: 'Network Filesystem', advice: 'Can be re-mounted on target host' };
    if (d.includes('gp') || d.includes('azure')) return { cat: 'Cloud Block Storage', advice: 'Create snapshot, attach to target' };
    if (d.includes('overlay') || d.includes('overlay2')) return { cat: 'Internal Docker', advice: 'Will be recreated on target; data not portable' };
    return { cat: 'Other', advice: 'Manual transfer required' };
  };

  if (loading) return (
    <Modal title={`Volume: ${volumeName}`} onClose={onClose}>
      <div className="loading-center"><div className="spinner spinner-lg" /></div>
    </Modal>
  );

  if (error) return (
    <Modal title={`Volume: ${volumeName}`} onClose={onClose}>
      <div className="text-danger">Error: {error}</div>
    </Modal>
  );

  const { cat, advice } = categoryInfo(data?.Driver);
  const options = sanitizeOptions(data?.Options);
  const usedBy = data?.UsageData?.RefCount > 0 ? (data?.Containers || []) : [];

  return (
    <Modal title={`Volume: ${volumeName}`} onClose={onClose}>
      <div style={{ display: 'grid', gap: '16px' }}>
        {/* Basic info */}
        <div className="card">
          <h3>Basic Info</h3>
          <table>
            <tbody>
              <tr><td style={{ color: 'var(--text-secondary)', width: '140px' }}>Driver</td><td className="mono">{data?.Driver || '—'}</td></tr>
              <tr><td style={{ color: 'var(--text-secondary)' }}>Category</td><td><span style={{ color: 'var(--accent)' }}>{cat}</span></td></tr>
              <tr><td style={{ color: 'var(--text-secondary)' }}>Mountpoint</td><td className="mono" style={{ fontSize: '0.8rem' }}>{data?.Mountpoint || '—'}</td></tr>
              <tr><td style={{ color: 'var(--text-secondary)' }}>Scope</td><td>{data?.Scope || 'local'}</td></tr>
              <tr><td style={{ color: 'var(--text-secondary)' }}>Created</td><td>{data?.CreatedAt || '—'}</td></tr>
            </tbody>
          </table>
        </div>

        {/* Migration advice */}
        <div className="card" style={{ borderLeft: '3px solid var(--accent)' }}>
          <h3>Migration Advice</h3>
          <div style={{ fontSize: '0.85rem', color: 'var(--text-primary)' }}>{advice}</div>
        </div>

        {/* Size info */}
        {(data?.SizeBytes !== undefined || data?.SizeBytes !== null) && (
          <div className="card">
            <h3>Size &amp; Usage</h3>
            <table>
              <tbody>
                <tr><td style={{ color: 'var(--text-secondary)', width: '140px' }}>Size</td><td className="mono">{bytesToSize(data?.SizeBytes)}</td></tr>
                {data?.FileCount !== undefined && (
                  <tr><td style={{ color: 'var(--text-secondary)' }}>File Count</td><td className="mono">{data?.FileCount}</td></tr>
                )}
                {data?.LastModified && (
                  <tr><td style={{ color: 'var(--text-secondary)' }}>Last Modified</td><td>{data?.LastModified}</td></tr>
                )}
              </tbody>
            </table>
          </div>
        )}

        {/* Used by containers */}
        {usedBy.length > 0 && (
          <div className="card">
            <h3>Used By ({usedBy.length} container{usedBy.length > 1 ? 's' : ''})</h3>
            <table>
              <thead>
                <tr>
                  <th>Container</th>
                  <th>Mount Path</th>
                  <th>Mode</th>
                </tr>
              </thead>
              <tbody>
                {usedBy.map((c, i) => (
                  <tr key={i}>
                    <td className="mono" style={{ fontSize: '0.8rem' }}>
                      {c.Name || c.ID?.substring(0, 12) || '—'}
                    </td>
                    <td className="mono" style={{ fontSize: '0.75rem' }}>
                      {c.MountPath || c.Destination || '—'}
                    </td>
                    <td>{c.Mode || c.RW ? 'rw' : 'ro'}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        {/* Options */}
        {options && Object.keys(options).length > 0 && (
          <div className="card">
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
              <h3 style={{ margin: 0 }}>Options</h3>
              <button
                className="btn-sm"
                onClick={handleRevealOptions}
                style={revealConfirm ? { background: 'var(--yellow-dim)', borderColor: 'var(--yellow)', color: '#fff' } : {}}
              >
                {revealedOpts ? '🔓 Revealed' : revealConfirm ? '⚠ Confirm Reveal' : '🔒 Reveal'}
              </button>
            </div>
            {revealConfirm && (
              <div style={{ fontSize: '0.8rem', color: 'var(--yellow)', marginBottom: '8px' }}>
                ⚠ This action will be logged. Click again to confirm.
              </div>
            )}
            <table>
              <tbody>
                {Object.entries(options).map(([k, v]) => (
                  <tr key={k}>
                    <td style={{ color: 'var(--text-secondary)', width: '140px' }} className="mono">{k}</td>
                    <td className="mono" style={{ fontSize: '0.8rem' }}>{String(v)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </Modal>
  );
}
