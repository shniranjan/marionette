# Marionette — Project Inception Record

> Generated 2026-06-15 from `docs/project-inception-template.md`  
> Filled from the design conversation between user and Hermes Agent

---

## 1. Concept

**Project name:** `marionette`

**One-line pitch:** A centralized Docker infrastructure management platform — single container, multi-host, container migration, Swarm orchestration, Nginx load balancing.

**What problem does it solve?** Managing Docker across multiple hosts requires SSH-ing into each machine or using Portainer (heavy, enterprise-gated). Nobody provides guided container migration between hosts. Nobody combines Swarm management with a clean, minimal UI.

**Who is it for?** Homelab operators, small DevOps teams, anyone running Docker on 2-20 hosts who wants a single pane of glass.

**Why build it now?** Docker Socket Proxy is mature. Bollard (Rust Docker SDK) fully covers the Docker API including Swarm. Portainer alternatives (Arcane, Dockhand, Komodo) are Go/Node/Rust but none have Python-like approachability OR container migration. There's a gap.

---

## 2. Ecosystem Research

### Existing solutions

| Competitor | Tech Stack | License | Stars | What it does well | What it doesn't |
|------------|-----------|---------|-------|-------------------|-----------------|
| Portainer CE | Go + React | Zlib | Dominant | Multi-host, Swarm, K8s, mature | RBAC/SSO/scanning behind paywall, heavy, no migration |
| Arcane | Go | BSD-3 | 4,400 | GitOps, free SSO, single binary | No vuln scanning, no migration |
| Dockhand | Bun + SvelteKit | BSL 1.1 | New | Security scanning, auto-updates | BSL restricts SaaS, new/rough |
| Dockge | Node.js | MIT | ~12K | Compose-focused, simple | No raw container mgmt, no migration |
| Komodo | Rust | GPL-3 | Growing | Granular RBAC, OAuth | Heavy, enterprise-oriented |

### Positioning — where we fit

**Gap:** Nobody combines (1) multi-host management, (2) container migration, (3) Swarm orchestration, (4) Nginx LB management, and (5) a clean minimal UI in a single tool. Marionette fills all five.

**Unique selling point:** Container migration wizard. Nobody has this.

### Technology landscape

| What | Current version/state | Relevance to us |
|------|----------------------|-----------------|
| Bollard (Rust Docker SDK) | 0.17 | Full Docker API + Swarm support |
| Docker Engine API | v1.54 (Docker 29.5.3, June 2026) | Target API version |
| Docker Socket Proxy | tecnativa/docker-socket-proxy | Remote host access without root exposure |
| Docker Swarm | Alive and well in 2026 | 42 lines of YAML vs K8s' 170+. Right call for marionette. |
| React | 19 + Vite | CRA deprecated Feb 2025. Vite is standard. |
| Nginx | Open-source, graceful reload | Dynamic upstream by regenerating config + nginx -s reload |

---

## 3. Architecture Decision

### Proposed stack

| Layer | Choice | Why |
|-------|--------|-----|
| Backend runtime | Rust (Axum + Bollard) | Performance, single binary, no GC, official Docker SDK |
| API gateway | Node 22 + TypeScript (Fastify) | Auth middleware, SPA serving, WS relay |
| Frontend framework | React 19 + Vite | Same ecosystem as null, fast dev |
| CSS approach | Custom CSS, no framework | Minimal, no dependency churn. 3 theme presets. |
| Real-time transport | WebSocket (container logs/stats) + SSE (Docker events) | Native Axum WS, native Fastify proxy |
| Deployment model | Single container (supervisord: Rust + Node) | Simplest deployment |
| License | AGPL v3 | Strong copyleft, closes SaaS loophole |

### Rejected alternatives

| Option | Reason rejected |
|--------|----------------|
| Python (FastAPI + docker-py) | User directive: no Python |
| Go (single binary) | Two languages (Go + React), user preferred Rust core |
| Rust napi-rs addon | Cross-compilation hell, platform-specific .node files |
| Rust only (no Node gateway) | Need auth middleware, SPA serving — Fastify does this trivially |
| Kubernetes | Overkill. Swarm matches marionette's simplicity + power philosophy |
| Nginx Plus (paid) | Dynamic upstream API is paid. Open-source nginx reload is zero-downtime. |

### Architecture diagram

```
┌──────────────────────────────────────────────────────────────────┐
│                         marionette (single container)                   │
│                                                                    │
│  ┌──────────────┐  ┌──────────────┐  ┌─────────────────────────┐ │
│  │ React 19 SPA │  │ Node Gateway │  │ Rust Core (Axum+bollard)│ │
│  │ (Vite)       │  │ (Fastify)    │  │                         │ │
│  │              │  │              │  │ Endpoints:               │ │
│  │ 8 pages     │  │ auth + proxy │  │ local (socket)           │ │
│  │ 3 themes    │  │ WS relay     │  │ host-b (tcp://:2375)     │ │
│  └──────────────┘  └──────────────┘  │ host-c (tcp://:2375)     │ │
│                                       │ docker compose (shell)   │ │
│                                       └────────────┬────────────┘ │
└────────────────────────────────────────────────────┼──────────────┘
                                         │            │
                                    unix socket  tcp :2375
                                         │            │
                                    ┌────┴────┐  ┌────┴──────────┐
                                    │ Host A  │  │ Host B / C     │
                                    │ (local) │  │ socket-proxy   │
                                    │ docker  │  │ docker         │
                                    └─────────┘  └───────────────┘
```

### Single container or multi-service?

- [x] Sidecar (two processes, one container) — supervisord manages Rust + Node

### Multi-arch needed?

- [x] amd64 + arm64 (QEMU in CI)

---

## 4. Feature Expansion

### Expansion checklist

| Domain | Explored? | Decision |
|--------|:---------:|----------|
| Multi-host / remote management | ✅ | Socket Proxy on each host. Phase 2. |
| Orchestration (Swarm/K8s) | ✅ | Swarm — simpler, right-sized. Phase 3. |
| Load balancing / reverse proxy | ✅ | Nginx config generation + label-driven. Phase 4. |
| Migration between hosts | ✅ | **Killer feature.** 9-step guided wizard. Phase 2. |
| Authentication / RBAC | ✅ | Access key (X-Marionette-Key). Simple, effective. Phase 1. |
| External storage / volume drivers | ✅ | Classify by driver type. Reconnect or warn. Phase 2. |
| Database / service discovery | ✅ | Connection detection + migration review panel. Phase 2. |
| Backup / restore | — | Not in scope. |
| Monitoring / alerting | — | Not in scope (use dedicated tools). |
| CLI / API / SDK | — | Not in scope (UI-focused). |
| Mobile / responsive | — | Desktop tool. No mobile layout. |

### Feature matrix (by phase)

| Feature | Phase 1 | Phase 2 | Phase 3 | Phase 4 |
|---------|:-------:|:-------:|:-------:|:-------:|
| Containers (CRUD + lifecycle) | ✅ | — | — | — |
| Images (list, pull, remove) | ✅ | — | — | — |
| Volumes (CRUD + deep inspect) | ✅ | — | — | — |
| Networks (CRUD + connect) | ✅ | — | — | — |
| Stacks (compose edit + deploy) | ✅ | — | — | — |
| System (info, prune, events) | ✅ | — | — | — |
| 3 themes (dark, light, sepia) | ✅ | — | — | — |
| Access key auth | ✅ | — | — | — |
| Multi-host (endpoints) | — | ✅ | — | — |
| Container migration wizard | — | ✅ | — | — |
| Volume sync overrides | — | ✅ | — | — |
| DB connection review | — | ✅ | — | — |
| Event-driven updates (SSE) | — | ✅ | — | — |
| Swarm management | — | — | ✅ | — |
| Nginx LB management | — | — | — | ✅ |

---

## 5. Feature Deep-Dive: Container Migration

### What happens step-by-step (user flow)?

```
1. Admin selects container on host-a, clicks "Migrate"
2. Marionette inspects container, volumes, env vars, network config
3. If part of stack → recommend migrating entire stack
4. Admin selects target host from connected endpoints
5. Marionette presents migration strategy (auto-selected default, admin can override)
6. Admin reviews database connections — marionette detects DB_HOST, REDIS_URL, etc.
   and offers fixes (migrate together, update hostname, leave as-is)
7. Admin reviews per-volume sync plan — can override paths, transfer method, compression
8. Dry run — marionette shows exact commands that will execute
9. Admin confirms → execute with progress bar
10. Post-migration: verify connectivity, option to remove from source
```

### What edge cases exist?

| Situation | Handling |
|-----------|----------|
| Volume shared by multiple containers | Detect, warn, must migrate all or detach |
| Volume is 500GB | Pre-flight disk check, estimate time, offer cancel |
| Target disk full | Refuse. Show exact deficit. |
| Same volume name exists on target | Prompt: rename, overwrite, skip |
| Bind mount is kernel path | Auto-skip. Never migrate /proc, /sys, /var/run |
| Bind mount is relative path | Warn, cannot resolve, admin must provide absolute |
| Volume plugin mismatch | Warn, offer fallback to local |
| Container is running | Stop first (cold). CRIU experimental option. |
| SSH key not configured | Option C: marionette generates commands, admin runs manually |

### What can go wrong?

| Failure | Recovery |
|---------|----------|
| Transfer interrupted mid-way | Resume from partial (rsync). Or restart entire transfer. |
| Target container fails to start | Rollback: restart on source (source container was stopped, not removed) |
| Database unreachable after migration | Connection review lets admin test before finalizing. Fix and retry. |
| Credentials exposed during transfer | SSH-encrypted transfer. Warn admin about compose file with secrets. |
| Network partition during transfer | Timeout. Admin retries. |

### What does the admin decide vs what is automated?

| Admin decides | Automated |
|---------------|-----------|
| Which host to migrate to | Container inspection + config extraction |
| Migration strategy (stack, single, CRIU) | Volume classification (local/NFS/cloud) |
| Per-volume sync overrides | Generate compose file from container config |
| Database connection fixes | Export/transfer/import pipeline |
| Compression level | Progress tracking |
| Whether to remove from source | Post-migration connectivity test |
| SSH key provision method | Audit log entries |

---

## 6. Security Review

### Auth model

- [x] Access key / API key — `MARIONETTE_KEY` env var, `X-Marionette-Key` header

### Credential handling

| Surface | Where credentials appear | Protection |
|---------|--------------------------|------------|
| Environment variables | Container inspect, marionette UI, compose files | Masked by default in UI. Reveal requires confirmation + audit log. |
| Volume driver Options | docker volume inspect | Sanitized before display. Never transmitted. |
| Compose file during migration | Generated by marionette, transferred via SCP | Warn if contains secrets. SSH-encrypted. Remind deletion. |
| Docker socket | /var/run/docker.sock | Socket proxy on remote hosts. Granular permissions. |
| SSH keys | Migration transfer | Option C: marionette never holds keys. Admin runs commands manually. |

### Attack surface

| Threat | Severity | Mitigation |
|--------|:--------:|------------|
| Compromised marionette UI (stolen key) | Critical | Mandatory MARIONETTE_KEY, rate limiting, audit log |
| Exposed socket proxy port | Critical | Bind to Docker network only, never 0.0.0.0 |
| Secrets leaked in compose during transfer | High | SSH-encrypted transfer, admin warned, reminded to delete |
| Secrets visible in marionette UI | High | Masked by default, reveal with confirmation |
| MITM on host-to-host transfer | High | Enforce SSH (SCP/rsync over SSH). Warn on plain rsync. |
| SSH key theft from marionette container | High | Option C: marionette never holds keys |
| Orphaned credentials post-migration | Medium | Admin reminded to rotate + cleanup |
| Marionette container root access | Medium | Run as non-root, read-only FS |

### Transport security

| Data in transit | Encryption | Notes |
|-----------------|:----------:|-------|
| Marionette ↔ Docker socket (local) | N/A (Unix socket) | Local only |
| Marionette ↔ Socket proxy (remote) | ❌ HTTP (by design) | Docker network isolation. TLS if cross-network. |
| Host A ↔ Host B (migration) | ✅ SSH | SCP/rsync over SSH |
| Browser ↔ Marionette gateway | ✅ Planned | Reverse proxy (nginx) with TLS in production |

### Socket / API access

- Docker socket = root on host. Protected by Socket Proxy with granular permissions on remote hosts.
- Local socket: only marionette container has access. Container binds to Docker network only.

---

## 7. Performance Audit

### Through each layer

| Layer | Design | Bottleneck? | Fix |
|-------|--------|:-----------:|-----|
| Rust backend | tokio async, Axum | No | — |
| Docker API calls | Sequential in early design | Yes | `tokio::join!` for parallel calls |
| Node gateway | Fastify reverse proxy | No (50k req/s, we need 10) | — |
| Frontend rendering | React 19, custom CSS | Yes (>100 rows) | Virtual scrolling (@tanstack/react-virtual) |
| WebSocket streaming | Per-viewer Docker stream | Yes (N viewers = N streams) | Stream fan-out (broadcast channel) |
| Volume size calculation | Temp container per volume | Yes (500ms/vol) | Cache 120s TTL |
| Migration transfer | Export-to-file-then-SCP | Yes (2x disk I/O) | Pipe directly (no intermediate file) |
| Polling | 5s interval | Yes (constant load) | Docker events SSE (push, not pull) |

### Caching strategy

| What | TTL | Invalidation trigger |
|------|-----|---------------------|
| Volume sizes | 120s | Volume create/remove event |
| Container list | 5s | Container start/die/destroy event |
| System info | 60s | Rarely changes |
| Image list | 30s | Image pull/remove event |

### Specific optimization opportunities

| Priority | Fix | Effort | Impact |
|:---------|-----|--------|--------|
| **Now** | Parallel Docker API calls (tokio::join!) | 30 min | 3-5x dashboard speed |
| **Now** | Volume size caching | 20 min | Eliminates 500ms/volume |
| **Now** | Virtual scrolling | 2 hours | Smooth with 500+ containers |
| **Now** | Log scrollback cap (10k lines) | 10 min | No browser memory leak |
| **Now** | Endpoint connection timeout (5s) | 15 min | Fast failure |
| Phase 2 | Pipe-direct migration | 1 hour | 2x migration speed |
| Phase 2 | pigz/zstd compression | 30 min | 2-4x compression speed |
| Phase 2 | Parallel volume transfer | 1 hour | N volumes in parallel |
| Phase 2 | Event-driven updates (SSE) | 2 hours | Zero polling |
| Phase 2 | WebSocket log fan-out | 1 hour | N:1 stream reduction |
| Phase 3 | Batch stats endpoint | 1 hour | One WS for dashboard |

### Performance budget

| Metric | Target |
|--------|--------|
| Dashboard load (cold) | < 500ms |
| Dashboard load (cached) | < 100ms |
| Container list render (500 items) | < 100ms |
| Log streaming latency | < 1s |
| Migration throughput | > 80MB/s per volume |
| Memory at idle | < 100MB |
| CPU at idle | < 1% |
| Bundle size (first load) | < 150KB gzipped |

---

## 8. UI/UX Design

### Design philosophy

htop for Docker. Engineer-grade. Dark/dense/fast by default. Every piece of information earns its place. No transitions, no skeleton loaders, no "delightful" animations.

### Pages

| Page | Contents | Phase |
|------|----------|-------|
| Dashboard | Stat cards, recent events, system info | 1 |
| Containers | Table with multi-select + action bar | 1 |
| Container Detail | 6 tabs: Info, Logs, Stats, Config, Env, Network | 1 |
| Stacks | List + YML editor (CodeMirror) + deploy | 1 |
| Images | Table + pull modal + remove + history | 1 |
| Volumes | Table + create + remove + prune + deep inspect | 1 |
| Networks | Table + create + remove + connect/disconnect | 1 |
| System | Info + prune + events stream | 1 |
| Migration | 9-step wizard (Phase 2) | 2 |
| Endpoints | Manage remote hosts (Phase 2) | 2 |
| Swarm | Nodes, services, tasks, secrets, configs (Phase 3) | 3 |
| Nginx | LB management, upstream config (Phase 4) | 4 |

### Component inventory

| Component | Reusable? | Notes |
|-----------|:---------:|-------|
| Sidebar | ✅ | Nav + theme switcher + endpoint switcher (Phase 2) |
| StatusBadge | ✅ | ● running / ◌ stopped / ✕ error |
| ActionBar | ✅ | Context-sensitive action buttons |
| StatCard | ✅ | Large number + label |
| ContainerTable | ✅ | Virtual scrolling, multi-select |
| LogViewer | ✅ | WebSocket, monospace, auto-scroll |
| StatsPanel | ✅ | Sparklines, memory bar |
| YamlEditor | ✅ | CodeMirror 6 wrapper |
| JsonTree | ✅ | Collapsible JSON viewer |
| SecretMask | ✅ | •••••• toggle |
| ConnectionReview | ✅ | DB migration review |
| MigrationPlan | ✅ | Strategy + volume overrides |
| VolumeInspector | ✅ | Deep volume details |
| Modal | ✅ | Dark overlay, Esc to close |
| Toast | ✅ | Stack, auto-dismiss |
| Spinner | ✅ | CSS only |

### Color themes

- [x] Dark (GitHub dark palette)
- [x] Light (clean paper)
- [x] Sepia (Solarized warm)

### Density

- [x] Dense (terminal-like, minimal whitespace) — with proper typography so it doesn't look like raw terminal

---

## 9. Implementation Plan

### Phased delivery

| Phase | Deliverable | Novelty |
|-------|------------|---------|
| 1 — Local | Single-host: containers, images, volumes, networks, stacks, system. 3 themes. Auth. Performance fixes. | Solid Docker UI |
| 2 — Multi-host + Migration | Endpoints, host switcher, migration wizard, volume sync, DB connection review, event-driven updates | **First Docker UI with migration** |
| 3 — Swarm | Swarm management: nodes, services, tasks, secrets, configs, visualizer | Full orchestration |
| 4 — Nginx LB | Label-driven upstream management, zero-downtime reload | Traffic management |

### Project structure

```
marionette/
├── Dockerfile
├── docker-compose.yml
├── supervisord.conf
├── Makefile
├── .env.example
├── .gitignore
├── LICENSE                       # AGPL v3
├── README.md
├── docs/
│   ├── quickstart.md
│   ├── architecture.md
│   ├── security.md
│   ├── api-reference.md
│   └── project-inception-template.md
├── core/                         # Rust backend (Axum + bollard)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── docker.rs
│       ├── compose.rs
│       ├── migration.rs
│       ├── audit.rs
│       ├── routes/
│       ├── ws/
│       └── models.rs
├── gateway/                      # Node/TS gateway (Fastify)
│   ├── package.json
│   ├── tsconfig.json
│   └── src/
│       ├── index.ts
│       ├── auth.ts
│       └── proxy.ts
├── frontend/                     # React 19 + Vite
│   ├── package.json
│   ├── vite.config.js
│   └── src/
│       ├── main.jsx
│       ├── App.jsx
│       ├── api/client.js
│       ├── context/ThemeContext.jsx
│       ├── pages/
│       ├── components/
│       └── styles/
├── scripts/
│   └── entrypoint.sh
└── .github/workflows/
    ├── ci.yml
    └── publish.yml
```

### Key dependencies

| Dependency | Version | Purpose |
|-----------|---------|---------|
| bollard | 0.17 | Rust Docker SDK |
| axum | 0.8 | Rust HTTP framework |
| tokio | 1 | Async runtime |
| moka | latest | Caching |
| fastify | 5.x | Node HTTP server |
| @fastify/http-proxy | 10.x | Reverse proxy to Rust |
| react | 19.x | UI framework |
| vite | 6.x | Build tool |
| @codemirror/view | 6.x | YML editor |
| @tanstack/react-virtual | 3.x | Virtual scrolling |
| tecnativa/docker-socket-proxy | latest | Remote host access |

---

## 10. Open Questions & Decisions Pending

| Question | Status | Resolution |
|----------|--------|------------|
| Python vs other backend? | Decided | No Python. Rust core + Node gateway. |
| Compose editor in scope? | Decided | Yes — stacks page with CodeMirror. No compose in Phase 1 plan? Yes, stacks included. |
| Auth model? | Decided | Access key (X-Marionette-Key). Simple, effective. |
| Kubernetes vs Swarm? | Decided | Swarm. Simpler, right-sized for marionette. |
| CRIU live migration? | Decided | Experimental option. Not default. |
| SSH key management for migration? | Decided | Option C (command generation) is safest. Admin runs manually. |
| Responsive/mobile? | Decided | No. Desktop tool. |

---

## 11. Key Design Patterns (to carry forward)

1. **Multi-client from day one** — Rust AppState holds `HashMap<String, Docker>`. Every route takes `?endpoint=`. Adding remote hosts later requires only UI, not backend refactor.
2. **Socket Proxy over custom agent** — Don't build an agent. Use the standard `tecnativa/docker-socket-proxy`. One `docker run` command per host.
3. **Pipe-direct transfer** — Never write intermediate files during migration. Pipe tar through SSH.
4. **Event-driven over polling** — Docker events SSE eliminates constant API polling. Push, don't pull.
5. **Sidecar over monolith or microservices** — Two processes (Rust + Node) in one container via supervisord. Simpler than multi-container, more flexible than monolith.
6. **Secret masking everywhere** — Env vars, volume options, compose files. Mask by default. Reveal with audit.
7. **Dry run before execute** — Migration wizard shows exact commands before running them. Admin confirms.

---

## 12. What We Learned

1. **Research before architecture.** The Socket Proxy discovery changed the multi-host design from "build a custom agent" to "use this standard container." Saved weeks.
2. **The expansion pattern is predictable.** Every subsystem explored leads to "what about X?" for adjacent domains. Following this template catches most of them upfront.
3. **Security review is uncomfortable but necessary.** Credential handling, socket exposure, SSH key management — these are easy to skip in the excitement of design but are where real-world deployments fail.
4. **Performance is not premature optimization.** The `tokio::join!` parallelization and volume caching are 30-minute fixes that prevent real user pain. Catching them in design is better than debugging slow dashboards in production.
5. **No Python was the right call.** The Rust + Node sidecar architecture is cleaner for this use case than forcing everything through Python.
