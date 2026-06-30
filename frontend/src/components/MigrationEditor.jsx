import { useState, useCallback, useEffect } from 'react';
import { api } from '../api/client';
import { useToast } from './Toast';

const TABS = ['Stack', 'Volumes', 'Databases', 'Env Vars', 'Images'];

function formatBytes(bytes) {
  if (!bytes && bytes !== 0) return '—';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function getDbTypeStr(dbType) {
  if (!dbType) return '—';
  const keys = Object.keys(dbType);
  if (keys.length === 0) return '—';
  if (dbType.Other) return dbType.Other;
  return keys[0];
}

const VOLUME_DRIVERS = ['local', 'nfs', 'overlay2', 'btrfs', 'zfs', 'tmpfs', 'cifs'];

export default function MigrationEditor({ plan, onSave }) {
  const toast = useToast();
  const [tab, setTab] = useState('Stack');
  const [saving, setSaving] = useState(false);
  const [dirty, setDirty] = useState(false);

  // Editable state — seeded from plan
  const [targetStackName, setTargetStackName] = useState(plan?.targetStackName || plan?.stackName || '');
  const [volumes, setVolumes] = useState([]);
  const [databases, setDatabases] = useState([]);
  const [envVars, setEnvVars] = useState([]);
  const [images, setImages] = useState([]);

  // Reveal state for passwords
  const [revealedPasswords, setRevealedPasswords] = useState({});

  useEffect(() => {
    setTargetStackName(plan?.targetStackName || plan?.stackName || '');
    setVolumes((plan?.volumes || []).map(v => ({ ...v })));
    setDatabases((plan?.databases || []).map(d => ({
      ...d,
      preTransferCommands: [...(d.preTransferCommands || [])],
      postTransferCommands: [...(d.postTransferCommands || [])],
    })));
    setEnvVars((plan?.envVars || []).map(e => ({ ...e })));
    setImages((plan?.images || []).map(im => ({ ...im })));
    setDirty(false);
  }, [plan]);

  const markDirty = useCallback(() => setDirty(true), []);

  // ── Stack tab ──
  const handleStackNameChange = useCallback((val) => {
    setTargetStackName(val);
    markDirty();
  }, [markDirty]);

  // ── Volumes tab ──
  const handleVolumeChange = useCallback((idx, field, value) => {
    setVolumes(prev => {
      const next = [...prev];
      next[idx] = { ...next[idx], [field]: value };
      return next;
    });
    markDirty();
  }, [markDirty]);

  // ── Databases tab ──
  const handleDbChange = useCallback((idx, field, value) => {
    setDatabases(prev => {
      const next = [...prev];
      next[idx] = { ...next[idx], [field]: value };
      return next;
    });
    markDirty();
  }, [markDirty]);

  const handleDbCommandsChange = useCallback((idx, field, value) => {
    setDatabases(prev => {
      const next = [...prev];
      next[idx] = { ...next[idx], [field]: value.split('\n') };
      return next;
    });
    markDirty();
  }, [markDirty]);

  // ── Env Vars tab ──
  const handleEnvChange = useCallback((idx, field, value) => {
    setEnvVars(prev => {
      const next = [...prev];
      next[idx] = { ...next[idx], [field]: value };
      return next;
    });
    markDirty();
  }, [markDirty]);

  const handleAddEnvVar = useCallback(() => {
    setEnvVars(prev => [...prev, { serviceName: '', varName: '', sourceValue: '', targetValue: '', isSensitive: false, willBreak: false, breakReason: '' }]);
    markDirty();
  }, [markDirty]);

  const handleRemoveEnvVar = useCallback((idx) => {
    setEnvVars(prev => prev.filter((_, i) => i !== idx));
    markDirty();
  }, [markDirty]);

  // ── Images tab ──
  const handleImageChange = useCallback((idx, field, value) => {
    setImages(prev => {
      const next = [...prev];
      next[idx] = { ...next[idx], [field]: value };
      return next;
    });
    markDirty();
  }, [markDirty]);

  // ── Save ──
  const handleSave = useCallback(async () => {
    if (!plan?.planId) return;
    setSaving(true);
    try {
      const edits = {
        targetStackName,
        volumes,
        databases: databases.map(d => ({
          ...d,
          preTransferCommands: d.preTransferCommands || [],
          postTransferCommands: d.postTransferCommands || [],
        })),
        envVars,
        images,
      };
      const updated = await api.post(`/api/migration/unified/plan/${plan.planId}/edit`, edits);
      setDirty(false);
      toast('Changes saved', 'success');
      if (onSave) onSave(updated);
    } catch (err) {
      toast('Save failed: ' + err.message, 'error');
    } finally {
      setSaving(false);
    }
  }, [plan, targetStackName, volumes, databases, envVars, images, toast, onSave]);

  const togglePassword = useCallback((idx) => {
    setRevealedPasswords(prev => ({ ...prev, [idx]: !prev[idx] }));
  }, []);

  // ── Render tabs ──
  const renderTabContent = () => {
    switch (tab) {
      // ── Stack ──
      case 'Stack':
        return (
          <div style={{ display: 'grid', gap: '16px' }}>
            <div className="card">
              <h3>Target Stack Name</h3>
              <input
                type="text"
                value={targetStackName}
                onChange={e => handleStackNameChange(e.target.value)}
                placeholder={plan?.stackName || 'Stack name'}
                style={{ width: '100%', maxWidth: '400px', marginTop: '8px' }}
              />
            </div>
            <div className="card">
              <h3>Architecture</h3>
              <div style={{ display: 'grid', gap: '8px', marginTop: '8px' }}>
                <div>
                  <span style={{ color: 'var(--text-secondary)' }}>Source: </span>
                  <strong>{plan?.sourceArchitecture || 'unknown'}</strong>
                </div>
                <div>
                  <span style={{ color: 'var(--text-secondary)' }}>Target: </span>
                  <strong>{plan?.targetArchitecture || 'unknown'}</strong>
                </div>
                {plan?.sourceArchitecture && plan?.targetArchitecture &&
                 plan.sourceArchitecture !== plan.targetArchitecture && (
                  <div style={{
                    padding: '10px 14px',
                    background: 'var(--yellow-dim)',
                    borderRadius: '6px',
                    color: 'var(--yellow)',
                    fontSize: '0.85rem',
                  }}>
                    ⚠ Architecture mismatch — images built for {plan.sourceArchitecture} may not run on {plan.targetArchitecture}
                  </div>
                )}
                {plan?.sourceArchitecture && plan?.targetArchitecture &&
                 plan.sourceArchitecture === plan.targetArchitecture && (
                  <div style={{
                    padding: '10px 14px',
                    background: 'var(--green-dim)',
                    borderRadius: '6px',
                    color: 'var(--green)',
                    fontSize: '0.85rem',
                  }}>
                    ✓ Architecture match: {plan.sourceArchitecture} → {plan.targetArchitecture}
                  </div>
                )}
              </div>
            </div>
            {plan?.migrationType && (
              <div className="card">
                <h3>Migration Type</h3>
                <div style={{ marginTop: '8px' }}>
                  <span style={{
                    display: 'inline-block',
                    padding: '4px 12px',
                    background: 'var(--accent-dim)',
                    borderRadius: '12px',
                    fontSize: '0.8rem',
                    fontWeight: 600,
                    color: 'var(--accent)',
                  }}>
                    {plan.migrationType === 'compose' ? '📚 Compose Stack' : '📦 Container'}
                  </span>
                </div>
              </div>
            )}
            {plan?.estimatedSizeBytes > 0 && (
              <div className="card" style={{ borderLeft: '3px solid var(--accent)' }}>
                <h3>Estimated Transfer Size</h3>
                <div className="mono" style={{ fontSize: '1.2rem', color: 'var(--accent)', marginTop: '4px' }}>
                  {formatBytes(plan.estimatedSizeBytes)}
                </div>
              </div>
            )}
          </div>
        );

      // ── Volumes ──
      case 'Volumes':
        return (
          <div className="card">
            <h3>Volume Configuration ({volumes.length})</h3>
            {volumes.length === 0 ? (
              <div style={{ color: 'var(--text-secondary)', padding: '24px', textAlign: 'center' }}>
                No volumes to configure
              </div>
            ) : (
              <div style={{ overflowX: 'auto' }}>
                <table>
                  <thead>
                    <tr>
                      <th>Source Name</th>
                      <th>Target Name</th>
                      <th>Driver</th>
                      <th>Size</th>
                      <th>Skip</th>
                    </tr>
                  </thead>
                  <tbody>
                    {volumes.map((v, idx) => (
                      <tr key={idx} style={{ opacity: v.skip ? 0.4 : 1 }}>
                        <td className="mono" style={{ fontWeight: 500 }}>{v.sourceName || '—'}</td>
                        <td>
                          <input
                            type="text"
                            value={v.targetName || ''}
                            onChange={e => handleVolumeChange(idx, 'targetName', e.target.value)}
                            placeholder={v.sourceName || ''}
                            style={{ fontSize: '0.8rem', padding: '4px 8px', width: '140px' }}
                            disabled={v.skip}
                          />
                        </td>
                        <td>
                          <select
                            value={v.targetDriver || v.driver || 'local'}
                            onChange={e => handleVolumeChange(idx, 'targetDriver', e.target.value)}
                            style={{ fontSize: '0.75rem', padding: '4px 8px' }}
                            disabled={v.skip}
                          >
                            {VOLUME_DRIVERS.map(d => (
                              <option key={d} value={d}>{d}</option>
                            ))}
                          </select>
                        </td>
                        <td className="mono" style={{ fontSize: '0.8rem' }}>
                          {formatBytes(v.sizeBytes)}
                        </td>
                        <td>
                          <input
                            type="checkbox"
                            checked={v.skip || false}
                            onChange={e => handleVolumeChange(idx, 'skip', e.target.checked)}
                            style={{ accentColor: 'var(--red)', width: '16px', height: '16px', cursor: 'pointer' }}
                          />
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>
        );

      // ── Databases ──
      case 'Databases':
        return (
          <div style={{ display: 'grid', gap: '16px' }}>
            {databases.length === 0 ? (
              <div className="card">
                <div style={{ color: 'var(--text-secondary)', padding: '24px', textAlign: 'center' }}>
                  No database services detected
                </div>
              </div>
            ) : (
              databases.map((db, idx) => (
                <div key={idx} className="card" style={{ borderLeft: '3px solid var(--accent)' }}>
                  <h3>{db.serviceName || `Database ${idx + 1}`}</h3>

                  {/* Service info row */}
                  <div style={{
                    display: 'flex', flexWrap: 'wrap', gap: '8px', marginBottom: '16px',
                    padding: '8px 12px', background: 'var(--bg-tertiary)', borderRadius: '6px',
                    fontSize: '0.8rem',
                  }}>
                    <span><strong>Type:</strong> {getDbTypeStr(db.dbType)}</span>
                    <span style={{ marginLeft: '12px' }}><strong>Image:</strong> {db.image || '—'}</span>
                    {db.version && <span style={{ marginLeft: '12px' }}><strong>Version:</strong> {db.version}</span>}
                    {db.hasReplication && (
                      <span style={{ marginLeft: '12px', color: 'var(--green)' }}>🔗 Replication</span>
                    )}
                  </div>

                  {/* Editable fields */}
                  <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '12px', marginBottom: '16px' }}>
                    <div>
                      <label style={{ fontSize: '0.75rem', fontWeight: 600, color: 'var(--text-secondary)', display: 'block', marginBottom: '4px' }}>
                        Username
                      </label>
                      <input
                        type="text"
                        value={db.username || ''}
                        onChange={e => handleDbChange(idx, 'username', e.target.value)}
                        placeholder="username"
                        style={{ width: '100%' }}
                      />
                    </div>
                    <div>
                      <label style={{ fontSize: '0.75rem', fontWeight: 600, color: 'var(--text-secondary)', display: 'block', marginBottom: '4px' }}>
                        Password
                      </label>
                      <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
                        <input
                          type={revealedPasswords[idx] ? 'text' : 'password'}
                          value={db.password || ''}
                          onChange={e => handleDbChange(idx, 'password', e.target.value)}
                          placeholder="password"
                          style={{ flex: 1 }}
                        />
                        <button
                          className="btn-sm"
                          onClick={() => togglePassword(idx)}
                          style={{ fontSize: '0.7rem', padding: '2px 10px', whiteSpace: 'nowrap' }}
                        >
                          {revealedPasswords[idx] ? 'Hide' : 'Show'}
                        </button>
                      </div>
                      {db.passwordMasked && !db.password && (
                        <div style={{ fontSize: '0.7rem', color: 'var(--text-secondary)', marginTop: '2px' }}>
                          Detected: {db.passwordMasked}
                        </div>
                      )}
                    </div>
                    <div>
                      <label style={{ fontSize: '0.75rem', fontWeight: 600, color: 'var(--text-secondary)', display: 'block', marginBottom: '4px' }}>
                        Port
                      </label>
                      <input
                        type="text"
                        value={db.port || ''}
                        onChange={e => handleDbChange(idx, 'port', e.target.value)}
                        placeholder="5432"
                        style={{ width: '100%', maxWidth: '120px' }}
                      />
                    </div>
                    <div>
                      <label style={{ fontSize: '0.75rem', fontWeight: 600, color: 'var(--text-secondary)', display: 'block', marginBottom: '4px' }}>
                        Database Name
                      </label>
                      <input
                        type="text"
                        value={db.databaseName || ''}
                        onChange={e => handleDbChange(idx, 'databaseName', e.target.value)}
                        placeholder="database"
                        style={{ width: '100%' }}
                      />
                    </div>
                  </div>

                  {/* Pre/post transfer commands */}
                  <div style={{ marginBottom: '12px' }}>
                    <label style={{ fontSize: '0.75rem', fontWeight: 600, color: 'var(--text-secondary)', display: 'block', marginBottom: '4px' }}>
                      Pre-transfer Commands (one per line)
                    </label>
                    <textarea
                      value={(db.preTransferCommands || []).join('\n')}
                      onChange={e => handleDbCommandsChange(idx, 'preTransferCommands', e.target.value)}
                      placeholder="e.g., pg_dump -U postgres mydb > /tmp/dump.sql"
                      rows={3}
                      style={{ width: '100%', fontSize: '0.75rem', fontFamily: 'monospace' }}
                    />
                  </div>
                  <div>
                    <label style={{ fontSize: '0.75rem', fontWeight: 600, color: 'var(--text-secondary)', display: 'block', marginBottom: '4px' }}>
                      Post-transfer Commands (one per line)
                    </label>
                    <textarea
                      value={(db.postTransferCommands || []).join('\n')}
                      onChange={e => handleDbCommandsChange(idx, 'postTransferCommands', e.target.value)}
                      placeholder="e.g., psql -U postgres -d mydb < /tmp/dump.sql"
                      rows={3}
                      style={{ width: '100%', fontSize: '0.75rem', fontFamily: 'monospace' }}
                    />
                  </div>
                </div>
              ))
            )}
          </div>
        );

      // ── Env Vars ──
      case 'Env Vars':
        return (
          <div className="card">
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '12px' }}>
              <h3 style={{ margin: 0 }}>Environment Variables ({envVars.length})</h3>
              <button className="btn" onClick={handleAddEnvVar}>
                + Add Variable
              </button>
            </div>
            {envVars.length === 0 ? (
              <div style={{ color: 'var(--text-secondary)', padding: '24px', textAlign: 'center' }}>
                No environment variables. Click "+ Add Variable" to add one.
              </div>
            ) : (
              <div style={{ overflowX: 'auto' }}>
                <table>
                  <thead>
                    <tr>
                      <th style={{ width: '15%' }}>Service</th>
                      <th style={{ width: '20%' }}>Variable</th>
                      <th style={{ width: '25%' }}>Source Value</th>
                      <th style={{ width: '25%' }}>Target Value</th>
                      <th style={{ width: '10%' }}>⏘</th>
                      <th style={{ width: '5%' }}></th>
                    </tr>
                  </thead>
                  <tbody>
                    {envVars.map((ev, idx) => (
                      <tr key={idx}>
                        <td>
                          <input
                            type="text"
                            value={ev.serviceName || ''}
                            onChange={e => handleEnvChange(idx, 'serviceName', e.target.value)}
                            placeholder="service"
                            style={{ fontSize: '0.75rem', padding: '4px 8px', width: '100%' }}
                          />
                        </td>
                        <td>
                          <input
                            type="text"
                            value={ev.varName || ''}
                            onChange={e => handleEnvChange(idx, 'varName', e.target.value)}
                            placeholder="VAR_NAME"
                            style={{ fontSize: '0.75rem', padding: '4px 8px', width: '100%' }}
                          />
                        </td>
                        <td style={{ fontSize: '0.8rem' }}>
                          {ev.isSensitive && ev.sourceValue ? (
                            <span style={{ letterSpacing: '2px', color: 'var(--text-secondary)' }}>••••••••</span>
                          ) : (
                            <span style={{ color: 'var(--text-secondary)' }}>{ev.sourceValue || '—'}</span>
                          )}
                        </td>
                        <td>
                          <input
                            type={ev.isSensitive ? 'password' : 'text'}
                            value={ev.targetValue || ''}
                            onChange={e => handleEnvChange(idx, 'targetValue', e.target.value)}
                            placeholder={ev.sourceValue ? 'Override target value' : 'value'}
                            style={{ fontSize: '0.75rem', padding: '4px 8px', width: '100%' }}
                          />
                        </td>
                        <td>
                          {ev.willBreak && (
                            <span
                              title={ev.breakReason || 'May break after migration'}
                              style={{
                                color: 'var(--red)',
                                fontWeight: 600,
                                fontSize: '1rem',
                                cursor: 'help',
                              }}
                            >
                              ⚠
                            </span>
                          )}
                        </td>
                        <td>
                          <button
                            className="btn-sm"
                            onClick={() => handleRemoveEnvVar(idx)}
                            style={{ color: 'var(--red)', padding: '2px 8px', fontSize: '0.8rem' }}
                            title="Remove variable"
                          >
                            ✕
                          </button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>
        );

      // ── Images ──
      case 'Images':
        return (
          <div className="card">
            <h3>Image Overrides ({images.length})</h3>
            {images.length === 0 ? (
              <div style={{ color: 'var(--text-secondary)', padding: '24px', textAlign: 'center' }}>
                No image overrides needed
              </div>
            ) : (
              <table>
                <thead>
                  <tr>
                    <th>Service</th>
                    <th>Old Image</th>
                    <th>New Image</th>
                    <th>Major Version Change</th>
                  </tr>
                </thead>
                <tbody>
                  {images.map((im, idx) => (
                    <tr key={idx}>
                      <td style={{ fontWeight: 500 }}>{im.serviceName || '—'}</td>
                      <td className="mono" style={{ fontSize: '0.8rem', color: 'var(--text-secondary)' }}>
                        {im.oldImage || '—'}
                      </td>
                      <td>
                        <input
                          type="text"
                          value={im.newImage || ''}
                          onChange={e => handleImageChange(idx, 'newImage', e.target.value)}
                          placeholder={im.oldImage || ''}
                          style={{ fontSize: '0.8rem', padding: '4px 8px', width: '220px' }}
                        />
                      </td>
                      <td>
                        {im.majorVersionChange ? (
                          <span style={{
                            display: 'inline-block',
                            padding: '2px 8px',
                            background: 'var(--yellow-dim)',
                            borderRadius: '8px',
                            fontSize: '0.7rem',
                            color: 'var(--yellow)',
                            fontWeight: 600,
                          }}>
                            ⚠ Major
                          </span>
                        ) : (
                          <span style={{ color: 'var(--text-secondary)', fontSize: '0.8rem' }}>—</span>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        );

      default:
        return null;
    }
  };

  return (
    <div>
      {/* Tab bar */}
      <div style={{
        display: 'flex',
        gap: '0',
        marginBottom: '20px',
        background: 'var(--bg-secondary)',
        borderRadius: '8px',
        padding: '4px',
        border: '1px solid var(--border)',
      }}>
        {TABS.map(t => (
          <button
            key={t}
            onClick={() => setTab(t)}
            style={{
              flex: 1,
              padding: '10px 16px',
              textAlign: 'center',
              borderRadius: '6px',
              background: tab === t ? 'var(--accent-dim)' : 'transparent',
              color: tab === t ? 'var(--accent)' : 'var(--text-secondary)',
              fontWeight: tab === t ? 600 : 400,
              fontSize: '0.85rem',
              border: 'none',
              cursor: 'pointer',
              transition: 'all 0.15s',
            }}
          >
            {t}
          </button>
        ))}
      </div>

      {/* Tab content */}
      <div style={{ minHeight: '300px' }}>
        {renderTabContent()}
      </div>

      {/* Save bar */}
      <div style={{
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
        marginTop: '20px',
        padding: '16px',
        background: 'var(--bg-secondary)',
        borderRadius: '8px',
        border: '1px solid var(--border)',
      }}>
        <span style={{ fontSize: '0.85rem', color: dirty ? 'var(--yellow)' : 'var(--text-secondary)' }}>
          {dirty ? '⚠ Unsaved changes' : '✓ All changes saved'}
        </span>
        <button
          className="btn-primary"
          onClick={handleSave}
          disabled={saving}
        >
          {saving ? 'Saving...' : '💾 Save Changes'}
        </button>
      </div>
    </div>
  );
}
