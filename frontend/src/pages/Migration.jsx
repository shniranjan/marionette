import { useState, useEffect, useCallback } from 'react';
import { api } from '../api/client';
import Spinner from '../components/Spinner';
import Modal from '../components/Modal';
import { useToast } from '../components/Toast';
import ConnectionReview from '../components/ConnectionReview';
import MigrationPlan from '../components/MigrationPlan';
import VolumeInspector from '../components/VolumeInspector';
import MigrationEditor from '../components/MigrationEditor';
import PreflightPanel from '../components/PreflightPanel';
import { getCurrentEndpoint, setEndpoint } from '../components/EndpointSwitcher';

const TOTAL_STEPS = 5;
const STEP_LABELS = [
  'Source',
  'Discovery',
  'Review & Edit',
  'Pre-Flight',
  'Execute & Verify',
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
                maxWidth: '70px',
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

  // ── Wizard state ──
  const [step, setStep] = useState(1);
  const [completedSteps, setCompletedSteps] = useState(new Set());
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);

  // ── Step 1: Source selection ──
  const [migrationType, setMigrationType] = useState(null); // 'container' | 'compose'
  const [endpoints, setEndpoints] = useState([]);
  const [sourceEndpoint, setSourceEndpoint] = useState('');

  // Container-specific
  const [containers, setContainers] = useState([]);
  const [containerSearch, setContainerSearch] = useState('');
  const [selectedContainer, setSelectedContainer] = useState(null);

  // Compose-specific
  const [targetEndpoint, setTargetEndpoint] = useState('');
  const [stackName, setStackName] = useState('');

  // ── Step 2: Discovery result ──
  const [plan, setPlan] = useState(null);
  const [analysis, setAnalysis] = useState(null); // old container analysis for backward compat

  // ── Step 3: Review & Edit (handled by MigrationEditor) ──

  // ── Step 4: Pre-flight (handled by PreflightPanel) ──

  // ── Step 5: Execute ──
  const [execPhase, setExecPhase] = useState('pending');
  const [execCommands, setExecCommands] = useState([]);
  const [execResults, setExecResults] = useState({});
  const [execStartTime, setExecStartTime] = useState(null);
  const [verification, setVerification] = useState(null);

  // Old container path state (reused for backward compat)
  const [strategy, setStrategy] = useState({});
  const [transferMethod, setTransferMethod] = useState('rsync-over-ssh');
  const [compression, setCompression] = useState('pigz');
  const [postOptions, setPostOptions] = useState({});
  const [volumeOverrides, setVolumeOverrides] = useState({});
  const [connectionResolutions, setConnectionResolutions] = useState({});
  const [migrationId, setMigrationId] = useState(null);
  const [targetStackNameLegacy, setTargetStackNameLegacy] = useState('');
  const [dryRunResult, setDryRunResult] = useState(null);

  // Audit
  const [revealAudit, setRevealAudit] = useState([]);
  const auditLog = useCallback((action, detail) => {
    const entry = { timestamp: new Date().toISOString(), action, detail };
    const audit = JSON.parse(localStorage.getItem('marionette-audit-log') || '[]');
    audit.push(entry);
    localStorage.setItem('marionette-audit-log', JSON.stringify(audit));
    setRevealAudit(prev => [...prev, entry]);
  }, []);

  // Volume inspector
  const [inspectVolume, setInspectVolume] = useState(null);

  // ── Load endpoints on mount ──
  useEffect(() => {
    (async () => {
      try {
        const data = await api.get('/api/endpoints');
        const eps = Array.isArray(data) ? data : (data?.endpoints || []);
        setEndpoints(eps);
      } catch {/* ignore */}
    })();
  }, []);

  // Auto-fill target for compose
  useEffect(() => {
    if (migrationType === 'compose' && !targetEndpoint && endpoints.length > 0) {
      const others = endpoints.filter(e => (e.id || e.Id) !== sourceEndpoint);
      if (others.length === 1) {
        setTargetEndpoint(others[0].id || others[0].Id);
      }
    }
  }, [migrationType, endpoints, sourceEndpoint, targetEndpoint]);

  // ── STEP 1: Source selection ──

  const filteredContainers = containers.filter(c => {
    const name = (c.Name || c.name || '').toLowerCase();
    const img = (c.Image || c.image || '').toLowerCase();
    const q = containerSearch.toLowerCase();
    return !q || name.includes(q) || img.includes(q);
  });

  const handleSourceSelectContainer = async (epId) => {
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

  const handleSourceChangeCompose = useCallback((epId) => {
    setSourceEndpoint(epId);
    setEndpoint(epId);
  }, []);

  // ── STEP 2: Discovery ──

  const handleDiscover = async () => {
    if (migrationType === 'container') {
      if (!sourceEndpoint || !selectedContainer) return;
      setLoading(true);
      setError(null);
      try {
        // Try unified endpoint first
        const result = await api.post('/api/migration/unified/analyze', {
          migrationType: 'container',
          sourceEndpoint,
          targetEndpoint: targetEndpoint || 'local',
          containerId: selectedContainer.Id || selectedContainer.id,
          containerName: (selectedContainer.Name || selectedContainer.name || '').replace(/^\//, ''),
        });
        setPlan(result);
        setAnalysis({ ...result, envVars: parseEnvVars(result.envVars || []) });
        setCompletedSteps(prev => new Set([...prev, 1]));
        setStep(2);
      } catch (err) {
        // Fall back to old analyze
        try {
          const result = await api.post('/api/migration/analyze', {
            source_endpoint: sourceEndpoint,
            container_id: selectedContainer.Id || selectedContainer.id,
          });
          setAnalysis({ ...result, envVars: parseEnvVars(result.envVars || []) });
          // Convert old analysis to unified plan shape for editor
          const unifiedPlan = {
            planId: result.migrationId || 'legacy-' + Date.now(),
            migrationType: 'container',
            sourceEndpoint,
            targetEndpoint: targetEndpoint || '',
            stackName: (selectedContainer.Name || selectedContainer.name || '').replace(/^\//, ''),
            targetStackName: '',
            sourceArchitecture: result.sourceArchitecture,
            targetArchitecture: result.targetArchitecture,
            volumes: (result.volumes || []).map(v => ({
              sourceName: v.name,
              targetName: v.name,
              driver: v.driver,
              targetDriver: v.driver,
              sizeBytes: v.sizeBytes,
              mountPoint: v.mountPoint,
              skip: false,
              transferMethod: v.transferMethod,
            })),
            databases: (result.dbConnections || []).map(c => ({
              serviceName: c.varName || c.name || result.containerName || '',
              dbType: c.dbType || {},
              username: c.username,
              password: '',
              passwordMasked: c.valueMasked,
              port: c.port,
              databaseName: c.databaseName,
              image: result.image || '',
              version: '',
              preTransferCommands: [],
              postTransferCommands: [],
              hasReplication: false,
              connectivityVerified: false,
            })),
            envVars: (result.envVars || []).map(e => ({
              serviceName: result.containerName || '',
              varName: e.name,
              sourceValue: e.value,
              targetValue: e.value,
              isSensitive: e.isSensitive,
              willBreak: false,
              breakReason: '',
            })),
            images: [],
            warnings: result.warnings || [],
            estimatedSizeBytes: result.estimatedSizeBytes || 0,
            createdAt: new Date().toISOString(),
          };
          setPlan(unifiedPlan);
          setCompletedSteps(prev => new Set([...prev, 1]));
          setStep(2);
        } catch (err2) {
          toast('Analysis failed: ' + (err2.message || err.message), 'error');
        }
      } finally {
        setLoading(false);
      }
    } else if (migrationType === 'compose') {
      if (!sourceEndpoint || !targetEndpoint) {
        toast('Select both source and target endpoints', 'error');
        return;
      }
      if (!stackName.trim()) {
        toast('Enter a stack name', 'error');
        return;
      }
      setLoading(true);
      setError(null);
      try {
        const result = await api.post('/api/migration/unified/analyze', {
          migrationType: 'compose',
          sourceEndpoint,
          targetEndpoint,
          stackName: stackName.trim(),
        });
        setPlan(result);
        setCompletedSteps(prev => new Set([...prev, 1]));
        setStep(2);
        toast('Analysis complete', 'success');
      } catch (err) {
        // Fall back to old compose analyze
        try {
          const result = await api.post('/migration/compose/analyze', {
            sourceEndpoint,
            targetEndpoint,
            stackName: stackName.trim(),
          });
          // Convert old compose diff to unified plan shape
          const diff = result.diff || {};
          const unifiedPlan = {
            planId: 'legacy-compose-' + Date.now(),
            migrationType: 'compose',
            sourceEndpoint,
            targetEndpoint,
            stackName: stackName.trim(),
            targetStackName: stackName.trim(),
            sourceArchitecture: result.sourceArchitecture,
            targetArchitecture: result.targetArchitecture,
            volumes: (diff.volumeChanges || []).map(vc => ({
              sourceName: vc.sourceName || vc.name,
              targetName: vc.name,
              driver: vc.driver,
              targetDriver: vc.driver,
              sizeBytes: vc.sizeBytes,
              mountPoint: '',
              skip: vc.changeType === 'removed',
              transferMethod: '',
            })),
            databases: (diff.databaseServices || []).map(ds => ({
              serviceName: ds.serviceName,
              dbType: ds.dbType || {},
              username: ds.username,
              password: '',
              passwordMasked: ds.passwordMasked,
              port: ds.port,
              databaseName: ds.databaseName,
              image: ds.image || '',
              version: ds.version,
              preTransferCommands: ds.preTransferCommands || [],
              postTransferCommands: ds.postTransferCommands || [],
              hasReplication: ds.hasReplication || false,
              connectivityVerified: false,
            })),
            envVars: (diff.envChanges || []).map(ec => ({
              serviceName: ec.serviceName,
              varName: ec.varName,
              sourceValue: ec.oldValue,
              targetValue: ec.newValue || ec.oldValue,
              isSensitive: ec.isSensitive || false,
              willBreak: false,
              breakReason: '',
            })),
            services: (diff.serviceChanges || []).map(sc => ({
              name: sc.name,
              action: sc.changeType === 'removed' ? 'Skip' : 'Migrate',
              imageOverride: sc.imageNew,
            })),
            images: (diff.imageChanges || []).map(ic => ({
              serviceName: ic.serviceName,
              oldImage: ic.oldImage,
              newImage: ic.newImage,
              majorVersionChange: ic.majorVersionChange || false,
            })),
            warnings: diff.warnings || result.warnings || [],
            estimatedSizeBytes: 0,
            createdAt: new Date().toISOString(),
          };
          setPlan(unifiedPlan);
          setCompletedSteps(prev => new Set([...prev, 1]));
          setStep(2);
          toast('Analysis complete (legacy mode)', 'success');
        } catch (err2) {
          toast('Analysis failed: ' + (err2.message || err.message), 'error');
        }
      } finally {
        setLoading(false);
      }
    }
  };

  // ── STEP 2: Discovery result display ──
  const renderDiscoveryResult = () => {
    if (!plan) return null;

    return (
      <div style={{ display: 'grid', gap: '16px' }}>
        {/* Summary card */}
        <div className="card">
          <h3>Migration Plan Summary</h3>
          <table>
            <tbody>
              <tr><td style={{ color: 'var(--text-secondary)', width: '140px' }}>Type</td><td>
                <span style={{
                  display: 'inline-block', padding: '2px 10px', borderRadius: '10px',
                  background: 'var(--accent-dim)', color: 'var(--accent)',
                  fontSize: '0.8rem', fontWeight: 600,
                }}>
                  {plan.migrationType === 'compose' ? '📚 Compose' : '📦 Container'}
                </span>
              </td></tr>
              <tr><td style={{ color: 'var(--text-secondary)' }}>Source</td><td>{plan.sourceEndpoint}</td></tr>
              <tr><td style={{ color: 'var(--text-secondary)' }}>Target</td><td>{plan.targetEndpoint}</td></tr>
              <tr><td style={{ color: 'var(--text-secondary)' }}>Stack</td><td className="mono">{plan.stackName}</td></tr>
              <tr><td style={{ color: 'var(--text-secondary)' }}>Volumes</td><td>{plan.volumes?.length || 0}</td></tr>
              <tr><td style={{ color: 'var(--text-secondary)' }}>Databases</td><td>{plan.databases?.length || 0}</td></tr>
              <tr><td style={{ color: 'var(--text-secondary)' }}>Env Vars</td><td>{plan.envVars?.length || 0}</td></tr>
              {plan.estimatedSizeBytes > 0 && (
                <tr><td style={{ color: 'var(--text-secondary)' }}>Est. Size</td><td className="mono">
                  {(plan.estimatedSizeBytes / 1073741824).toFixed(1)} GB
                </td></tr>
              )}
            </tbody>
          </table>
        </div>

        {/* Warnings */}
        {(plan.warnings || []).length > 0 && (
          <div className="card" style={{ borderLeft: '3px solid var(--yellow)' }}>
            <h3 style={{ color: 'var(--yellow)' }}>⚠ Warnings</h3>
            {plan.warnings.map((w, i) => (
              <div key={i} style={{ padding: '4px 0', fontSize: '0.85rem', color: 'var(--yellow)' }}>
                ⚠ {w}
              </div>
            ))}
          </div>
        )}
      </div>
    );
  };

  // ── STEP 3: Review & Edit ──
  const handleEditorSave = useCallback((updatedPlan) => {
    if (updatedPlan) {
      setPlan(updatedPlan);
    }
    toast('Plan saved', 'success');
  }, [toast]);

  const handleEditorProceed = useCallback(() => {
    setCompletedSteps(prev => new Set([...prev, 3]));
    setStep(4);
  }, []);

  // ── STEP 4: Pre-flight ──
  const handlePreflightContinue = useCallback(() => {
    setCompletedSteps(prev => new Set([...prev, 4]));
    setStep(5);
  }, []);

  const handlePreflightBack = useCallback(() => {
    setStep(3);
  }, []);

  // ── STEP 5: Execute ──
  const handleStartExecution = async () => {
    setExecPhase('running');
    setExecStartTime(Date.now());
    setExecResults({});

    try {
      if (plan?.planId && !plan.planId.startsWith('legacy-')) {
        // Unified execution
        const result = await api.post(`/api/migration/unified/plan/${plan.planId}/execute`, {});
        const results = result.results || [];
        const resultMap = {};
        results.forEach((r, i) => {
          resultMap[i] = {
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
        setExecPhase(allSuccess ? 'done' : 'completed_with_errors');
        setVerification(result.verification || result);
      } else {
        // Legacy container execution
        await api.post(`/api/migration/${migrationId || plan?.planId?.replace('legacy-', '')}/execute`, {});
        setExecPhase('done');
        setVerification({ status: 'completed' });
      }

      setCompletedSteps(prev => new Set([...prev, 5]));
    } catch (err) {
      toast('Execution failed: ' + err.message, 'error');
      setExecPhase('pending');
    }
  };

  // ── Navigation ──
  const canGoNext = () => {
    switch (step) {
      case 1: return !!migrationType;
      case 2: return !!plan;
      case 3: return true;
      case 4: return true; // Preflight has its own continue
      case 5: return execPhase === 'done' || execPhase === 'completed_with_errors';
      default: return false;
    }
  };

  const handleNext = () => {
    switch (step) {
      case 1: {
        // Validate before discovery
        if (migrationType === 'container' && !selectedContainer) {
          toast('Select a container first', 'error');
          return;
        }
        if (migrationType === 'compose' && !stackName.trim()) {
          toast('Enter a stack name', 'error');
          return;
        }
        handleDiscover();
        break;
      }
      case 2: setCompletedSteps(prev => new Set([...prev, 2])); setStep(3); break;
      case 3: handleEditorProceed(); break;
      case 4: handlePreflightContinue(); break;
      case 5: handleDone(); break;
    }
  };

  const handlePrev = () => {
    if (step > 1) setStep(step - 1);
  };

  const handleDone = () => {
    navigate('dashboard');
    toast('Migration wizard complete 🎉', 'success');
  };

  // ── Post-migration actions (legacy container path) ──
  const handlePostMigration = async (action) => {
    try {
      if (action === 'remove_source') {
        await api.delete(`/api/containers/${selectedContainer?.Id || selectedContainer?.id}`);
        toast('Container removed from source', 'success');
      } else if (action === 'rollback') {
        if (migrationId) {
          await api.post(`/api/migration/${migrationId}/rollback`);
        }
        toast('Rollback initiated', 'info');
      }
    } catch (err) {
      toast('Action failed: ' + err.message, 'error');
    }
  };

  // ── Render Step Content ──
  const renderStepContent = () => {
    switch (step) {
      // ── STEP 1: Source Type Selection ──
      case 1:
        return (
          <div style={{ display: 'grid', gap: '24px' }}>
            {/* Type selector cards */}
            <div style={{
              display: 'grid',
              gridTemplateColumns: '1fr 1fr',
              gap: '16px',
            }}>
              <div
                className="card"
                onClick={() => setMigrationType('container')}
                style={{
                  cursor: 'pointer',
                  border: migrationType === 'container' ? '2px solid var(--accent)' : '2px solid var(--border)',
                  background: migrationType === 'container' ? 'var(--bg-tertiary)' : 'var(--bg-secondary)',
                  textAlign: 'center',
                  padding: '24px',
                  transition: 'all 0.15s',
                }}
              >
                <div style={{ fontSize: '2rem', marginBottom: '8px' }}>📦</div>
                <h3 style={{ margin: '0 0 4px' }}>Container Migration</h3>
                <div style={{ fontSize: '0.8rem', color: 'var(--text-secondary)' }}>
                  Single container cold migration with volume transfer
                </div>
              </div>
              <div
                className="card"
                onClick={() => setMigrationType('compose')}
                style={{
                  cursor: 'pointer',
                  border: migrationType === 'compose' ? '2px solid var(--accent)' : '2px solid var(--border)',
                  background: migrationType === 'compose' ? 'var(--bg-tertiary)' : 'var(--bg-secondary)',
                  textAlign: 'center',
                  padding: '24px',
                  transition: 'all 0.15s',
                }}
              >
                <div style={{ fontSize: '2rem', marginBottom: '8px' }}>📚</div>
                <h3 style={{ margin: '0 0 4px' }}>Compose Migration</h3>
                <div style={{ fontSize: '0.8rem', color: 'var(--text-secondary)' }}>
                  Full Docker Compose stack migration between endpoints
                </div>
              </div>
            </div>

            {/* Container-specific form */}
            {migrationType === 'container' && (
              <div className="card">
                <h3>Container Source</h3>
                <div style={{ marginBottom: '12px' }}>
                  <label style={{ fontSize: '0.8rem', fontWeight: 600, color: 'var(--text-secondary)', display: 'block', marginBottom: '4px' }}>
                    Source Endpoint
                  </label>
                  <select
                    value={sourceEndpoint}
                    onChange={e => handleSourceSelectContainer(e.target.value)}
                    style={{ width: '100%' }}
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
                  <div>
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
                      <span style={{ fontWeight: 600, fontSize: '0.9rem' }}>Containers</span>
                      <input
                        type="text"
                        placeholder="Search containers..."
                        value={containerSearch}
                        onChange={e => setContainerSearch(e.target.value)}
                        style={{ width: '240px' }}
                      />
                    </div>
                    {loading ? (
                      <div style={{ textAlign: 'center', padding: '24px' }}><Spinner /></div>
                    ) : containers.length === 0 ? (
                      <div style={{ color: 'var(--text-secondary)', padding: '24px', textAlign: 'center' }}>
                        No containers found on this endpoint
                      </div>
                    ) : (
                      <div style={{ maxHeight: '350px', overflow: 'auto' }}>
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
                                      display: 'inline-block', padding: '2px 10px', borderRadius: '12px',
                                      fontSize: '0.7rem', fontWeight: 600,
                                      color: (c.State || c.state || '').toLowerCase() === 'running' ? 'var(--green)' : 'var(--red)',
                                      background: (c.State || c.state || '').toLowerCase() === 'running' ? 'var(--green-dim)' : 'var(--red-dim)',
                                    }}>
                                      {c.State || c.state || 'unknown'}
                                    </span>
                                  </td>
                                  <td>
                                    {isCompose ? (
                                      <span style={{
                                        display: 'inline-block', padding: '1px 8px', borderRadius: '10px',
                                        background: 'var(--bg-tertiary)', fontSize: '0.7rem', color: 'var(--accent)',
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
            )}

            {/* Compose-specific form */}
            {migrationType === 'compose' && (
              <div className="card">
                <h3>Compose Source</h3>
                <div style={{
                  display: 'grid',
                  gridTemplateColumns: '1fr 1fr',
                  gap: '16px',
                  marginBottom: '16px',
                }}>
                  <div>
                    <label style={{ fontSize: '0.8rem', fontWeight: 600, color: 'var(--text-secondary)', display: 'block', marginBottom: '4px' }}>
                      Source Endpoint
                    </label>
                    <select value={sourceEndpoint} onChange={e => handleSourceChangeCompose(e.target.value)} style={{ width: '100%' }}>
                      <option value="">— Select endpoint —</option>
                      <option value="local">Local</option>
                      {endpoints.map(ep => (
                        <option key={ep.id || ep.Id} value={ep.id || ep.Id}>{ep.name || ep.Name}</option>
                      ))}
                    </select>
                  </div>
                  <div>
                    <label style={{ fontSize: '0.8rem', fontWeight: 600, color: 'var(--text-secondary)', display: 'block', marginBottom: '4px' }}>
                      Target Endpoint
                    </label>
                    <select value={targetEndpoint} onChange={e => setTargetEndpoint(e.target.value)} style={{ width: '100%' }}>
                      <option value="">— Select endpoint —</option>
                      <option value="local">Local</option>
                      {endpoints.filter(ep => (ep.id || ep.Id) !== sourceEndpoint).map(ep => (
                        <option key={ep.id || ep.Id} value={ep.id || ep.Id}>{ep.name || ep.Name}</option>
                      ))}
                    </select>
                  </div>
                </div>
                <div>
                  <label style={{ fontSize: '0.8rem', fontWeight: 600, color: 'var(--text-secondary)', display: 'block', marginBottom: '4px' }}>
                    Stack Name
                  </label>
                  <input
                    type="text"
                    value={stackName}
                    onChange={e => setStackName(e.target.value)}
                    placeholder="e.g., wordpress, monitoring, myapp"
                    style={{ width: '100%' }}
                    onKeyDown={e => { if (e.key === 'Enter') handleNext(); }}
                  />
                </div>
              </div>
            )}
          </div>
        );

      // ── STEP 2: Discovery ──
      case 2:
        if (!plan) {
          return (
            <div style={{ textAlign: 'center', padding: '40px' }}>
              <Spinner size="lg" />
              <div style={{ marginTop: '16px', color: 'var(--text-secondary)' }}>
                Analyzing migration...
              </div>
            </div>
          );
        }
        return renderDiscoveryResult();

      // ── STEP 3: Review & Edit ──
      case 3:
        return (
          <MigrationEditor
            plan={plan}
            onSave={handleEditorSave}
          />
        );

      // ── STEP 4: Pre-Flight ──
      case 4:
        return (
          <PreflightPanel
            planId={plan?.planId}
            onContinue={handlePreflightContinue}
            onBack={handlePreflightBack}
          />
        );

      // ── STEP 5: Execute & Verify ──
      case 5: {
        const elapsed = execStartTime ? Math.floor((Date.now() - execStartTime) / 1000) : 0;
        const elapsedStr = elapsed > 60 ? `${Math.floor(elapsed / 60)}m ${elapsed % 60}s` : `${elapsed}s`;
        const hasResults = Object.keys(execResults).length > 0;

        return (
          <div style={{ display: 'grid', gap: '16px' }}>
            {/* Execute */}
            {execPhase === 'pending' && (
              <div style={{ textAlign: 'center', padding: '32px' }}>
                <div style={{ fontSize: '1.2rem', fontWeight: 600, marginBottom: '16px' }}>
                  Ready to Execute Migration
                </div>
                <div style={{ color: 'var(--text-secondary)', marginBottom: '24px' }}>
                  {plan?.migrationType === 'compose'
                    ? 'This will stop services on source, transfer volumes, and deploy on target with rollback capability.'
                    : 'This will transfer container volumes from source to target.'}
                </div>
                <button className="btn-primary" onClick={handleStartExecution} style={{ fontSize: '1rem', padding: '12px 32px' }}>
                  ▶ Start Execution
                </button>
              </div>
            )}

            {execPhase === 'running' && !hasResults && (
              <div style={{ textAlign: 'center', padding: '32px' }}>
                <Spinner size="lg" />
                <div style={{ marginTop: '16px', color: 'var(--text-secondary)' }}>
                  Executing migration...
                </div>
              </div>
            )}

            {/* Results */}
            {hasResults && (
              <>
                <div className="card">
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
                    <h3 style={{ margin: 0 }}>
                      {execPhase === 'done' ? '✓ Execution Complete' : '⚠ Completed with Errors'}
                    </h3>
                    <span style={{ fontSize: '0.8rem', color: 'var(--text-secondary)' }}>{elapsedStr}</span>
                  </div>
                  <div style={{
                    height: '6px',
                    background: 'var(--bg-tertiary)',
                    borderRadius: '3px',
                    overflow: 'hidden',
                    marginBottom: '16px',
                  }}>
                    <div style={{
                      height: '100%',
                      width: '100%',
                      background: execPhase === 'done' ? 'var(--green)' : 'var(--yellow)',
                      borderRadius: '3px',
                      transition: 'width 0.5s ease',
                    }} />
                  </div>

                  <div style={{ display: 'grid', gap: '4px' }}>
                    {Object.entries(execResults).map(([idx, r]) => {
                      const failed = r.exit_code !== 0;
                      return (
                        <div key={idx} style={{
                          display: 'flex',
                          alignItems: 'center',
                          gap: '10px',
                          padding: '8px 12px',
                          borderRadius: '6px',
                          background: 'var(--bg-tertiary)',
                          border: failed ? '1px solid var(--red)' : '1px solid var(--green-dim)',
                        }}>
                          <span style={{ fontSize: '0.9rem', width: '20px', textAlign: 'center' }}>
                            {!failed ? <span style={{ color: 'var(--green)' }}>✓</span> :
                             <span style={{ color: 'var(--red)' }}>✗</span>}
                          </span>
                          <code style={{
                            flex: 1,
                            fontSize: '0.75rem',
                            color: failed ? 'var(--red)' : 'var(--green)',
                            overflow: 'hidden',
                            textOverflow: 'ellipsis',
                            whiteSpace: 'nowrap',
                          }}>
                            {r.command ? r.command.substring(0, 80) : `Task ${parseInt(idx) + 1}`}
                          </code>
                          {r.exit_code !== undefined && (
                            <span style={{
                              fontSize: '0.7rem',
                              color: r.exit_code === 0 ? 'var(--green)' : 'var(--red)',
                              fontWeight: 600,
                            }}>
                              exit={r.exit_code}
                            </span>
                          )}
                        </div>
                      );
                    })}
                  </div>
                </div>

                {/* Command details */}
                {Object.entries(execResults).map(([idx, r]) => {
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
                      🔄 Rollback
                    </button>
                  </div>
                </div>
              </>
            )}
          </div>
        );
      }

      default:
        return null;
    }
  };

  return (
    <div>
      <div className="section-header">
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <h1>🚚 Migration</h1>
          {migrationType && (
            <span style={{
              display: 'inline-block',
              padding: '2px 10px',
              borderRadius: '10px',
              background: 'var(--accent-dim)',
              color: 'var(--accent)',
              fontSize: '0.75rem',
              fontWeight: 600,
            }}>
              {migrationType === 'compose' ? '📚 Compose' : '📦 Container'}
            </span>
          )}
        </div>
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
        {loading && step !== 4 ? (
          <div style={{ textAlign: 'center', padding: '40px' }}>
            <Spinner size="lg" />
            <div style={{ marginTop: '16px', color: 'var(--text-secondary)' }}>
              {step === 2 ? 'Analyzing migration...' : 'Loading...'}
            </div>
          </div>
        ) : (
          renderStepContent()
        )}
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

        {step !== 4 && (
          <button
            className="btn-primary"
            onClick={handleNext}
            disabled={loading || !canGoNext()}
          >
            {step === TOTAL_STEPS ? '✅ Finish' :
             step === 1 ? 'Analyze →' :
             step === 3 ? 'Pre-Flight →' :
             loading ? 'Loading...' : 'Next →'}
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
