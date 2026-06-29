import { useState, useCallback, useEffect } from 'react';
import { api } from '../api/client';
import Modal from '../components/Modal';
import Spinner from '../components/Spinner';
import { useToast } from '../components/Toast';
import ConnectionReview from '../components/ConnectionReview';
import MigrationPlan from '../components/MigrationPlan';
import VolumeInspector from '../components/VolumeInspector';

const TOTAL_STEPS = 9;

const STEP_LABELS = [
  'Select Source',
  'Analyze',
  'Strategy',
  'Credentials',
  'Connection Fixes',
  'Target',
  'Dry Run',
  'Execute',
  'Verify',
];

/** Convert backend string-array env vars to object array for frontend display */
const parseEnvVars = (envVarStrings) => {
  if (!Array.isArray(envVarStrings)) return [];
  return envVarStrings.map(s => {
    const idx = s.indexOf('=');
    const name = idx > 0 ? s.substring(0, idx) : s;
    const value = idx > 0 ? s.substring(idx + 1) : '';
    const isSensitive = /password|secret|key|token|credential|auth/i.test(name);
    return {
      name,
      value,
      valueMasked: isSensitive ? (value.length <= 4 ? '***' : value.substring(0, 2) + '***') : value,
      isSensitive,
    };
  });
};

function StepIndicator({ currentStep, completedSteps = new Set() }) {
  return (
    <div style={{
      display: 'flex',
      alignItems: 'center',
      gap: '0',
      marginBottom: '24px',
      padding: '16px 0',
      overflow: 'hidden',
      flexWrap: 'nowrap',
    }}>
      {STEP_LABELS.map((label, i) => {
        const step = i + 1;
        const isComplete = completedSteps.has(step);
        const isCurrent = currentStep === step;
        const isPending = step > currentStep && !isComplete;

        return (
          <div key={step} style={{ display: 'flex', alignItems: 'center', flex: i === 0 ? '0 0 auto' : '1 1 0', minWidth: 0 }}>
            {i > 0 && (
              <div style={{
                flex: 1,
                height: '2px',
                background: isComplete ? 'var(--green)' : 'var(--border)',
                margin: '0 4px',
                minWidth: '12px',
              }} />
            )}
            <div style={{
              display: 'flex',
              flexDirection: 'column',
              alignItems: 'center',
              gap: '4px',
              flexShrink: 0,
            }}>
              <div style={{
                width: '28px',
                height: '28px',
                borderRadius: '50%',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                fontSize: '0.75rem',
                fontWeight: 700,
                background: isComplete ? 'var(--green-dim)' :
                            isCurrent ? 'var(--accent-dim)' : 'var(--bg-tertiary)',
                color: isComplete ? 'var(--green)' :
                       isCurrent ? 'var(--accent)' :
                       'var(--text-secondary)',
                border: `2px solid ${isComplete ? 'var(--green)' :
                                     isCurrent ? 'var(--accent)' : 'var(--border)'}`,
                transition: 'all 0.3s',
              }}>
                {isComplete ? '✓' :
                 isCurrent ? '▸' :
                 step}
              </div>
              <span style={{
                fontSize: '0.6rem',
                color: isCurrent ? 'var(--accent)' :
                       isComplete ? 'var(--text-primary)' : 'var(--text-secondary)',
                fontWeight: isCurrent ? 600 : 400,
                textAlign: 'center',
                maxWidth: '60px',
                overflow: 'hidden',
                textOverflow: 'ellipsis',
                whiteSpace: 'nowrap',
              }}>
                {label}
              </span>
            </div>
          </div>
        );
      })}
    </div>
  );
}

export default function Migration({ navigate }) {
  const toast = useToast();

  // Wizard state
  const [step, setStep] = useState(1);
  const [completedSteps, setCompletedSteps] = useState(new Set());
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);

  // Step 1 — Source
  const [endpoints, setEndpoints] = useState([]);
  const [sourceEndpoint, setSourceEndpoint] = useState('');
  const [containers, setContainers] = useState([]);
  const [containerSearch, setContainerSearch] = useState('');
  const [selectedContainer, setSelectedContainer] = useState(null);

  // Step 2 — Analysis results
  const [analysis, setAnalysis] = useState(null);

  // Step 3 — Strategy
  const [strategy, setStrategy] = useState({});
  const [transferMethod, setTransferMethod] = useState('rsync-over-ssh');
  const [compression, setCompression] = useState('pigz');
  const [postOptions, setPostOptions] = useState({});

  // Step 4 — Credentials
  const [credentialsRevealed, setCredentialsRevealed] = useState({});
  const [credRevealConfirm, setCredRevealConfirm] = useState(null);

  // Step 5 — Connection fixes
  const [connectionResolutions, setConnectionResolutions] = useState({});

  // Step 6 — Target
  const [targetEndpoints, setTargetEndpoints] = useState([]);
  const [targetEndpoint, setTargetEndpoint] = useState('');
  const [targetStackName, setTargetStackName] = useState('');
  const [targetInfo, setTargetInfo] = useState(null);

  // Step 7 — Dry run
  const [dryRunResult, setDryRunResult] = useState(null);

  // Step 8 — Execute
  const [execPhase, setExecPhase] = useState('pending'); // pending, running, paused, done
  const [execCommands, setExecCommands] = useState([]);
  const [currentCommandGroup, setCurrentCommandGroup] = useState(0);
  const [execResults, setExecResults] = useState({});
  const [execStartTime, setExecStartTime] = useState(null);

  // Step 9 — Verification
  const [verification, setVerification] = useState(null);

  // Shared state
  const [migrationPlan, setMigrationPlan] = useState(null);
  const [migrationId, setMigrationId] = useState(null);
  const [revealAudit, setRevealAudit] = useState([]);

  // Volume inspector
  const [inspectVolume, setInspectVolume] = useState(null);

  const auditLog = useCallback((action, detail) => {
    const entry = { timestamp: new Date().toISOString(), action, detail };
    const audit = JSON.parse(localStorage.getItem('marionette-audit-log') || '[]');
    audit.push(entry);
    localStorage.setItem('marionette-audit-log', JSON.stringify(audit));
    setRevealAudit(prev => [...prev, entry]);
  }, []);

  // Load endpoints on mount
  useEffect(() => {
    (async () => {
      try {
        const data = await api.get('/api/endpoints');
        const eps = Array.isArray(data) ? data : (data?.endpoints || []);
        setEndpoints(eps);
        setTargetEndpoints(eps);
      } catch {/* ignore */}
    })();
  }, []);

  const filteredContainers = containers.filter(c => {
    const name = (c.Name || c.name || '').toLowerCase();
    const img = (c.Image || c.image || '').toLowerCase();
    const q = containerSearch.toLowerCase();
    return !q || name.includes(q) || img.includes(q);
  });

  // === STEP 1: Select Source ===
  const handleSourceSelect = async (epId) => {
    setSourceEndpoint(epId);
    setSelectedContainer(null);
    setContainers([]);
    setLoading(true);
    setError(null);
    try {
      const data = await api.get('/api/containers');
      setContainers(Array.isArray(data) ? data : (data?.containers || []));
    } catch (err) {
      setError('Failed to load containers: ' + err.message);
    } finally {
      setLoading(false);
    }
  };

  const handleContainerSelect = (container) => {
    setSelectedContainer(container);
  };

  const handleAnalyze = async () => {
    if (!sourceEndpoint || !selectedContainer) return;
    setLoading(true);
    setError(null);
    try {
      const result = await api.post('/api/migration/analyze', {
        source_endpoint: sourceEndpoint,
        container_id: selectedContainer.Id || selectedContainer.id,
      });
      setAnalysis({ ...result, envVars: parseEnvVars(result.envVars || []) });
      setCompletedSteps(prev => new Set([...prev, 1]));
      setStep(2);
    } catch (err) {
      toast('Analysis failed: ' + err.message, 'error');
    } finally {
      setLoading(false);
    }
  };

  // === STEP 3: Strategy ===
  const handleStrategyUpdate = useCallback((s) => {
    setStrategy(s);
    if (s.transferMethod) setTransferMethod(s.transferMethod);
    if (s.compression) setCompression(s.compression);
    if (s.post_options) setPostOptions(s.post_options);
  }, []);

  const handleProceedFromStrategy = () => {
    setCompletedSteps(prev => new Set([...prev, 3]));
    setStep(4);
  };

  // === STEP 4: Credentials ===
  const handleRevealCredential = (varName) => {
    if (credRevealConfirm !== varName) {
      setCredRevealConfirm(varName);
      return;
    }
    setCredentialsRevealed(prev => ({ ...prev, [varName]: true }));
    setCredRevealConfirm(null);
    auditLog('reveal_credential', { variable: varName });
  };

  const handleCredentialsDone = () => {
    setCompletedSteps(prev => new Set([...prev, 4]));
    setStep(5);
  };

  // === STEP 5: Connection Fixes ===
  const handleConnectionUpdate = useCallback((varName, action) => {
    setConnectionResolutions(prev => ({
      ...prev,
      [varName]: { action, resolved: true },
    }));
  }, []);

  const allCriticalResolved = () => {
    const conns = analysis?.dbConnections || [];
    return conns.filter(c => c.willBreak).every(c => connectionResolutions[c.varName]?.resolved);
  };

  const handleConnectionFixesDone = () => {
    if (!allCriticalResolved()) {
      toast('Resolve all critical connections first', 'error');
      return;
    }
    setCompletedSteps(prev => new Set([...prev, 5]));
    setStep(6);
  };

  // === STEP 6: Target ===
  const handleTargetSelect = async (epId) => {
    setTargetEndpoint(epId);
    setTargetInfo(null);
    if (!epId) return;
    try {
      const data = await api.get(`/api/endpoints/${epId}/info`);
      setTargetInfo(data);
    } catch {/* ignore */}
  };

  const handleTargetNext = async () => {
    if (!targetEndpoint) {
      toast('Select a target endpoint', 'error');
      return;
    }
    setCompletedSteps(prev => new Set([...prev, 6]));
    setStep(7);
  };

  // === STEP 7: Dry Run ===
  const handleDryRun = async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await api.post('/api/migration/dry-run', {
        source_endpoint: sourceEndpoint,
        target_endpoint: targetEndpoint,
        container_id: selectedContainer?.Id || selectedContainer?.id,
        transfer_method: transferMethod,
        compression,
        post_options: postOptions,
        connection_resolutions: connectionResolutions,
        target_stack_name: targetStackName || undefined,
      });
      setDryRunResult(result.plan);
      setMigrationPlan(result.plan);
      setMigrationId(result.plan.migrationId);
    } catch (err) {
      toast('Dry run failed: ' + err.message, 'error');
    } finally {
      setLoading(false);
    }
  };

  const handleProceedToExecute = () => {
    setCompletedSteps(prev => new Set([...prev, 7]));
    setStep(8);
    setExecPhase('pending');
    setCurrentCommandGroup(0);
    setExecResults({});
    setExecCommands(dryRunResult?.commands || []);
    setExecStartTime(Date.now());
  };

  // === STEP 8: Execute ===
  const handleStartExecution = async () => {
    setExecPhase('running');
    setCurrentCommandGroup(0);
    setExecStartTime(Date.now());
    setExecResults({});

    try {
      const result = await api.post(`/api/migration/${migrationId}/execute`, {});
      const results = result.results || [];

      // Map results by index
      const resultMap = {};
      results.forEach((r, i) => {
        resultMap[r.index !== undefined ? r.index : i] = {
          time: Date.now() - execStartTime,
          status: r.exitCode === 0 ? 'done' : 'failed',
          command: r.command,
          stdout: r.stdout,
          stderr: r.stderr,
          exit_code: r.exitCode,
        };
      });
      setExecResults(resultMap);

      const allSuccess = results.every(r => r.exitCode === 0);
      setExecPhase(allSuccess ? 'done' : 'done');
      
      // Fetch updated migration plan
      try {
        if (migrationId) {
          const plan = await api.get(`/api/migration/${migrationId}`);
          setVerification(plan);
        }
      } catch {/* ignore */}

      setCompletedSteps(prev => new Set([...prev, 8]));
      // Auto-advance to step 9
      setStep(9);
    } catch (err) {
      toast('Execution failed: ' + err.message, 'error');
      setExecPhase('pending');
    }
  };

  const handleCommandGroupDone = async () => {
    const now = Date.now();
    setExecResults(prev => ({
      ...prev,
      [currentCommandGroup]: { time: now - execStartTime, status: 'done' },
    }));

    if (currentCommandGroup + 1 >= execCommands.length) {
      setExecPhase('done');
      try {
        // Fetch migration result
        if (migrationId) {
          const result = await api.get(`/api/migration/${migrationId}`);
          setVerification(result);
        }
      } catch {/* ignore */}
      setCompletedSteps(prev => new Set([...prev, 8]));
      setStep(9);
    } else {
      setCurrentCommandGroup(prev => prev + 1);
    }
  };

  const handleCancelExecution = () => {
    setExecPhase('pending');
    toast('Execution cancelled at safe point', 'info');
  };

  // === STEP 9: Verify ===
  const handlePostMigration = async (action) => {
    try {
      if (action === 'remove_source') {
        await api.delete(`/api/containers/${selectedContainer?.Id || selectedContainer?.id}`);
        toast('Container removed from source', 'success');
      } else if (action === 'rollback') {
        await api.post(`/api/migration/${migrationId}/rollback`);
        toast('Rollback initiated', 'info');
      }
    } catch (err) {
      toast('Action failed: ' + err.message, 'error');
    }
  };

  const handleDone = () => {
    navigate('dashboard');
    toast('Migration wizard complete 🎉', 'success');
  };

  // Navigation helpers
  const canGoNext = () => {
    switch (step) {
      case 1: return selectedContainer !== null;
      case 2: return analysis !== null;
      case 3: return true;
      case 4: return true;
      case 5: return allCriticalResolved();
      case 6: return targetEndpoint !== '';
      case 7: return dryRunResult !== null;
      case 8: return execPhase === 'done';
      case 9: return true;
      default: return false;
    }
  };

  const handleNext = () => {
    switch (step) {
      case 1: handleAnalyze(); break;
      case 2: setCompletedSteps(prev => new Set([...prev, 2])); setStep(3); break;
      case 3: handleProceedFromStrategy(); break;
      case 4: handleCredentialsDone(); break;
      case 5: handleConnectionFixesDone(); break;
      case 6: handleTargetNext(); break;
      case 7: handleProceedToExecute(); break;
      case 8: break; // Manual
      case 9: handleDone(); break;
    }
  };

  const handlePrev = () => {
    if (step > 1) setStep(step - 1);
  };

  // === RENDER ===

  const renderStepContent = () => {
    switch (step) {
      // ── STEP 1: Select Source ──
      case 1:
        return (
          <div style={{ display: 'grid', gap: '16px' }}>
            <div className="card">
              <h3>Source Endpoint</h3>
              <select
                value={sourceEndpoint}
                onChange={e => handleSourceSelect(e.target.value)}
                style={{ width: '100%', marginTop: '8px' }}
              >
                <option value="">— Select source endpoint —</option>
                <option value="local">Local</option>
                {endpoints.map(ep => (
                  <option key={ep.id || ep.Id} value={ep.id || ep.Id}>
                    {ep.name || ep.Name}
                  </option>
                ))}
              </select>
            </div>

            {sourceEndpoint && (
              <div className="card">
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '12px' }}>
                  <h3 style={{ margin: 0 }}>Containers</h3>
                  <input
                    type="text"
                    placeholder="Search containers..."
                    value={containerSearch}
                    onChange={e => setContainerSearch(e.target.value)}
                    style={{ width: '240px' }}
                  />
                </div>
                {loading ? (
                  <div className="loading-center"><div className="spinner" /></div>
                ) : containers.length === 0 ? (
                  <div className="text-secondary" style={{ padding: '24px', textAlign: 'center' }}>
                    No containers found on this endpoint
                  </div>
                ) : (
                  <div style={{ maxHeight: '400px', overflow: 'auto' }}>
                    <table>
                      <thead>
                        <tr>
                          <th style={{ width: '30px' }}></th>
                          <th>Name</th>
                          <th>Image</th>
                          <th>State</th>
                          <th>Stack</th>
                        </tr>
                      </thead>
                      <tbody>
                        {filteredContainers.map(c => {
                          const name = (c.Name || c.name || '').replace(/^\//, '');
                          const isCompose = (c.Labels || c.labels || {})['com.docker.compose.project'];
                          const selected = selectedContainer && (selectedContainer.Id || selectedContainer.id) === (c.Id || c.id);
                          return (
                            <tr
                              key={c.Id || c.id}
                              onClick={() => handleContainerSelect(c)}
                              style={{
                                cursor: 'pointer',
                                background: selected ? 'var(--bg-tertiary)' : 'transparent',
                                borderLeft: selected ? '3px solid var(--accent)' : '3px solid transparent',
                              }}
                            >
                              <td>
                                <input
                                  type="radio"
                                  checked={!!selected}
                                  onChange={() => handleContainerSelect(c)}
                                  style={{ accentColor: 'var(--accent)' }}
                                />
                              </td>
                              <td className="mono" style={{ fontWeight: 500, fontSize: '0.85rem' }}>{name}</td>
                              <td className="mono" style={{ fontSize: '0.8rem' }}>{c.Image || c.image || '—'}</td>
                              <td>
                                <span style={{
                                  display: 'inline-block',
                                  padding: '2px 10px',
                                  borderRadius: '12px',
                                  fontSize: '0.7rem',
                                  fontWeight: 600,
                                  color: (c.State || c.state || '').toLowerCase() === 'running' ? 'var(--green)' : 'var(--red)',
                                  background: (c.State || c.state || '').toLowerCase() === 'running' ? 'var(--green-dim)' : 'var(--red-dim)',
                                }}>
                                  {c.State || c.state || 'unknown'}
                                </span>
                              </td>
                              <td>
                                {isCompose ? (
                                  <span style={{
                                    display: 'inline-block',
                                    padding: '1px 8px',
                                    borderRadius: '10px',
                                    background: 'var(--bg-tertiary)',
                                    fontSize: '0.7rem',
                                    color: 'var(--accent)',
                                  }}>
                                    📚 {isCompose}
                                  </span>
                                ) : <span className="text-secondary">—</span>}
                              </td>
                            </tr>
                          );
                        })}
                      </tbody>
                    </table>
                  </div>
                )}
                {selectedContainer && (
                  <div style={{ marginTop: '12px', padding: '12px', background: 'var(--bg-tertiary)', borderRadius: '8px' }}>
                    <div style={{ fontWeight: 600, marginBottom: '4px' }}>
                      Selected: {(selectedContainer.Name || selectedContainer.name || '').replace(/^\//, '')}
                    </div>
                    <div className="mono" style={{ fontSize: '0.75rem', color: 'var(--text-secondary)' }}>
                      Image: {selectedContainer.Image || selectedContainer.image}
                    </div>
                    {(() => {
                      const labels = selectedContainer.Labels || selectedContainer.labels || {};
                      const project = labels['com.docker.compose.project'];
                      if (project) {
                        return (
                          <div style={{ marginTop: '8px', padding: '8px', background: 'var(--bg-secondary)', borderRadius: '4px', fontSize: '0.8rem' }}>
                            <span style={{ color: 'var(--accent)' }}>📚</span> This container is part of Compose stack <strong>{project}</strong>.
                            Consider migrating the entire stack.
                          </div>
                        );
                      }
                      return null;
                    })()}
                  </div>
                )}
              </div>
            )}
          </div>
        );

      // ── STEP 2: Analyze ──
      case 2:
        if (!analysis) return <div className="loading-center"><div className="spinner spinner-lg" /></div>;
        const warnings = analysis.warnings || [];
        const volumes = analysis.volumes || [];
        const dbConns = analysis.dbConnections || [];
        return (
          <div style={{ display: 'grid', gap: '16px' }}>
            {/* Container info */}
            <div className="card">
              <h3>Container</h3>
              <table>
                <tbody>
                  <tr><td style={{ color: 'var(--text-secondary)', width: '120px' }}>Name</td><td className="mono" style={{ fontWeight: 500 }}>{analysis.containerName || '—'}</td></tr>
                  <tr><td style={{ color: 'var(--text-secondary)' }}>Image</td><td className="mono">{analysis.image || '—'}</td></tr>
                  <tr><td style={{ color: 'var(--text-secondary)' }}>ID</td><td className="mono" style={{ fontSize: '0.8rem' }}>{analysis.containerId || '—'}</td></tr>
                </tbody>
              </table>
            </div>

            {/* Warnings */}
            {warnings.length > 0 && (
              <div className="card" style={{ borderLeft: '3px solid var(--yellow)' }}>
                <h3 style={{ color: 'var(--yellow)' }}>⚠ Warnings</h3>
                {warnings.map((w, i) => (
                  <div key={i} style={{ padding: '6px 0', fontSize: '0.85rem', color: 'var(--yellow)' }}>
                    ⚠ {w}
                  </div>
                ))}
              </div>
            )}

            {/* Volumes */}
            {volumes.length > 0 && (
              <div className="card">
                <h3>Volumes ({volumes.length})</h3>
                <table>
                  <thead>
                    <tr>
                      <th>Name</th>
                      <th>Driver</th>
                      <th>Category</th>
                      <th>Size</th>
                      <th>Shared</th>
                      <th>Method</th>
                      <th></th>
                    </tr>
                  </thead>
                  <tbody>
                    {volumes.map(v => {
                      const sizeGB = v.sizeBytes ? (v.sizeBytes / 1073741824).toFixed(1) : '—';
                      return (
                        <tr key={v.name}>
                          <td className="mono" style={{ fontWeight: 500 }}>{v.name}</td>
                          <td>{v.driver || '—'}</td>
                          <td>{v.driverCategory || '—'}</td>
                          <td className="mono">{sizeGB === '—' ? '—' : `${sizeGB} GB`}</td>
                          <td>{v.shared ? '🔗 Yes' : 'No'}</td>
                          <td><span className="mono" style={{ fontSize: '0.75rem', color: 'var(--accent)' }}>{v.transferMethod || '—'}</span></td>
                          <td>
                            <button className="btn-sm" onClick={() => setInspectVolume(v.name)}>🔍 Inspect</button>
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            )}

            {/* DB Connections */}
            {dbConns.length > 0 && (
              <div className="card">
                <h3>Database Connections ({dbConns.length})</h3>
                <ConnectionReview
                  connections={dbConns.map(c => ({
                    ...c,
                    resolved: connectionResolutions[c.varName]?.resolved || false,
                    resolution: connectionResolutions[c.varName]?.action,
                  }))}
                  onUpdate={(varName, action) => setConnectionResolutions(prev => ({
                    ...prev, [varName]: { action, resolved: true }
                  }))}
                  blocked={false}
                />
              </div>
            )}

            {/* Size estimate */}
            {analysis.estimatedSizeBytes > 0 && (
              <div className="card" style={{ borderLeft: '3px solid var(--accent)' }}>
                <h3>Estimated Transfer Size</h3>
                <div className="mono" style={{ fontSize: '1.2rem', color: 'var(--accent)' }}>
                  {(analysis.estimatedSizeBytes / 1073741824).toFixed(1)} GB
                  {analysis.compressed ? ' (compressed estimate)' : ''}
                </div>
              </div>
            )}
          </div>
        );

      // ── STEP 3: Strategy ──
      case 3:
        return (
          <MigrationPlan
            plan={analysis || {}}
            volumes={analysis?.volumes || []}
            onUpdate={handleStrategyUpdate}
          />
        );

      // ── STEP 4: Credentials Review ──
      case 4: {
        const envVars = analysis?.envVars || [];
        const volOpts = (analysis?.volumes || []).filter(v => v.options && Object.keys(v.options).length > 0);
        const hasComposeSecrets = analysis?.hasComposeSecrets;
        return (
          <div style={{ display: 'grid', gap: '16px' }}>
            <div className="card" style={{ borderLeft: '3px solid var(--yellow)' }}>
              <h3>⚠ Security Notice</h3>
              <div style={{ fontSize: '0.85rem', color: 'var(--text-secondary)' }}>
                All credentials are masked by default. Revealing them will be logged in the audit trail.
                Remember to delete any temporary files created during migration.
              </div>
            </div>

            {hasComposeSecrets && (
              <div className="card" style={{ borderLeft: '3px solid var(--red)', background: 'var(--red-dim)' }}>
                <div style={{ color: '#fff', fontSize: '0.85rem' }}>
                  ⚠ This container uses Docker Compose secrets. Secrets will NOT be transferred.
                  You must manually re-create secrets on the target host.
                </div>
              </div>
            )}

            {envVars.length > 0 && (
              <div className="card">
                <h3>Environment Variables ({envVars.length})</h3>
                <table>
                  <thead>
                    <tr>
                      <th>Variable</th>
                      <th>Value</th>
                      <th></th>
                    </tr>
                  </thead>
                  <tbody>
                    {envVars.map((env, i) => {
                      const isRevealed = credentialsRevealed[env.name];
                      const isConfirming = credRevealConfirm === env.name;
                      const looksSensitive = env.isSensitive;
                      return (
                        <tr key={i}>
                          <td className="mono" style={{ fontWeight: 500 }}>{env.name}</td>
                          <td className="mono" style={{ fontSize: '0.8rem' }}>
                            {isRevealed ? (env.valueMasked || env.value || '') : (
                              <span style={{ letterSpacing: '2px', color: looksSensitive ? 'var(--yellow)' : 'inherit' }}>
                                ••••••••
                              </span>
                            )}
                          </td>
                          <td>
                            <button
                              className="btn-sm"
                              onClick={() => handleRevealCredential(env.name)}
                              style={isConfirming ? { background: 'var(--yellow-dim)', borderColor: 'var(--yellow)', color: '#fff' } : {}}
                            >
                              {isRevealed ? 'Hide' : isConfirming ? '⚠ Confirm' : '👁 Reveal'}
                            </button>
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            )}

            {volOpts.length > 0 && (
              <div className="card">
                <h3>Volume Options ({volOpts.length} volumes with options)</h3>
                <div style={{ fontSize: '0.8rem', color: 'var(--text-secondary)', marginBottom: '12px' }}>
                  Volume options may contain sensitive data. Use the Volume Inspector for details.
                </div>
                {volOpts.map(v => (
                  <div key={v.name} style={{ display: 'flex', alignItems: 'center', gap: '12px', marginBottom: '6px' }}>
                    <span className="mono">{v.name}</span>
                    <button className="btn-sm" onClick={() => setInspectVolume(v.name)}>🔍 Inspect</button>
                  </div>
                ))}
              </div>
            )}

            {envVars.length === 0 && !hasComposeSecrets && volOpts.length === 0 && (
              <div className="text-secondary" style={{ padding: '24px', textAlign: 'center' }}>
                No credentials to review
              </div>
            )}

            {revealAudit.length > 0 && (
              <div className="card" style={{ borderLeft: '3px solid var(--yellow)' }}>
                <h3>🔍 Audit Log ({revealAudit.length} entries this session)</h3>
                <div style={{ maxHeight: '120px', overflow: 'auto', fontSize: '0.75rem' }}>
                  {revealAudit.map((entry, i) => (
                    <div key={i} className="mono" style={{ padding: '2px 0', color: 'var(--text-secondary)' }}>
                      [{entry.timestamp}] {entry.action} — {entry.detail?.variable || entry.detail}
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        );
      }

      // ── STEP 5: Connection Fixes ──
      case 5: {
        const conns = (analysis?.dbConnections || []).map(c => ({
          ...c,
          resolved: connectionResolutions[c.varName]?.resolved || false,
          resolution: connectionResolutions[c.varName]?.action,
        }));
        return (
          <div>
            <div className="card mb-16">
              <h3>Connection Review</h3>
              <div style={{ fontSize: '0.85rem', color: 'var(--text-secondary)', marginBottom: '12px' }}>
                Review database connections that may break after migration. All values are masked.
              </div>
            </div>
            <ConnectionReview
              connections={conns}
              onUpdate={handleConnectionUpdate}
              blocked={true}
            />
          </div>
        );
      }

      // ── STEP 6: Target ──
      case 6:
        return (
          <div style={{ display: 'grid', gap: '16px' }}>
            <div className="card">
              <h3>Target Endpoint</h3>
              <select
                value={targetEndpoint}
                onChange={e => handleTargetSelect(e.target.value)}
                style={{ width: '100%', marginTop: '8px' }}
              >
                <option value="">— Select target endpoint —</option>
                <option value="local">Local</option>
                {targetEndpoints.filter(ep => (ep.id || ep.Id) !== sourceEndpoint).map(ep => (
                  <option key={ep.id || ep.Id} value={ep.id || ep.Id}>
                    {ep.name || ep.Name}
                  </option>
                ))}
              </select>
            </div>

            {targetInfo && (
              <div className="card">
                <h3>Target Info</h3>
                <table>
                  <tbody>
                    <tr><td style={{ color: 'var(--text-secondary)', width: '160px' }}>Status</td><td>
                      <span style={{ color: (targetInfo.status || '').toLowerCase() === 'connected' ? 'var(--green)' : 'var(--red)' }}>
                        {targetInfo.status || 'unknown'}
                      </span>
                    </td></tr>
                    <tr><td style={{ color: 'var(--text-secondary)' }}>Docker Version</td><td className="mono">{targetInfo.dockerVersion || '—'}</td></tr>
                    <tr><td style={{ color: 'var(--text-secondary)' }}>Containers</td><td>{targetInfo.containerCount ?? '—'}</td></tr>
                    {targetInfo.diskFreeBytes != null && (
                      <tr><td style={{ color: 'var(--text-secondary)' }}>Disk Free</td><td className="mono">{(targetInfo.diskFreeBytes / 1073741824).toFixed(1)} GB</td></tr>
                    )}
                    {targetInfo.diskTotalBytes != null && (
                      <tr><td style={{ color: 'var(--text-secondary)' }}>Disk Total</td><td className="mono">{(targetInfo.diskTotalBytes / 1073741824).toFixed(1)} GB</td></tr>
                    )}
                  </tbody>
                </table>
              </div>
            )}

            <div className="card">
              <h3>Target Stack Name</h3>
              <input
                type="text"
                value={targetStackName}
                onChange={e => setTargetStackName(e.target.value)}
                placeholder="Leave blank to use original name"
                style={{ width: '100%', marginTop: '4px' }}
              />
            </div>
          </div>
        );

      // ── STEP 7: Dry Run ──
      case 7:
        return (
          <div style={{ display: 'grid', gap: '16px' }}>
            {!dryRunResult ? (
              <div style={{ textAlign: 'center', padding: '32px' }}>
                <div className="text-secondary mb-16">Run a dry-run to preview all commands before execution</div>
                <button className="btn-primary" onClick={handleDryRun} disabled={loading}>
                  {loading ? 'Running...' : '🚀 Run Dry Run'}
                </button>
              </div>
            ) : (
              <>
                {/* Warnings banner */}
                {dryRunResult.warnings?.length > 0 && (
                  <div className="card" style={{ borderLeft: '3px solid var(--yellow)' }}>
                    <h3 style={{ color: 'var(--yellow)' }}>⚠ Warnings</h3>
                    {dryRunResult.warnings.map((w, i) => (
                      <div key={i} style={{ padding: '4px 0', fontSize: '0.85rem', color: 'var(--yellow)' }}>
                        ⚠ {w}
                      </div>
                    ))}
                  </div>
                )}

                {/* Security banners */}
                {transferMethod === 'rsync-over-ssh' && (
                  <div className="card" style={{ borderLeft: '3px solid var(--green)' }}>
                    <div style={{ color: 'var(--green)', fontSize: '0.85rem' }}>
                      🔒 SSH transfer — all data encrypted in transit
                    </div>
                  </div>
                )}

                {analysis?.estimatedSizeBytes > 10 * 1073741824 && (
                  <div className="card" style={{ borderLeft: '3px solid var(--yellow)' }}>
                    <div style={{ color: 'var(--yellow)', fontSize: '0.85rem' }}>
                      ⚠ Large volume ({((analysis?.estimatedSizeBytes || 0) / 1073741824).toFixed(1)} GB) —
                      transfer may take significant time
                    </div>
                  </div>
                )}

                <div className="card" style={{ borderLeft: '3px solid var(--red)' }}>
                  <div style={{ color: 'var(--red)', fontSize: '0.8rem' }}>
                    ⚠ These commands are for REVIEW only. Actual execution requires admin confirmation.
                    Credentials remain masked at all times.
                  </div>
                </div>

                {/* Commands */}
                <div className="card">
                  <h3>Migration Commands</h3>
                  {dryRunResult.commands?.map((cmd, i) => (
                    <div key={i} style={{ marginBottom: '12px' }}>
                      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '4px' }}>
                        <span style={{ fontSize: '0.75rem', color: 'var(--text-secondary)' }}>
                          Step {i + 1}: {cmd.host === 'source' ? '🖥 Source' : cmd.host === 'target' ? '🖥 Target' : '⚙ System'}
                        </span>
                        <button
                          className="btn-sm"
                          onClick={() => {
                            navigator.clipboard.writeText(cmd.command || cmd);
                            toast('Copied to clipboard', 'success');
                          }}
                          style={{ fontSize: '0.65rem', padding: '2px 8px' }}
                        >
                          📋 Copy
                        </button>
                      </div>
                      <pre style={{
                        background: 'var(--bg-primary)',
                        border: '1px solid var(--border)',
                        borderRadius: '6px',
                        padding: '12px',
                        fontSize: '0.78rem',
                        fontFamily: '"JetBrains Mono", monospace',
                        color: 'var(--green)',
                        overflow: 'auto',
                        whiteSpace: 'pre-wrap',
                        wordBreak: 'break-all',
                      }}>
                        <code>{typeof cmd === 'string' ? cmd : cmd.command}</code>
                      </pre>
                      {cmd.annotation && (
                        <div style={{ fontSize: '0.7rem', color: 'var(--text-secondary)', marginTop: '2px' }}>
                          {cmd.annotation}
                        </div>
                      )}
                    </div>
                  ))}
                </div>

                {/* Summary */}
                <div className="card">
                  <h3>Migration Summary</h3>
                  <table>
                    <tbody>
                      <tr><td style={{ color: 'var(--text-secondary)', width: '180px' }}>Migration ID</td><td className="mono">{dryRunResult.migrationId || '—'}</td></tr>
                      <tr><td style={{ color: 'var(--text-secondary)' }}>Source</td><td>{dryRunResult.sourceEndpoint || sourceEndpoint}</td></tr>
                      <tr><td style={{ color: 'var(--text-secondary)' }}>Target</td><td>{dryRunResult.targetEndpoint || targetEndpoint}</td></tr>
                      <tr><td style={{ color: 'var(--text-secondary)' }}>Container</td><td className="mono">{dryRunResult.containerName || '—'}</td></tr>
                      <tr><td style={{ color: 'var(--text-secondary)' }}>Estimated Size</td><td className="mono">{dryRunResult.estimatedSizeBytes > 0 ? `${(dryRunResult.estimatedSizeBytes / 1073741824).toFixed(1)} GB` : '—'}</td></tr>
                      <tr><td style={{ color: 'var(--text-secondary)' }}>Method</td><td className="mono">{transferMethod}</td></tr>
                      <tr><td style={{ color: 'var(--text-secondary)' }}>Compression</td><td className="mono">{compression}</td></tr>
                    </tbody>
                  </table>
                </div>
              </>
            )}
          </div>
        );

      // ── STEP 8: Execute ──
      case 8: {
        const totalGroups = execCommands.length;
        const elapsed = execStartTime ? Math.floor((Date.now() - execStartTime) / 1000) : 0;
        const elapsedStr = elapsed > 60 ? `${Math.floor(elapsed / 60)}m ${elapsed % 60}s` : `${elapsed}s`;
        const hasResults = Object.keys(execResults).length > 0;

        return (
          <div style={{ display: 'grid', gap: '16px' }}>
            {/* Progress */}
            <div className="card">
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
                <h3 style={{ margin: 0 }}>
                  {execPhase === 'pending' ? 'Ready to Execute' :
                   execPhase === 'running' ? 'Executing...' :
                   'Execution Complete'}
                </h3>
                <span style={{ fontSize: '0.8rem', color: 'var(--text-secondary)' }}>{elapsedStr}</span>
              </div>
              {/* Progress bar */}
              <div style={{
                height: '6px',
                background: 'var(--bg-tertiary)',
                borderRadius: '3px',
                overflow: 'hidden',
                marginBottom: '16px',
              }}>
                <div style={{
                  height: '100%',
                  width: `${execPhase === 'done' ? 100 : execPhase === 'running' ? 50 : 0}%`,
                  background: execPhase === 'done' ? 'var(--green)' : 'var(--accent)',
                  borderRadius: '3px',
                  transition: 'width 0.5s ease',
                }} />
              </div>

              {/* Results list after execution */}
              {hasResults && (
                <div style={{ display: 'grid', gap: '4px' }}>
                  {execCommands.map((cmd, i) => {
                    const result = execResults[i];
                    const isComplete = !!result;
                    const failed = result && result.exit_code !== 0;

                    return (
                      <div key={i} style={{
                        display: 'flex',
                        alignItems: 'center',
                        gap: '10px',
                        padding: '8px 12px',
                        borderRadius: '6px',
                        background: 'var(--bg-tertiary)',
                        border: failed ? '1px solid var(--red)' : isComplete ? '1px solid var(--green-dim)' : '1px solid transparent',
                      }}>
                        <span style={{
                          fontSize: '0.9rem',
                          width: '20px',
                          textAlign: 'center',
                        }}>
                          {isComplete && !failed ? <span style={{ color: 'var(--green)' }}>✓</span> :
                           failed ? <span style={{ color: 'var(--red)' }}>✗</span> :
                           <span style={{ color: 'var(--text-secondary)' }}>○</span>}
                        </span>
                        <code style={{
                          flex: 1,
                          fontSize: '0.75rem',
                          color: failed ? 'var(--red)' : isComplete ? 'var(--green)' : 'var(--text-secondary)',
                          overflow: 'hidden',
                          textOverflow: 'ellipsis',
                          whiteSpace: 'nowrap',
                        }}>
                          {typeof cmd === 'string' ? cmd : (cmd.command || cmd).substring(0, 80)}
                        </code>
                        {result?.exit_code !== undefined && (
                          <span style={{
                            fontSize: '0.7rem',
                            color: result.exit_code === 0 ? 'var(--green)' : 'var(--red)',
                            fontWeight: 600,
                          }}>
                            exit={result.exit_code}
                          </span>
                        )}
                      </div>
                    );
                  })}
                </div>
              )}
            </div>

            {/* Command output details */}
            {hasResults && Object.entries(execResults).map(([idx, r]) => {
              if (!r.stdout && !r.stderr) return null;
              return (
                <div key={idx} className="card">
                  <h3 style={{ color: r.exit_code !== 0 ? 'var(--red)' : 'var(--green)' }}>
                    Command {parseInt(idx) + 1} {r.exit_code === 0 ? '✓' : '✗'}
                  </h3>
                  <pre style={{
                    background: 'var(--bg-primary)',
                    border: '1px solid var(--border)',
                    borderRadius: '6px',
                    padding: '12px',
                    fontSize: '0.75rem',
                    fontFamily: '"JetBrains Mono", monospace',
                    color: 'var(--accent)',
                    overflow: 'auto',
                    whiteSpace: 'pre-wrap',
                    wordBreak: 'break-all',
                    marginBottom: r.stderr ? '8px' : '0',
                  }}>
                    <code>{r.command || ''}</code>
                  </pre>
                  {r.stdout && (
                    <div style={{ marginBottom: r.stderr ? '6px' : '0' }}>
                      <div style={{ fontSize: '0.7rem', color: 'var(--text-secondary)', marginBottom: '4px' }}>stdout:</div>
                      <pre style={{
                        background: 'var(--bg-primary)',
                        border: '1px solid var(--border)',
                        borderRadius: '6px',
                        padding: '8px',
                        fontSize: '0.7rem',
                        fontFamily: '"JetBrains Mono", monospace',
                        color: 'var(--green)',
                        overflow: 'auto',
                        whiteSpace: 'pre-wrap',
                        wordBreak: 'break-all',
                        maxHeight: '150px',
                      }}>
                        {r.stdout}
                      </pre>
                    </div>
                  )}
                  {r.stderr && (
                    <div>
                      <div style={{ fontSize: '0.7rem', color: 'var(--yellow)', marginBottom: '4px' }}>stderr:</div>
                      <pre style={{
                        background: 'var(--bg-primary)',
                        border: '1px solid var(--red-dim)',
                        borderRadius: '6px',
                        padding: '8px',
                        fontSize: '0.7rem',
                        fontFamily: '"JetBrains Mono", monospace',
                        color: 'var(--red)',
                        overflow: 'auto',
                        whiteSpace: 'pre-wrap',
                        wordBreak: 'break-all',
                        maxHeight: '150px',
                      }}>
                        {r.stderr}
                      </pre>
                    </div>
                  )}
                </div>
              );
            })}

            {/* Loading spinner while executing */}
            {execPhase === 'running' && !hasResults && (
              <div style={{ textAlign: 'center', padding: '32px' }}>
                <div className="spinner spinner-lg" style={{ margin: '0 auto 16px' }} />
                <div style={{ color: 'var(--text-secondary)' }}>Executing migration commands via shell...</div>
              </div>
            )}

            {/* Action buttons */}
            <div className="btn-group" style={{ justifyContent: 'center' }}>
              {execPhase === 'pending' && (
                <button className="btn-primary" onClick={handleStartExecution}>
                  ▶ Start Execution
                </button>
              )}
              {execPhase === 'done' && (
                <div style={{ textAlign: 'center' }}>
                  <div style={{ color: 'var(--green)', fontSize: '1.1rem', fontWeight: 600, marginBottom: '8px' }}>
                    ✓ All commands completed
                  </div>
                  <div style={{ fontSize: '0.8rem', color: 'var(--text-secondary)' }}>
                    Advancing to verification...
                  </div>
                </div>
              )}
            </div>
          </div>
        );
      }

      // ── STEP 9: Verify ──
      case 9:
        const v = verification || {};
        const hasVolumes = (v.volumes || []).length > 0;
        const hasCommands = (v.commands || []).length > 0;
        const hasDbConnections = (v.dbConnections || []).length > 0;
        const brokenDbs = (v.dbConnections || []).filter(c => c.willBreak);
        const hasWarnings = (v.warnings || []).length > 0;
        const isSuccess = !hasWarnings && brokenDbs.length === 0;
        const allSteps = [
          { name: 'Container Export', status: hasCommands ? 'success' : 'skipped' },
          { name: 'Volume Transfer', status: hasVolumes ? 'success' : 'skipped' },
          { name: 'DB Connection Migration', status: !hasDbConnections ? 'skipped' : brokenDbs.length > 0 ? 'failed' : 'success' },
          { name: 'Container Import', status: hasCommands ? 'success' : 'skipped' },
          { name: 'Connectivity Test', status: 'pending' },
        ];

        return (
          <div style={{ display: 'grid', gap: '16px' }}>
            <div className="card" style={{ borderLeft: '3px solid ' + (isSuccess ? 'var(--green)' : 'var(--red)') }}>
              <h3>Migration {isSuccess ? 'Successful ✓' : 'Completed with issues ⚠'}</h3>
              <table>
                <tbody>
                  <tr><td style={{ color: 'var(--text-secondary)', width: '160px' }}>Duration</td><td>{'—'}</td></tr>
                  <tr><td style={{ color: 'var(--text-secondary)' }}>Bytes Transferred</td><td className="mono">{v.estimatedSizeBytes ? `${(v.estimatedSizeBytes / 1073741824).toFixed(2)} GB` : '—'}</td></tr>
                  <tr><td style={{ color: 'var(--text-secondary)' }}>Container</td><td className="mono">{v.containerName || analysis?.containerName || '—'}</td></tr>
                  <tr><td style={{ color: 'var(--text-secondary)' }}>Source → Target</td><td>{sourceEndpoint} → {targetEndpoint}</td></tr>
                </tbody>
              </table>
            </div>

            {/* Step-by-step results */}
            <div className="card">
              <h3>Step Results</h3>
              <div style={{ display: 'grid', gap: '6px' }}>
                {allSteps.map((s, i) => (
                  <div key={i} style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: '10px',
                    padding: '8px 12px',
                    borderRadius: '6px',
                    background: 'var(--bg-tertiary)',
                  }}>
                    <span style={{
                      color: s.status === 'success' ? 'var(--green)' :
                             s.status === 'failed' ? 'var(--red)' :
                             s.status === 'skipped' ? 'var(--yellow)' : 'var(--text-secondary)',
                      fontWeight: 600,
                    }}>
                      {s.status === 'success' ? '✓' : s.status === 'failed' ? '✗' : s.status === 'skipped' ? '⊘' : '—'}
                    </span>
                    <span style={{ flex: 1 }}>{s.name}</span>
                    <span style={{ fontSize: '0.75rem', color: 'var(--text-secondary)' }}>
                      {s.status || 'pending'}
                    </span>
                  </div>
                ))}
              </div>
            </div>

            {/* Connectivity test note */}
            {!isSuccess && (
              <div className="card" style={{ borderLeft: `3px solid var(--yellow)` }}>
                <h3>Connectivity Test</h3>
                <div style={{ fontSize: '0.85rem', color: 'var(--yellow)' }}>
                  ⚠ Manual connectivity verification required — check target endpoint
                </div>
              </div>
            )}
            {isSuccess && (
              <div className="card" style={{ borderLeft: `3px solid var(--green)` }}>
                <h3>Connectivity Test</h3>
                <div style={{ fontSize: '0.85rem', color: 'var(--green)' }}>
                  ✓ No issues detected — verify on target endpoint
                </div>
              </div>
            )}

            {/* Post-migration actions */}
            <div className="card">
              <h3>Post-Migration Actions</h3>
              <div className="btn-group">
                <button
                  className="btn-danger"
                  onClick={() => handlePostMigration('remove_source')}
                >
                  🗑 Remove from Source
                </button>
                <button
                  className="btn-warning"
                  onClick={() => handlePostMigration('rollback')}
                >
                  🔄 Restart on Source (Rollback)
                </button>
              </div>
            </div>

            {/* Audit log */}
            {revealAudit.length > 0 && (
              <div className="card" style={{ borderLeft: '3px solid var(--yellow)' }}>
                <h3>🔍 Migration Audit Log</h3>
                <div style={{ maxHeight: '150px', overflow: 'auto', fontSize: '0.75rem' }}>
                  {revealAudit.map((entry, i) => (
                    <div key={i} className="mono" style={{ padding: '2px 0', color: 'var(--text-secondary)' }}>
                      [{entry.timestamp}] {entry.action} — {entry.detail?.variable || JSON.stringify(entry.detail)}
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        );
    }
  };

  return (
    <div>
      <div className="section-header">
        <h1>🚚 Container Migration</h1>
        <div className="btn-group">
          <button className="btn-sm" onClick={() => navigate('dashboard')}>
            ← Dashboard
          </button>
        </div>
      </div>

      {error && (
        <div className="text-danger mb-16" style={{ padding: '12px', background: 'var(--red-dim)', borderRadius: '6px' }}>
          {error}
        </div>
      )}

      <StepIndicator currentStep={step} completedSteps={completedSteps} />

      <div style={{ minHeight: '300px' }}>
        {renderStepContent()}
      </div>

      {/* Navigation footer */}
      <div style={{
        display: 'flex',
        justifyContent: 'space-between',
        marginTop: '24px',
        padding: '16px 0',
        borderTop: '1px solid var(--border)',
      }}>
        <button
          onClick={handlePrev}
          disabled={step === 1}
        >
          ← Previous
        </button>

        <div className="text-secondary" style={{ fontSize: '0.8rem' }}>
          Step {step} of {TOTAL_STEPS}
        </div>

        {step !== 8 && (
          <button
            className="btn-primary"
            onClick={handleNext}
            disabled={loading || !canGoNext()}
          >
            {step === 9 ? '✅ Finish' : step === 1 ? 'Analyze →' : loading ? 'Loading...' : 'Next →'}
          </button>
        )}
      </div>

      {/* Volume Inspector Modal */}
      {inspectVolume && (
        <VolumeInspector
          volumeName={inspectVolume}
          onClose={() => setInspectVolume(null)}
        />
      )}
    </div>
  );
}
