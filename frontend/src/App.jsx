import { useState, useEffect, useCallback } from 'react';
import { getKey } from './api/client';
import { ToastProvider, ToastStyles } from './components/Toast';
import AuthGate from './components/AuthGate';
import Sidebar from './components/Sidebar';
import EndpointSwitcher, { getCurrentEndpoint, setEndpoint } from './components/EndpointSwitcher';
import Breadcrumb from './components/Breadcrumb';
import Dashboard from './pages/Dashboard';
import Containers from './pages/Containers';
import ContainerDetail from './pages/ContainerDetail';
import Images from './pages/Images';
import Volumes from './pages/Volumes';
import Networks from './pages/Networks';
import Stacks from './pages/Stacks';
import System from './pages/System';
import Endpoints from './pages/Endpoints';
import Migration from './pages/Migration';
import MigrationCompose from './pages/MigrationCompose';
import Swarm from './pages/Swarm';
import Nginx from './pages/Nginx';
import Routes from './pages/Routes';
import Templates from './pages/Templates';
import ErrorBoundary from './components/ErrorBoundary';
import MaintenanceOverlay from './components/MaintenanceOverlay';

const PAGES = {
  dashboard: Dashboard,
  containers: Containers,
  containerDetail: ContainerDetail,
  images: Images,
  volumes: Volumes,
  networks: Networks,
  stacks: Stacks,
  system: System,
  endpoints: Endpoints,
  migration: Migration,
  migrationCompose: MigrationCompose,
  swarm: Swarm,
  nginx: Nginx,
  routes: Routes,
  templates: Templates,
};

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
  migrationCompose: 'Compose Migrate',
  swarm: 'Swarm',
  nginx: 'Nginx LB',
  routes: 'Routes',
  templates: 'Templates',
};

export default function App() {
  const [authenticated, setAuthenticated] = useState(false);
  const [page, setPage] = useState('dashboard');
  const [pageProps, setPageProps] = useState({});
  const [currentEndpoint, setCurrentEndpoint] = useState(() => getCurrentEndpoint());
  const [recents, setRecents] = useState(() => {
    try {
      const raw = localStorage.getItem('marionette_recents');
      return raw ? JSON.parse(raw) : [];
    } catch { return []; }
  });

  const checkAuth = useCallback(() => {
    setAuthenticated(!!getKey());
  }, []);

  useEffect(() => {
    checkAuth();
  }, [checkAuth]);

  useEffect(() => {
    const handler = () => {
      setAuthenticated(false);
      setPage('dashboard');
    };
    window.addEventListener('auth:expired', handler);
    return () => window.removeEventListener('auth:expired', handler);
  }, []);

  useEffect(() => {
    const handler = (e) => {
      setCurrentEndpoint(e.detail);
    };
    window.addEventListener('endpoint:changed', handler);
    return () => window.removeEventListener('endpoint:changed', handler);
  }, []);

  const navigate = useCallback((p, props = {}) => {
    setPage(p);
    setPageProps(props);

    // Track recent navigation (skip dashboard)
    if (p !== 'dashboard') {
      setRecents((prev) => {
        const id = props.id || null;
        const name = props.name || PAGE_LABELS[p] || p;
        const entry = { id, name, type: p, timestamp: Date.now() };

        // Remove existing entry with same type+id
        const filtered = prev.filter(
          (r) => !(r.type === p && r.id === id),
        );

        // Cap at 10 and prepend
        const next = [entry, ...filtered].slice(0, 10);
        try { localStorage.setItem('marionette_recents', JSON.stringify(next)); } catch {}
        return next;
      });
    }
  }, []);

  const handleEndpointChange = useCallback((id) => {
    setEndpoint(id);
  }, []);

  if (!authenticated) {
    return <AuthGate onSuccess={checkAuth} />;
  }

  const PageComponent = PAGES[page] || Dashboard;

  return (
    <ToastProvider>
    <ToastStyles />
    <MaintenanceOverlay />
    <div className="app-layout">
        <Sidebar
          currentPage={page}
          onNavigate={navigate}
          currentEndpoint={currentEndpoint}
          onEndpointChange={handleEndpointChange}
          recents={recents}
        />
        <main className="main-content">
          <Breadcrumb page={page} pageProps={pageProps} onNavigate={navigate} />
          <ErrorBoundary key={page}>
            <PageComponent navigate={navigate} currentEndpoint={currentEndpoint} {...pageProps} />
          </ErrorBoundary>
        </main>
      </div>
    </ToastProvider>
  );
}
