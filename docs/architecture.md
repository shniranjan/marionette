# Architecture

How marionette works under the hood.

---

## Overview

Marionette is a **single container** running two processes under supervisord:

```
┌─────────────────────────────────────────────────────┐
│                  marionette container                      │
│                                                     │
│  ┌────────────────┐    ┌──────────────────────────┐ │
│  │ supervisord    │    │                          │ │
│  │                │    │                          │ │
│  │ ┌────────────┐ │    │                          │ │
│  │ │ marionette-core  │ │    │                          │ │
│  │ │ (Rust)     │◀┼────┼── /var/run/docker.sock  │ │
│  │ │ :9119      │ │    │        (bind mount)      │ │
│  │ └────────────┘ │    │                          │ │
│  │                │    │                          │ │
│  │ ┌────────────┐ │    │                          │ │
│  │ │ marionette-      │ │    │                          │ │
│  │ │ gateway    │◀┼────┼── :8000 (browser)        │ │
│  │ │ (Node)     │ │    │                          │ │
│  │ │ :8000      │─┼────┼──▶ marionette-core:9119        │ │
│  │ └────────────┘ │    │                          │ │
│  └────────────────┘    └──────────────────────────┘ │
│                                                     │
│  /stacks  ◀── bind mount (compose files)            │
└─────────────────────────────────────────────────────┘
```

---

## Component Design

### 1. Rust Core (`marionette-core`)

**Purpose:** All Docker interaction. Exposes internal REST + WebSocket API on `127.0.0.1:9119`.

**Tech:** Axum (HTTP framework), Bollard (Docker SDK), Tokio (async runtime).

**Key design decisions:**

- **Multi-client from day one.** `AppState` contains `HashMap<String, Docker>` — one client per endpoint. Every route accepts `?endpoint=` query param. Adds ~50 lines now, saves full refactor later.

- **No database.** Marionette is stateless. All state lives in Docker (containers, volumes, networks) and on the filesystem (/stacks). Audit logs are in-memory (Phase 1) with optional SQLite (Phase 2).

- **`docker compose` via shell.** Bollard doesn't have a compose API. The Rust core shells out to `docker compose` for stack operations. This is the same approach used by Docker Desktop and Portainer.

- **Caching with `moka`.** TTL-based cache for volume sizes, system info, container lists. Invalidated on relevant Docker events.

**Module map:**

```
core/src/
├── main.rs           # Axum server startup, router, AppState init
├── docker.rs         # Docker client factory (socket + HTTP)
├── compose.rs        # docker compose shell wrapper
├── migration.rs      # Migration workflow orchestrator (Phase 2)
├── audit.rs          # Audit logging (ring buffer)
├── routes/
│   ├── containers.rs # Container CRUD + lifecycle
│   ├── images.rs     # Image list, pull, remove, history
│   ├── volumes.rs    # Volume CRUD + deep inspection
│   ├── networks.rs   # Network CRUD + connect/disconnect
│   ├── stacks.rs     # Stack CRUD + deploy/stop/down
│   ├── endpoints.rs  # Remote endpoint management (Phase 2)
│   ├── swarm.rs      # Swarm management (Phase 3)
│   └── system.rs     # Info, version, events (SSE), prune
├── ws/
│   ├── logs.rs       # Container log streaming (WebSocket)
│   ├── stats.rs      # Container stats streaming (WebSocket)
│   └── deploy.rs     # Stack deploy output streaming
└── models.rs         # Request/response serde types
```

### 2. Node Gateway (`marionette-gateway`)

**Purpose:** Authentication, reverse proxy to Rust core, static file serving (React SPA).

**Tech:** Fastify (HTTP server), `@fastify/http-proxy` (reverse proxy), `@fastify/static` (SPA serving).

**Key design decisions:**

- **Thin layer.** The gateway does NOT contain business logic. It checks the access key, then passes everything through to Rust.

- **WebSocket passthrough.** `@fastify/http-proxy` natively passes WebSocket frames without parsing. No manual WS relay needed.

- **SPA fallback.** Any request not matching `/api/*` serves `index.html` — standard SPA pattern, no server-side routing needed.

- **Rate limiting.** 5 failed auth attempts → 30-second lockout per IP. Prevents brute-force key guessing.

**Auth flow:**

```
Browser                    Gateway                    Rust Core
  │                          │                          │
  │ GET /api/containers      │                          │
  │ X-Marionette-Key: secret       │                          │
  │─────────────────────────▶│                          │
  │                          │ validate key             │
  │                          │ if invalid → 401         │
  │                          │                          │
  │                          │ GET /containers          │
  │                          │─────────────────────────▶│
  │                          │                          │ process
  │                          │◀─────────────────────────│
  │◀─────────────────────────│                          │
  │  200 + JSON              │                          │
```

### 3. React Frontend

**Purpose:** Web UI for managing Docker resources.

**Tech:** React 19, Vite, CodeMirror 6 (YML editor), `@tanstack/react-virtual` (virtual scrolling).

**Key design decisions:**

- **No router library.** State-based routing — a `page` string in App state determines which component renders. Simpler, fewer dependencies.

- **No CSS framework.** Custom CSS with CSS custom properties for theming. Three preset themes (dark/light/sepia). No runtime theme generation.

- **Virtual scrolling.** Tables with >100 items use `@tanstack/react-virtual` to render only visible rows.

- **API client pattern.** Single `api/client.js` module exports named functions for every endpoint. Auto-attaches `X-Marionette-Key` header from localStorage.

- **Theme context.** React Context + `data-theme` attribute on `<html>`. CSS variables cascade globally. Persisted to localStorage.

---

## Data Flow

### Viewing containers

```
Browser                    Gateway                    Rust Core              Docker
  │                          │                          │                      │
  │ GET /api/containers      │                          │                      │
  │─────────────────────────▶│                          │                      │
  │                          │ validate auth            │                      │
  │                          │ proxy →                  │                      │
  │                          │─────────────────────────▶│                      │
  │                          │                          │ docker.list_cont..() │
  │                          │                          │─────────────────────▶│
  │                          │                          │◀─────────────────────│
  │                          │◀─────────────────────────│                      │
  │◀─────────────────────────│                          │                      │
  │  JSON                    │                          │                      │
```

### Streaming container logs

```
Browser                    Gateway                    Rust Core              Docker
  │                          │                          │                      │
  │ WS /api/containers/      │                          │                      │
  │    abc/logs/stream        │                          │                      │
  │─────────────────────────▶│                          │                      │
  │                          │ WS passthrough           │                      │
  │                          │─────────────────────────▶│                      │
  │                          │                          │ docker.logs(         │
  │                          │                          │   follow: true)      │
  │                          │                          │─────────────────────▶│
  │                          │                          │◀── stream ───────────│
  │                          │◀── WS frame ─────────────│                      │
  │◀── WS frame ────────────│                          │                      │
  │  {"stream":"stdout",     │                          │                      │
  │   "text":"server ready"} │                          │                      │
```

---

## Multi-Host Architecture (Phase 2)

```
┌──────────────────────────────────────────────────────────────┐
│                        marionette (central)                         │
│                                                               │
│  Rust Core: HashMap<String, Docker>                           │
│  ┌──────────────────────────────────────────────────────────┐│
│  │ "local" → Docker::connect_with_socket("/var/run/...")    ││
│  │ "prod"  → Docker::connect_with_http("tcp://10.0.0.5")   ││
│  │ "stage" → Docker::connect_with_http("tcp://10.0.0.6")   ││
│  └──────────────────────────────────────────────────────────┘│
└──────────────────────────────────────────────────────────────┘
          │                    │                    │
     unix socket          tcp :2375           tcp :2375
          │                    │                    │
     ┌────┴────┐         ┌─────┴─────┐        ┌─────┴─────┐
     │  Host A │         │  Host B   │        │  Host C   │
     │ (local) │         │socket-proxy│       │socket-proxy│
     │  docker │         │  docker    │       │  docker    │
     └─────────┘         └───────────┘        └───────────┘
```

Each remote host runs `tecnativa/docker-socket-proxy` — a single container that exposes the Docker API over HTTP with granular permissions. Marionette connects via bollard's HTTP client. No custom agent code.

---

## Migration Architecture (Phase 2)

Container migration is a 9-step workflow orchestrated by marionette:

```
Admin clicks "Migrate"
        │
        ▼
┌─────────────────┐
│ 1. Inspect      │  Read container config, volumes, env vars, networks
└────────┬────────┘
         ▼
┌─────────────────┐
│ 2. Classify     │  Categorize volumes (local/NFS/cloud), detect DB connections
└────────┬────────┘
         ▼
┌─────────────────┐
│ 3. Plan         │  Generate migration strategy. Admin selects target host.
└────────┬────────┘
         ▼
┌─────────────────┐
│ 4. Review       │  Admin reviews volume sync plan, DB connection fixes
└────────┬────────┘
         ▼
┌─────────────────┐
│ 5. Dry Run      │  Show exact commands that will execute. No changes made.
└────────┬────────┘
         ▼
┌─────────────────┐
│ 6. Execute      │  Stop → export → transfer → import → start
│                 │  Progress bar. Parallel volume transfer.
└────────┬────────┘
         ▼
┌─────────────────┐
│ 7. Verify       │  Health check, DB connectivity test
└────────┬────────┘
         ▼
┌─────────────────┐
│ 8. Cleanup      │  Option: remove from source. Rotate credentials.
└────────┬────────┘
         ▼
┌─────────────────┐
│ 9. Audit        │  Log all actions
└─────────────────┘
```

Marionette never sees the data. It orchestrates commands on source and target hosts via their Docker APIs. Transfer happens directly between hosts (SCP/rsync over SSH).

---

## Performance Design

| Concern | Solution |
|---------|----------|
| Slow dashboard load | `tokio::join!` parallelizes 5 Docker API calls |
| Repeated volume size calculation | 120s TTL cache |
| Large container lists (500+) | Virtual scrolling — render visible rows only |
| Polling overhead | Docker events SSE — push-based refresh (Phase 2) |
| N viewers = N log streams | WebSocket fan-out — one Docker stream broadcast to N |
| Large volume migration | Pipe-direct transfer (no intermediate files), pigz compression |

---

## Security Model

See [Security](security.md) for the full threat model. Key points:

- **Auth:** Access key on all `/api/*` routes via `X-Marionette-Key` header
- **Credentials:** Masked in UI. Never transmitted in raw driver options.
- **Remote hosts:** Socket proxy with granular permissions. No full socket access.
- **Migration:** SSH-encrypted transfer. Option C: marionette never holds SSH keys.
- **Audit:** All mutating actions logged with admin key hash.

---

## Design Decisions Log

| Decision | Rationale | Date |
|----------|-----------|------|
| Rust core + Node gateway (not monolith) | Auth middleware + SPA serving is trivial in Fastify. Separate concerns. | 2026-06-15 |
| Rust sidecar over napi-rs addon | Avoid cross-compilation build hell for `.node` binaries | 2026-06-15 |
| No Python | User directive | 2026-06-15 |
| Docker Swarm over Kubernetes | 42 lines of YAML vs 170+. Right-sized for marionette's use case. | 2026-06-15 |
| Access key over JWT + login page | Docker socket = root. Adding users + DB is security theater. Key is simpler and effective. | 2026-06-15 |
| Socket Proxy over custom agent | Standard, battle-tested, zero maintenance. One docker run command per host. | 2026-06-15 |
| Pipe-direct over file-based migration | 2x faster, no intermediate disk I/O | 2026-06-15 |
| AGPL v3 | Strong copyleft, closes SaaS loophole | 2026-06-15 |
