import { useTheme } from '../context/ThemeContext';

const themes = [
  { key: 'dark', icon: '🌙', label: 'Dark' },
  { key: 'light', icon: '☀️', label: 'Light' },
  { key: 'sepia', icon: '📖', label: 'Sepia' },
];

export default function ThemeSwitcher() {
  const { theme, setTheme } = useTheme();

  const cycle = () => {
    const idx = themes.findIndex((t) => t.key === theme);
    const next = themes[(idx + 1) % themes.length];
    setTheme(next.key);
  };

  const current = themes.find((t) => t.key === theme) || themes[0];

  return (
    <button
      onClick={cycle}
      title={`Theme: ${current.label}`}
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: '6px',
        padding: '6px 12px',
        background: 'var(--bg-tertiary)',
        border: '1px solid var(--border)',
        borderRadius: '8px',
        cursor: 'pointer',
        fontSize: '0.85rem',
        color: 'var(--text-primary)',
      }}
    >
      <span style={{ fontSize: '1.1rem' }}>{current.icon}</span>
      <span>{current.label}</span>
    </button>
  );
}
