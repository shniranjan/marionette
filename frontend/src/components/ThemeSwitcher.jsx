import { useTheme } from '../context/ThemeContext';

const COLOR_PALETTES = [
  { key: 'blue', label: 'Blue', css: '' },
  { key: 'slate', label: 'Slate', css: 'slate' },
  { key: 'amber', label: 'Amber', css: 'amber' },
  { key: 'green', label: 'Green', css: 'green' },
  { key: 'violet', label: 'Violet', css: 'violet' },
  { key: 'rose', label: 'Rose', css: 'red' },
];

const MODES = [
  { key: 'dark', icon: '🌙', label: 'Dark' },
  { key: 'light', icon: '☀️', label: 'Light' },
  { key: 'sepia', icon: '📖', label: 'Sepia' },
];

export default function ThemeSwitcher() {
  const { theme, setTheme } = useTheme();

  // Parse current theme: "dark-blue" → mode=dark, color=blue
  const [currentMode, currentColor] = theme.includes('-')
    ? theme.split('-')
    : [theme, 'blue'];

  const cycleMode = () => {
    const idx = MODES.findIndex((m) => m.key === currentMode);
    const next = MODES[(idx + 1) % MODES.length];
    setTheme(`${next.key}-${currentColor}`);
  };

  const setColor = (colorKey) => {
    setTheme(`${currentMode}-${colorKey}`);
  };

  const modeInfo = MODES.find((m) => m.key === currentMode) || MODES[0];
  const colorInfo = COLOR_PALETTES.find((c) => c.key === currentColor) || COLOR_PALETTES[0];

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
      {/* Mode toggle */}
      <button
        onClick={cycleMode}
        title={`Mode: ${modeInfo.label}`}
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '6px',
          padding: '5px 10px',
          background: 'var(--card-bg)',
          border: '1px solid var(--card-border)',
          borderRadius: '6px',
          cursor: 'pointer',
          fontSize: '0.8rem',
          color: 'var(--pico-color)',
        }}
      >
        <span>{modeInfo.icon}</span>
        <span>{modeInfo.label}</span>
      </button>

      {/* Color palette dots */}
      <div style={{ display: 'flex', gap: '4px', justifyContent: 'center' }}>
        {COLOR_PALETTES.map((c) => (
          <button
            key={c.key}
            onClick={() => setColor(c.key)}
            title={c.label}
            style={{
              width: '16px',
              height: '16px',
              borderRadius: '50%',
              border: currentColor === c.key ? '2px solid var(--pico-color)' : '2px solid transparent',
              cursor: 'pointer',
              padding: 0,
              background: c.key === 'blue' ? '#3b82f6'
                : c.key === 'slate' ? '#64748b'
                : c.key === 'amber' ? '#f59e0b'
                : c.key === 'green' ? '#22c55e'
                : c.key === 'violet' ? '#8b5cf6'
                : '#f43f5e',
            }}
          />
        ))}
      </div>
    </div>
  );
}
