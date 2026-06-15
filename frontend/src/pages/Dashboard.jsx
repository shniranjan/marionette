import { useState, useEffect } from 'react';
import { api } from '../api/client';
import StatCard from '../components/StatCard';
import Spinner from '../components/Spinner';

export default function Dashboard({ navigate }) {
  const [containers, setContainers] = useState(null);
  const [system, setSystem] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);

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
  }, []);

  if (loading) return <div className="loading-center"><Spinner size="lg" /></div>;
  if (error) return <div className="text-danger">Error: {error}</div>;

  const containerList = Array.isArray(containers) ? containers : (containers?.containers || []);
  const running = containerList.filter((c) => c.State === 'running').length;
  const paused = containerList.filter((c) => c.State === 'paused').length;
  const stopped = containerList.filter((c) => c.State === 'exited' || c.State === 'stopped').length;
  const images = system?.Images ?? system?.images ?? '—';
  const volumes = system?.Volumes ?? system?.volumes ?? '—';
  const networks = system?.Networks ?? system?.networks ?? '—';

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
      </div>

      {/* System Info */}
      {system && (
        <div className="card mb-24">
          <h2>System Information</h2>
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '8px', fontSize: '0.85rem' }}>
            {system.OSType && (
              <>
                <span className="text-secondary">OS</span>
                <span className="mono">{system.OSType} {system.Architecture || ''}</span>
              </>
            )}
            {system.DockerRootDir && (
              <>
                <span className="text-secondary">Docker Root</span>
                <span className="mono">{system.DockerRootDir}</span>
              </>
            )}
            {system.NCPU != null && (
              <>
                <span className="text-secondary">CPUs</span>
                <span className="mono">{system.NCPU}</span>
              </>
            )}
            {system.MemTotal != null && (
              <>
                <span className="text-secondary">Total Memory</span>
                <span className="mono">{formatBytes(system.MemTotal)}</span>
              </>
            )}
            {system.Name && (
              <>
                <span className="text-secondary">Hostname</span>
                <span className="mono">{system.Name}</span>
              </>
            )}
            {system.ServerVersion && (
              <>
                <span className="text-secondary">Docker Version</span>
                <span className="mono">{system.ServerVersion}</span>
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
