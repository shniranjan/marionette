import { useEffect, useRef } from 'react';

export default function YamlEditor({ value, onChange, readOnly, fill }) {
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

  return (
    <textarea
      ref={textareaRef}
      value={value}
      onChange={(e) => onChange(e.target.value)}
      onKeyDown={handleKeyDown}
      readOnly={readOnly}
      spellCheck={false}
      style={{
        width: '100%',
        minHeight: fill ? undefined : '300px',
        height: fill ? '100%' : undefined,
        fontFamily: "'JetBrains Mono', monospace",
        fontSize: '0.85rem',
        lineHeight: '1.5',
        padding: '12px',
        background: 'var(--card-bg)',
        color: 'var(--pico-color)',
        border: '1px solid var(--card-border)',
        borderRadius: '6px',
        resize: fill ? 'none' : 'vertical',
        tabSize: 2,
      }}
      placeholder="# docker-compose.yml&#10;services:&#10;  web:&#10;    image: nginx:latest&#10;    ports:&#10;      - '80:80'&#10;"
    />
  );
}
