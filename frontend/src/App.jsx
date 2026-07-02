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
  migrationCompose: Migration,
  swarm: Swarm,
  nginx: Nginx,
  routes: Routes,
  templates: Templates,
};

export default function App() {
  const [authenticated, setAuthenticated] = useState(false);
  const [page, setPage] = useState('dashboard');
  const [pageProps, setPageProps] = useState({});
  const [currentEndpoint, setCurrentEndpoint] = useState(() => getCurrentEndpoint());

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
