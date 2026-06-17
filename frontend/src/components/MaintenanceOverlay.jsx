import { useState, useEffect, useRef } from 'react';
import { api } from '../api/client';

export default function MaintenanceOverlay() {
  const [down, setDown] = useState(false);
  const [seconds, setSeconds] = useState(0);
  const failures = useRef(0);
  const interval = useRef(null);

  useEffect(() => {
    const check = async () => {
      try {
        await api.get('/health');
        failures.current = 0;
        if (down) setDown(false);
      } catch {
        failures.current++;
        if (failures.current >= 2 && !down) {
          setDown(true);
          setSeconds(0);
        }
      }
    };

    check();
    interval.current = setInterval(check, 5000);

    return () => clearInterval(interval.current);
  }, [down]);

  useEffect(() => {
    if (!down) return;
    const timer = setInterval(() => setSeconds(s => s + 1), 1000);
    return () => clearInterval(timer);
  }, [down]);

  if (!down) return null;

  const minutes = Math.floor(seconds / 60);
  const secs = seconds % 60;

  return (
    <div style={{
      position: 'fixed',
      inset: 0,
      background: 'var(--pico-background-color, #0d1117)',
      zIndex: 9999,
      display: 'flex',
      flexDirection: 'column',
      alignItems: 'center',
      justifyContent: 'center',
      color: 'var(--pico-color, #e6edf3)',
      fontFamily: 'inherit',
      padding: '24px',
    }}>
      <div style={{ fontSize: '4rem', marginBottom: '16px' }}>⚙️</div>
      <h1 style={{ fontSize: '1.5rem', fontWeight: 600, marginBottom: '8px' }}>
        Marionette is unreachable
      </h1>
      <p style={{ color: 'var(--pico-muted-color, #8b949e)', fontSize: '0.9rem', textAlign: 'center', maxWidth: '400px', lineHeight: 1.6 }}>
        The management server is currently down. Your containers are still running — this only affects the dashboard.
      </p>

      <div style={{
        marginTop: '24px',
        padding: '16px 24px',
        background: 'var(--card-bg, #161b22)',
        border: '1px solid var(--card-border, #30363d)',
        borderRadius: '8px',
        fontSize: '0.85rem',
        color: 'var(--pico-muted-color)',
        textAlign: 'center',
      }}>
        <div style={{ marginBottom: '4px' }}>Checking every 5 seconds</div>
        <div style={{ fontSize: '1.5rem', fontFamily: 'monospace', color: 'var(--pico-color)' }}>
          {String(minutes).padStart(2, '0')}:{String(secs).padStart(2, '0')}
        </div>
        <div style={{ marginTop: '8px', fontSize: '0.8rem' }}>
          Auto-reconnects when detected
        </div>
      </div>

      <div style={{ marginTop: '32px', display: 'flex', gap: '12px' }}>
        <button onClick={() => window.location.reload()} style={{
          padding: '8px 20px',
          background: 'var(--accent)',
          border: 'none',
          borderRadius: '6px',
          color: '#fff',
          fontSize: '0.85rem',
          cursor: 'pointer',
        }}>
          Retry Now
        </button>
      </div>
    </div>
  );
}
