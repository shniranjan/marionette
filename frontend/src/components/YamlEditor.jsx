import { useRef } from 'react';

export default function YamlEditor({ value, onChange, readOnly, fill, showLineNumbers }) {
  const textareaRef = useRef(null);

  const handleKeyDown = (e) => {
    if (e.key === 'Tab') {
      e.preventDefault();
      const ta = textareaRef.current;
      const start = ta.selectionStart;
      const end = ta.selectionEnd;
      const newValue = value.substring(0, start) + '  ' + value.substring(end);
      onChange(newValue);
      setTimeout(() => {
        ta.selectionStart = ta.selectionEnd = start + 2;
      }, 0);
    }
  };

  const lines = showLineNumbers ? value.split('\n') : [];
  const lineCount = lines.length;

  return (
    <div style={{
      display: 'flex',
      width: '100%',
      minHeight: fill ? undefined : '300px',
      height: fill ? '100%' : undefined,
      fontFamily: "'JetBrains Mono', monospace",
      fontSize: '0.85rem',
      lineHeight: '1.5',
      background: 'var(--card-bg)',
      border: '1px solid var(--card-border)',
      borderRadius: '6px',
      overflow: 'hidden',
    }}>
      {showLineNumbers && (
        <div style={{
          padding: '12px 8px 12px 12px',
          textAlign: 'right',
          color: 'var(--text-secondary)',
          userSelect: 'none',
          background: 'var(--bg-tertiary)',
          borderRight: '1px solid var(--card-border)',
          minWidth: `${String(lineCount || 1).length + 1}ch`,
          overflow: 'hidden',
          whiteSpace: 'pre',
          lineHeight: '1.5',
        }}>
          {lines.map((_, i) => (
            <div key={i}>{i + 1}</div>
          ))}
        </div>
      )}
      <textarea
        ref={textareaRef}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={handleKeyDown}
        readOnly={readOnly}
        spellCheck={false}
        style={{
          flex: 1,
          border: 'none',
          background: 'transparent',
          color: 'var(--pico-color)',
          fontFamily: 'inherit',
          fontSize: 'inherit',
          lineHeight: 'inherit',
          padding: '12px',
          resize: fill ? 'none' : 'vertical',
          tabSize: 2,
          outline: 'none',
        }}
        placeholder="# docker-compose.yml&#10;services:&#10;  web:&#10;    image: nginx:latest&#10;    ports:&#10;      - '80:80'&#10;"
      />
    </div>
  );
}
