export default function StatusBadge({ state }) {
  const colors = {
    running: 'var(--green)',
    paused: 'var(--yellow)',
    stopped: 'var(--red)',
    exited: 'var(--red)',
    removing: 'var(--red)',
    dead: 'var(--red)',
  };
  const bgColors = {
    running: 'var(--green-dim)',
    paused: 'var(--yellow-dim)',
    stopped: 'var(--red-dim)',
    exited: 'var(--red-dim)',
    removing: 'var(--red-dim)',
    dead: 'var(--red-dim)',
  };
  const s = (state || '').toLowerCase();
  const color = colors[s] || 'var(--text-secondary)';
  const bg = bgColors[s] || 'var(--bg-tertiary)';

  return (
    <span style={{
      display: 'inline-block',
      padding: '2px 10px',
      borderRadius: '12px',
      fontSize: '0.75rem',
      fontWeight: 600,
      color,
      background: bg,
      textTransform: 'capitalize',
    }}>
      {state || 'unknown'}
    </span>
  );
}
