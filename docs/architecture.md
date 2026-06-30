# Architecture

Marionette is a self-contained Docker container that manages Docker infrastructure. Three coordinated processes run inside a single container, connected via a TypeScript gateway.

## Process Model

| Process | Language | Role |
|---------|----------|------|
| **Core** | Rust (Axum) | Docker operations via bollard, SQLite state, API handlers |
| **Gateway** | TypeScript (Fastify) | Authentication, API routing, static SPA serving, HTTP→HTTPS redirect |
| **Frontend** | React (Vite) | Single-page application served as static assets |

The gateway is the only process exposed to the network. It handles auth (X-Marionette-Key header), proxies `/api/*` requests to the Rust core on an internal port, and serves the React SPA for all other routes. WebSocket connections bypass auth (browsers can't set custom headers on WS upgrades).

## Data Flow

```
Browser → HTTPS :8443 → Gateway (Fastify)
                           ├── /api/* → Core (Rust/Axum) → Docker API (bollard)
                           ├── /ws/*  → Core (WebSocket upgrade)
                           └── /*     → React SPA (static files)

HTTP  :8000 → 301 redirect → HTTPS :8443
```

## State Management

All persistent state lives in a single SQLite database (WAL mode, foreign keys enabled):

- **Endpoints** — Docker host connections (URI, TLS cert path, status)
- **Users** — API keys and roles (admin, operator, viewer)
- **Routes** — Reverse proxy route table for AuxGate integration
- **Audit Log** — All mutating actions with timestamps

Docker clients are lazy-connected and cached per endpoint. The endpoint registry is the single source of truth — on startup, endpoints load from SQLite and clients reconnect using stored connection strings and TLS certificate paths.

## Multi-Host Support

Marionette connects to Docker hosts via:

| Method | Connection String | Auth |
|--------|------------------|------|
| Local socket | `unix:///var/run/docker.sock` | None |
| Plain TCP | `tcp://host:2375` | None (use socket proxy) |
| TLS | `https://host:2376` | Client certs (ca.pem, cert.pem, key.pem) |

Each endpoint has its own TLS certificate directory, replacing the single global `DOCKER_CERT_PATH` env var. The built-in setup script generator creates certificates, configures the Docker daemon for TLS, handles systemd conflicts, and opens firewall ports.

## Migration Engine

Marionette has two migration modes:

**Container Wizard (9-step):** Analyzes a running container — enumerates volumes, detects database connections, generates transfer commands with compression and transfer-method branching. Designed for cold migration (container stopped during transfer).

**Compose-Template (latch-pipe):** Diff source↔target compose files, pre-creates volumes and pulls images on target, pipes volumes through the Docker API via ephemeral alpine containers (no SSH, no temp files). Switchover engine with health-check polling and automatic rollback.

## Frontend

React SPA built with Vite, using Pico CSS as the design foundation. Key patterns:

- **SortableTable** — shared component for all list pages with inline selection, sorting, and compact single-line rows
- **FilterBar + useFilters** — client-side text search and state filtering across all list pages
- **EndpointSwitcher** — sidebar dropdown for switching between Docker hosts; dispatches `endpoint:changed` event for all pages to reload
- **Modal** — reusable with `size="large"` for YAML editors and log viewers
- **WebSocket** — live container logs, stats streaming with history charts (Recharts), and terminal shell access (xterm.js)

## Gateway Auth

The gateway's auth middleware checks the `X-Marionette-Key` header against SHA-256 hashed keys in the SQLite users table. WebSocket upgrade requests bypass auth (browsers cannot set custom headers on WebSocket connections). On first run, the `MARIONETTE_KEY` env var seeds an admin user.

## Companion Projects

Marionette manages infrastructure; companion projects provide the infrastructure. They are standalone Docker images usable without Marionette. Marionette adds management UI via shared volume config — zero runtime coupling.

| Project | Role |
|---------|------|
| **AuxGate** | Nginx reverse proxy with TLS termination, API key auth, rate limiting |
| **Router** | Network container (nftables + dnsmasq + WireGuard) for port forwarding, firewall, NAT |
| **MQTT Manager** | Mosquitto broker with web GUI for managing users, ACLs, and topic inspection |

## Resilience

- **Maintenance Overlay** — client-side detection polls `/health` every 5 seconds; after 2 consecutive failures shows fullscreen overlay with outage timer and retry button
- **Auto-TLS** — self-signed certificate auto-generated on first run, persisted across restarts via mounted volume
- **HTTP→HTTPS redirect** — plain HTTP on port 8000 returns 301 redirect to HTTPS, preserving the request path
