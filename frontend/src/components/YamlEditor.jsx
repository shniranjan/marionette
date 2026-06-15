import { useEffect, useRef } from 'react';

// Lightweight CodeMirror 6 wrapper. We use a simple textarea with syntax classes
// since bundling full CodeMirror requires additional setup. The YAML editor uses
// a styled textarea with monospace font — CodeMirror can be added as an enhancement.
// See package.json: codemirror is listed as a dependency for future integration.

export default function YamlEditor({ value, onChange, readOnly }) {
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
        minHeight: '300px',
        fontFamily: "'JetBrains Mono', monospace",
        fontSize: '0.8rem',
        lineHeight: '1.5',
        padding: '12px',
        background: 'var(--bg-tertiary)',
        color: 'var(--text-primary)',
        border: '1px solid var(--border)',
        borderRadius: '6px',
        resize: 'vertical',
        tabSize: 2,
      }}
      placeholder="# docker-compose.yml&#10;services:&#10;  web:&#10;    image: nginx:latest&#10;    ports:&#10;      - '80:80'&#10;"
    />
  );
}
