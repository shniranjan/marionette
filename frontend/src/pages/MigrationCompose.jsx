import { useState, useEffect, useCallback } from 'react';
import { api } from '../api/client';
import Spinner from '../components/Spinner';
import Modal from '../components/Modal';
import ComposeEditor from '../components/ComposeEditor';
import DiffPanel from '../components/DiffPanel';
import TransferProgress from '../components/TransferProgress';
import { useToast } from '../components/Toast';
import { getCurrentEndpoint, setEndpoint } from '../components/EndpointSwitcher';

const STEPS = [
  { id: 'analyze', label: '1. Analyze', description: 'Compare compose files between source and target' },
  { id: 'prepare', label: '2. Prepare Target', description: 'Create volumes and pull images on target' },
  { id: 'transfer', label: '3. Transfer Volumes', description: 'Copy volume data from source to target' },
];

export default function MigrationCompose({ navigate }) {
  const toast = useToast();

  // Endpoints
  const [endpoints, setEndpoints] = useState([]);
  const [sourceEndpoint, setSourceEndpoint] = useState(() => getCurrentEndpoint());
  const [targetEndpoint, setTargetEndpoint] = useState('');

  // Stack name
  const [stackName, setStackName] = useState('');

  // State
  const [loading, setLoading] = useState(false);
  const [currentStep, setCurrentStep] = useState('analyze');

  // Analysis results
  const [analysis, setAnalysis] = useState(null);
  const [sourceYaml, setSourceYaml] = useState('');
  const [targetYaml, setTargetYaml] = useState('');
  const [showSourceYaml, setShowSourceYaml] = useState(false);
  const [showTargetYaml, setShowTargetYaml] = useState(false);

  // Prepare results
  const [prepareResult, setPrepareResult] = useState(null);

  // Transfer results
  const [transferResult, setTransferResult] = useState(null);

  // Load endpoints on mount
  useEffect(() => {
    (async () => {
      try {
        const data = await api.get('/api/endpoints');
        const eps = Array.isArray(data) ? data : (data?.endpoints || []);
        setEndpoints(eps);
      } catch { /* ignore */ }
    })();
  }, []);

  // Auto-fill target endpoint if only one non-source endpoint exists
  useEffect(() => {
    if (!targetEndpoint && endpoints.length > 0) {
      const others = endpoints.filter(e => (e.id || e.Id) !== sourceEndpoint);
      if (others.length === 1) {
        setTargetEndpoint(others[0].id || others[0].Id);
      }
    }
  }, [endpoints, sourceEndpoint, targetEndpoint]);

  const handleSourceChange = useCallback((epId) => {
    setSourceEndpoint(epId);
    setEndpoint(epId);
    // Reset analysis when source changes
    setAnalysis(null);
    setPrepareResult(null);
    setTransferResult(null);
    setCurrentStep('analyze');
  }, []);

  // ── Step 1: Analyze ──
  const handleAnalyze = async () => {
    if (!sourceEndpoint || !targetEndpoint) {
      toast('Select both source and target endpoints', 'error');
      return;
    }
    if (!stackName.trim()) {
      toast('Enter a stack name', 'error');
      return;
    }

    setLoading(true);
    try {
      const result = await api.post('/migration/compose/analyze', {
        sourceEndpoint,
        targetEndpoint,
        stackName: stackName.trim(),
      });
      setAnalysis(result);
      // Store YAML if present
      if (result.sourceComposeYaml) setSourceYaml(result.sourceComposeYaml);
      if (result.targetComposeYaml) setTargetYaml(result.targetComposeYaml);
      setCurrentStep('prepare');
      toast('Analysis complete', 'success');
    } catch (err) {
      toast('Analysis failed: ' + err.message, 'error');
    } finally {
      setLoading(false);
    }
  };

  // ── Step 2: Prepare Target ──
  const handlePrepare = async () => {
    if (!analysis) return;

    // Extract volumes from diff
    const volumes = {};
    const volumeChanges = analysis.diff?.volumeChanges || [];
    volumeChanges.forEach(vc => {
      if (vc.changeType === 'added' || vc.changeType === 'modified') {
        volumes[vc.name] = vc.driver || 'local';
      }
    });

    setLoading(true);
    try {
      const result = await api.post('/migration/compose/prepare', {
        targetEndpoint,
        stackName: stackName.trim(),
        composeYaml: sourceYaml || '',
        volumes,
        pullImages: true,
      });
      setPrepareResult(result);
      setCurrentStep('transfer');
      const failedCount = (result.errors || []).length;
      if (failedCount > 0) {
        toast(`Target prepared with ${failedCount} warnings`, 'info');
      } else {
        toast('Target prepared successfully', 'success');
      }
    } catch (err) {
      toast('Prepare failed: ' + err.message, 'error');
    } finally {
      setLoading(false);
    }
  };

  // ── Step 3: Transfer Volumes ──
  const handleTransfer = async () => {
    if (!prepareResult) return;

    const volumeResults = (prepareResult.results || []).filter(r => r.type === 'volume' && r.status === 'created');
    if (volumeResults.length === 0) {
      toast('No volumes to transfer', 'info');
      return;
    }

    const transfers = volumeResults.map(vr => ({
      sourceVolume: `${stackName.trim()}_${vr.name}`,
      targetVolume: `${stackName.trim()}_${vr.name}`,
    }));

    setLoading(true);
    try {
      const result = await api.post('/migration/transfer', {
        sourceEndpoint,
        targetEndpoint,
        transfers,
        compression: 'gzip',
      });
      setTransferResult(result);
      const status = result.status;
      if (status === 'success') {
        toast('Transfer complete ✓', 'success');
      } else if (status === 'partial_success') {
        toast('Transfer completed with some failures', 'info');
      } else {
        toast('Transfer failed', 'error');
      }
    } catch (err) {
      toast('Transfer failed: ' + err.message, 'error');
    } finally {
      setLoading(false);
    }
  };

  const handleReset = () => {
    setAnalysis(null);
    setSourceYaml('');
    setTargetYaml('');
    setPrepareResult(null);
    setTransferResult(null);
    setCurrentStep('analyze');
  };

  // ── Endpoint selector (inline pattern from Migration.jsx) ──
  const renderEndpointSelect = (label, value, onChange, excludeId) => (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '4px', minWidth: 0 }}>
      <label style={{ fontSize: '0.8rem', fontWeight: 600, color: 'var(--text-secondary)' }}>{label}</label>
      <select
        value={value}
        onChange={e => onChange(e.target.value)}
        style={{ width: '100%' }}
      >
        <option value="">— Select endpoint —</option>
        <option value="local">Local</option>
        {endpoints
          .filter(ep => (ep.id || ep.Id) !== excludeId)
          .map(ep => {
            const id = ep.id || ep.Id;
            const name = ep.name || ep.Name || id;
            return <option key={id} value={id}>{name}</option>;
          })}
      </select>
    </div>
  );

  return (
    <div style={{ padding: '24px', maxWidth: '1200px', margin: '0 auto' }}>
      <div style={{ marginBottom: '24px' }}>
        <h2 style={{ margin: '0 0 4px' }}>Compose Template Migration</h2>
        <p style={{ color: 'var(--text-secondary)', fontSize: '0.85rem', margin: 0 }}>
          Analyze, prepare, and transfer Docker Compose stacks between endpoints
        </p>
      </div>

      {/* Step indicator */}
      <div style={{
        display: 'flex',
        gap: '0',
        marginBottom: '24px',
        background: 'var(--bg-secondary)',
        borderRadius: '8px',
        padding: '4px',
        border: '1px solid var(--border)',
      }}>
        {STEPS.map((step, i) => {
          const isCurrent = currentStep === step.id;
          const isDone = (
            (step.id === 'analyze' && (currentStep === 'prepare' || currentStep === 'transfer')) ||
            (step.id === 'prepare' && currentStep === 'transfer') ||
            (step.id === 'transfer' && transferResult)
          );
          return (
            <div
              key={step.id}
              style={{
                flex: 1,
                padding: '10px 16px',
                textAlign: 'center',
                borderRadius: '6px',
                background: isCurrent ? 'var(--accent-dim)' : isDone ? 'var(--green-dim)' : 'transparent',
                color: isCurrent ? 'var(--accent)' : isDone ? 'var(--green)' : 'var(--text-secondary)',
                fontWeight: isCurrent ? 600 : 400,
                fontSize: '0.8rem',
                transition: 'all 0.2s',
              }}
              title={step.description}
            >
              {isDone ? '✓ ' : i + 1 + '. '}{step.label}
            </div>
          );
        })}
      </div>

      {/* Loading overlay */}
      {loading && (
        <div style={{ display: 'flex', justifyContent: 'center', padding: '40px' }}>
          <Spinner size="lg" />
        </div>
      )}

      {/* ── Step 1: Analyze ── */}
      {!loading && (
        <div className="card" style={{ padding: '20px' }}>
          {/* Endpoint selectors */}
          <div style={{
            display: 'grid',
            gridTemplateColumns: '1fr 1fr',
            gap: '16px',
            marginBottom: '16px',
          }}>
            {renderEndpointSelect('Source Endpoint', sourceEndpoint, handleSourceChange)}
            {renderEndpointSelect('Target Endpoint', targetEndpoint, setTargetEndpoint, sourceEndpoint)}
          </div>

          {/* Stack name */}
          <div style={{ marginBottom: '16px' }}>
            <label style={{ fontSize: '0.8rem', fontWeight: 600, color: 'var(--text-secondary)' }}>Stack Name</label>
            <input
              type="text"
              value={stackName}
              onChange={e => setStackName(e.target.value)}
              placeholder="e.g., wordpress, monitoring, myapp"
              style={{ width: '100%', marginTop: '4px' }}
              onKeyDown={e => { if (e.key === 'Enter') handleAnalyze(); }}
            />
          </div>

          {/* Action buttons */}
          <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
            <button onClick={handleAnalyze} disabled={loading}>
              🔍 Analyze
            </button>
            {analysis && (
              <>
                <button
                  className="outline"
                  onClick={() => setShowSourceYaml(true)}
                >
                  📄 View Source Compose
                </button>
                <button
                  className="outline"
                  onClick={() => setShowTargetYaml(true)}
                >
                  📄 View Target Compose
                </button>
                <button
                  className="outline contrast"
                  onClick={handleReset}
                >
                  ↺ Reset
                </button>
              </>
            )}
          </div>

          {/* Diff Panel */}
          {analysis?.diff && (
            <div style={{ marginTop: '20px' }}>
              <DiffPanel diff={analysis.diff} />
            </div>
          )}

          {/* Prepare button */}
          {analysis && currentStep === 'prepare' && (
            <div style={{ marginTop: '20px', paddingTop: '16px', borderTop: '1px solid var(--border)' }}>
              <button onClick={handlePrepare} disabled={loading}>
                ⚙ Prepare Target
              </button>
              <span style={{ marginLeft: '12px', fontSize: '0.8rem', color: 'var(--text-secondary)' }}>
                Creates volumes and pulls images on target endpoint
              </span>
            </div>
          )}

          {/* Prepare results */}
          {prepareResult && (
            <div style={{ marginTop: '20px' }}>
              <h3 style={{ fontSize: '0.95rem', marginBottom: '12px' }}>
                Prepare Results ({prepareResult.status})
              </h3>
              <div style={{ display: 'grid', gap: '8px' }}>
                {(prepareResult.results || []).map((r, i) => (
                  <div key={i} style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: '10px',
                    padding: '8px 12px',
                    background: r.status === 'failed' ? 'var(--red-dim)' : 'var(--green-dim)',
                    borderRadius: '6px',
                    fontSize: '0.85rem',
                  }}>
                    <span style={{
                      color: r.status === 'failed' ? 'var(--red)' : 'var(--green)',
                      fontWeight: 600,
                    }}>
                      {r.status === 'failed' ? '✗' : '✓'}
                    </span>
                    <span style={{ flex: 1 }}>{r.type}: {r.name}</span>
                    <span style={{ color: 'var(--text-secondary)' }}>{r.status}</span>
                    {r.error && <span style={{ color: 'var(--red)', fontSize: '0.75rem' }}>{r.error}</span>}
                  </div>
                ))}
              </div>

              {/* Transfer button */}
              {currentStep === 'transfer' && (
                <div style={{ marginTop: '16px', paddingTop: '16px', borderTop: '1px solid var(--border)' }}>
                  <button onClick={handleTransfer} disabled={loading}>
                    🚀 Transfer Volumes
                  </button>
                  <span style={{ marginLeft: '12px', fontSize: '0.8rem', color: 'var(--text-secondary)' }}>
                    Copies volume data from source to target endpoints
                  </span>
                </div>
              )}
            </div>
          )}

          {/* Transfer results */}
          {transferResult && (
            <div style={{ marginTop: '20px' }}>
              <h3 style={{ fontSize: '0.95rem', marginBottom: '12px' }}>Transfer Results</h3>
              <TransferProgress
                results={transferResult.results}
                totalBytes={transferResult.totalBytes}
                status={transferResult.status}
              />
              {transferResult.warnings?.length > 0 && (
                <div style={{ marginTop: '12px' }}>
                  {transferResult.warnings.map((w, i) => (
                    <div key={i} style={{
                      padding: '6px 12px',
                      background: 'var(--yellow-dim)',
                      borderRadius: '4px',
                      fontSize: '0.8rem',
                      marginBottom: '4px',
                    }}>
                      ⚠ {w}
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>
      )}

      {/* Source YAML Modal */}
      {showSourceYaml && (
        <Modal title="Source Compose YAML" onClose={() => setShowSourceYaml(false)} size="large">
          <ComposeEditor
            label={`${stackName || 'stack'} (${sourceEndpoint})`}
            value={sourceYaml}
            readOnly
          />
        </Modal>
      )}

      {/* Target YAML Modal */}
      {showTargetYaml && (
        <Modal title="Target Compose YAML" onClose={() => setShowTargetYaml(false)} size="large">
          <ComposeEditor
            label={`${stackName || 'stack'} (${targetEndpoint})`}
            value={targetYaml}
            readOnly
          />
        </Modal>
      )}
    </div>
  );
}
