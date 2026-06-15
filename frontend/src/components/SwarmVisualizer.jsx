import { useState, useMemo, useRef, useEffect, useCallback } from 'react';

function formatBytes(bytes) {
  if (!bytes || bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}

function statusColor(status) {
  switch ((status || '').toLowerCase()) {
    case 'ready': return 'var(--green)';
    case 'down': return 'var(--red)';
    default: return 'var(--yellow)';
  }
}

export default function SwarmVisualizer({ swarm, nodes, services, tasks }) {
  const containerRef = useRef(null);
  const [dims, setDims] = useState({ width: 800, height: 500 });
  const [tooltip, setTooltip] = useState(null);

  const measure = useCallback(() => {
    if (containerRef.current) {
      const w = containerRef.current.clientWidth || 800;
      setDims({ width: w, height: Math.max(500, w * 0.55) });
    }
  }, []);

  useEffect(() => {
    measure();
    window.addEventListener('resize', measure);
    return () => window.removeEventListener('resize', measure);
  }, [measure]);

  const layout = useMemo(() => {
    const nodeList = Array.isArray(nodes) ? nodes : [];
    const serviceList = Array.isArray(services) ? services : [];
    const taskList = Array.isArray(tasks) ? tasks : [];
    const w = dims.width;
    const h = dims.height;

    if (nodeList.length === 0) return { nodePositions: [], servicePositions: [], links: [] };

    const cols = Math.min(nodeList.length, Math.ceil(Math.sqrt(nodeList.length * (w / h))));
    const rows = Math.ceil(nodeList.length / cols);
    const cellW = (w - 60) / cols;
    const cellH = (h - 100) / rows;

    const nodePositions = nodeList.map((node, i) => {
      const col = i % cols;
      const row = Math.floor(i / cols);
      return {
        id: node.ID || node.id,
        hostname: node.Description?.Hostname || node.Spec?.Hostname || node.hostname || '?',
        role: (node.Spec?.Role || node.role || 'worker').toLowerCase(),
        status: (node.Status?.State || node.status || 'unknown').toLowerCase(),
        availability: (node.Spec?.Availability || node.availability || 'active').toLowerCase(),
        engineVersion: node.Description?.Engine?.EngineVersion || node.engine_version || '',
        cpuCount: node.Description?.Resources?.NanoCPUs ? Math.round(node.Description.Resources.NanoCPUs / 1e9) : (node.cpu_count || 0),
        memoryBytes: node.Description?.Resources?.MemoryBytes || node.memory_bytes || 0,
        x: 30 + col * cellW + cellW / 2,
        y: 70 + row * cellH + cellH / 2,
        width: Math.max(140, cellW * 0.7),
        height: Math.max(70, cellH * 0.6),
      };
    });

    const nodeMap = {};
    nodePositions.forEach((n) => { nodeMap[n.id] = n; });

    const servicePositions = [];
    const links = [];

    serviceList.forEach((svc, si) => {
      const svcTasks = taskList.filter((t) =>
        (t.ServiceID || t.service) === (svc.ID || svc.id)
      );
      const svcReplicas = (svc.Spec?.Mode?.Replicated?.Replicas ?? svc.replicas ?? 0);

      svcTasks.forEach((task, ti) => {
        const nodeId = task.NodeID || task.node;
        const node = nodeMap[nodeId];
        if (node) {
          const sx = node.x - node.width / 2 + 10 + (ti % 4) * 28;
          const sy = node.y + 4 + Math.floor(ti / 4) * 18;
          servicePositions.push({
            id: svc.ID || svc.id,
            name: svc.Spec?.Name || svc.name || '?',
            image: (svc.Spec?.TaskTemplate?.ContainerSpec?.Image || svc.image || '').split('@')[0],
            taskStatus: (task.Status?.State || task.status || 'pending').toLowerCase(),
            x: sx,
            y: sy,
            nodeId: node.id,
          });
          links.push({
            from: node.id,
            to: svc.ID || svc.id,
            x1: sx + 12,
            y1: sy + 8,
            x2: node.x + node.width / 2 - 8,
            y2: sy + 8,
          });
        }
      });

      if (svcTasks.length === 0) {
        const node = nodePositions[si % nodePositions.length];
        if (node) {
          servicePositions.push({
            id: svc.ID || svc.id,
            name: svc.Spec?.Name || svc.name || '?',
            image: (svc.Spec?.TaskTemplate?.ContainerSpec?.Image || svc.image || '').split('@')[0],
            taskStatus: 'pending',
            x: node.x - node.width / 2 + 10,
            y: node.y + 4,
            nodeId: node.id,
          });
        }
      }
    });

    return { nodePositions, servicePositions, links };
  }, [nodes, services, tasks, dims]);

  const { nodePositions, servicePositions, links } = layout;

  const showTooltip = useCallback((content, evt) => {
    const rect = containerRef.current?.getBoundingClientRect();
    setTooltip({
      content,
      x: (evt.clientX - (rect?.left || 0)) + 12,
      y: (evt.clientY - (rect?.top || 0)) - 10,
    });
  }, []);

  const hideTooltip = useCallback(() => setTooltip(null), []);

  if (!swarm && (!nodes || nodes.length === 0)) {
    return (
      <div ref={containerRef} className="card" style={{ padding: '48px', textAlign: 'center' }}>
        <div style={{ fontSize: '3rem', marginBottom: '12px' }}>🐝</div>
        <h3>No Swarm Cluster Active</h3>
        <p className="text-secondary">
          Initialize or join a Swarm to see the visualizer.
        </p>
      </div>
    );
  }

  if (nodePositions.length === 0) {
    return (
      <div ref={containerRef} className="card" style={{ padding: '24px', textAlign: 'center' }}>
        <SpinnerIcon />
        <p className="text-secondary" style={{ marginTop: '12px' }}>Loading topology...</p>
      </div>
    );
  }

  return (
    <div ref={containerRef} style={{ position: 'relative', width: '100%' }}>
      <svg
        width={dims.width}
        height={dims.height}
        style={{ display: 'block', background: 'var(--bg-secondary)', borderRadius: '8px', border: '1px solid var(--border)' }}
      >
        <defs>
          <marker id="arrowhead" markerWidth="6" markerHeight="4" refX="6" refY="2" orient="auto">
            <polygon points="0 0, 6 2, 0 4" fill="var(--text-secondary)" />
          </marker>
        </defs>

        {links.map((link, i) => (
          <line
            key={`link-${i}`}
            x1={link.x1} y1={link.y1}
            x2={link.x2} y2={link.y2}
            stroke="var(--border)"
            strokeWidth="1"
            strokeDasharray="3,2"
            markerEnd="url(#arrowhead)"
          />
        ))}

        {nodePositions.map((node) => {
          const isManager = node.role === 'manager';
          const borderColor = isManager ? 'var(--accent)' : 'var(--border)';
          const fill = node.status === 'ready' ? 'var(--bg-tertiary)' : 'var(--bg-secondary)';
          const statusCol = statusColor(node.status);

          return (
            <g key={`node-${node.id}`}
              onMouseEnter={(e) => showTooltip(nodeTooltipContent(node), e)}
              onMouseLeave={hideTooltip}
            >
              <rect
                x={node.x - node.width / 2}
                y={node.y - node.height / 2}
                width={node.width}
                height={node.height}
                rx="6"
                fill={fill}
                stroke={borderColor}
                strokeWidth={isManager ? 2.5 : 1.5}
              />
              {isManager && (
                <text
                  x={node.x - node.width / 2 + 10}
                  y={node.y - node.height / 2 + 18}
                  fontSize="14"
                >👑</text>
              )}
              <text
                x={node.x}
                y={node.y - 6}
                textAnchor="middle"
                fontSize="11"
                fontWeight="600"
                fill="var(--text-primary)"
              >
                {node.hostname.length > 18 ? node.hostname.slice(0, 16) + '…' : node.hostname}
              </text>
              <text
                x={node.x}
                y={node.y + 12}
                textAnchor="middle"
                fontSize="10"
                fill="var(--text-secondary)"
              >
                {node.role} · {node.cpuCount} CPU · {formatBytes(node.memoryBytes)}
              </text>
              <circle
                cx={node.x + node.width / 2 - 14}
                cy={node.y - node.height / 2 + 14}
                r="5"
                fill={statusCol}
                stroke="var(--bg-secondary)"
                strokeWidth="1"
              />
            </g>
          );
        })}

        {servicePositions.map((svc, i) => (
          <g key={`svc-${i}`}
            onMouseEnter={(e) => showTooltip(svcTooltipContent(svc), e)}
            onMouseLeave={hideTooltip}
          >
            <rect
              x={svc.x}
              y={svc.y}
              width="24"
              height="14"
              rx="3"
              fill="var(--accent-dim)"
              stroke="var(--accent)"
              strokeWidth="1"
              opacity="0.85"
            />
            <text
              x={svc.x + 12}
              y={svc.y + 10}
              textAnchor="middle"
              fontSize="7"
              fill="#fff"
            >
              {svc.name.slice(0, 4)}
            </text>
          </g>
        ))}
      </svg>

      {/* Tooltip */}
      {tooltip && (
        <div style={{
          position: 'absolute',
          left: tooltip.x,
          top: tooltip.y,
          background: 'var(--bg-tertiary)',
          border: '1px solid var(--border)',
          borderRadius: '6px',
          padding: '10px 14px',
          fontSize: '0.8rem',
          color: 'var(--text-primary)',
          boxShadow: 'var(--card-shadow)',
          pointerEvents: 'none',
          zIndex: 100,
          maxWidth: '280px',
        }}>
          {tooltip.content}
        </div>
      )}

      {/* Legend */}
      <div style={{
        display: 'flex', gap: '16px', padding: '12px 8px',
        fontSize: '0.75rem', flexWrap: 'wrap',
      }}>
        <LegendItem color="var(--accent)" label="Manager" icon="👑" />
        <LegendItem color="var(--border)" label="Worker" />
        <LegendItem color="var(--green)" label="Ready" dot />
        <LegendItem color="var(--yellow)" label="Warning" dot />
        <LegendItem color="var(--red)" label="Down" dot />
        <LegendItem color="var(--accent-dim)" label="Service" rect />
      </div>
    </div>
  );
}

function LegendItem({ color, label, icon, dot, rect: isRect }) {
  return (
    <span style={{ display: 'inline-flex', alignItems: 'center', gap: '6px', color: 'var(--text-secondary)' }}>
      {dot ? (
        <span style={{ width: '10px', height: '10px', borderRadius: '50%', background: color, display: 'inline-block' }} />
      ) : isRect ? (
        <span style={{ width: '16px', height: '9px', borderRadius: '2px', background: color, display: 'inline-block', border: `1px solid ${color}` }} />
      ) : null}
      {icon && <span style={{ fontSize: '12px' }}>{icon}</span>}
      {label}
    </span>
  );
}

function nodeTooltipContent(node) {
  return (
    <div style={{ lineHeight: 1.5 }}>
      <div style={{ fontWeight: 600, marginBottom: '4px' }}>
        {node.role === 'manager' ? '👑 ' : ''}{node.hostname}
      </div>
      <div><span className="text-secondary">Role:</span> {node.role}</div>
      <div><span className="text-secondary">Status:</span> {node.status}</div>
      <div><span className="text-secondary">Availability:</span> {node.availability}</div>
      {node.engineVersion && (
        <div><span className="text-secondary">Engine:</span> {node.engineVersion}</div>
      )}
      <div><span className="text-secondary">CPU:</span> {node.cpuCount}</div>
      <div><span className="text-secondary">Memory:</span> {formatBytes(node.memoryBytes)}</div>
    </div>
  );
}

function svcTooltipContent(svc) {
  return (
    <div style={{ lineHeight: 1.5 }}>
      <div style={{ fontWeight: 600, marginBottom: '4px' }}>{svc.name}</div>
      <div className="mono" style={{ fontSize: '0.7rem' }}>{svc.image}</div>
      <div><span className="text-secondary">Status:</span> {svc.taskStatus}</div>
    </div>
  );
}

function SpinnerIcon() {
  return (
    <div style={{
      display: 'inline-block',
      width: '32px', height: '32px',
      border: '3px solid var(--border)',
      borderTopColor: 'var(--accent)',
      borderRadius: '50%',
      animation: 'spin 0.6s linear infinite',
    }} />
  );
}
