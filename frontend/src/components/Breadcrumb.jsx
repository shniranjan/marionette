import { useState, useEffect, useRef } from 'react';

const PAGE_LABELS = {
  dashboard: 'Dashboard',
  containers: 'Containers',
  containerDetail: 'Container',
  images: 'Images',
  volumes: 'Volumes',
  networks: 'Networks',
  stacks: 'Stacks',
  system: 'System',
  endpoints: 'Endpoints',
  migration: 'Migration',
  swarm: 'Swarm',
  nginx: 'Nginx LB',
  routes: 'Routes',
  templates: 'Templates',
};

function pageLabel(page, props = {}) {
  const base = PAGE_LABELS[page] || page;
  if (page === 'containerDetail' && props.name) {
    return props.name.replace(/^\//, '');
  }
  return base;
}

/**
 * Breadcrumb component that tracks navigation history.
 * Renders clickable breadcrumb segments: Dashboard > Containers > nginx-proxy
 */
export default function Breadcrumb({ page, pageProps, onNavigate }) {
  const [stack, setStack] = useState([]);
  const prevPage = useRef(page);

  useEffect(() => {
    // Only push to stack when page actually changes
    if (page === prevPage.current) return;
    prevPage.current = page;

    setStack((prev) => {
      // Find if this page is already in the stack (back-navigation)
      const existingIdx = prev.findIndex(
        (s) => s.page === page && s.page === 'containerDetail'
          ? s.props?.id === pageProps?.id
          : s.page === page
      );

      if (existingIdx >= 0) {
        // Clicked back to a page already in history — truncate to that point
        return prev.slice(0, existingIdx + 1);
      }

      // Set root reset: if navigating to a top-level page, clear stack
      const isTopLevel = page !== 'containerDetail';
      // Special case: navigating from containerDetail to containers keeps context
      const isBackToContainers =
        page === 'containers' &&
        prev.length > 0 &&
        prev[prev.length - 1].page === 'containerDetail';

      if (isTopLevel && !isBackToContainers) {
        // Reset stack — start fresh from this page
        return [{ page, props: pageProps, label: pageLabel(page, pageProps) }];
      }

      // Push new entry
      return [
        ...prev,
        { page, props: pageProps, label: pageLabel(page, pageProps) },
      ];
    });
  }, [page, pageProps]);

  // Initialize stack on first render
  useEffect(() => {
    if (stack.length === 0) {
      setStack([
        { page, props: pageProps, label: pageLabel(page, pageProps) },
      ]);
    }
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  if (stack.length <= 1) return null;

  const handleClick = (index) => {
    const entry = stack[index];
    if (entry.page === page) return; // already there
    // Pop to this point
    setStack((prev) => prev.slice(0, index + 1));
    onNavigate(entry.page, entry.props);
  };

  return (
    <nav aria-label="breadcrumb" style={{ marginBottom: '16px' }}>
      <ul
        style={{
          display: 'flex',
          listStyle: 'none',
          padding: 0,
          margin: 0,
          gap: '6px',
          alignItems: 'center',
          flexWrap: 'wrap',
          fontSize: '0.85rem',
        }}
      >
        {stack.map((entry, i) => {
          const isLast = i === stack.length - 1;
          return (
            <li key={`${entry.page}-${i}`} style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
              {i > 0 && (
                <span style={{ color: 'var(--text-secondary)', fontSize: '0.75rem' }}>›</span>
              )}
              {isLast ? (
                <span style={{ color: 'var(--text-primary)', fontWeight: 600 }}>
                  {entry.label}
                </span>
              ) : (
                <button
                  onClick={() => handleClick(i)}
                  style={{
                    border: 'none',
                    background: 'none',
                    color: 'var(--accent)',
                    cursor: 'pointer',
                    padding: 0,
                    fontSize: 'inherit',
                    textDecoration: 'underline',
                    textUnderlineOffset: '2px',
                  }}
                >
                  {entry.label}
                </button>
              )}
            </li>
          );
        })}
      </ul>
    </nav>
  );
}
