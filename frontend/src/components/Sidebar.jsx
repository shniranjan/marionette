import ThemeSwitcher from './ThemeSwitcher';
import EndpointSwitcher from './EndpointSwitcher';

const NAV_ITEMS = [
  { id: 'dashboard', label: 'Dashboard', icon: '📊' },
  { id: 'containers', label: 'Containers', icon: '📦' },
  { id: 'images', label: 'Images', icon: '🖼' },
  { id: 'volumes', label: 'Volumes', icon: '💾' },
  { id: 'networks', label: 'Networks', icon: '🌐' },
  { id: 'endpoints', label: 'Endpoints', icon: '🔌' },
  { id: 'swarm', label: 'Swarm', icon: '🐝' },
  { id: 'nginx', label: 'Nginx LB', icon: '⚖️' },
  { id: 'stacks', label: 'Stacks', icon: '📚' },
  { id: 'routes', label: 'Routes', icon: '🔀' },
  { id: 'templates', label: 'Templates', icon: '📋' },
  { id: 'migration', label: 'Migration', icon: '🚚' },
  { id: 'migrationCompose', label: 'Compose Migrate', icon: '📋' },
  { id: 'system', label: 'System', icon: '⚙' },
];

const RECENT_ICONS = {
  containers: '📦',
  containerDetail: '📦',
  images: '🖼',
  volumes: '💾',
  networks: '🌐',
  stacks: '📚',
  swarm: '🐝',
  nginx: '⚖️',
  routes: '🔀',
  templates: '📋',
  migration: '🚚',
  migrationCompose: '📋',
  system: '⚙',
};

function timeAgo(ts) {
  const sec = Math.floor((Date.now() - ts) / 1000);
  if (sec < 60) return 'now';
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}m`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr}h`;
  const d = Math.floor(hr / 24);
  return `${d}d`;
}

export default function Sidebar({ currentPage, onNavigate, currentEndpoint, onEndpointChange, recents = [] }) {
  const hasRecents = recents.length > 0;

  return (
    <aside style={{
      width: '220px',
      minWidth: '220px',
      height: '100vh',
      background: 'var(--bg-secondary)',
      borderRight: '1px solid var(--border)',
      display: 'flex',
      flexDirection: 'column',
      overflow: 'visible',
    }}>
      {/* Brand */}
      <div style={{
        padding: '20px 16px',
        borderBottom: '1px solid var(--border)',
        textAlign: 'center',
      }}>
        <div style={{ fontSize: '1.8rem' }}>🪆</div>
        <div style={{ fontWeight: 700, fontSize: '1.1rem', color: 'var(--text-primary)' }}>
          Marionette
        </div>
        <div className="text-secondary" style={{ fontSize: '0.7rem', textTransform: 'uppercase', letterSpacing: '0.1em' }}>
          Docker Manager
        </div>
      </div>

      {/* Endpoint Switcher */}
      <div style={{ padding: '10px 12px', borderBottom: '1px solid var(--border)' }}>
        <EndpointSwitcher currentEndpoint={currentEndpoint} onEndpointChange={onEndpointChange} />
      </div>

      {/* Navigation */}
      <nav style={{ flex: 1, padding: '8px', overflowY: 'auto' }}>
        {NAV_ITEMS.map((item) => {
          const active = currentPage === item.id ||
            (item.id === 'containers' && currentPage === 'containerDetail');
          return (
            <button
              key={item.id}
              onClick={() => onNavigate(item.id)}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: '10px',
                width: '100%',
                padding: '10px 12px',
                border: 'none',
                borderRadius: '8px',
                background: active ? 'var(--bg-tertiary)' : 'transparent',
                color: active ? 'var(--accent)' : 'var(--text-secondary)',
                fontWeight: active ? 600 : 400,
                cursor: 'pointer',
                textAlign: 'left',
                fontSize: '0.85rem',
                marginBottom: '2px',
                transition: 'all 0.1s',
              }}
              onMouseEnter={(e) => {
                if (!active) e.currentTarget.style.background = 'var(--bg-tertiary)';
              }}
              onMouseLeave={(e) => {
                if (!active) e.currentTarget.style.background = 'transparent';
              }}
            >
              <span style={{ fontSize: '1rem', width: '20px', textAlign: 'center' }}>{item.icon}</span>
              {item.label}
            </button>
          );
        })}
      </nav>

      {/* Recently Viewed */}
      {hasRecents && (
        <div style={{
          borderTop: '1px solid var(--border)',
          padding: '8px',
          maxHeight: '180px',
          overflowY: 'auto',
        }}>
          <div style={{
            fontSize: '0.65rem',
            textTransform: 'uppercase',
            letterSpacing: '0.08em',
            color: 'var(--text-secondary)',
            padding: '4px 8px 6px',
            fontWeight: 600,
          }}>
            Recently Viewed
          </div>
          {recents.slice(0, 5).map((r, i) => {
            const pageForNav = r.type === 'containerDetail' ? 'containerDetail' : r.type;
            const active = currentPage === r.type &&
              (r.type !== 'containerDetail' || !r.id ||
                (currentPage === 'containerDetail'));
            return (
              <button
                key={`${r.type}-${r.id || ''}-${i}`}
                onClick={() => onNavigate(pageForNav, r.id ? { id: r.id, name: r.name } : {})}
                title={`${r.name} · ${timeAgo(r.timestamp)}`}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: '6px',
                  width: '100%',
                  padding: '5px 8px',
                  border: 'none',
                  borderRadius: '6px',
                  background: active ? 'var(--bg-tertiary)' : 'transparent',
                  color: active ? 'var(--accent)' : 'var(--text-secondary)',
                  cursor: 'pointer',
                  textAlign: 'left',
                  fontSize: '0.75rem',
                  marginBottom: '1px',
                  transition: 'all 0.1s',
                  whiteSpace: 'nowrap',
                  overflow: 'hidden',
                  textOverflow: 'ellipsis',
                }}
                onMouseEnter={(e) => {
                  if (!active) e.currentTarget.style.background = 'var(--bg-tertiary)';
                }}
                onMouseLeave={(e) => {
                  if (!active) e.currentTarget.style.background = 'transparent';
                }}
              >
                <span style={{ fontSize: '0.75rem', flexShrink: 0 }}>
                  {RECENT_ICONS[r.type] || '📄'}
                </span>
                <span style={{ overflow: 'hidden', textOverflow: 'ellipsis' }}>
                  {r.name.length > 16 ? r.name.substring(0, 14) + '…' : r.name}
                </span>
                <span style={{
                  marginLeft: 'auto',
                  fontSize: '0.6rem',
                  color: 'var(--text-secondary)',
                  flexShrink: 0,
                }}>
                  {timeAgo(r.timestamp)}
                </span>
              </button>
            );
          })}
        </div>
      )}

      {/* Theme switcher at bottom */}
      <div style={{ padding: '12px', borderTop: '1px solid var(--border)' }}>
        <ThemeSwitcher />
      </div>
    </aside>
  );
}
