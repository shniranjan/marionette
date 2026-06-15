import { useState } from 'react';
import { setKey } from '../api/client';

export default function AuthGate({ onSuccess }) {
  const [key, setKeyInput] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e) => {
    e.preventDefault();
    setLoading(true);
    setError('');

    try {
      const headers = {};
      if (key.trim()) {
        headers['X-Marionette-Key'] = key.trim();
      }
      const res = await fetch('/api/health', { headers });
      if (!res.ok) {
        throw new Error('Invalid authentication key');
      }
      if (key.trim()) {
        setKey(key.trim());
      }
      onSuccess();
    } catch (err) {
      setError(err.message || 'Failed to authenticate');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={{
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      height: '100vh',
      background: 'var(--bg-primary)',
    }}>
      <div className="card" style={{ width: '380px', padding: '32px' }}>
        <div style={{ textAlign: 'center', marginBottom: '24px' }}>
          <div style={{ fontSize: '2.5rem', marginBottom: '8px' }}>🪆</div>
          <h1 style={{ margin: '0 0 4px' }}>Marionette</h1>
          <p className="text-secondary">Docker Management Platform</p>
        </div>

        <form onSubmit={handleSubmit}>
          <label style={{ display: 'block', marginBottom: '6px', fontWeight: 500 }}>
            Authentication Key
          </label>
          <input
            type="password"
            value={key}
            onChange={(e) => setKeyInput(e.target.value)}
            placeholder="Leave empty for dev mode..."
            style={{ width: '100%', marginBottom: '12px' }}
            autoFocus
          />
          {error && (
            <div style={{
              color: 'var(--red)',
              fontSize: '0.8rem',
              marginBottom: '12px',
              padding: '8px',
              background: 'var(--red-dim)',
              borderRadius: '4px',
            }}>
              {error}
            </div>
          )}
          <p style={{ fontSize: '0.75rem', color: 'var(--text-secondary)', marginBottom: '12px' }}>
            {key.trim() ? 'Connect with your access key' : 'Dev mode — no key required'}
          </p>
          <button
            type="submit"
            className="btn-primary"
            disabled={loading}
            style={{ width: '100%', padding: '10px' }}
          >
            {loading ? 'Connecting...' : 'Connect'}
          </button>
        </form>
      </div>
    </div>
  );
}
