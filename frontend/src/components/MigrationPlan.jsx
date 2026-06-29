import { useState, useCallback } from 'react';

const TRANSFER_METHODS = [
  { id: 'scp', label: 'SCP', desc: 'Secure copy over SSH. Good for single files/small volumes.', icon: '🔒' },
  { id: 'rsync-over-ssh', label: 'Rsync over SSH', desc: 'Delta-transfer over SSH. Best for large volumes with incremental changes.', icon: '🔄' },
  { id: 'pipe-direct', label: 'Pipe Direct', desc: 'Direct pipe via SSH (docker export | ssh docker import). No temp files.', icon: '⚡' },
  { id: 'export-s3', label: 'Export to S3', desc: 'Archive to S3 bucket, download on target. Good for cross-region.', icon: '☁️' },
];

const COMPRESSION_LEVELS = [
  { id: 'pigz', label: 'pigz (parallel gzip)', speed: 'Fast', ratio: 'Medium', est: '~2:1' },
  { id: 'zstd', label: 'zstd', speed: 'Very Fast', ratio: 'High', est: '~3:1' },
  { id: 'lz4', label: 'lz4', speed: 'Extremely Fast', ratio: 'Low', est: '~1.5:1' },
  { id: 'none', label: 'No Compression', speed: 'N/A', ratio: 'None', est: '1:1' },
];

export default function MigrationPlan({ plan = {}, volumes = [], onUpdate }) {
  const [transferMethod, setTransferMethod] = useState(plan.transferMethod || 'rsync-over-ssh');
  const [compression, setCompression] = useState(plan.compression || 'pigz');
  const [postOptions, setPostOptions] = useState({
    startOnTarget: plan.startOnTarget !== false,
    verifyConnectivity: plan.verifyConnectivity !== false,
    removeFromSource: false,
    rotateCredentials: false,
  });
  const [volumeOverrides, setVolumeOverrides] = useState({});

  const handleTransferChange = useCallback((method) => {
    setTransferMethod(method);
    if (onUpdate) onUpdate({ transfer_method: method, compression, post_options: postOptions, volume_overrides: volumeOverrides });
  }, [compression, postOptions, volumeOverrides, onUpdate]);

  const handleCompressionChange = useCallback((comp) => {
    setCompression(comp);
    if (onUpdate) onUpdate({ transfer_method: transferMethod, compression: comp, post_options: postOptions, volume_overrides: volumeOverrides });
  }, [transferMethod, postOptions, volumeOverrides, onUpdate]);

  const handlePostOption = useCallback((key) => {
    const updated = { ...postOptions, [key]: !postOptions[key] };
    setPostOptions(updated);
    if (onUpdate) onUpdate({ transfer_method: transferMethod, compression, post_options: updated, volume_overrides: volumeOverrides });
  }, [transferMethod, compression, volumeOverrides, onUpdate]);

  const handleVolumeOverride = useCallback((volName, field, value) => {
    const updated = { ...volumeOverrides, [volName]: { ...(volumeOverrides[volName] || {}), [field]: value } };
    setVolumeOverrides(updated);
    if (onUpdate) onUpdate({ transfer_method: transferMethod, compression, post_options: postOptions, volume_overrides: updated });
  }, [transferMethod, compression, postOptions, onUpdate]);

  const selectedMethod = TRANSFER_METHODS.find(m => m.id === transferMethod);
  const selectedCompression = COMPRESSION_LEVELS.find(c => c.id === compression);

  return (
    <div style={{ display: 'grid', gap: '20px' }}>
      {/* Transfer Method */}
      <div className="card">
        <h3>Transfer Method</h3>
        <div style={{ display: 'grid', gap: '10px' }}>
          {TRANSFER_METHODS.map((m) => (
            <label
              key={m.id}
              style={{
                display: 'flex',
                alignItems: 'flex-start',
                gap: '12px',
                padding: '12px',
                border: `2px solid ${transferMethod === m.id ? 'var(--accent)' : 'var(--border)'}`,
                borderRadius: '8px',
                cursor: 'pointer',
                background: transferMethod === m.id ? 'var(--bg-tertiary)' : 'transparent',
                transition: 'all 0.15s',
              }}
            >
              <input
                type="radio"
                name="transfer_method"
                value={m.id}
                checked={transferMethod === m.id}
                onChange={() => handleTransferChange(m.id)}
                style={{ marginTop: '2px', accentColor: 'var(--accent)' }}
              />
              <div style={{ flex: 1 }}>
                <div style={{ fontWeight: 600, marginBottom: '4px' }}>
                  {m.icon} {m.label}
                </div>
                <div style={{ fontSize: '0.8rem', color: 'var(--text-secondary)' }}>
                  {m.desc}
                </div>
              </div>
            </label>
          ))}
        </div>
      </div>

      {/* Compression */}
      <div className="card">
        <h3>Compression</h3>
        <table>
          <thead>
            <tr>
              <th style={{ width: '40px' }}></th>
              <th>Algorithm</th>
              <th>Speed</th>
              <th>Ratio</th>
              <th>Est. Compression</th>
            </tr>
          </thead>
          <tbody>
            {COMPRESSION_LEVELS.map((c) => (
              <tr
                key={c.id}
                onClick={() => handleCompressionChange(c.id)}
                style={{
                  cursor: 'pointer',
                  background: compression === c.id ? 'var(--bg-tertiary)' : 'transparent',
                }}
              >
                <td>
                  <input
                    type="radio"
                    name="compression"
                    value={c.id}
                    checked={compression === c.id}
                    onChange={() => handleCompressionChange(c.id)}
                    style={{ accentColor: 'var(--accent)' }}
                  />
                </td>
                <td className="mono" style={{ fontWeight: compression === c.id ? 600 : 400 }}>
                  {c.label}
                </td>
                <td>{c.speed}</td>
                <td>{c.ratio}</td>
                <td className="mono">{c.est}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {/* Post-Migration Options */}
      <div className="card">
        <h3>Post-Migration Actions</h3>
        <div style={{ display: 'grid', gap: '8px' }}>
          <label style={{ display: 'flex', alignItems: 'center', gap: '10px', cursor: 'pointer' }}>
            <input
              type="checkbox"
              checked={postOptions.startOnTarget}
              onChange={() => handlePostOption('startOnTarget')}
              style={{ accentColor: 'var(--accent)', width: '16px', height: '16px' }}
            />
            <span>Start container on target host</span>
          </label>
          <label style={{ display: 'flex', alignItems: 'center', gap: '10px', cursor: 'pointer' }}>
            <input
              type="checkbox"
              checked={postOptions.verifyConnectivity}
              onChange={() => handlePostOption('verifyConnectivity')}
              style={{ accentColor: 'var(--accent)', width: '16px', height: '16px' }}
            />
            <span>Verify connectivity after migration</span>
          </label>
          <label style={{ display: 'flex', alignItems: 'center', gap: '10px', cursor: 'pointer' }}>
            <input
              type="checkbox"
              checked={postOptions.removeFromSource}
              onChange={() => handlePostOption('removeFromSource')}
              style={{ accentColor: 'var(--accent)', width: '16px', height: '16px' }}
            />
            <span style={{ color: postOptions.removeFromSource ? 'var(--red)' : 'inherit' }}>
              ⚠ Remove container from source host
            </span>
          </label>
          <label style={{ display: 'flex', alignItems: 'center', gap: '10px', cursor: 'pointer' }}>
            <input
              type="checkbox"
              checked={postOptions.rotateCredentials}
              onChange={() => handlePostOption('rotateCredentials')}
              style={{ accentColor: 'var(--accent)', width: '16px', height: '16px' }}
            />
            <span>Rotate credentials after migration</span>
          </label>
        </div>
      </div>

      {/* Volume Overrides */}
      {volumes.length > 0 && (
        <div className="card">
          <h3>Per-Volume Transfer Overrides</h3>
          <div style={{ fontSize: '0.8rem', color: 'var(--text-secondary)', marginBottom: '12px' }}>
            Customize transfer method or target path per volume
          </div>
          <table>
            <thead>
              <tr>
                <th>Volume</th>
                <th>Size</th>
                <th>Transfer Method</th>
                <th>Custom Path</th>
              </tr>
            </thead>
            <tbody>
              {volumes.map((v) => {
                const override = volumeOverrides[v.name] || {};
                return (
                  <tr key={v.name}>
                    <td className="mono" style={{ fontWeight: 500 }}>{v.name}</td>
                    <td className="mono" style={{ fontSize: '0.8rem' }}>
                      {v.sizeBytes ? `${(v.sizeBytes / 1073741824).toFixed(1)} GB` : '—'}
                    </td>
                    <td>
                      <select
                        value={override.transferMethod || v.transferMethod || transferMethod}
                        onChange={(e) => handleVolumeOverride(v.name, 'transfer_method', e.target.value)}
                        style={{ fontSize: '0.75rem', padding: '4px 8px' }}
                      >
                        <option value="inherit">Inherit ({v.transferMethod || transferMethod})</option>
                        {TRANSFER_METHODS.map(m => (
                          <option key={m.id} value={m.id}>{m.label}</option>
                        ))}
                      </select>
                    </td>
                    <td>
                      <input
                        type="text"
                        value={override.custom_path || ''}
                        onChange={(e) => handleVolumeOverride(v.name, 'custom_path', e.target.value)}
                        placeholder="Default path"
                        style={{ fontSize: '0.75rem', padding: '4px 8px', width: '180px' }}
                      />
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}

      {/* Summary */}
      <div className="card" style={{ borderLeft: '3px solid var(--accent)' }}>
        <h3>Migration Summary</h3>
        <div style={{ display: 'grid', gap: '6px', fontSize: '0.85rem' }}>
          <div>
            <span style={{ color: 'var(--text-secondary)' }}>Method: </span>
            <span className="mono">{selectedMethod?.label || transferMethod}</span>
          </div>
          <div>
            <span style={{ color: 'var(--text-secondary)' }}>Compression: </span>
            <span className="mono">{selectedCompression?.label || compression}</span>
          </div>
          <div>
            <span style={{ color: 'var(--text-secondary)' }}>Start on target: </span>
            <span>{postOptions.startOnTarget ? 'Yes ✓' : 'No'}</span>
          </div>
          <div>
            <span style={{ color: 'var(--text-secondary)' }}>Verify connectivity: </span>
            <span>{postOptions.verifyConnectivity ? 'Yes ✓' : 'No'}</span>
          </div>
          {plan.estimatedSizeBytes > 0 && (
            <div>
              <span style={{ color: 'var(--text-secondary)' }}>Estimated size: </span>
              <span className="mono">
                {(plan.estimatedSizeBytes / 1073741824).toFixed(1)} GB
                {plan.compressed ? ' (compressed)' : ''}
              </span>
            </div>
          )}
          {volumes.length > 0 && (
            <div>
              <span style={{ color: 'var(--text-secondary)' }}>Volumes: </span>
              <span>{volumes.length} volume{volumes.length > 1 ? 's' : ''}</span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
