import { useState, useEffect } from 'react';
import { api } from '../api/client';
import StatCard from '../components/StatCard';
import Spinner from '../components/Spinner';

export default function Dashboard({ navigate }) {
  const [containers, setContainers] = useState(null);
  const [system, setSystem] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [reloadVersion, setReloadVersion] = useState(0);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const [containersData, systemData] = await Promise.all([
          api.get('/api/containers'),
          api.get('/api/system'),
        ]);
        if (!cancelled) {
          setContainers(containersData);
          setSystem(systemData);
        }
      } catch (err) {
        if (!cancelled) setError(err.message);
      } finally {
        if (!cancelled) setLoading(false);
      }
    }
    load();
    return () => { cancelled = true; };
  }, [reloadVersion]);

  // Reload when endpoint changes (EndpointSwitcher dispatches 'endpoint:changed')
  useEffect(() => {
    const handler = () => setReloadVersion(v => v + 1);
    window.addEventListener('endpoint:changed', handler);
    return () => window.removeEventListener('endpoint:changed', handler);
  }, []);

  if (loading) return <div className="loading-center"><Spinner size="lg" /></div>;
  if (error) return <div className="text-danger">Error: {error}</div>;

  const containerList = Array.isArray(containers) ? containers : (containers?.containers || []);
  const running = containerList.filter((c) => c.state === 'running').length;
  const paused = containerList.filter((c) => c.state === 'paused').length;
  const stopped = containerList.filter((c) => c.state === 'exited' || c.state === 'stopped').length;
  const images = system?.images ?? '—';
  const volumes = system?.volumes ?? '—';
  const networks = system?.networks ?? '—';
  const hostCpus = system?.cpuCount ?? '—';
  const hostMemory = system?.memoryBytes != null ? formatBytes(system.memoryBytes) : '—';

  return (
    <div>
      <h1>Dashboard</h1>

      <div className="stats-grid">
        <StatCard
          icon="📦"
          value={containerList.length}
          label="Total Containers"
          onClick={() => navigate('containers')}
        />
        <StatCard
          icon="▶"
          value={running}
          label="Running"
          color="var(--green)"
          onClick={() => navigate('containers')}
        />
        <StatCard
          icon="⏸"
          value={paused}
          label="Paused"
          color="var(--yellow)"
          onClick={() => navigate('containers')}
        />
        <StatCard
          icon="⏹"
          value={stopped}
          label="Stopped"
          color="var(--red)"
          onClick={() => navigate('containers')}
        />
        <StatCard
          icon="🖼"
          value={images}
          label="Images"
          onClick={() => navigate('images')}
        />
        <StatCard
          icon="💾"
          value={volumes}
          label="Volumes"
          onClick={() => navigate('volumes')}
        />
        <StatCard
          icon="🌐"
          value={networks}
          label="Networks"
          onClick={() => navigate('networks')}
        />
        <StatCard
          icon="🧠"
          value={hostMemory}
          label="Host Memory"
          color="var(--blue)"
        />
        <StatCard
          icon="⚙️"
          value={hostCpus}
          label="Host CPUs"
          color="var(--blue)"
        />
      </div>

      {/* Running containers quick list */}
      {running > 0 && (
        <div className="card mb-24">
          <h2>Running Containers ({running})</h2>
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px' }}>
            {containerList
              .filter(c => c.state === 'running')
              .sort((a, b) => (a.name || '').localeCompare(b.name || ''))
              .map(c => (
                <button
                  key={c.id || c.Id}
                  className="btn-sm outline"
                  style={{ fontFamily: 'var(--pico-font-family-monospace)', fontSize: '0.8rem' }}
                  onClick={() => navigate('containerDetail', { id: c.id || c.Id, name: c.name || c.Name })}
                  title={c.image || c.Image}
                >
                  {c.name || c.Name}
                </button>
              ))}
          </div>
        </div>
      )}

      {/* System Info */}
      {system && (
        <div className="card mb-24">
          <h2>System Information</h2>
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '8px', fontSize: '0.85rem' }}>
            {system.os && (
              <>
                <span className="text-secondary">OS</span>
                <span className="mono">{system.os} {system.architecture || ''}</span>
              </>
            )}
            {system.kernelVersion && (
              <>
                <span className="text-secondary">Kernel</span>
                <span className="mono">{system.kernelVersion}</span>
              </>
            )}
            {system.driver && (
              <>
                <span className="text-secondary">Storage Driver</span>
                <span className="mono">{system.driver}</span>
              </>
            )}
            {system.cpuCount != null && (
              <>
                <span className="text-secondary">CPUs</span>
                <span className="mono">{system.cpuCount}</span>
              </>
            )}
            {system.memoryBytes != null && (
              <>
                <span className="text-secondary">Total Memory</span>
                <span className="mono">{formatBytes(system.memoryBytes)}</span>
              </>
            )}
            {system.dockerVersion && (
              <>
                <span className="text-secondary">Docker Version</span>
                <span className="mono">{system.dockerVersion}</span>
              </>
            )}
          </div>
        </div>
      )}

      {/* Quick links */}
      <div className="card">
        <h2>Quick Actions</h2>
        <div className="btn-group">
          <button onClick={() => navigate('stacks')}>📚 Manage Stacks</button>
          <button onClick={() => navigate('images')}>🖼 Pull Image</button>
          <button onClick={() => navigate('system')}>⚙ System Settings</button>
        </div>
      </div>
    </div>
  );
}

function formatBytes(bytes) {
  if (!bytes || bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}
