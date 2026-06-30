import { useState, useEffect, useCallback } from 'react';
import { api } from '../api/client';
import Spinner from './Spinner';
import { useToast } from './Toast';

function StatusIcon({ status }) {
  const config = {
    pass: { icon: '✓', color: 'var(--green)', bg: 'var(--green-dim)' },
    warn: { icon: '⚠', color: 'var(--yellow)', bg: 'var(--yellow-dim)' },
    fail: { icon: '✗', color: 'var(--red)', bg: 'var(--red-dim)' },
  };
  const c = config[status] || { icon: '?', color: 'var(--text-secondary)', bg: 'var(--bg-tertiary)' };

  return (
    <span style={{
      display: 'inline-flex',
      alignItems: 'center',
      justifyContent: 'center',
      width: '28px',
      height: '28px',
      borderRadius: '50%',
      background: c.bg,
      color: c.color,
      fontWeight: 700,
      fontSize: '0.9rem',
      flexShrink: 0,
    }}>
      {c.icon}
    </span>
  );
}

function StatusBadgeSmall({ status }) {
  const labels = { pass: 'Pass', warn: 'Warn', fail: 'Fail' };
  const colors = {
    pass: { fg: 'var(--green)', bg: 'var(--green-dim)' },
    warn: { fg: 'var(--yellow)', bg: 'var(--yellow-dim)' },
    fail: { fg: 'var(--red)', bg: 'var(--red-dim)' },
  };
  const c = colors[status] || colors.pass;
  return (
    <span style={{
      display: 'inline-block',
      padding: '2px 10px',
      borderRadius: '10px',
      fontSize: '0.7rem',
      fontWeight: 600,
      background: c.bg,
      color: c.fg,
      textTransform: 'uppercase',
      letterSpacing: '0.04em',
    }}>
      {labels[status] || status}
    </span>
  );
}

export default function PreflightPanel({ planId, onContinue, onBack }) {
  const toast = useToast();
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [results, setResults] = useState(null);

  const runPreflight = useCallback(async () => {
    if (!planId) return;
    setLoading(true);
    setError(null);
    try {
      const data = await api.post(`/api/migration/unified/plan/${planId}/preflight`);
      setResults(data);
    } catch (err) {
      setError(err.message || 'Pre-flight check failed');
      toast('Pre-flight check failed: ' + err.message, 'error');
    } finally {
      setLoading(false);
    }
  }, [planId, toast]);

  useEffect(() => {
    runPreflight();
  }, [runPreflight]);

  const checks = results?.checks || [];

  const passCount = checks.filter(c => c.status === 'pass').length;
  const warnCount = checks.filter(c => c.status === 'warn').length;
  const failCount = checks.filter(c => c.status === 'fail').length;

  const canContinue = failCount === 0;

  if (loading) {
    return (
      <div style={{ textAlign: 'center', padding: '40px' }}>
        <Spinner size="lg" />
        <div style={{ marginTop: '16px', color: 'var(--text-secondary)' }}>
          Running pre-flight checks...
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div style={{ textAlign: 'center', padding: '32px' }}>
        <div style={{
          padding: '16px',
          background: 'var(--red-dim)',
          borderRadius: '8px',
          color: 'var(--red)',
          marginBottom: '16px',
        }}>
          ✗ {error}
        </div>
        <div style={{ display: 'flex', gap: '8px', justifyContent: 'center' }}>
          <button onClick={runPreflight}>↻ Retry</button>
          {onBack && <button className="outline" onClick={onBack}>← Back to Editor</button>}
        </div>
      </div>
    );
  }

  return (
    <div style={{ display: 'grid', gap: '16px' }}>
      {/* Summary bar */}
      <div style={{
        display: 'flex',
        alignItems: 'center',
        gap: '16px',
        padding: '16px',
        background: 'var(--bg-secondary)',
        borderRadius: '8px',
        border: `1px solid ${canContinue ? 'var(--green)' : 'var(--red)'}`,
      }}>
        <div style={{ flex: 1 }}>
          <div style={{ fontWeight: 600, fontSize: '1rem', marginBottom: '4px' }}>
            {canContinue ? '✓ Ready to Proceed' : '⚠ Issues Detected'}
          </div>
          <div style={{ fontSize: '0.8rem', color: 'var(--text-secondary)' }}>
            {passCount} passed, {warnCount} warnings, {failCount} failures
          </div>
        </div>
        <div style={{ display: 'flex', gap: '6px' }}>
          <span style={{
            padding: '4px 10px',
            borderRadius: '12px',
            background: 'var(--green-dim)',
            color: 'var(--green)',
            fontSize: '0.75rem',
            fontWeight: 600,
          }}>
            ✓ {passCount}
          </span>
          {warnCount > 0 && (
            <span style={{
              padding: '4px 10px',
              borderRadius: '12px',
              background: 'var(--yellow-dim)',
              color: 'var(--yellow)',
              fontSize: '0.75rem',
              fontWeight: 600,
            }}>
              ⚠ {warnCount}
            </span>
          )}
          {failCount > 0 && (
            <span style={{
              padding: '4px 10px',
              borderRadius: '12px',
              background: 'var(--red-dim)',
              color: 'var(--red)',
              fontSize: '0.75rem',
              fontWeight: 600,
            }}>
              ✗ {failCount}
            </span>
          )}
        </div>
      </div>

      {/* Checks grid */}
      {checks.length === 0 ? (
        <div className="card">
          <div style={{ textAlign: 'center', padding: '24px', color: 'var(--text-secondary)' }}>
            No pre-flight checks configured for this migration type
          </div>
        </div>
      ) : (
        <div style={{ display: 'grid', gap: '8px' }}>
          {checks.map((check, idx) => (
            <div
              key={idx}
              style={{
                display: 'flex',
                alignItems: 'flex-start',
                gap: '12px',
                padding: '14px 16px',
                background: 'var(--bg-secondary)',
                borderRadius: '8px',
                border: '1px solid var(--border)',
              }}
            >
              <StatusIcon status={check.status} />
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '4px' }}>
                  <span style={{ fontWeight: 600, fontSize: '0.9rem' }}>{check.name}</span>
                  <StatusBadgeSmall status={check.status} />
                </div>
                {check.message && (
                  <div style={{ fontSize: '0.8rem', color: 'var(--text-secondary)', lineHeight: 1.5 }}>
                    {check.message}
                  </div>
                )}
                {check.suggestion && check.status !== 'pass' && (
                  <div style={{
                    marginTop: '6px',
                    padding: '8px 12px',
                    background: check.status === 'fail' ? 'var(--red-dim)' : 'var(--yellow-dim)',
                    borderRadius: '6px',
                    fontSize: '0.8rem',
                    color: check.status === 'fail' ? 'var(--red)' : 'var(--yellow)',
                  }}>
                    <strong>Suggestion:</strong> {check.suggestion}
                  </div>
                )}
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Action buttons */}
      <div style={{
        display: 'flex',
        gap: '12px',
        justifyContent: 'center',
        marginTop: '8px',
        padding: '16px 0',
        borderTop: '1px solid var(--border)',
      }}>
        {onBack && (
          <button className="outline" onClick={onBack}>
            ← Fix Issues
          </button>
        )}
        <button
          className="btn-primary"
          onClick={onContinue}
          disabled={!canContinue}
          style={!canContinue ? { opacity: 0.5, cursor: 'not-allowed' } : {}}
          title={!canContinue ? 'Resolve all failures before continuing' : ''}
        >
          {canContinue ? '▶ Continue Anyway' : `✗ ${failCount} failure${failCount > 1 ? 's' : ''} must be fixed`}
        </button>
      </div>
    </div>
  );
}
