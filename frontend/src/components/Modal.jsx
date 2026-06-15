export default function Modal({ title, children, footer, onClose }) {
  const handleOverlay = (e) => {
    if (e.target === e.currentTarget) onClose();
  };

  const handleKey = (e) => {
    if (e.key === 'Escape') onClose();
  };

  return (
    <div className="modal-overlay" onClick={handleOverlay} onKeyDown={handleKey}>
      <div className="modal-content" role="dialog" aria-modal="true">
        <div className="modal-header">
          <h2>{title}</h2>
          <button onClick={onClose} style={{ border: 'none', background: 'none', fontSize: '1.2rem', padding: '0 4px' }}>
            ✕
          </button>
        </div>
        <div className="modal-body">{children}</div>
        {footer && <div className="modal-footer">{footer}</div>}
      </div>
    </div>
  );
}
