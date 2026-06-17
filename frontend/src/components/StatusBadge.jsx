export default function StatusBadge({ state }) {
  const s = (state || '').toLowerCase();
  const colorVar = {
    running: 'var(--green)',
    paused: 'var(--yellow)',
    stopped: 'var(--red)',
    exited: 'var(--red)',
    removing: 'var(--red)',
    dead: 'var(--red)',
  }[s] || 'var(--pico-muted-color, #8b949e)';

  return (
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
      {state || 'unknown'}
    </span>
  );
}
