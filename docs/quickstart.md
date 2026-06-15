# Quickstart Guide

Get marionette running in under 5 minutes.

---

## Local Setup (Single Docker Host)

### Prerequisites

- Docker Engine 24+ installed and running
- `docker compose` (v2) or `docker-compose` (v1)

### Option 1: Docker Run (fastest)

```bash
docker run -d --name marionette \
  -v /var/run/docker.sock:/var/run/docker.sock \
  -v $(pwd)/stacks:/stacks \
  -e MARIONETTE_KEY=your-secret-key \
  -p 8000:8000 \
  ghcr.io/shniranjan/marionette:latest
```

Open `http://localhost:8000`. Enter `your-secret-key` when prompted.

### Option 2: Docker Compose

Create `docker-compose.yml`:

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

```bash
# Create .env file
echo "MARIONETTE_KEY=$(openssl rand -hex 32)" > .env

# Start
docker compose up -d

# View logs
docker compose logs -f

# Stop
docker compose down
```

### Option 3: Build from Source

```bash
git clone https://github.com/shniranjan/marionette.git
cd marionette
docker compose up -d --build
```

---

## Remote Host Setup (Multi-Host)

### On Each Remote Host

Run the Docker Socket Proxy — one command:

```bash
docker run -d --name marionette-proxy \
  --restart unless-stopped \
  -v /var/run/docker.sock:/var/run/docker.sock:ro \
  -p 127.0.0.1:2375:2375 \
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
  tecnativa/docker-socket-proxy
```

> **Security note:** The proxy binds to `127.0.0.1:2375` only. If marionette runs on a different machine, change to the appropriate bind address and ensure network-level access control (firewall, VPN, or TLS).

### In Marionette

1. Open marionette web UI
2. Navigate to **Endpoints** (Phase 2)
3. Click **Add Endpoint**
4. Enter name (e.g., "production-us") and connection string (e.g., `tcp://10.0.0.5:2375`)
5. Click **Test Connection** — should show green checkmark
6. Click **Save**
7. Use the host switcher in the sidebar to switch between endpoints

---

## First Steps

After logging in:

### 1. Explore the Dashboard

The dashboard shows:
- **Stat cards:** Running/stopped containers, image count, volume count, network count
- **CPU & Memory:** Current host usage
- **Recent Events:** Last 50 Docker events (start, stop, die, pull, etc.)
- **System Info:** Docker version, API version, OS, kernel, architecture

### 2. View Your Containers

- Click **Containers** in the sidebar
- Table shows: name, status, image, ports, uptime, CPU%, memory
- **Click** a row to select it
- **Double-click** to open the detail view
- Use the action bar below to: start, stop, restart, kill, pause, remove, rename

### 3. View Container Details

The detail view has 6 tabs:
- **Info:** Full `docker inspect` output as a collapsible JSON tree
- **Logs:** Live streaming logs with auto-scroll and filter
- **Stats:** CPU sparkline, memory bar, network I/O, block I/O, PIDs
- **Config:** Image, command, entrypoint, env vars, ports, volumes, restart policy
- **Env:** Full environment variable table (passwords masked)
- **Network:** Connected networks, IPs, aliases

### 4. Create a Stack

- Navigate to **Stacks**
- Click **+ New Stack**
- Enter a name (e.g., "my-app")
- A skeleton `docker-compose.yml` appears in the editor
- Write your compose file
- Click **Save** then **Deploy**
- Watch the deploy output in real time

### 5. Pull an Image

- Navigate to **Images**
- Click **Pull**
- Enter image name (e.g., `nginx:alpine`)
- Click **Pull** — watch progress
- The image appears in the table when complete

---

## Upgrading Marionette

### Docker Compose

```bash
# Pull latest image
docker compose pull

# Recreate the container
docker compose up -d

# Verify
docker compose logs marionette
```

### Docker Run

```bash
# Stop and remove existing
docker stop marionette && docker rm marionette

# Pull latest
docker pull ghcr.io/shniranjan/marionette:latest

# Run with same options as before
docker run -d --name marionette \
  -v /var/run/docker.sock:/var/run/docker.sock \
  -v /opt/stacks:/stacks \
  -e MARIONETTE_KEY=your-secret-key \
  -p 8000:8000 \
  ghcr.io/shniranjan/marionette:latest
```

---

## Uninstalling

```bash
# Stop and remove container
docker compose down

# Remove image
docker rmi ghcr.io/shniranjan/marionette:latest

# Remove data (stacks directory)
rm -rf ./stacks
```

---

## Next Steps

- [User Manual](user-manual.md) — detailed documentation for every feature
- [Tutorial](tutorial.md) — guided walkthroughs for common tasks
- [Security](security.md) — hardening your marionette deployment
- [Troubleshooting](troubleshooting.md) — common issues and fixes
- [FAQ](faq.md) — frequently asked questions
