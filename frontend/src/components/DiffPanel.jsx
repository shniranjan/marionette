function ChangeBadge({ type }) {
  const colors = {
    added: { bg: 'var(--green-dim)', fg: 'var(--green)' },
    removed: { bg: 'var(--red-dim)', fg: 'var(--red)' },
    modified: { bg: 'var(--yellow-dim)', fg: 'var(--yellow)' },
    renamed: { bg: 'var(--accent-dim)', fg: 'var(--accent)' },
    unchanged: { bg: 'var(--bg-tertiary)', fg: 'var(--text-secondary)' },
  };
  const c = colors[type] || colors.unchanged;
  return (
    <span style={{
      display: 'inline-block',
      padding: '1px 6px',
      borderRadius: '4px',
      fontSize: '0.7rem',
      fontWeight: 600,
      background: c.bg,
      color: c.fg,
      textTransform: 'uppercase',
      letterSpacing: '0.04em',
    }}>
      {type}
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
  return `${(ms / 60000).toFixed(1)}m`;
}

function SummaryTable({ rows }) {
  if (!rows || rows.length === 0) {
    return <p style={{ color: 'var(--text-secondary)', fontSize: '0.8rem', padding: '8px 0' }}>No changes detected.</p>;
  }
  return (
    <table style={{ width: '100%', marginTop: '8px' }}>
      <thead>
        <tr>
          {Object.keys(rows[0]).map((k) => (
            <th key={k} style={{ fontSize: '0.75rem', textTransform: 'capitalize' }}>
              {k.replace(/([A-Z])/g, ' $1').trim()}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.map((row, i) => (
          <tr key={i}>
            {Object.entries(row).map(([k, v], j) => (
              <td key={j} style={{ fontSize: '0.8rem' }}>
                {k === 'changeType' || k === 'type' ? <ChangeBadge type={v} /> :
                 k === 'bytesTransferred' ? formatBytes(v) :
                 k === 'durationMs' ? formatMs(v) :
                 typeof v === 'boolean' ? (v ? '✓' : '✗') :
                 v ?? '—'}
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function CommandBlock({ commands, label }) {
  const [copied, setCopied] = React.useState(false);

  if (!commands || commands.length === 0) return null;

  const text = commands.join('\n');

  const handleCopy = () => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }).catch(() => {});
  };

  return (
    <div style={{ marginTop: '8px' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '4px' }}>
        <span style={{ fontSize: '0.75rem', fontWeight: 600, color: 'var(--text-secondary)' }}>{label}</span>
        <button
          onClick={handleCopy}
          style={{
            fontSize: '0.65rem',
            padding: '2px 8px',
            border: '1px solid var(--border-color)',
            borderRadius: '4px',
            background: copied ? 'var(--green-dim)' : 'var(--bg-tertiary)',
            color: copied ? 'var(--green)' : 'var(--text-secondary)',
            cursor: 'pointer',
          }}
        >
          {copied ? '✓ Copied' : 'Copy'}
        </button>
      </div>
      <pre
        style={{
          background: 'var(--bg-tertiary)',
          border: '1px solid var(--border-color)',
          borderRadius: '6px',
          padding: '10px 14px',
          fontSize: '0.72rem',
          fontFamily: "'SF Mono', 'Fira Code', 'Cascadia Code', monospace",
          color: 'var(--text-primary)',
          overflowX: 'auto',
          whiteSpace: 'pre-wrap',
          wordBreak: 'break-word',
          margin: 0,
          lineHeight: '1.5',
        }}
      >
        <code>{text}</code>
      </pre>
    </div>
  );
}

function DatabaseServiceDetail({ ds }) {
  const dbTypeStr = typeof ds.dbType === 'string' ? ds.dbType :
    (ds.dbType?.PostgreSQL || ds.dbType?.MySQL || ds.dbType?.MongoDB || ds.dbType?.Redis || ds.dbType?.Other || '—');

  return (
    <div style={{
      padding: '12px',
      background: 'var(--bg-secondary)',
      borderRadius: '8px',
      marginTop: '8px',
    }}>
      <table style={{ width: '100%', marginBottom: '12px' }}>
        <thead>
          <tr>
            <th style={{ fontSize: '0.75rem', textAlign: 'left' }}>Service</th>
            <th style={{ fontSize: '0.75rem', textAlign: 'left' }}>Type</th>
            <th style={{ fontSize: '0.75rem', textAlign: 'left' }}>Image</th>
            <th style={{ fontSize: '0.75rem', textAlign: 'left' }}>Version</th>
            {ds.username && <th style={{ fontSize: '0.75rem', textAlign: 'left' }}>User</th>}
            {ds.port && <th style={{ fontSize: '0.75rem', textAlign: 'left' }}>Port</th>}
            {ds.databaseName && <th style={{ fontSize: '0.75rem', textAlign: 'left' }}>Database</th>}
            <th style={{ fontSize: '0.75rem', textAlign: 'left' }}>Replication</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td style={{ fontSize: '0.8rem' }}><strong>{ds.serviceName}</strong></td>
            <td style={{ fontSize: '0.8rem' }}>{dbTypeStr}</td>
            <td style={{ fontSize: '0.8rem' }}>{ds.image}</td>
            <td style={{ fontSize: '0.8rem' }}>{ds.version || '—'}</td>
            {ds.username && <td style={{ fontSize: '0.8rem' }}>{ds.username}</td>}
            {ds.port && <td style={{ fontSize: '0.8rem' }}>{ds.port}</td>}
            {ds.databaseName && <td style={{ fontSize: '0.8rem' }}>{ds.databaseName}</td>}
            <td style={{ fontSize: '0.8rem' }}>{ds.hasReplication ? '✓' : '✗'}</td>
          </tr>
        </tbody>
      </table>

      <CommandBlock commands={ds.preTransferCommands} label="Pre-transfer Commands" />
      <CommandBlock commands={ds.postTransferCommands} label="Post-transfer Commands" />
    </div>
  );
}

export default function DiffPanel({ diff }) {
  if (!diff) return null;

  const volumeChanges = diff.volumeChanges || [];
  const serviceChanges = diff.serviceChanges || [];
  const imageChanges = diff.imageChanges || [];
  const envChanges = diff.envChanges || [];
  const portChanges = diff.portChanges || [];
  const architecture = diff.architecture;
  const databaseServices = diff.databaseServices || [];
  const warnings = diff.warnings || [];

  const hasAnyChanges =
    volumeChanges.length > 0 ||
    serviceChanges.length > 0 ||
    imageChanges.length > 0 ||
    envChanges.length > 0 ||
    portChanges.length > 0 ||
    architecture ||
    databaseServices.length > 0;

  if (!hasAnyChanges && warnings.length === 0) {
    return (
      <article style={{ padding: '24px', textAlign: 'center' }}>
        <p style={{ color: 'var(--text-secondary)' }}>No differences detected — compose files are identical.</p>
      </article>
    );
  }

  return (
    <div style={{ display: 'grid', gap: '8px' }}>
      {/* Volume Changes */}
      {volumeChanges.length > 0 && (
        <details open>
          <summary style={{ cursor: 'pointer', padding: '8px 0', fontWeight: 600, fontSize: '0.9rem' }}>
            Volume Changes ({volumeChanges.length})
          </summary>
          <SummaryTable rows={volumeChanges.map(vc => ({
            name: vc.name,
            type: vc.changeType,
            driver: vc.driver || 'local',
            details: vc.details,
            source: vc.sourceName,
          }))} />
        </details>
      )}

      {/* Service Changes */}
      {serviceChanges.length > 0 && (
        <details open>
          <summary style={{ cursor: 'pointer', padding: '8px 0', fontWeight: 600, fontSize: '0.9rem' }}>
            Service Changes ({serviceChanges.length})
          </summary>
          <SummaryTable rows={serviceChanges.map(sc => ({
            service: sc.name,
            type: sc.changeType,
            oldImage: sc.imageOld,
            newImage: sc.imageNew,
            details: sc.details,
          }))} />
        </details>
      )}

      {/* Image Changes */}
      {imageChanges.length > 0 && (
        <details open>
          <summary style={{ cursor: 'pointer', padding: '8px 0', fontWeight: 600, fontSize: '0.9rem' }}>
            Image Changes ({imageChanges.length})
          </summary>
          <SummaryTable rows={imageChanges.map(ic => ({
            service: ic.serviceName,
            oldImage: ic.oldImage,
            newImage: ic.newImage,
            majorVersion: ic.majorVersionChange,
          }))} />
        </details>
      )}

      {/* Env Changes */}
      {envChanges.length > 0 && (
        <details>
          <summary style={{ cursor: 'pointer', padding: '8px 0', fontWeight: 600, fontSize: '0.9rem' }}>
            Environment Changes ({envChanges.length})
          </summary>
          <SummaryTable rows={envChanges.map(ec => ({
            service: ec.serviceName,
            variable: ec.varName,
            oldValue: ec.isSensitive ? '***' : ec.oldValue,
            newValue: ec.isSensitive ? '***' : ec.newValue,
            sensitive: ec.isSensitive,
          }))} />
        </details>
      )}

      {/* Port Changes */}
      {portChanges.length > 0 && (
        <details>
          <summary style={{ cursor: 'pointer', padding: '8px 0', fontWeight: 600, fontSize: '0.9rem' }}>
            Port Changes ({portChanges.length})
          </summary>
          <SummaryTable rows={portChanges.map(pc => ({
            service: pc.serviceName,
            type: pc.changeType,
            mapping: pc.portMapping,
          }))} />
        </details>
      )}

      {/* Architecture Warning */}
      {architecture && (
        <details open={!!architecture.mismatch}>
          <summary style={{
            cursor: 'pointer',
            padding: '8px 0',
            fontWeight: 600,
            fontSize: '0.9rem',
            color: architecture.mismatch ? 'var(--red)' : 'var(--text-primary)',
          }}>
            Architecture {architecture.mismatch ? '⚠️ Mismatch' : '✓ Match'}
          </summary>
          <div style={{
            padding: '12px',
            background: architecture.mismatch ? 'var(--red-dim)' : 'var(--green-dim)',
            borderRadius: '8px',
            fontSize: '0.85rem',
          }}>
            <div>Source: <strong>{architecture.sourceArch || 'unknown'}</strong></div>
            <div>Target: <strong>{architecture.targetArch || 'unknown'}</strong></div>
            {architecture.warning && (
              <div style={{ marginTop: '4px', color: 'var(--red)' }}>{architecture.warning}</div>
            )}
          </div>
        </details>
      )}

      {/* Database Services */}
      {databaseServices.length > 0 && (
        <details>
          <summary style={{ cursor: 'pointer', padding: '8px 0', fontWeight: 600, fontSize: '0.9rem' }}>
            Database Services ({databaseServices.length})
          </summary>
          {databaseServices.map((ds, i) => (
            <DatabaseServiceDetail key={i} ds={ds} />
          ))}
        </details>
      )}

      {/* Warnings */}
      {warnings.length > 0 && (
        <details open>
          <summary style={{ cursor: 'pointer', padding: '8px 0', fontWeight: 600, fontSize: '0.9rem', color: 'var(--yellow)' }}>
            ⚠ Warnings ({warnings.length})
          </summary>
          <ul style={{ margin: '4px 0 0 20px', padding: 0 }}>
            {warnings.map((w, i) => (
              <li key={i} style={{ color: 'var(--text-secondary)', fontSize: '0.8rem', marginBottom: '4px' }}>{w}</li>
            ))}
          </ul>
        </details>
      )}
    </div>
  );
}
