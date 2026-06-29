export default function StatusBadge({ state, health }) {
  const s = (state || '').toLowerCase();
  const colorVar = {
    running: 'var(--green)',
    paused: 'var(--yellow)',
    stopped: 'var(--red)',
    exited: 'var(--red)',
    removing: 'var(--red)',
    dead: 'var(--red)',
  }[s] || 'var(--pico-muted-color, #8b949e)';

  const healthColors = {
    healthy: 'var(--green)',
    unhealthy: 'var(--red)',
    starting: 'var(--yellow)',
  };

  const h = (health || '').toLowerCase();
  const healthColor = healthColors[h] || 'var(--pico-muted-color, #8b949e)';

  return (
    <span style={{ display: 'inline-flex', flexDirection: 'column', gap: '2px', alignItems: 'flex-start' }}>
      {state && (
        <span style={{
          display: 'inline-block',
          padding: '1px 10px',
          borderRadius: '12px',
          fontSize: '0.75rem',
          fontWeight: 600,
          color: colorVar,
          background: `color-mix(in srgb, ${colorVar} 15%, transparent)`,
          border: `1px solid color-mix(in srgb, ${colorVar} 30%, transparent)`,
          textTransform: 'capitalize',
          lineHeight: '1.6',
        }}>
          {state}
        </span>
      )}
      {health && (
        <span style={{
          display: 'inline-flex',
          alignItems: 'center',
          gap: '4px',
          fontSize: '0.7rem',
          fontWeight: 500,
          color: healthColor,
        }}>
          <span style={{
            display: 'inline-block',
            width: '6px',
            height: '6px',
            borderRadius: '50%',
            background: healthColor,
          }} />
          {health}
        </span>
      )}
    </span>
  );
}
