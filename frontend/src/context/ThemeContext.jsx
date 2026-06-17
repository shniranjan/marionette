import { createContext, useContext, useState, useEffect, useCallback } from 'react';

const ThemeContext = createContext();

const STORAGE_KEY = 'marionette-theme';
const DEFAULT_THEME = 'dark-blue';

// Pico color variant CSS: maps color key → import path
const COLOR_CSS = {
  blue: '',           // default, already in pico.classless.min.css
  slate: 'slate',
  amber: 'amber',
  green: 'green',
  violet: 'violet',
  rose: 'red',
};

function getStoredTheme() {
  try {
    return localStorage.getItem(STORAGE_KEY) || DEFAULT_THEME;
  } catch {
    return DEFAULT_THEME;
  }
}

let colorLinkEl = null;

function applyColorVariant(colorKey) {
  const variant = COLOR_CSS[colorKey];
  if (!variant) {
    // Default blue — remove any custom color CSS
    if (colorLinkEl) {
      colorLinkEl.remove();
      colorLinkEl = null;
    }
    return;
  }

  if (!colorLinkEl) {
    colorLinkEl = document.createElement('link');
    colorLinkEl.rel = 'stylesheet';
    colorLinkEl.id = 'pico-color-variant';
    document.head.appendChild(colorLinkEl);
  }
  colorLinkEl.href = `/assets/pico.classless.${variant}.min.css`;
}

export function ThemeProvider({ children }) {
  const [theme, setThemeState] = useState(getStoredTheme);

  const setTheme = useCallback((t) => {
    setThemeState(t);
    try {
      localStorage.setItem(STORAGE_KEY, t);
    } catch { /* ignore */ }
  }, []);

  useEffect(() => {
    // Parse theme: "dark-blue" → mode=dark, color=blue
    const [mode, color] = theme.includes('-')
      ? theme.split('-')
      : [theme, 'blue'];

    // Set Pico dark/light mode
    if (mode === 'dark' || mode === 'sepia') {
      document.documentElement.setAttribute('data-theme', 'dark');
    } else {
      document.documentElement.setAttribute('data-theme', 'light');
    }

    // Apply color variant
    applyColorVariant(color);

    // Custom CSS variables for accent colors (used by badges, buttons, charts)
    const root = document.documentElement.style;
    const palettes = {
      blue:   { accent: '#3b82f6', accentDim: '#2563eb', green: '#22c55e', greenDim: '#16a34a', yellow: '#eab308', yellowDim: '#ca8a04', red: '#ef4444', redDim: '#dc2626' },
      slate:  { accent: '#64748b', accentDim: '#475569', green: '#22c55e', greenDim: '#16a34a', yellow: '#eab308', yellowDim: '#ca8a04', red: '#ef4444', redDim: '#dc2626' },
      amber:  { accent: '#f59e0b', accentDim: '#d97706', green: '#22c55e', greenDim: '#16a34a', yellow: '#eab308', yellowDim: '#ca8a04', red: '#ef4444', redDim: '#dc2626' },
      green:  { accent: '#22c55e', accentDim: '#16a34a', green: '#22c55e', greenDim: '#16a34a', yellow: '#eab308', yellowDim: '#ca8a04', red: '#ef4444', redDim: '#dc2626' },
      violet: { accent: '#8b5cf6', accentDim: '#7c3aed', green: '#22c55e', greenDim: '#16a34a', yellow: '#eab308', yellowDim: '#ca8a04', red: '#ef4444', redDim: '#dc2626' },
      rose:   { accent: '#f43f5e', accentDim: '#e11d48', green: '#22c55e', greenDim: '#16a34a', yellow: '#eab308', yellowDim: '#ca8a04', red: '#ef4444', redDim: '#dc2626' },
    };
    const p = palettes[color] || palettes.blue;
    root.setProperty('--accent', p.accent);
    root.setProperty('--accent-dim', p.accentDim);
    root.setProperty('--green', p.green);
    root.setProperty('--green-dim', p.greenDim);
    root.setProperty('--yellow', p.yellow);
    root.setProperty('--yellow-dim', p.yellowDim);
    root.setProperty('--red', p.red);
    root.setProperty('--red-dim', p.redDim);
  }, [theme]);

  return (
    <ThemeContext.Provider value={{ theme, setTheme }}>
      {children}
    </ThemeContext.Provider>
  );
}

export function useTheme() {
  const ctx = useContext(ThemeContext);
  if (!ctx) throw new Error('useTheme must be used within ThemeProvider');
  return ctx;
}

export { ThemeContext };
