import { useState, useCallback } from 'react';

export default function SecretMask({ value, label }) {
  const [revealed, setRevealed] = useState(false);

  const toggle = useCallback(() => {
    setRevealed((r) => !r);
  }, []);

  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
      <span className="text-secondary" style={{ minWidth: '140px' }}>{label}</span>
      <code style={{
        flex: 1,
        padding: '2px 8px',
        background: 'var(--bg-tertiary)',
        borderRadius: '4px',
      }}>
        {revealed ? value : '••••••••'}
      </code>
      <button className="btn-sm" onClick={toggle}>
        {revealed ? 'Hide' : 'Reveal'}
      </button>
    </div>
  );
}
