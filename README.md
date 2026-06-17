# Marionette — Docker Infrastructure Management Platform

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](LICENSE)
[![Build](https://github.com/shniranjan/marionette/actions/workflows/ci.yml/badge.svg)](https://github.com/shniranjan/marionette/actions/workflows/ci.yml)

A centralized Docker infrastructure management platform. Manage containers, images, volumes, networks, stacks, and Swarm clusters across multiple hosts — from a single, minimal web UI. Includes the only guided container migration wizard in any Docker management tool.

---

## Quick Start

```bash
# Clone and build
git clone https://github.com/shniranjan/marionette.git
cd marionette
docker compose up -d --build

# Open https://localhost:8000 (self-signed cert on first run)
# Or http://localhost:8000 if TLS certs aren't mounted
```

> **TLS is automatic.** On first start, Marionette generates a self-signed certificate. Your browser will show a warning — accept it on LAN. Mount your own cert at `./certs/` to override. See [TLS Configuration](#tls-configuration).

Or with docker-compose:

```yaml
services:
  marionette:
    build: .
    image: marionette:local
    container_name: marionette
    ports:
      - "8000:8000"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
      - ./stacks:/stacks
      - ./data:/data
      - ./certs:/app/certs               # TLS cert persistence
    environment:
      - MARIONETTE_KEY=${MARIONETTE_KEY:-}
      # - TLS_KEY=/path/to/your/key.pem   # optional: use your own cert
      # - TLS_CERT=/path/to/your/cert.pem
    restart: unless-stopped
```

For multi-host and advanced setup, see [Quickstart Guide](docs/quickstart.md).

---

## Features

| Module | Capabilities |
|--------|-------------|
| **Dashboard** | Container counts, resource usage, system info, quick actions |
| **Containers** | List, inspect, start/stop/restart/kill/pause/rename/remove. Live logs and stats streaming via WebSocket. 6-tab detail view (info, logs, stats, env, mounts, network). |
| **Images** | List, pull (with progress), inspect, remove, layer history |
| **Volumes** | List, create, remove, prune, deep inspection (driver, size, usage, file count) |
| **Networks** | List, create, remove, connect/disconnect containers, prune |
| **Stacks** | List compose stacks, edit YAML (CodeMirror), save, deploy, stop, down. Detects running/stopped status. Supports both `docker-compose.yml` and `compose.yml`. |
| **Endpoints** | Connect multiple Docker hosts. Host switcher in sidebar. Connection testing. |
| **Swarm** | Nodes, services, tasks, secrets, configs. Init/join/leave. Scale and update services. Visualizer. |
| **Nginx LB** | Label-driven upstream config generation (`marionette.lb.*`). Regenerate, test, and reload nginx config from the UI. |
| **Migration** | 9-step guided wizard. Cold migration with volume sync. Database connection review. Dry run. Command-only (no SSH keys stored). |
| **System** | Docker info, version, events stream (SSE), prune all resource types, audit log |
| **Auth** | Access key (`X-Marionette-Key` header). Multiple key support. Dev mode (no key required). |
| **Themes** | Dark, Light, Sepia — persists across sessions |

---

## Architecture

```
┌──────────────────────────────────────────────┐
│              marionette (single container)    │
│                                               │
│  ┌───────────┐  ┌──────────┐  ┌───────────┐  │
│  │ React SPA │  │ Fastify  │  │ Rust Core │  │
│  │ (Vite)    │──│ Gateway  │──│ (Axum)    │  │
│  │           │  │ :8000    │  │ :9119     │  │
│  └───────────┘  └──────────┘  └─────┬─────┘  │
│                                      │        │
│  ┌──────────┐                        │        │
│  │  Nginx   │                        │        │
│  │  :80/443 │                        │        │
│  └──────────┘                        │        │
└──────────────────────────────────────┼────────┘
                                       │
                              /var/run/docker.sock
                                       │
                                Docker Daemon
```

**Tech Stack:** Rust (Axum + bollard) | Node 22 + TypeScript (Fastify) | React 19 + Vite | CodeMirror 6 | Nginx | supervisord

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

## TLS Configuration

Marionette auto-generates a self-signed TLS certificate on first startup. HTTPS works immediately — your browser will show a warning; accept it for LAN use.

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

| Env Var | Required | Default | Description |
|---------|:--------:|---------|-------------|
| `MARIONETTE_KEY` | Production | — | Access key for web UI. Empty = no auth (dev only). Multiple keys: `key1,key2` |
| `MARIONETTE_STACKS_DIR` | No | `/stacks` | Directory for docker-compose stack files |
| `MARIONETTE_DB_PATH` | No | `/data/marionette.db` | SQLite database path for audit log |
| `MARIONETTE_NGINX_DIR` | No | `/etc/nginx/upstreams` | Output directory for generated nginx upstream configs |
| `MARIONETTE_LOG_LEVEL` | No | `info` | Log level: trace, debug, info, warn, error |

---

## Security

- **Access Key:** All `/api/*` requests require `X-Marionette-Key` header when `MARIONETTE_KEY` is set. WebSocket connections are exempt (browsers cannot set custom headers on WebSocket).
- **Credential Masking:** Environment variables and volume driver options are masked by default in the UI
- **Socket Proxy:** Remote hosts use `tecnativa/docker-socket-proxy` with granular API permissions
- **Audit Log:** All mutating actions are logged with timestamp, admin key hash, and target
- **No SSH Keys Stored:** Migration transfer uses command generation — marionette never holds SSH credentials

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
