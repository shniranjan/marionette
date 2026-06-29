# Marionette — Docker Infrastructure Management Platform

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](LICENSE)
[![Build](https://github.com/shniranjan/marionette/actions/workflows/ci.yml/badge.svg)](https://github.com/shniranjan/marionette/actions/workflows/ci.yml)

A centralized Docker infrastructure management platform. Manage containers, images, volumes, networks, stacks, and Swarm clusters across multiple hosts — from a single, minimal web UI. Includes the only guided container migration wizard in any Docker management tool.

---

## Quick Start

```bash
# Pull and run
docker compose up -d

# Or build from source
git clone https://github.com/shniranjan/marionette.git
cd marionette
docker compose -f docker-compose.yml up -d --build

# HTTPS on port 8443 (self-signed cert on first run)
# HTTP on port 8000 → redirects to HTTPS automatically
# Open https://localhost:8443 (or https://<your-server-ip>:8443)
```

> **TLS is automatic.** On first start, Marionette generates a self-signed certificate. Your browser will show a warning — accept it on LAN. Mount your own cert at `./certs/` to override.

With docker-compose:

```yaml
services:
  marionette:
    image: ghcr.io/shniranjan/marionette:v0.4.0
    container_name: marionette
    ports:
      - "8000:8000"     # HTTP → 301 redirect to HTTPS
      - "8443:8443"     # HTTPS app
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
      - ./stacks:/stacks
      - ./data:/data
      - ./certs:/app/certs               # TLS cert persistence + endpoint client certs
    environment:
      - MARIONETTE_KEY=${MARIONETTE_KEY:-}
    restart: unless-stopped
```

For multi-host and advanced setup, see [Quickstart Guide](docs/quickstart.md).

---

## Features

| Module | Capabilities |
|--------|-------------|
| **Dashboard** | Container counts, resource usage, system info, quick actions |
| **Containers** | List with selection toolbar (start/stop/restart/remove), inspect, rename. Live logs and stats streaming via WebSocket (CPU/Memory/Network history charts). 6-tab detail view. |
| **Images** | List with selection toolbar, pull (with progress), inspect, remove, layer history |
| **Volumes** | List with selection toolbar, create, remove, prune, deep inspection (driver, size, usage, file count) |
| **Networks** | List with selection toolbar, create, remove, connect/disconnect containers, prune |
| **Stacks** | List compose stacks, edit YAML (CodeMirror), save-only or save & deploy. Detects running/stopped status. Supports both `docker-compose.yml` and `compose.yml`. |
| **Endpoints** | Connect multiple Docker hosts (unix, TCP, TLS). Host switcher in sidebar. Per-endpoint TLS certificate paths. Connection testing. Setup-script generator with auto firewall detection (ufw/firewalld) and systemd integration. |
| **Swarm** | Nodes, services, tasks, secrets, configs. Init/join/leave. Scale and update services. |
| **Nginx LB** | Label-driven upstream config generation (`marionette.lb.*`). Regenerate, test, and reload nginx config from the UI. |
| **Migration** | 9-step guided wizard. Cold migration with volume sync. Database connection review. Dry run. Command-only (no SSH keys stored). |
| **System** | Docker info, version, events stream, prune all resource types, audit log |
| **Auth** | Access key authentication. Multiple key support. Dev mode available. |
| **Design** | Pico CSS foundation. 6 color palettes × 3 modes (18 visual variants). Dark/Light/Sepia × Blue/Slate/Amber/Green/Violet/Rose. |
| **TLS** | Auto-generated self-signed certificate on first run. Persists across restarts. HTTP :8000 → 301 redirect to HTTPS :8443. Overridable with your own cert. |
| **Resilience** | Maintenance overlay detects server downtime, shows timer, auto-reconnects when back online. |

---

## Roadmap

### In progress (Marionette core)

- **Route management** — CRUD route table for the AuxGate reverse proxy. Auto-discover containers as targets. Per-route authentication keys.
- **Role-based access** — Admin, operator, and viewer roles. Per-user API keys with granular permissions.

### Planned (standalone companion projects)

| Project | Description | Status |
|---------|-------------|--------|
| **AuxGate** | Nginx-based HTTP reverse proxy. TLS termination, API key auth, rate limiting, IP whitelist. Single-container sidecar. Config via env vars. Works standalone or managed by Marionette. | Designed |
| **Router** | Network-layer container (nftables + dnsmasq + WireGuard). Port forwarding, firewall, NAT, DNS, DHCP, VPN. Managed from Marionette UI. | Designed |
| **MQTT Manager** | Mosquitto broker with web GUI. Deploy, manage users/ACLs, live topic tree, message inspector. | Designed |

> **Design principle:** Companion projects are standalone Docker images usable without Marionette. Marionette adds management UI on top — zero runtime coupling via shared volume config.

---

## Architecture

**Tech Stack:** Rust + Node.js + React + SQLite

Marionette is a self-contained Docker container with three coordinated processes: a Rust backend for Docker operations, a TypeScript gateway for auth and API routing, and a React SPA frontend. All state is persisted in SQLite.

Marionette manages containers and writes route config. AuxGate (separate container) reads config and proxies traffic. Zero runtime coupling — Marionette can be down, AuxGate still routes. AuxGate can be deployed standalone without Marionette.

---

## Documentation

| Document | Contents |
|----------|----------|
| [Quickstart Guide](docs/quickstart.md) | Step-by-step from zero to working marionette. Local + multi-host. |
| [Architecture](docs/architecture.md) | Data flow, component design, design decisions, tradeoffs |
| [Security](docs/security.md) | Threat model, credential handling, best practices, audit logging |
| [User Manual](docs/user-manual.md) | Every page and feature documented with screenshots |
| [API Reference](docs/api-reference.md) | All Rust core endpoints with request/response examples |
| [Tutorial](docs/tutorial.md) | Guided walkthroughs for common workflows |
| [FAQ](docs/faq.md) | Common questions and answers |
| [Troubleshooting](docs/troubleshooting.md) | Error messages, causes, and fixes |
| [Contributing](docs/contributing.md) | Dev setup, conventions, PR process |

---

## TLS Configuration

Marionette auto-generates a self-signed TLS certificate on first startup. Two ports are exposed:

| Port | Protocol | Behavior |
|------|----------|----------|
| 8000 | HTTP | 301 redirect to `https://<host>:8443` |
| 8443 | HTTPS | The application (Fastify + React SPA) |

### Using your own certificate

Mount your cert files to `/app/certs/` and set the env vars:

```yaml
volumes:
  - ./certs:/app/certs              # certs persisted across restarts
environment:
  - TLS_KEY=/app/certs/privkey.pem  # path inside container
  - TLS_CERT=/app/certs/fullchain.pem
```

| Env Var | Default | Description |
|---------|---------|-------------|
| `TLS_KEY` | auto-generated | Path to TLS private key (PEM) |
| `TLS_CERT` | auto-generated | Path to TLS certificate (PEM) |
| `TLS_CERT_DIR` | `/app/certs` | Where auto-generated cert is stored |

### Behavior

1. If `TLS_KEY` and `TLS_CERT` are set and files exist → use them
2. If `./certs` is mounted and contains a previously generated cert → reuse it
3. Otherwise → auto-generate a self-signed cert on first run

The cert persists across restarts when `./certs:/app/certs` is mounted. Remove the volume to regenerate.

---

## Configuration

| Env Var | Required | Default | Description |
|---------|:--------:|---------|-------------|
| `MARIONETTE_KEY` | Production | — | Access key for web UI. Empty = no auth (dev only). Multiple keys: `key1,key2` |
| `MARIONETTE_STACKS_DIR` | No | `/stacks` | Directory for docker-compose stack files |
| `MARIONETTE_DB_PATH` | No | `/data/marionette.db` | SQLite database path (endpoints, users, routes, audit log) |
| `MARIONETTE_LOG_LEVEL` | No | `info` | Log level: trace, debug, info, warn, error |

---

## Remote Docker Setup

Connect to remote Docker hosts via TLS. Use the built-in setup script generator (Endpoints page → 🔧 Setup Script) which:

1. Generates CA + server + client certificates
2. Configures Docker daemon for TLS (`daemon.json`)
3. Handles systemd `-H fd://` conflicts (auto drop-in override)
4. Opens firewall port (auto-detects ufw or firewalld)
5. Copies client certs with correct permissions

Each endpoint can have its own TLS certificate path (`certPath` field), replacing the single global `DOCKER_CERT_PATH` env var.

---

## Security

- **TLS by default:** Auto-generated self-signed certificate on first run. HTTP → HTTPS redirect. Override with your own cert via `TLS_KEY`/`TLS_CERT` env vars.
- **Access Key:** All API requests require authentication when `MARIONETTE_KEY` is set.
- **Credential Masking:** Environment variables and volume driver options are masked by default in the UI
- **Socket Proxy:** Remote hosts use `tecnativa/docker-socket-proxy` with granular API permissions
- **Audit Log:** All mutating actions are logged with timestamp, admin key hash, and target
- **No SSH Keys Stored:** Migration transfer uses command generation — marionette never holds SSH credentials
- **Maintenance Overlay:** Client-side detection of server downtime with auto-reconnect

See [Security](docs/security.md) for the full threat model and mitigations.

---

## Disclaimer

**This software is provided "as is", without warranty of any kind.** Marionette interacts directly with the Docker daemon and can start, stop, remove, and migrate containers. These are destructive operations.

- **Data loss:** Container removal, volume pruning, and migration can result in permanent data loss. Always back up before migrating.
- **Service disruption:** Stopping, restarting, or migrating containers will cause downtime. Test in staging environments first.
- **Access control:** Anyone with the `MARIONETTE_KEY` has full control over your Docker infrastructure. Use a strong key, rotate it regularly, and never expose it in client-side code or logs.
- **Migration:** The migration wizard generates shell commands for you to run manually. Review every command before executing. Marionette does not validate the safety of generated commands.
- **Remote hosts:** Connecting to remote Docker hosts via Socket Proxy extends the attack surface. Use TLS, firewalls, and granular proxy permissions.
- **No liability:** The authors and contributors are not responsible for any damage, data loss, or service disruption caused by the use of this software.

**By using marionette, you accept these risks.**

---

## License

GNU Affero General Public License v3.0 — see [LICENSE](LICENSE).

AGPL v3 ensures that modified versions of marionette offered as a network service must make their source code available. This closes the "SaaS loophole" present in permissive licenses.
