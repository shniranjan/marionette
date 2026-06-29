# Marionette Quickstart Guide

Step-by-step from zero to a working Marionette deployment — local Docker host, then multi-host.

---

## Prerequisites

- Docker Engine 20.10+ on the host(s) you want to manage
- A machine to run Marionette (can be the same as the Docker host)
- `docker compose` plugin (v2+) or `docker-compose` standalone

---

## 1. Local Setup (Single Host)

This is the fastest path. Marionette runs on the same host it manages.

### 1.1 Clone and start

```bash
git clone https://github.com/shniranjan/marionette.git
cd marionette
```

### 1.2 Set your access key

```bash
export MARIONETTE_KEY="your-secure-key-here"
```

> Without this, Marionette runs with **no authentication** (dev mode only — not for production).

### 1.3 Start

```bash
docker compose up -d
```

Marionette is now running:

| URL | What |
|-----|------|
| `https://localhost:8443` | Main app (HTTPS) |
| `http://localhost:8000` | Redirects to HTTPS |

> **First-run TLS:** Marionette auto-generates a self-signed certificate. Your browser will show a warning — accept it on LAN. Mount your own cert at `./certs/` to override.

### 1.4 Log in

Open `https://localhost:8443`. You'll see the auth prompt. Enter your `MARIONETTE_KEY`.

The Dashboard shows your local Docker host connected automatically as the "local" endpoint.

---

## 2. Remote Docker Host (via Socket Proxy)

To manage a remote Docker host, use `tecnativa/docker-socket-proxy` — a lightweight container that exposes the Docker API over TCP with granular permissions.

### 2.1 On the remote host — deploy the socket proxy

```bash
# On the remote Docker host (e.g., 192.168.1.100)
docker run -d --name socket-proxy \
  --restart unless-stopped \
  -p 2375:2375 \
  -v /var/run/docker.sock:/var/run/docker.sock:ro \
  -e POST=1 \
  -e IMAGES=2 \
  -e BUILD=2 \
  -e CONTAINERS=2 \
  -e VOLUMES=2 \
  -e NETWORKS=2 \
  -e SERVICES=2 \
  -e TASKS=2 \
  -e EXEC=0 \
  -e AUTH=0 \
  -e SECRETS=0 \
  tecnativa/docker-socket-proxy
```

| Permission | Value | Why |
|-----------|:-----:|-----|
| `POST=1` | read | Needed for build, pull, create |
| `IMAGES=2` | read-write | Pull needs write on images |
| `BUILD=2` | read-write | Docker build needs write |
| `EXEC=0` | none | Security boundary — no shell access |
| `AUTH=0` | none | No credential exposure |

### 2.2 On the remote host — open firewall

```bash
# ufw
sudo ufw allow 2375/tcp comment 'Docker socket proxy'

# firewalld
sudo firewall-cmd --permanent --add-port=2375/tcp && sudo firewall-cmd --reload
```

### 2.3 In Marionette — add the endpoint

1. Go to **Endpoints** in the sidebar
2. Click **+ Add Endpoint**
3. Connection string: `tcp://192.168.1.100:2375`
4. Give it a name (e.g., "homelab")
5. Click **Test Connection** → should show green ✓
6. Click **Save**

The new host appears in the sidebar endpoint switcher. Switch to it — the Dashboard now shows that host's containers.

> **Security note:** Socket proxy over plain TCP is for trusted LANs only. For production or internet-facing hosts, use TLS (next section).

---

## 3. Remote Docker Host (TLS — Production)

For production or any host not on a trusted LAN, use Docker's native TLS. Marionette's built-in setup script generator handles everything.

### 3.1 Generate the setup script

1. Go to **Endpoints** → **🔧 Setup Script**
2. Enter the remote host's IP address and SSH port
3. Copy the generated script

### 3.2 Run on the remote host

```bash
# SSH to the remote host
ssh user@192.168.1.100

# Paste and run the script as root
sudo bash marionette-setup.sh
```

The script does:
- Generates CA, server, and client certificates
- Configures Docker daemon with TLS (`daemon.json`)
- Handles systemd `-H fd://` conflicts (auto drop-in override)
- Opens port 2376 on the firewall (ufw or firewalld)
- Copies client certs to `$HOME/marionette-certs/` with correct permissions

### 3.3 Copy client certs to the Marionette host

```bash
# From your local machine (where Marionette runs)
scp -r user@192.168.1.100:~/marionette-certs ./certs/remote-host
```

### 3.4 Add the endpoint in Marionette

1. Go to **Endpoints** → **+ Add Endpoint**
2. Connection string: `https://192.168.1.100:2376`
3. Cert Path: `/app/certs/remote-host`
4. Test connection → ✓
5. Save

Your remote Docker host is now connected with TLS encryption.

> **Multiple TLS hosts?** Each endpoint can have its own cert path. Create separate directories under `./certs/` (e.g., `./certs/host-a`, `./certs/host-b`) and mount `./certs:/app/certs` in docker-compose.

---

## 4. Full docker-compose.yml Reference

```yaml
services:
  marionette:
    image: ghcr.io/shniranjan/marionette:latest
    container_name: marionette
    ports:
      - "8000:8000"     # HTTP → 301 redirect to HTTPS
      - "8443:8443"     # HTTPS app
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock   # Local Docker access
      - ./stacks:/stacks                              # Compose stack files
      - ./data:/data                                  # SQLite database
      - ./certs:/app/certs                            # TLS + endpoint client certs
    environment:
      - MARIONETTE_KEY=${MARIONETTE_KEY:-}
      - MARIONETTE_STACKS_DIR=/stacks
      - MARIONETTE_DB_PATH=/data/marionette.db
      - MARIONETTE_LOG_LEVEL=info
    restart: unless-stopped
    networks:
      - marionette-net

networks:
  marionette-net:
    name: marionette_marionette-net
```

---

## 5. First Steps After Setup

### Add your first stack

1. Create a `docker-compose.yml` in `./stacks/my-app/`
2. Go to **Stacks** in Marionette
3. Your stack appears — click **Deploy**

### Explore a container

1. Go to **Containers**
2. Click a container name → detail view with tabs:
   - **Info** — full Docker inspect as JSON tree
   - **Logs** — live streaming logs with timestamps
   - **Stats** — real-time CPU/Memory/Network charts (recharts)
   - **Shell** — interactive terminal (bash/sh/ash selector)
   - **Env** — environment variables (secrets masked)
   - **Mounts** — volumes and bind mounts
   - **Network** — IPs, ports, gateways
   - **Labels** — editable Docker labels

### Migrate a container

1. Go to **Migration** → **Start New Migration**
2. Step through the 9-step wizard:
   - Select endpoint and container
   - Review analysis (volumes, DB connections, warnings)
   - Choose transfer strategy and compression
   - Resolve any flagged issues
   - Select target endpoint
   - Review dry-run commands
   - **Execute** — Marionette runs the commands automatically
   - Verify migration result

---

## 6. Troubleshooting

### "Connection refused" on remote endpoint

Check the socket proxy is running:
```bash
# On the remote host
docker ps --filter name=socket-proxy
curl http://localhost:2375/version
```

### "Permission denied" on operations

Verify socket proxy permissions:
```bash
docker inspect socket-proxy --format '{{range .Config.Env}}{{println .}}{{end}}'
```

Ensure `POST=1`, `IMAGES=2`, `BUILD=2` if you're building images.

### TLS endpoint shows "connection error"

1. Verify Docker is listening on 2376: `sudo ss -tlnp | grep 2376`
2. Check firewall: `sudo ufw status | grep 2376`
3. Verify cert files exist in the mounted path: check inside the Marionette container at `/app/certs/<endpoint-name>/`

### Container shell shows "executable file not found"

The container may not have `bash`. Use the shell selector dropdown in the Terminal header bar to switch to `sh` or `ash` (Alpine containers).

### Self-signed cert warning

Marionette auto-generates a cert on first run. To use a real certificate:
1. Mount your cert to `./certs/marionette-key.pem` and `./certs/marionette-cert.pem`
2. Or set `TLS_KEY` and `TLS_CERT` environment variables

---

## Next Steps

- [Architecture](architecture.md) — how Marionette works under the hood
- [User Manual](user-manual.md) — every page and feature in detail
- [Security](security.md) — threat model and best practices
- [API Reference](api-reference.md) — all backend endpoints
