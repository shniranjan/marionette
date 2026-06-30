function StatusBadge({ status }) {
  const colors = {
    success: { bg: 'var(--green-dim)', fg: 'var(--green)' },
    failed: { bg: 'var(--red-dim)', fg: 'var(--red)' },
    skipped: { bg: 'var(--bg-tertiary)', fg: 'var(--text-secondary)' },
    pending: { bg: 'var(--yellow-dim)', fg: 'var(--yellow)' },
  };
  const c = colors[status] || colors.skipped;
  return (
    <span style={{
      display: 'inline-block',
      padding: '2px 8px',
      borderRadius: '4px',
      fontSize: '0.7rem',
      fontWeight: 600,
      background: c.bg,
      color: c.fg,
      textTransform: 'uppercase',
      letterSpacing: '0.04em',
    }}>
      {status}
    </span>
  );
}

function formatBytes(bytes) {
  if (!bytes && bytes !== 0) return '—';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function formatMs(ms) {
  if (!ms && ms !== 0) return '—';
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  const m = Math.floor(ms / 60000);
  const s = ((ms % 60000) / 1000).toFixed(0);
  return `${m}m ${s}s`;
}

export default function TransferProgress({ results, totalBytes, status }) {
  if (!results || results.length === 0) return null;

  const successCount = results.filter(r => r.status === 'success').length;
  const failedCount = results.filter(r => r.status === 'failed').length;
  const skippedCount = results.filter(r => r.status === 'skipped').length;

  return (
    <div style={{ display: 'grid', gap: '12px' }}>
      {/* Summary */}
      <div style={{
        display: 'flex',
        gap: '16px',
        padding: '12px 16px',
        background: 'var(--bg-tertiary)',
        borderRadius: '8px',
        alignItems: 'center',
        flexWrap: 'wrap',
      }}>
        <div style={{ fontWeight: 600, fontSize: '0.9rem' }}>
          Transfer {status === 'success' ? '✓ Complete' : status === 'partial_success' ? '⚠ Partial' : '✗ Failed'}
        </div>
        <div style={{ display: 'flex', gap: '12px', fontSize: '0.8rem', color: 'var(--text-secondary)' }}>
          <span style={{ color: 'var(--green)' }}>{successCount} success</span>
          {failedCount > 0 && <span style={{ color: 'var(--red)' }}>{failedCount} failed</span>}
          {skippedCount > 0 && <span>{skippedCount} skipped</span>}
        </div>
        <div style={{ marginLeft: 'auto', fontSize: '0.8rem', color: 'var(--text-secondary)' }}>
          Total: {formatBytes(totalBytes)}
        </div>
      </div>

      {/* Per-volume results */}
      {results.map((r, i) => (
        <div
          key={i}
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: '12px',
            padding: '10px 14px',
            background: r.status === 'failed' ? 'var(--red-dim)' : 'var(--bg-secondary)',
            border: '1px solid var(--border)',
            borderRadius: '8px',
            flexWrap: 'wrap',
          }}
        >
          <div style={{ flex: '1 1 120px', minWidth: 0 }}>
            <div style={{ fontWeight: 600, fontSize: '0.85rem', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
              {r.sourceVolume}
            </div>
            {r.sourceVolume !== r.targetVolume && (
              <div style={{ fontSize: '0.7rem', color: 'var(--text-secondary)' }}>
                → {r.targetVolume}
              </div>
            )}
          </div>
          <StatusBadge status={r.status} />
          <span style={{ fontSize: '0.8rem', color: 'var(--text-secondary)', minWidth: '60px' }}>
            {formatBytes(r.bytesTransferred)}
          </span>
          <span style={{ fontSize: '0.8rem', color: 'var(--text-secondary)', minWidth: '80px', textAlign: 'right' }}>
            {formatMs(r.durationMs)}
          </span>
          {r.error && (
            <div style={{
              flex: '1 1 100%',
              fontSize: '0.75rem',
              color: 'var(--red)',
              padding: '4px 8px',
              background: 'var(--bg-tertiary)',
              borderRadius: '4px',
              marginTop: '4px',
            }}>
              {r.error}
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
