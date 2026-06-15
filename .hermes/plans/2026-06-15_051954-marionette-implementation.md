# Doll — Docker Infrastructure Management Platform — Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Build a centralized Docker infrastructure management platform — multi-host, container migration, Swarm orchestration, Nginx load balancing, and a full-featured web UI. Single container deployment.

**Architecture:** Rust binary (Axum + bollard) serves as the Docker interaction core, connecting to multiple Docker hosts via Socket Proxy. Node/TypeScript gateway (Fastify) enforces access-key auth, proxies requests to Rust, and serves the React SPA. Both processes run in one container under supervisord.

**Tech Stack:** Rust (Axum, bollard, tokio), Node 22 + TypeScript (Fastify), React 19 + Vite, CodeMirror 6, supervisord, Docker multi-arch (amd64 + arm64).

**Auth:** Access key via `DOLL_KEY` env var. Gateway checks `X-Doll-Key` header on every `/api/*` request. No key configured → allow all (local dev). Key set → mandatory for all API access. Frontend stores key in localStorage, shows auth gate on 401.

**License:** AGPL v3

---

## Phased Roadmap

| Phase | Deliverable | Novelty |
|-------|------------|---------|
| **1 — Local** | Single-host: containers, images, volumes, networks, stacks, system. Full CLI parity. 3 themes. Rust core designed with multi-client architecture from day one. | Solid Docker UI, shippable standalone. |
| **2 — Multi-host + Migration** | Endpoint management. Add remote hosts via Socket Proxy. Host switcher. **Container migration wizard** — the killer feature nobody else has. | First Docker UI with built-in guided migration. |
| **3 — Swarm** | Swarm management: nodes, services, tasks, secrets, configs. Init/join/leave. Visualizer. | Full orchestration without Portainer. |
| **4 — Nginx LB** | Nginx config generator. Label-driven upstream management. Traffic routing across hosts. Zero-downtime reload. | Full infrastructure management. |

---

## Project Structure

```
doll/
├── Dockerfile                    # Multi-stage: Rust build → Node + React build → runtime
├── docker-compose.yml            # Service + socket + /stacks mounts
├── supervisord.conf              # Runs doll-core + doll-gateway
├── Makefile
├── .env.example                  # DOLL_KEY=<generate-me>
├── .gitignore
├── LICENSE                       # AGPL v3
├── README.md
├── docs/
│   ├── quickstart.md
│   ├── architecture.md
│   ├── security.md               # Security model + threat mitigation
│   └── api-reference.md
├── core/                         # Rust backend
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs               # Axum server entry, router, multi-endpoint state
│       ├── docker.rs             # Docker client factory (socket + TCP per endpoint)
│       ├── compose.rs            # docker compose shell wrapper
│       ├── migration.rs          # Migration workflow orchestrator
│       ├── audit.rs              # Audit logging
│       ├── routes/
│       │   ├── mod.rs
│       │   ├── containers.rs     # list, inspect, start, stop, restart, kill, pause, remove, rename
│       │   ├── images.rs         # list, inspect, pull, remove, history
│       │   ├── volumes.rs        # list, inspect, create, remove, prune
│       │   ├── volumes_inspect.rs # deep volume inspection: driver, options, size, usage
│       │   ├── networks.rs       # list, inspect, create, remove, connect, disconnect, prune
│       │   ├── stacks.rs         # list, read, save, deploy, stop, down, remove
│       │   ├── endpoints.rs      # CRUD for Docker endpoints (Phase 2)
│       │   ├── system.rs         # info, version, events (SSE), prune
│       │   └── swarm.rs          # nodes, services, tasks, secrets, configs (Phase 3)
│       ├── ws/
│       │   ├── mod.rs
│       │   ├── logs.rs           # Stream container logs via WebSocket
│       │   ├── stats.rs          # Stream container stats via WebSocket
│       │   └── deploy.rs         # Stream stack deploy output (Phase 1)
│       └── models.rs             # Request/response serde types
├── gateway/                      # Node/TypeScript API gateway
│   ├── package.json
│   ├── tsconfig.json
│   └── src/
│       ├── index.ts              # Fastify entry, auth middleware, proxy, serve SPA
│       ├── auth.ts               # X-Doll-Key header validation, rate limiting
│       └── proxy.ts              # Reverse proxy → localhost:9119
├── frontend/                     # React 19 + Vite
│   ├── package.json
│   ├── vite.config.js
│   ├── index.html
│   └── src/
│       ├── main.jsx
│       ├── App.jsx
│       ├── api/
│       │   └── client.js         # Fetch wrapper, auto-attaches X-Doll-Key
│       ├── context/
│       │   └── ThemeContext.jsx   # Dark / Light / Sepia
│       ├── pages/
│       │   ├── Dashboard.jsx
│       │   ├── Containers.jsx
│       │   ├── ContainerDetail.jsx
│       │   ├── Migration.jsx     # Migration wizard (Phase 2)
│       │   ├── Stacks.jsx        # Stack list + YML editor
│       │   ├── Images.jsx
│       │   ├── Volumes.jsx
│       │   ├── Networks.jsx
│       │   ├── Endpoints.jsx     # Endpoint management (Phase 2)
│       │   ├── Swarm.jsx         # Swarm management (Phase 3)
│       │   ├── Nginx.jsx         # Nginx LB management (Phase 4)
│       │   └── System.jsx
│       ├── components/
│       │   ├── Sidebar.jsx
│       │   ├── EndpointSwitcher.jsx # Host switcher (Phase 2)
│       │   ├── ThemeSwitcher.jsx
│       │   ├── AuthGate.jsx
│       │   ├── ContainerTable.jsx
│       │   ├── StatusBadge.jsx
│       │   ├── ActionBar.jsx
│       │   ├── StatCard.jsx
│       │   ├── LogViewer.jsx
│       │   ├── StatsPanel.jsx
│       │   ├── YamlEditor.jsx
│       │   ├── JsonTree.jsx
│       │   ├── VolumeInspector.jsx   # Deep volume inspection UI
│       │   ├── ConnectionReview.jsx  # Database connection migration review
│       │   ├── MigrationPlan.jsx     # Migration strategy selection + volume overrides
│       │   ├── SecretMask.jsx        # Masked/reveal toggle for credentials
│       │   ├── Modal.jsx
│       │   ├── Toast.jsx
│       │   └── Spinner.jsx
│       └── styles/
│           ├── global.css
│           ├── dark.css
│           ├── light.css
│           └── sepia.css
├── scripts/
│   └── entrypoint.sh
└── .github/
    └── workflows/
        ├── ci.yml
        └── publish.yml
```

---

## Cross-Cutting Design: Multi-Client Architecture (built from Phase 1)

The Rust core must be designed for multiple Docker clients from day one, even though Phase 1 only has one (local socket).

### AppState design

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use bollard::Docker;

#[derive(Debug, Clone)]
pub struct DockerEndpoint {
    pub id: String,           // uuid
    pub name: String,          // "local", "production-us", "staging"
    pub connection: String,   // "unix:///var/run/docker.sock" or "tcp://10.0.0.5:2375"
    pub status: EndpointStatus,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum EndpointStatus {
    Connected,
    Disconnected,
    Error(String),
}

pub struct AppState {
    pub endpoints: RwLock<HashMap<String, DockerEndpoint>>,
    pub clients: RwLock<HashMap<String, Docker>>,  // endpoint_id → Docker client
    pub default_endpoint: String,  // "local"
    pub audit_log: RwLock<Vec<AuditEntry>>,
}
```

### Route pattern

Every route takes `?endpoint=` query param, defaults to `default_endpoint`:

```rust
async fn list_containers(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ContainerListParams>,
) -> Result<Json<Vec<ContainerSummary>>, AppError> {
    let endpoint = params.endpoint.unwrap_or(state.default_endpoint.clone());
    let clients = state.clients.read().await;
    let docker = clients.get(&endpoint)
        .ok_or(AppError::EndpointNotFound(endpoint))?;

    let containers = docker.list_containers::<String>(...).await?;
    Ok(Json(containers.into_iter().map(...).collect()))
}
```

Each endpoint gets its own Docker client. Adding a new endpoint = adding to the HashMap. No restart needed.

---

## Phase 1: Single-Host Foundation (unchanged core)

Tasks 1–16 from the previous plan with these additions:

### Task XX: Volume deep inspection endpoint

**Endpoint:** `GET /volumes/:name/inspect?endpoint=local`

Returns:
```json
{
  "name": "hermes_data",
  "driver": "local",
  "driver_type": "filesystem",
  "driver_category": "local",
  "migration_advice": "transfer",
  "mountpoint": "/var/lib/docker/volumes/hermes_data/_data",
  "size_bytes": 2415919104,
  "size_human": "2.3GB",
  "file_count": 12450,
  "last_modified": "2026-06-15T10:30:00Z",
  "used_by": ["hermes-app", "hermes-backup"],
  "shared": true,
  "options": {},
  "options_sanitized": {},
  "labels": {"com.example": "backup"},
  "scope": "local",
  "needs_chown": false,
  "mount_count": 2
}
```

**Logic:**
```rust
async fn inspect_volume_deep(
    state: &AppState,
    endpoint_id: &str,
    volume_name: &str,
) -> Result<VolumeDeepInspection, AppError> {
    let docker = state.get_client(endpoint_id)?;
    let vol = docker.inspect_volume(volume_name).await?;

    // Classify driver
    let (driver_category, migration_advice) = classify_driver(&vol.driver);

    // Get size if local
    let (size_bytes, file_count, last_modified) = if vol.driver == "local" {
        get_volume_size(&vol.mountpoint).await?
    } else {
        (None, None, None)
    };

    // Find containers using this volume
    let used_by = find_volume_users(&docker, volume_name).await?;

    // Sanitize options — mask secrets
    let options_sanitized = sanitize_options(&vol.driver, &vol.options);

    Ok(VolumeDeepInspection { ... })
}

fn classify_driver(driver: &str) -> (&'static str, &'static str) {
    match driver {
        "local" | "local-persist" => ("filesystem", "transfer"),
        "nfs" | "cifs" | "smb"    => ("network", "reconnect"),
        "rclone"                   => ("cloud", "reconnect"),
        "rexray" | "cloudstor"    => ("cloud_block", "reconnect"),
        "glusterfs"                => ("distributed", "reconnect"),
        _                          => ("unknown", "warn"),
    }
}
```

### Task XX: Volume size computation (inside docker container)

Doll needs to measure volume sizes. Since doll is in a container, it can't directly access `/var/lib/docker/volumes/...`. Two approaches:

**A) Shell on the host via Docker API:**
```rust
// Run a temporary container that mounts the volume and measures it
let result = docker.create_container(
    Some(CreateContainerOptions { name: "doll-vol-size" }),
    Config {
        image: Some("alpine"),
        cmd: Some(vec!["du", "-sb", "/data"]),
        volumes: Some(hashmap!{ volume_name => hashmap!{} }),
        host_config: Some(HostConfig {
            binds: Some(vec![format!("{}:/data", volume_name)]),
            auto_remove: Some(true),
            ..Default::default()
        }),
        ..Default::default()
    },
).await?;
docker.start_container("doll-vol-size", None).await?;
let output = docker.wait_container("doll-vol-size", ...).await?;
// Parse "2415919104\t/data" → size
```

**B) Direct host path access** (only works if doll has host path access via socket):
Doll can't directly run `du` on the host. Option A is the clean approach.

### Task XX: Sanitize options utility

```rust
fn sanitize_options(driver: &str, options: &HashMap<String, String>) -> HashMap<String, String> {
    let secret_keys: &[&str] = match driver {
        "cifs" | "smb" => &["password", "secret", "credentials"],
        "rclone" => &["s3-access-key-id", "s3-secret-access-key", "s3-session-token",
                       "gcs-service-account-file", "azure-account-key"],
        "nfs" => &[],  // NFS options don't typically contain secrets
        _ => &["password", "secret", "key", "token", "credentials", "access-key"],
    };

    options.iter().map(|(k, v)| {
        if secret_keys.iter().any(|sk| k.to_lowercase().contains(sk)) {
            (k.clone(), "••••••••".to_string())
        } else {
            (k.clone(), v.clone())
        }
    }).collect()
}
```

### Task XX: Audit logging

```rust
struct AuditEntry {
    timestamp: chrono::DateTime<chrono::Utc>,
    action: String,        // "container.stop", "migration.start", "secret.reveal"
    endpoint_id: String,
    target: String,        // container id, volume name, etc.
    detail: String,         // human-readable
    admin_key_hash: String, // SHA-256 of the X-Doll-Key used
}
```

Log all mutating actions. In-memory ring buffer for Phase 1, SQLite for Phase 2+. Frontend shows audit on System page.

---

## Phase 2: Multi-Host + Migration

### Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                        doll (central)                         │
│                                                               │
│  ┌─────────┐  ┌─────────┐  ┌────────────────────────────┐   │
│  │ React   │  │ Fastify │  │  Rust Core                 │   │
│  │ UI      │  │ Gateway │  │                             │   │
│  │         │  │         │  │  Endpoints:                 │   │
│  │ Endpoint│  │         │  │  local → unix:///var/run/.. │   │
│  │ Switcher│  │         │  │  prod  → tcp://10.0.0.5    │   │
│  │         │  │         │  │  stage → tcp://10.0.0.6    │   │
│  │ Migration│  │         │  │                             │   │
│  │ Wizard  │  │         │  │  docker compose (local)     │   │
│  └─────────┘  └─────────┘  └────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘
          │                     │                     │
     unix socket           tcp :2375            tcp :2375
          │                     │                     │
     ┌────┴────┐          ┌─────┴─────┐        ┌─────┴─────┐
     │  Host A │          │  Host B   │        │  Host C   │
     │ (local) │          │socket-proxy│       │socket-proxy│
     │  docker │          │  docker    │       │  docker    │
     └─────────┘          └───────────┘        └───────────┘
```

### Remote host setup (admin does once)

```bash
# On each remote host — one command
docker run -d --name doll-proxy \
  --network doll-net \
  -v /var/run/docker.sock:/var/run/docker.sock:ro \
  -e ALLOW_CONTAINERS=true \
  -e ALLOW_START=true \
  -e ALLOW_STOP=true \
  -e ALLOW_RESTARTS=true \
  -e ALLOW_CREATE=true \
  -e ALLOW_DELETE=true \
  -e ALLOW_IMAGES=true \
  -e ALLOW_INFO=true \
  -e ALLOW_EVENTS=true \
  -e ALLOW_LOGS=true \
  -e ALLOW_EXEC=true \
  -e ALLOW_NETWORKS=true \
  -e ALLOW_VOLUMES=true \
  -e ALLOW_SECRETS=false \
  -e ALLOW_CONFIGS=false \
  tecnativa/docker-socket-proxy
```

### Endpoint management (Rust core)

```
GET    /endpoints                     — list all
POST   /endpoints                     — add { name, connection, tags }
GET    /endpoints/:id                 — detail + status
PATCH  /endpoints/:id                 — update connection/details
DELETE /endpoints/:id                 — remove (disconnect)
POST   /endpoints/:id/test            — test connectivity
POST   /endpoints/:id/reconnect       — reconnect
```

### Frontend: Endpoint management page

Table: name, connection string, status indicator (● connected / ◌ disconnected / ✕ error), container count, tags. Add button opens modal with name + connection + tags. Test button verifies connectivity before saving.

### Frontend: Host switcher

Small dropdown in sidebar header showing current host. Switchable at any time. All pages re-fetch data for selected endpoint. URL param `?endpoint=prod` persists.

---

## Migration Wizard — Detailed Design

### Step 1: Select

Admin selects container(s) on source host, clicks "Migrate." If container is part of a compose stack, doll detects the label and suggests migrating the entire stack instead.

### Step 2: Analyze

Doll inspects the container and presents findings:

```
Container: hermes-app (host-a, running)
Image: hermes:latest
Stack: hermes-web (3 services) — migrate entire stack recommended

Volumes (3):
  hermes_config    local    12MB    shared: no   → transfer
  hermes_data      local    2.3GB   shared: no   → transfer
  /opt/nginx/conf  bind     4KB     host path    → needs review

Database connections (2):
  DB_HOST=postgres  → container 'postgres' on host-a  ⚠ will break
  REDIS_URL=redis://redis:6379  → container 'redis' on host-a  ⚠ will break

Network: hermes-net (bridge)
Ports: 80:80/tcp
```

### Step 3: Strategy

Doll recommends a strategy based on what it found. Admin can override:

| Detected condition | Default strategy |
|-------------------|-----------------|
| Part of compose stack | Stack migration (migrate all services together) |
| Named volumes only, no stack | Full snapshot (stop → export → transfer → start) |
| Has bind mounts | Recreate from compose (bind paths need manual review) |
| Has external volume driver | Recreate (reconnect driver on target) |
| Has database dependency | Stack migration (migrate DB together) or connection review |
| No volumes (stateless) | Recreate (just pull image and run) |

Admin can select:
- **Stack migration** — migrate all services in the compose stack
- **Full snapshot** — stop, export volumes, transfer, start
- **Recreate from compose** — generate compose file, move volumes, pull image, start
- **Clone + decommission** — create copy on target, switch traffic, remove source (needs LB)
- **CRIU checkpoint** (experimental) — freeze memory state, transfer, restore

### Step 4: Volume sync configuration

For each volume, admin can override the default plan:

```
Volume: hermes_data (2.3GB)
  Driver: local → will be transferred
  Transfer method: [SCP ▼]  (SCP | Rsync | Intermediate via S3/NFS)
  Timing: [After stop ▼]  (After stop | Pre-sync + delta | Live — CRIU only)
  Compression: [gzip -6 ▼]  (gzip -1 to -9 | none)
  Exclude: [________________] (e.g., *.log, cache/)
  Post-transfer hook: [________________] (script to run after data lands)

Volume: /opt/nginx/conf (bind mount)
  Source path: /opt/nginx/conf
  On target: [Create if missing ▼] (Create | Exists already | Skip creation)
  Target path: [/opt/nginx/conf________] (override)
  Transfer contents: [Yes ▼] (Yes | No — I will handle this)
```

### Step 5: Connection review

```
Database: PostgreSQL
  Source: DB_HOST=postgres (env var)
  Current resolution: container 'postgres' on host-a:5432
  After migration: will still point to host-a ⚠

  Fix: [Migrate DB together ▼]
       ├ Migrate DB together (recommended — keeps docker networking)
       ├ Replace with host-a:5432 (DB stays on source)
       ├ Replace with custom hostname: [________________]
       └ Leave as-is (I will handle manually)

Database: Redis
  Source: REDIS_URL=redis://user:••••••@redis:6379/0
  Password masked. [reveal]

  Fix: [Migrate together ▼]
```

All detected connections must be reviewed before proceeding. Cannot proceed with unresolved ⚠ connections.

### Step 6: Target selection

```
Target host: [host-b ▼]  (host-b | host-c | host-d)
  Available: 12GB disk, 4 containers running, Docker 29.5

Target stack name (if stack migration): [hermes-web__________]

Post-migration:
  [✓] Start container on target
  [ ] Verify database connectivity
  [ ] Remove from source after successful migration
  [ ] Rotate credentials after migration
```

### Step 7: Dry run

Simulates everything without touching containers. Shows exact commands that will run:

```
Dry run — these commands will execute:

[host-a] docker stop hermes-app
[host-a] docker run --rm -v hermes_config:/data -v /tmp/doll:/out alpine tar czf /out/hermes_config.tar.gz -C /data .
[host-a] docker run --rm -v hermes_data:/data -v /tmp/doll:/out alpine tar czf /out/hermes_data.tar.gz -C /data .
[host-a→host-b] scp /tmp/doll/*.tar.gz user@host-b:/tmp/doll/
[host-b] docker volume create hermes_config
[host-b] docker run --rm -v hermes_config:/data -v /tmp/doll:/in alpine tar xzf /in/hermes_config.tar.gz -C /data
[host-b] docker volume create hermes_data
[host-b] docker run --rm -v hermes_data:/data -v /tmp/doll:/in alpine tar xzf /in/hermes_data.tar.gz -C /data
[host-b] docker compose -f /tmp/doll/hermes-web.yml up -d
[host-b] docker exec hermes-app pg_isready -h postgres -U hermes

Warning: Transfer includes database credentials in env vars. SSH transfer is encrypted.
         Delete compose file from /tmp/doll/ on both hosts after migration.
```

### Step 8: Execute + progress

```
Migration in progress:
  ████████████████░░░░░░  78%  Transferring hermes_data (1.8GB / 2.3GB)

  ✓ Stop hermes-app
  ✓ Export hermes_config (12MB)
  ▸ Export hermes_data (2.3GB)
  ○ Transfer hermes_config (12MB)
  ○ Transfer hermes_data (2.3GB)
  ○ Import volumes on host-b
  ○ Start container
  ○ Verify connectivity
```

### Step 9: Verification

```
Migration complete in 4m 12s

  ✓ Stop hermes-app
  ✓ Export volumes (2.3GB)
  ✓ Transfer to host-b
  ✓ Import volumes
  ✓ Start container
  ✓ Connectivity test
  ✗ PostgreSQL connectivity — pg_isready failed. Check connection string.
  ✓ Post-migration cleanup

Post-migration: [Remove from source] [Restart on source (rollback)] [Done]
```

### Migration edge cases

| Situation | Handling |
|-----------|----------|
| **Container is running** | Stop first (cold). CRIU option for experimental live migration. |
| **Volume shared by multiple containers** | Detect and warn. Must migrate all or detach first. |
| **Volume is 500GB** | Pre-flight: check target disk space. Estimate transfer time. Offer cancel. |
| **Target disk full** | Refuse migration. Show: "Need 2.3GB, only 412MB available." |
| **Same volume name exists on target** | Prompt: rename, overwrite, or skip. |
| **Bind mount is a file not directory** | Detect. Transfer as file, not directory tree. |
| **Bind mount is kernel path (/proc, /sys, /var/run)** | Auto-skip. Never migrate. |
| **Bind mount is relative path** | Warn: cannot resolve. Admin must provide absolute path. |
| **Volume plugin mismatch** | Warn: driver not available on target. Offer fallback to local. |
| **Network name collision** | Auto-rename network on target with suffix. |
| **SSH key not configured** | Offer Option C: doll generates commands, admin runs them manually. |
| **Compromised source** | Admin initiated migration for incident response. Doll adds audit flag. |

---

## Phase 3: Swarm Management

### Endpoints

```
POST   /swarm/init                    — docker swarm init
POST   /swarm/join                    — docker swarm join { token, remote_addrs }
POST   /swarm/leave                   — docker swarm leave (?force=true)
GET    /swarm                         — swarm inspect
GET    /swarm/nodes                   — node list
GET    /swarm/nodes/:id               — node inspect
PATCH  /swarm/nodes/:id               — node update (availability, role, labels)
DELETE /swarm/nodes/:id               — node remove
GET    /swarm/services                — service list
GET    /swarm/services/:id            — service inspect
POST   /swarm/services/create         — service create
PATCH  /swarm/services/:id            — service update
DELETE /swarm/services/:id            — service remove
GET    /swarm/services/:id/logs       — service logs
POST   /swarm/services/:id/rollback   — rollback
GET    /swarm/tasks                   — task list (filter by service)
GET    /swarm/tasks/:id               — task inspect
GET    /swarm/tasks/:id/logs          — task logs
GET    /swarm/secrets                 — secret list
POST   /swarm/secrets/create          — create
DELETE /swarm/secrets/:id             — remove
GET    /swarm/configs                 — config list
POST   /swarm/configs/create          — create
DELETE /swarm/configs/:id             — remove
```

### Frontend: Swarm page

Three tabs: Nodes | Services | Secrets & Configs.

**Nodes:** Table with node hostname, role (manager/worker), status, availability, engine version, CPU/RAM. Action bar: promote/demote, drain, availability toggle, remove.

**Services:** Table with name, image, replicas (running/total), ports, mode. Click → detail: tasks list, logs, config. Action bar: scale (replicas), update (image/tag), rollback, remove.

**Visualizer:** SVG diagram of Swarm topology — nodes as boxes, services distributed across nodes. Managers highlighted.

---

## Phase 4: Nginx Load Balancing

### Design

Doll manages Nginx upstream configurations based on container labels:

```yaml
# docker-compose.yml
services:
  myapp:
    image: myapp:latest
    labels:
      doll.lb.enabled: "true"
      doll.lb.domain: "myapp.example.com"
      doll.lb.port: "3000"
      doll.lb.path: "/"
      doll.lb.ssl: "true"
      doll.lb.weight: "5"
    deploy:
      replicas: 3
```

Doll watches for containers with `doll.lb.*` labels, aggregates by `domain` + `path`, generates nginx upstream configs.

### Nginx config generation

```nginx
# Generated by doll — /etc/nginx/upstreams/myapp.conf
upstream doll_myapp {
    # container: myapp_1 (host-a:3000) — healthy
    server 10.0.0.5:3000 weight=5 max_fails=3 fail_timeout=30s;
    # container: myapp_2 (host-b:3000) — healthy
    server 10.0.0.6:3000 weight=5 max_fails=3 fail_timeout=30s;
    # container: myapp_3 (host-a:3001) — unhealthy, removed
}

server {
    listen 443 ssl;
    server_name myapp.example.com;

    location / {
        proxy_pass http://doll_myapp;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_read_timeout 60s;
    }
}
```

### Workflow

```
1. Container starts with doll.lb.* labels
2. Doll detects via Docker events
3. Doll checks: is there already an upstream for this domain+path?
   - No: create new upstream config + server block
   - Yes: add/update this server entry
4. Doll runs nginx -t (config check)
5. If valid: nginx -s reload (zero-downtime)
6. Container stops/removed → remove from upstream → regenerate → reload
```

### Health checks

Doll periodically health-checks upstream servers. If a container fails N times, it's temporarily removed from the upstream (commented out). If it recovers, it's reinstated. This is handled in the nginx config itself with `max_fails` + `fail_timeout` — no doll intervention needed for basic health checks.

### Nginx page in doll

Shows: all managed domains, upstream servers (host + port + status), last config generation. Action: force regenerate, test config, reload nginx, view rendered config.

---

## Security Model

### Access key (Phase 1)

- `DOLL_KEY` env var → mandatory in production
- Gateway checks `X-Doll-Key` on every `/api/*` request
- Rate limiting: 5 failed attempts → 30s lockout
- Frontend stores key in localStorage, auto-attaches
- 401 → clear stored key, show AuthGate
- Multiple keys supported for rotation: `DOLL_KEY=key1,key2` → either works

### Credential handling (Phase 2)

| Surface | Protection |
|---------|-----------|
| Env vars in UI | Masked by default: `DB_PASSWORD=••••••••`. [reveal] button with confirmation + audit log |
| Volume driver Options | Sanitized before display. Never transmitted raw between hosts |
| Compose file during migration | Warn if contains secrets. SSH-encrypted transfer. Remind deletion post-migration |
| SSH keys for transfer | Option C (command generation) is default — doll never holds SSH keys |
| Audit log | All actions logged: who, what, when, target. In-memory (Phase 1) → SQLite (Phase 2) |

### Socket proxy security (Phase 2)

- Bind to Docker internal network only (never `0.0.0.0`)
- Granular permissions per endpoint
- Revoke: ATTACH, SESSION, SECRETS, CONFIGS by default
- If exposed to LAN/WAN → TLS required on bollard connection

### Transport security

| Transfer | Encryption | doll's role |
|----------|:----------:|-------------|
| SCP | ✅ SSH | Orchestrates, never sees data |
| Rsync over SSH | ✅ SSH | Orchestrates, never sees data |
| Rsync daemon | ❌ None | **Warns admin, recommends SSH** |
| Export to S3 → import | ✅ TLS | Orchestrates, S3 creds in target env only |
| CRIU checkpoint | ✅ SSH | Orchestrates, checkpoint files via SCP |

---

## Performance Optimizations

### Phase 1 — Must-do (before ship)

#### P1: Parallel Docker API calls in Rust

All dashboard/stats handlers use `tokio::join!` to fetch multiple resources in parallel:

```rust
async fn dashboard_data(state: &AppState, endpoint: &str) -> Result<Dashboard, AppError> {
    let docker = state.get_client(endpoint)?;
    let (info, containers, images, volumes, networks) = tokio::join!(
        docker.info(),
        docker.list_containers::<String>(default_opts()),
        docker.list_images::<String>(default_opts()),
        docker.list_volumes::<String>(default_opts()),
        docker.list_networks::<String>(default_opts()),
    );
    Ok(Dashboard { info: info?, containers: containers?, images: images?, volumes: volumes?, networks: networks? })
}
```

Impact: 3-5x faster dashboard load.

#### P2: Volume size caching

Volume size computation via temp container costs ~500ms per volume. Cache results:

```rust
use moka::sync::Cache;
use std::time::Duration;

struct CacheLayer {
    volume_sizes: Cache<String, VolumeSize>,
    container_list: Cache<String, Vec<ContainerSummary>>,
    system_info: Cache<(), SystemInfo>,
}

impl CacheLayer {
    fn new() -> Self {
        Self {
            volume_sizes: Cache::builder()
                .time_to_live(Duration::from_secs(120))
                .build(),
            container_list: Cache::builder()
                .time_to_live(Duration::from_secs(5))
                .build(),
            system_info: Cache::builder()
                .time_to_live(Duration::from_secs(60))
                .build(),
        }
    }
}
```

Invalidate cache on relevant Docker events (container create/destroy, volume create/remove). Add `doll-size-cache` crate dependency.

#### P3: Virtual scrolling for tables

When container/image/volume lists exceed ~100 items, render only visible rows. Add `@tanstack/react-virtual` (~5KB gzipped):

```jsx
import { useVirtualizer } from '@tanstack/react-virtual';

function ContainerTable({ containers }) {
  const parentRef = useRef();
  const virtualizer = useVirtualizer({
    count: containers.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 40,  // row height
    overscan: 5,
  });

  return (
    <div ref={parentRef} style={{ height: '600px', overflow: 'auto' }}>
      <div style={{ height: virtualizer.getTotalSize() }}>
        {virtualizer.getVirtualItems().map(vRow => (
          <Row key={vRow.key} container={containers[vRow.index]} ... />
        ))}
      </div>
    </div>
  );
}
```

Impact: Smooth scrolling with 500+ containers. No DOM bloat.

#### P4: Log scrollback cap

Frontend log viewer caps at 10,000 lines. Oldest lines auto-discarded. Prevents browser memory leaks when tailing verbose containers for hours.

```javascript
const MAX_LINES = 10_000;
function addLine(lines, newLine) {
  const updated = [...lines, newLine];
  if (updated.length > MAX_LINES) {
    return updated.slice(updated.length - MAX_LINES);
  }
  return updated;
}
```

#### P5: Endpoint connection timeout

TCP connection to Docker socket/proxy must time out fast:

```rust
use tokio::time::timeout;

async fn get_client_timeout(state: &AppState, endpoint_id: &str) -> Result<Docker, AppError> {
    let clients = state.clients.read().await;
    let docker = clients.get(endpoint_id)
        .ok_or(AppError::EndpointNotFound(endpoint_id.into()))?;

    match timeout(Duration::from_secs(5), docker.ping()).await {
        Ok(Ok(_)) => Ok(docker.clone()),
        Ok(Err(e)) => Err(AppError::DockerUnreachable(e.to_string())),
        Err(_) => Err(AppError::DockerUnreachable("timeout after 5s".into())),
    }
}
```

Impact: Instant failure instead of 60-second TCP hang.

### Phase 2 — Efficiency upgrades

#### P6: Pipe-direct migration (no intermediate files)

Instead of export-to-file → SCP → import-from-file, pipe directly:

```bash
# Source host → target host in one pipeline
docker run --rm -v hermes_data:/data alpine sh -c \
  'apk add pigz >/dev/null 2>&1 && tar -I pigz -cf - -C /data .' \
  | ssh target 'docker run --rm -i -v hermes_data:/data alpine sh -c \
    "apk add pigz >/dev/null 2>&1 && tar -I pigz -xf - -C /data"'
```

Impact: 2x faster for large volumes. No intermediate disk I/O. No double write.

#### P7: Compression options

| Method | Speed (5GB) | Compression | CPU |
|--------|------------|-------------|-----|
| gzip (default) | ~120s | ~50% | 1 core |
| pigz (parallel gzip) | ~60s | ~50% | 4 cores |
| zstd -3 | ~45s | ~55% | 4 cores |
| zstd -1 (fast) | ~30s | ~40% | 4 cores |
| lz4 (fastest) | ~15s | ~20% | 4 cores |

Admin selects compression level in migration UI. Default: pigz (balanced).

#### P8: Parallel volume transfer

Multiple volumes transfer simultaneously via `tokio::spawn` per volume:

```rust
let handles: Vec<_> = volumes.iter().map(|vol| {
    let state = state.clone();
    let volume = vol.clone();
    tokio::spawn(async move {
        transfer_volume(&state, &volume).await
    })
}).collect();

for handle in handles {
    handle.await??;
}
```

Progress bar aggregates: "Volume 1: 78%, Volume 2: 45%, Volume 3: 12%."

#### P9: Event-driven updates (SSE) — eliminate polling

Replace 5-second polling with Docker events:

```
Frontend opens SSE → /api/system/events?endpoint=prod
Docker events stream → doll filters → pushes to frontend:
  "container:start:hermes-app" → frontend calls api.listContainers()
  "container:die:hermes-app"   → frontend calls api.listContainers()
  "image:pull:hermes:latest"   → frontend calls api.listImages()
  "volume:create:newvol"       → frontend calls api.listVolumes()
```

Impact: Zero polling. Data refreshes exactly when it changes. Thousands of unnecessary API calls eliminated per hour.

#### P10: WebSocket log fan-out

One Docker log stream → broadcast to N WebSocket viewers:

```rust
use tokio::sync::broadcast;

struct LogBroadcaster {
    senders: RwLock<HashMap<String, broadcast::Sender<String>>>,
}

impl LogBroadcaster {
    async fn subscribe(&self, container_id: &str, docker: &Docker) -> broadcast::Receiver<String> {
        let mut senders = self.senders.write().await;
        if let Some(sender) = senders.get(container_id) {
            return sender.subscribe();
        }
        let (tx, _) = broadcast::channel(1024);
        senders.insert(container_id.to_string(), tx.clone());

        let cid = container_id.to_string();
        let d = docker.clone();
        tokio::spawn(async move {
            let mut stream = d.logs(&cid, Some(LogsOptions { follow: true, ..default() }));
            while let Some(Ok(log)) = stream.next().await {
                if tx.send(serde_json::to_string(&log).unwrap()).is_err() {
                    break; // Last subscriber dropped
                }
            }
            senders.write().await.remove(&cid);
        });

        tx.subscribe()
    }
}
```

Impact: 3 admins watching same container = 1 Docker stream, not 3. N-fold reduction.

### Phase 3

#### P11: Batch stats endpoint

Single WebSocket multiplexing stats for multiple containers:

```
WS /containers/stats/batch?ids=abc,def,ghi
→ { container: "abc", cpu: 2.1, mem: 128 }
→ { container: "def", cpu: 0.3, mem: 256 }
→ { container: "ghi", cpu: 5.0, mem: 512 }
```

Dashboard uses one WS instead of N. Impact: 10 containers = 1 connection instead of 10.

### Performance budget

| Metric | Target | Enforced by |
|--------|--------|-------------|
| Dashboard load | < 500ms (cold), < 100ms (cached) | `tokio::join!` + caching |
| Container list render | < 100ms for 500 items | Virtual scrolling |
| Log streaming latency | < 1s from Docker to browser | WS fan-out |
| Migration throughput | > 80MB/s per volume | Pipe-direct + pigz |
| Memory (doll idle) | < 100MB | Rust + Node (measured) |
| CPU (doll idle) | < 1% | Tokio async, minimal polling |
| Bundle size (first load) | < 150KB gzipped | Code splitting per page |

---

## Color Themes

| Token | Dark | Light | Sepia |
|-------|------|-------|-------|
| bg-primary | #0d1117 | #ffffff | #fdf6e3 |
| bg-secondary | #161b22 | #f6f8fa | #eee8d5 |
| bg-tertiary | #21262d | #eaeef2 | #e0dcc8 |
| border | #30363d | #d0d7de | #d3cbb8 |
| text-primary | #e6edf3 | #1f2328 | #586e75 |
| text-secondary | #8b949e | #656d76 | #839496 |
| accent | #58a6ff | #0969da | #268bd2 |
| green | #3fb950 | #1a7f37 | #859900 |
| yellow | #d29922 | #9a6700 | #b58900 |
| red | #f85149 | #cf222e | #dc322f |
| font-ui | Inter, system | Inter, system | Inter, system |
| font-mono | JetBrains Mono | JetBrains Mono | JetBrains Mono |

---

## Verification Checklist — Full Platform

### Phase 1
- [ ] `curl localhost:9119/health` → "ok"
- [ ] `curl -H "x-doll-key: test" localhost:8000/api/containers` → 200 + JSON
- [ ] `curl localhost:8000/api/containers` → 401
- [ ] Containers: list, start, stop, restart, kill, pause, unpause, remove, rename
- [ ] Container detail: 6 tabs all render correctly
- [ ] Live log streaming (WebSocket)
- [ ] Live stats streaming (WebSocket)
- [ ] Volume deep inspection: driver, size, usage, sanitized options
- [ ] Images: list, pull, remove, inspect, history
- [ ] Volumes: list, create, remove, prune
- [ ] Networks: list, create, remove, connect, disconnect, prune
- [ ] Stacks: list, create, edit YML, save, deploy, stop, down, restart, remove
- [ ] Stack deploy output streams in real time
- [ ] System: info + version + prune all types
- [ ] Themes: dark / light / sepia switch and persist
- [ ] Secrets masking in UI (env vars, volume options)
- [ ] `docker compose up` → everything works on :8000
- [ ] Multi-arch: image builds for amd64 + arm64

### Phase 2
- [ ] Add remote endpoint (via socket proxy)
- [ ] Test endpoint connectivity
- [ ] Host switcher changes all data
- [ ] Migration wizard: all 9 steps work
- [ ] Volume sync overrides: custom paths, transfer methods
- [ ] Connection review: detect and fix DB connections
- [ ] Migration dry run shows correct commands
- [ ] Migration execute: stop → export → transfer → import → start
- [ ] Post-migration verification
- [ ] Rollback: restart on source
- [ ] Audit log entries for all migration actions

### Phase 3
- [ ] Swarm init/join/leave
- [ ] Node list + management
- [ ] Service create/list/update/scale/rollback
- [ ] Secret + config management
- [ ] Swarm visualizer

### Phase 4
- [ ] Nginx config generation from labels
- [ ] Upstream updates on container start/stop
- [ ] Zero-downtime nginx reload
- [ ] Health check integration
