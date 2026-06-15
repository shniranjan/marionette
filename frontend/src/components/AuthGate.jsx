import { useState } from 'react';
import { setKey } from '../api/client';

export default function AuthGate({ onSuccess }) {
  const [key, setKeyInput] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e) => {
    e.preventDefault();
    if (!key.trim()) return;
    setLoading(true);
    setError('');

    try {
      // Validate by hitting the health endpoint
      const res = await fetch('/api/health', {
        headers: { 'X-Marionette-Key': key.trim() },
      });
      if (!res.ok) {
        throw new Error('Invalid authentication key');
      }
      setKey(key.trim());
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
            placeholder="Enter your Marionette key..."
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
          <button
            type="submit"
            className="btn-primary"
            disabled={loading || !key.trim()}
            style={{ width: '100%', padding: '10px' }}
          >
            {loading ? 'Authenticating...' : 'Connect'}
          </button>
        </form>
      </div>
    </div>
  );
}
