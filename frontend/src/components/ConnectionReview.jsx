import { useState, useCallback } from 'react';

export default function ConnectionReview({ connections = [], onUpdate, blocked }) {
  const [revealed, setRevealed] = useState({});
  const [revealConfirm, setRevealConfirm] = useState(null);
  const [auditEntries, setAuditEntries] = useState([]);

  const handleReveal = useCallback((varName) => {
    if (revealConfirm !== varName) {
      setRevealConfirm(varName);
      return;
    }
    setRevealed(prev => ({ ...prev, [varName]: true }));
    setRevealConfirm(null);
    // Audit log
    const entry = {
      timestamp: new Date().toISOString(),
      action: 'reveal_connection',
      variable: varName,
    };
    setAuditEntries(prev => [...prev, entry]);
    const audit = JSON.parse(localStorage.getItem('marionette-audit-log') || '[]');
    audit.push(entry);
    localStorage.setItem('marionette-audit-log', JSON.stringify(audit));
  }, [revealConfirm]);

  const handleAction = useCallback((conn, action) => {
    if (onUpdate) onUpdate(conn.var_name, action);
  }, [onUpdate]);

  const criticalCount = connections.filter(c => c.will_break && !c.resolved).length;

  return (
    <div>
      {blocked && criticalCount > 0 && (
        <div style={{
          padding: '12px 16px',
          background: 'var(--red-dim)',
          border: '1px solid var(--red)',
          borderRadius: '8px',
          marginBottom: '16px',
          color: '#fff',
          fontSize: '0.85rem',
        }}>
          ⚠ {criticalCount} critical connection{criticalCount > 1 ? 's' : ''} must be resolved before proceeding.
        </div>
      )}

      {auditEntries.length > 0 && (
        <div style={{
          padding: '8px 12px',
          background: 'var(--bg-tertiary)',
          border: '1px solid var(--yellow)',
          borderRadius: '6px',
          marginBottom: '12px',
          fontSize: '0.75rem',
          color: 'var(--yellow)',
        }}>
          🔍 Audit: {auditEntries.length} credential{auditEntries.length > 1 ? 's' : ''} revealed this session
        </div>
      )}

      <table>
        <thead>
          <tr>
            <th>Env Variable</th>
            <th>Value</th>
            <th>Target Container</th>
            <th>Same Host?</th>
            <th>Will Break?</th>
            <th>Action</th>
          </tr>
        </thead>
        <tbody>
          {connections.length === 0 ? (
            <tr>
              <td colSpan={6} style={{ textAlign: 'center', color: 'var(--text-secondary)', padding: '24px' }}>
                No database connections detected
              </td>
            </tr>
          ) : (
            connections.map((conn, i) => {
              const isRevealed = revealed[conn.var_name];
              const isConfirming = revealConfirm === conn.var_name;
              const resolved = conn.resolved || false;
              return (
                <tr key={conn.var_name || i} style={{ opacity: resolved ? 0.5 : 1 }}>
                  <td className="mono" style={{ fontWeight: 500 }}>{conn.var_name || '—'}</td>
                  <td className="mono" style={{ fontSize: '0.8rem' }}>
                    {isRevealed ? (conn.value_masked || conn.value || '—') : (
                      <span style={{ letterSpacing: '2px' }}>••••••••</span>
                    )}
                    <button
                      className="btn-sm"
                      style={{
                        marginLeft: '8px',
                        fontSize: '0.65rem',
                        padding: '1px 6px',
                        ...(isConfirming ? { background: 'var(--yellow-dim)', borderColor: 'var(--yellow)', color: '#fff' } : {}),
                      }}
                      onClick={() => handleReveal(conn.var_name)}
                    >
                      {isRevealed ? 'Hide' : isConfirming ? '⚠ Confirm' : '👁'}
                    </button>
                    {isConfirming && (
                      <div style={{ color: 'var(--yellow)', fontSize: '0.7rem', marginTop: '2px' }}>
                        Click again to reveal (audited)
                      </div>
                    )}
                  </td>
                  <td className="mono" style={{ fontSize: '0.8rem' }}>
                    {conn.target_container || '—'}
                  </td>
                  <td>
                    <span style={{
                      color: conn.on_same_host ? 'var(--green)' : 'var(--yellow)',
                      fontWeight: 500,
                      fontSize: '0.8rem',
                    }}>
                      {conn.on_same_host ? 'Yes ✓' : 'No ✗'}
                    </span>
                  </td>
                  <td>
                    <span style={{
                      color: conn.will_break ? 'var(--red)' : 'var(--green)',
                      fontWeight: 600,
                      fontSize: '0.8rem',
                    }}>
                      {conn.will_break ? '⚠ YES' : '✓ No'}
                    </span>
                  </td>
                  <td>
                    {conn.will_break && !resolved ? (
                      <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                        {conn.fix_suggestion && (
                          <div style={{ fontSize: '0.7rem', color: 'var(--text-secondary)', marginBottom: '4px' }}>
                            {conn.fix_suggestion}
                          </div>
                        )}
                        <div className="btn-group">
                          <button
                            className="btn-sm"
                            style={{ fontSize: '0.7rem' }}
                            onClick={() => handleAction(conn, 'migrate_together')}
                          >
                            Migrate Together
                          </button>
                          <button
                            className="btn-sm"
                            style={{ fontSize: '0.7rem' }}
                            onClick={() => handleAction(conn, 'update_string')}
                          >
                            Update String
                          </button>
                          <button
                            className="btn-sm"
                            style={{ fontSize: '0.7rem' }}
                            onClick={() => handleAction(conn, 'skip')}
                          >
                            Skip
                          </button>
                        </div>
                      </div>
                    ) : resolved ? (
                      <span style={{ color: 'var(--green)', fontSize: '0.8rem' }}>
                        {conn.resolution || 'Resolved ✓'}
                      </span>
                    ) : (
                      <span style={{ color: 'var(--text-secondary)', fontSize: '0.8rem' }}>—</span>
                    )}
                  </td>
                </tr>
              );
            })
          )}
        </tbody>
      </table>
    </div>
  );
}
