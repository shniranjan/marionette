import { useState, useCallback } from 'react';

export default function ListToolbar({ selected, total, onClear, actions }) {
  if (!selected || selected.size === 0) return null;

  return (
    <div style={{
      display: 'flex',
      alignItems: 'center',
      gap: '12px',
      padding: '8px 16px',
      marginBottom: '12px',
      background: 'var(--accent-dim)',
      borderRadius: '8px',
      color: '#fff',
      fontSize: '0.85rem',
    }}>
      <span style={{ fontWeight: 600 }}>
        {selected.size} of {total} selected
      </span>
      <div className="btn-group" style={{ marginLeft: 'auto' }}>
        {actions.map((action, i) => (
          <button
            key={i}
            className={action.variant === 'danger' ? 'btn-danger' : ''}
            onClick={action.onClick}
            disabled={action.disabled}
            style={{
              padding: '4px 12px',
              fontSize: '0.8rem',
              background: action.variant ? undefined : 'rgba(255,255,255,0.15)',
              border: '1px solid rgba(255,255,255,0.2)',
              color: '#fff',
            }}
          >
            {action.label}
          </button>
        ))}
        <button
          onClick={onClear}
          style={{
            padding: '4px 12px',
            fontSize: '0.8rem',
            background: 'transparent',
            border: '1px solid rgba(255,255,255,0.2)',
            color: '#fff',
          }}
        >
          ✕ Clear
        </button>
      </div>
    </div>
  );
}

// Hook: manages selection state for a list of items
export function useSelection(items, idKey = 'id') {
  const [selected, setSelected] = useState(new Set());

  const toggle = useCallback((item) => {
    const key = item[idKey] || item.name || item;
    setSelected(prev => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }, [idKey]);

  const selectAll = useCallback(() => {
    const keys = items.map(item => item[idKey] || item.name).filter(Boolean);
    setSelected(new Set(keys));
  }, [items, idKey]);

  const clear = useCallback(() => setSelected(new Set()), []);

  return { selected, toggle, selectAll, clear };
}
