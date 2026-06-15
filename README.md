# Marionette — Docker Infrastructure Management Platform

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](LICENSE)
[![Build](https://github.com/shniranjan/marionette/actions/workflows/ci.yml/badge.svg)](https://github.com/shniranjan/marionette/actions/workflows/ci.yml)
[![Docker Pulls](https://img.shields.io/docker/pulls/shniranjan/marionette)](https://github.com/shniranjan/marionette/pkgs/container/marionette)

A centralized Docker infrastructure management platform. Manage containers, images, volumes, networks, stacks, and Swarm clusters across multiple hosts — from a single, minimal web UI. Includes the only guided container migration wizard in any Docker management tool.

---

## Why Marionette?

| You need | Marionette gives you |
|----------|---------------|
| Manage Docker across multiple hosts | Single dashboard, host switcher, no SSH required per host |
| Move containers between servers | **Guided 9-step migration wizard** — the only Docker UI that does this |
| Deploy and manage compose stacks | Built-in YML editor with syntax highlighting and one-click deploy |
| Orchestrate with Docker Swarm | Full Swarm management — nodes, services, tasks, secrets |
| Load balance across hosts | Label-driven Nginx upstream management, zero-downtime reload |
| A UI that doesn't get in your way | Dark/light/sepia themes, dense layout, keyboard shortcuts |

---

## Quick Start

```bash
# Pull the image
docker pull ghcr.io/shniranjan/marionette:latest

# Run (local Docker host only)
docker run -d --name marionette \
  -v /var/run/docker.sock:/var/run/docker.sock \
  -v /opt/stacks:/stacks \
  -e MARIONETTE_KEY=your-secret-key \
  -p 8000:8000 \
  ghcr.io/shniranjan/marionette:latest

# Open http://localhost:8000
# Enter your MARIONETTE_KEY when prompted
```

Or with docker-compose:

```yaml
services:
  marionette:
    image: ghcr.io/shniranjan/marionette:latest
    container_name: marionette
    ports:
      - "8000:8000"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
      - ./stacks:/stacks
    environment:
      - MARIONETTE_KEY=${MARIONETTE_KEY:-change-me}
    restart: unless-stopped
```

For multi-host setup, see [Quickstart Guide](docs/quickstart.md).

---

## Features

### Phase 1 — Available Now

| Module | Capabilities |
|--------|-------------|
| **Dashboard** | Container counts, resource usage, recent events, system info |
| **Containers** | List, inspect, start, stop, restart, kill, pause, unpause, remove, rename. Live logs and stats streaming. 6-tab detail view. |
| **Images** | List, pull (with progress), inspect, remove, layer history |
| **Volumes** | List, create, remove, prune, deep inspection (driver, size, usage) |
| **Networks** | List, create, remove, connect/disconnect containers, prune |
| **Stacks** | List docker-compose stacks, edit YML (CodeMirror), save, deploy, stop, down, restart |
| **System** | Docker info, version, events stream, prune all resource types |
| **Auth** | Access key (`X-Marionette-Key` header). Rate limiting. Multiple key support. |
| **Themes** | Dark, Light, Sepia — persists across sessions |

### Phase 2 — Coming Soon

- **Multi-Host:** Connect remote Docker hosts via Socket Proxy. Host switcher in UI.
- **Container Migration:** 9-step guided wizard. Cold migration with volume sync. Database connection review. Dry run. Rollback.
- **Event-Driven Updates:** SSE-based real-time refresh — zero polling.

### Phase 3 — Planned

- **Docker Swarm:** Nodes, services, tasks, secrets, configs. Init/join/leave. Visualizer.

### Phase 4 — Planned

- **Nginx Load Balancer:** Label-driven upstream config generation. Zero-downtime reload. Health check integration.

---

## Architecture

```
┌──────────────────────────────────────────────┐
│                  marionette (single container)       │
│                                               │
│  ┌───────────┐  ┌──────────┐  ┌───────────┐  │
│  │ React SPA │  │ Fastify  │  │ Rust Core │  │
│  │ (Vite)    │──│ Gateway  │──│ (Axum)    │  │
│  │           │  │ :8000    │  │ :9119     │  │
│  └───────────┘  └──────────┘  └─────┬─────┘  │
│                                      │        │
└──────────────────────────────────────┼────────┘
                                       │
                              /var/run/docker.sock
                                       │
                                Docker Daemon
```

**Tech Stack:** Rust (Axum + bollard) | Node 22 + TypeScript (Fastify) | React 19 + Vite | CodeMirror 6 | supervisord

For full architecture details, see [Architecture](docs/architecture.md).

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

## Configuration

| Env Var | Required | Default | Description |
|---------|:--------:|---------|-------------|
| `MARIONETTE_KEY` | Production | — | Access key for web UI. Empty = no auth (dev only). Multiple keys: `key1,key2` |
| `MARIONETTE_STACKS_DIR` | No | `/stacks` | Directory for docker-compose stack files |
| `MARIONETTE_LOG_LEVEL` | No | `info` | Log level: trace, debug, info, warn, error |

---

## Security

- **Access Key:** All `/api/*` requests require `X-Marionette-Key` header when `MARIONETTE_KEY` is set
- **Credential Masking:** Environment variables and volume driver options are masked by default in the UI
- **Socket Proxy:** Remote hosts use `tecnativa/docker-socket-proxy` with granular API permissions
- **Audit Log:** All mutating actions are logged with timestamp, admin key hash, and target
- **No SSH Keys Stored:** Migration transfer uses command generation — marionette never holds SSH credentials

See [Security](docs/security.md) for the full threat model and mitigations.

---

## License

GNU Affero General Public License v3.0 — see [LICENSE](LICENSE).

AGPL v3 ensures that modified versions of marionette offered as a network service must make their source code available. This closes the "SaaS loophole" present in permissive licenses.
