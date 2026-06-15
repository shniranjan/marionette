import { useState, useEffect, useCallback, createContext, useContext } from 'react';

const ToastContext = createContext();

let toastId = 0;

export function useToast() {
  return useContext(ToastContext);
}

export function ToastProvider({ children }) {
  const [toasts, setToasts] = useState([]);

  const addToast = useCallback((message, type = 'info', duration = 4000) => {
    const id = ++toastId;
    setToasts((prev) => [...prev, { id, message, type, exiting: false }]);
    if (duration > 0) {
      setTimeout(() => {
        setToasts((prev) => prev.map((t) => (t.id === id ? { ...t, exiting: true } : t)));
        setTimeout(() => {
          setToasts((prev) => prev.filter((t) => t.id !== id));
        }, 300);
      }, duration);
    }
  }, []);

  const removeToast = useCallback((id) => {
    setToasts((prev) => prev.map((t) => (t.id === id ? { ...t, exiting: true } : t)));
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 300);
  }, []);

  const toast = useCallback((message, type) => addToast(message, type), [addToast]);

  return (
    <ToastContext.Provider value={toast}>
      {children}
      <div className="toast-container">
        {toasts.map((t) => (
          <ToastItem key={t.id} toast={t} onDismiss={() => removeToast(t.id)} />
        ))}
      </div>
    </ToastContext.Provider>
  );
}

function ToastItem({ toast, onDismiss }) {
  const { message, type, exiting } = toast;
  const colors = {
    success: 'var(--green)',
    error: 'var(--red)',
    info: 'var(--accent)',
  };

  return (
    <div
      className={`toast toast-${type}${exiting ? ' toast-exit' : ''}`}
      style={{
        background: 'var(--bg-secondary)',
        border: `1px solid ${colors[type] || 'var(--border)'}`,
        borderLeft: `4px solid ${colors[type] || 'var(--border)'}`,
        color: 'var(--text-primary)',
        padding: '12px 16px',
        borderRadius: '8px',
        boxShadow: 'var(--card-shadow)',
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
        gap: '12px',
        minWidth: '280px',
        maxWidth: '420px',
        animation: exiting ? 'toastOut 0.3s ease forwards' : 'toastIn 0.3s ease',
        pointerEvents: 'auto',
      }}
    >
      <span style={{ flex: 1, fontSize: '0.85rem' }}>{message}</span>
      <button
        onClick={onDismiss}
        style={{ border: 'none', background: 'none', color: 'var(--text-secondary)', cursor: 'pointer', fontSize: '1rem', padding: '0 2px' }}
      >
        ✕
      </button>
    </div>
  );
}

/* Toast styles injected inline via style tag below */

export function ToastStyles() {
  return (
    <style>{`
      .toast-container {
        position: fixed;
        bottom: 20px;
        right: 20px;
        z-index: 2000;
        display: flex;
        flex-direction: column;
        gap: 8px;
        pointer-events: none;
      }
      @keyframes toastIn {
        from { transform: translateX(100%); opacity: 0; }
        to { transform: translateX(0); opacity: 1; }
      }
      @keyframes toastOut {
        from { transform: translateX(0); opacity: 1; }
        to { transform: translateX(100%); opacity: 0; }
      }
    `}</style>
  );
}
