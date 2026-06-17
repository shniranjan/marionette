export default function Modal({ title, children, footer, onClose, size }) {
  const isLarge = size === 'large';

  const handleOverlay = (e) => {
    if (e.target === e.currentTarget) onClose();
  };

  const handleKey = (e) => {
    if (e.key === 'Escape') onClose();
  };

  return (
    <div className="modal-overlay" onClick={handleOverlay} onKeyDown={handleKey}>
      <div
        className="modal-content"
        role="dialog"
        aria-modal="true"
        style={isLarge ? {
          width: '95vw',
          height: '85vh',
          minWidth: 'auto',
          maxWidth: '95vw',
          display: 'flex',
          flexDirection: 'column',
        } : undefined}
      >
        <div className="modal-header">
          <h2>{title}</h2>
          <button onClick={onClose} style={{ border: 'none', background: 'none', fontSize: '1.2rem', padding: '0 4px' }}>
            ✕
          </button>
        </div>
        <div className="modal-body" style={isLarge ? { flex: 1, overflow: 'auto', minHeight: 0, display: 'flex', flexDirection: 'column' } : undefined}>
          {children}
        </div>
        {footer && <div className="modal-footer">{footer}</div>}
      </div>
    </div>
  );
}
