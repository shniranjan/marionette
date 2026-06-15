import { useState } from 'react';

export default function JsonTree({ data }) {
  const [expanded, setExpanded] = useState({});

  const toggle = (path) => {
    setExpanded((prev) => ({ ...prev, [path]: !prev[path] }));
  };

  const renderValue = (value, path = '', depth = 0) => {
    if (value === null) return <span style={{ color: 'var(--text-secondary)' }}>null</span>;
    if (value === undefined) return <span style={{ color: 'var(--text-secondary)' }}>undefined</span>;

    if (typeof value === 'boolean') {
      return <span style={{ color: 'var(--accent)' }}>{String(value)}</span>;
    }
    if (typeof value === 'number') {
      return <span style={{ color: 'var(--green)' }}>{value}</span>;
    }
    if (typeof value === 'string') {
      return <span style={{ color: 'var(--yellow)' }}>"{value}"</span>;
    }

    if (Array.isArray(value)) {
      if (value.length === 0) return <span style={{ color: 'var(--text-secondary)' }}>[]</span>;
      const isOpen = expanded[path] !== false;
      return (
        <div style={{ paddingLeft: depth > 0 ? 16 : 0 }}>
          <span
            onClick={() => toggle(path)}
            style={{ cursor: 'pointer', userSelect: 'none', color: 'var(--text-secondary)' }}
          >
            {isOpen ? '▼' : '▶'} [{value.length}]
          </span>
          {isOpen && (
            <div style={{ paddingLeft: 16 }}>
              {value.map((item, i) => (
                <div key={i}>
                  <span style={{ color: 'var(--text-secondary)' }}>{i}: </span>
                  {renderValue(item, `${path}.${i}`, depth + 1)}
                </div>
              ))}
            </div>
          )}
        </div>
      );
    }

    if (typeof value === 'object') {
      const keys = Object.keys(value);
      if (keys.length === 0) return <span style={{ color: 'var(--text-secondary)' }}>{'{}'}</span>;
      const isOpen = expanded[path] !== false;
      return (
        <div style={{ paddingLeft: depth > 0 ? 16 : 0 }}>
          <span
            onClick={() => toggle(path)}
            style={{ cursor: 'pointer', userSelect: 'none', color: 'var(--text-secondary)' }}
          >
            {isOpen ? '▼' : '▶'} {'{'} {keys.length} {'}'}
          </span>
          {isOpen && (
            <div style={{ paddingLeft: 16 }}>
              {keys.map((k) => (
                <div key={k}>
                  <span style={{ color: 'var(--accent)' }}>"{k}"</span>
                  <span style={{ color: 'var(--text-secondary)' }}>: </span>
                  {renderValue(value[k], `${path}.${k}`, depth + 1)}
                </div>
              ))}
            </div>
          )}
        </div>
      );
    }

    return <span>{String(value)}</span>;
  };

  return (
    <div className="mono" style={{ fontSize: '0.8rem', lineHeight: '1.6' }}>
      {renderValue(data, 'root')}
    </div>
  );
}
