# FAQ — Frequently Asked Questions

---

## General

### What is marionette?

Marionette is a web-based interface for managing Docker containers, images, volumes, networks, stacks, and Swarm clusters. It runs as a single container and connects to one or more Docker hosts.

### How is marionette different from Portainer?

| | Marionette | Portainer |
|---|------|-----------|
| Setup | Single container, one command | Single container, one command |
| Multi-host | Socket Proxy (standard, open-source) | Custom agent or socket proxy |
| Container migration | ✅ Built-in 9-step wizard | ❌ Not available |
| Swarm | ✅ Full management (Phase 3) | ✅ Full management |
| Kubernetes | ❌ Not planned | ✅ |
| RBAC / SSO | ❌ Access key only | In paid Business Edition |
| UI style | Minimal, dense, engineer-focused | Full-featured, business-oriented |

### Is marionette free?

Yes. Marionette is open source under the AGPL v3 license. There are no paid tiers, no feature gating, no enterprise edition.

### What does "marionette" mean?

It's a distinctive, short name. Not an acronym. Like "null" (its sister project), it stands out and is easy to remember.

---

## Setup

### What are the minimum requirements?

- Docker Engine 24+ on the host running marionette
- 100MB RAM for marionette itself (idle)
- 200MB disk for the marionette image
- Browser with WebSocket support (any modern browser)

### Can marionette run on a Raspberry Pi?

Yes. Docker images are built for both `amd64` and `arm64`. Pull the same tag — Docker auto-selects the right architecture.

### Do I need to install anything on remote hosts?

One container: `tecnativa/docker-socket-proxy`. Run one command per remote host. See [Quickstart](quickstart.md).

### Can I run marionette without the access key?

Yes. If `MARIONETTE_KEY` is not set (or empty), the gateway allows all requests. This is intended for local development only. **Always set a key in production.**

---

## Usage

### Why can't I see my containers?

Check:
1. The Docker socket is mounted: `-v /var/run/docker.sock:/var/run/docker.sock`
2. The socket has read/write permissions
3. If using a remote endpoint, verify the Socket Proxy is running and accessible

See [Troubleshooting](troubleshooting.md).

### How do I add a stack?

1. Go to **Stacks**
2. Click **+ New Stack**
3. Enter a name — this creates `/stacks/<name>/docker-compose.yml`
4. Write your compose file in the editor
5. Click **Save** then **Deploy**

### Can marionette manage stacks that already exist?

Yes. If you already have a `/stacks/myapp/docker-compose.yml` file, marionette will detect and show it in the stacks list. No import needed.

### How do I view live container logs?

1. Double-click any container to open detail view
2. Go to the **Logs** tab
3. Logs stream in real time via WebSocket
4. Use the filter input to search, toggle timestamp display, or download

### How do I see which containers are using a volume?

1. Go to **Volumes**
2. Click **Inspect** on any volume
3. The "Used By" section shows all containers referencing this volume

### Does marionette support Docker Compose v3+?

Yes. Marionette runs `docker compose` which supports all compose file versions. The YML editor handles whatever syntax `docker compose` accepts.

---

## Security

### Is the access key secure?

The access key protects the marionette web UI. It's checked on every `/api/*` request. Rate limiting prevents brute-force attacks (5 failed attempts → 30s lockout).

However: **anyone with the key can control Docker on all connected hosts.** Treat it like a root password.

### Should I expose marionette to the internet?

Only behind a reverse proxy with TLS (nginx + Let's Encrypt). Never expose port 8000 directly. See [Security](security.md#marionette--browser).

### Does marionette store my Docker credentials or data?

No. Marionette is stateless. It does not store:
- Docker images or container data
- Environment variable values (displayed live, masked by default)
- Volume contents
- SSH keys (uses command generation model)

### What's the risk of the Socket Proxy?

The Socket Proxy exposes the Docker API over HTTP with granular permissions. If port 2375 is exposed to the internet, anyone can control your Docker host. Always:
- Bind to `127.0.0.1` or a Docker internal network
- Use firewall rules if marionette is on a different machine
- Use TLS if marionette and proxy are on different networks

---

## Migration (Phase 2)

### Can marionette migrate running containers?

Cold migration only (stop → transfer → start). CRIU live migration is available as an experimental option but may not work with all container types.

### Does migration transfer container data?

Yes. Named volumes are exported, compressed, transferred, and imported on the target host. Bind mounts require admin review — marionette can't automatically transfer arbitrary host paths.

### What about database connections?

Marionette detects database connection strings in environment variables (DB_HOST, DATABASE_URL, REDIS_URL, etc.) and shows them in the connection review panel. Admin decides whether to migrate the database container together, update the hostname, or handle manually.

### Can I roll back a migration?

If you haven't removed the source container yet: yes. Click "Restart on source" to restart the stopped source container. Once you remove the source, rollback requires a reverse migration.

---

## Troubleshooting

### The dashboard shows "Docker unreachable"

This means marionette can't connect to the Docker daemon. Check:
1. The Docker socket is mounted
2. Docker is running (`docker info`)
3. The marionette container has permission to access the socket

### I get a blank page or "Not Found"

The React SPA isn't loading. Check:
1. The browser console for errors
2. That `/api/health` returns `"ok"`
3. Try a hard refresh (`Ctrl+Shift+R`)

### WebSocket connection fails

Log streaming and stats use WebSockets. If they fail:
1. Check browser console for connection errors
2. If behind a reverse proxy, ensure WebSocket support is configured
3. The Fastify proxy handles WebSocket passthrough automatically

### The YML editor isn't saving

1. Check that `/stacks` is mounted and writable
2. Check the browser console for API errors
3. Try saving with `Ctrl+S`

### Migration fails mid-transfer

1. Check disk space on target host
2. Verify network connectivity between hosts
3. Check the audit log for specific error messages
4. The source container remains stopped but not removed — you can restart it

---

## Development

### Can I contribute?

Yes! See [Contributing](contributing.md) for setup instructions and conventions.

### What's the tech stack?

Rust (Axum + bollard) for the Docker core, Node/TypeScript (Fastify) for the API gateway, React 19 + Vite for the frontend. Single container via supervisord.

### Why Rust + Node instead of all-Rust or all-Node?

Rust handles Docker interaction (performance-critical, concurrent). Node handles auth, proxying, and SPA serving (trivial in Fastify, verbose in Rust). Two processes, one container, each doing what it's best at.
