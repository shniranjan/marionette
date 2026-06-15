# User Manual

Complete reference for every page and feature in marionette. Covers Phase 1 (single host). Phase 2+ features marked accordingly.

---

## Navigation

### Sidebar

The sidebar is always visible on the left. It contains:

- **Marionette logo/name** — at top
- **Navigation items** — Dashboard, Containers, Stacks, Images, Volumes, Networks, System
- **Theme switcher** — Sun ☀ (light), Moon 🌙 (dark), Coffee ☕ (sepia) — at bottom
- **Endpoint switcher** — dropdown to change active Docker host (Phase 2)

Active page is highlighted with a blue left border.

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `/` | Focus search/filter input (if present on current page) |
| `Enter` | Open selected container detail |
| `Escape` | Close modal, clear selection, blur input |
| `Ctrl+A` (or `Cmd+A`) | Select all containers in list |
| `Ctrl+S` (or `Cmd+S`) | Save in YML editor |
| `Shift+Click` | Range-select in tables |
| `Ctrl+Click` (or `Cmd+Click`) | Multi-select in tables |

### Theme

Three built-in themes. Toggle via the icon in the sidebar footer:

| Theme | Palette | Best for |
|-------|---------|----------|
| **Dark** | GitHub dark — deep blue-black background, blue accents | Default. Low eye strain. |
| **Light** | Clean paper — white background, blue accents | Bright environments |
| **Sepia** | Solarized warm — cream background, muted colors | Long sessions, reading |

Theme persists across browser sessions (localStorage).

---

## Dashboard

**Path:** Click "Dashboard" in sidebar or open marionette.

### Stat Cards

Top row shows key metrics as large numbers with labels:

| Card | What it shows |
|------|--------------|
| **Running** | Number of containers in "running" state across all connected hosts |
| **Stopped** | Number of containers in "exited", "created", or "paused" state |
| **Images** | Total images available |
| **Volumes** | Total volumes |
| **Networks** | Total networks (excluding built-in: bridge, host, none) |
| **Stacks** | Number of docker-compose stacks in /stacks directory |

### CPU & Memory

Second row shows current host resource usage. Percentage and absolute values.

### Recent Events

Scrollable list of the last 50 Docker events. Monospace font. Events include:

- `container create` — new container created
- `container start` — container started
- `container die` — container stopped/crashed
- `container destroy` — container removed
- `image pull` — image pulled
- `volume create` — volume created

Each event shows timestamp, event type, and affected object.

### System Info

Footer shows Docker version, API version, operating system, kernel version, and architecture.

---

## Containers

**Path:** Click "Containers" in sidebar.

### Table

| Column | Description |
|--------|-------------|
| **Name** | Container name. Click to open detail. |
| **Status** | Colored dot + state: ● running, ◌ stopped/paused, ✕ error/dead |
| **Image** | Image name + tag |
| **Ports** | Published ports (e.g., `0.0.0.0:8000→8000`) |
| **Uptime** | How long the container has been running (or "Stopped") |
| **CPU %** | Current CPU usage percentage |
| **Memory** | Current memory usage / limit |
| **Network I/O** | Total bytes sent / received |

### Selection

- **Single click:** Select row (blue highlight)
- **Shift + click:** Select range from previous selection
- **Ctrl/Cmd + click:** Add/remove from selection
- **Click empty space:** Clear selection

Selection persists when table refreshes (matched by container ID).

### Action Bar

Appears below the table when one or more rows are selected. Available actions:

| Action | Description | Multi-select? |
|--------|-------------|:------------:|
| **Start** | Start stopped container(s) | ✅ |
| **Stop** | Stop running container(s). Timeout dialog appears. | ✅ |
| **Restart** | Restart container(s) | ✅ |
| **Pause** | Pause running container(s) | ✅ |
| **Kill** | Force kill container(s). Signal selector (SIGKILL/SIGTERM). | ✅ |
| **Remove** | Remove container(s). Force checkbox (also removes if running). | ✅ |
| **Rename** | Rename a single container. Inline or modal. | ❌ |

### Auto-Refresh

Container list auto-refreshes every 5 seconds. A small indicator shows the countdown. Click the indicator to force immediate refresh.

---

## Container Detail

**Path:** Double-click a container row, or select and click "Inspect."

### Header

Shows container name, status badge, image name, and a "← Back" button.

### Tabs

#### 1. Info

Full `docker inspect` output displayed as a collapsible JSON tree.

- Click `▶` to expand nested objects/arrays
- Click `▼` to collapse
- **Expand All** / **Collapse All** buttons at top
- Click any key to copy its JSON path

#### 2. Logs

Live streaming container logs.

- **WebSocket connection** to the container's log stream
- **Monospace display** — dark background, syntax-colored for stdout (white) vs stderr (red)
- **Auto-scroll toggle:** When ON, view follows new lines. When OFF, scroll position is preserved.
- **Timestamp toggle:** Show/hide Docker timestamps
- **Filter input:** Type to search within visible logs. Matching lines highlighted.
- **Download:** Saves current log buffer as `.txt` file

> **Performance:** Log buffer capped at 10,000 lines. Oldest lines auto-discarded.

#### 3. Stats

Live container resource usage. Updates every 2 seconds.

- **CPU %:** Sparkline chart (last 60 data points) + current value
- **Memory:** Horizontal bar + usage/limit numbers
- **Network RX/TX:** Running counters
- **Block I/O:** Read/write counters
- **PIDs:** Number of processes inside the container

#### 4. Config

Container configuration as key-value pairs:

- Image + tag
- Command (full command line)
- Entrypoint
- Working directory
- Environment variables (collapsible table — see below)
- Published ports
- Mounted volumes
- Restart policy
- Resource limits (CPU shares, memory limit)

**Environment variables:** Values matching known secret patterns (containing "password", "secret", "key", "token") are masked as `••••••••`. Click the eye icon to reveal (with confirmation dialog). Every reveal is audit-logged.

#### 5. Env

Full environment variable table. Same masking rules as Config tab. Useful for scanning all env vars without other config noise.

#### 6. Network

Connected networks and their details:

- Network name
- IP address
- MAC address
- Gateway
- Aliases
- Link-local IPv6 (if any)

---

## Stacks

**Path:** Click "Stacks" in sidebar.

### List View

Table of docker-compose stacks found in `/stacks/` directory:

| Column | Description |
|--------|-------------|
| **Stack** | Directory name (project name) |
| **Services** | `N up / M total` — running services vs defined services |
| **Status** | ● All services up / ◌ Some down / ✕ All down |
| **Last Deployed** | Timestamp of last `docker compose up` |

### Actions (List View)

| Action | Description |
|--------|-------------|
| **Deploy** | `docker compose up -d`. Streams output. |
| **Stop** | `docker compose stop`. Stops all services. |
| **Down** | `docker compose down`. Option: include `--volumes`, `--rmi`. |
| **Restart** | `docker compose restart` |
| **Logs** | Opens aggregated log view for all services |
| **Edit** | Opens YML editor |
| **Remove** | Removes stack directory + `docker compose down` first |

### New Stack

Click **+ New Stack** → enter name → creates directory `/stacks/<name>/` with a skeleton `docker-compose.yml`.

### YML Editor

CodeMirror 6 with YAML syntax highlighting, line numbers, and auto-indent.

- **Save:** `Ctrl+S` or click Save button
- **Deploy:** Click Deploy → saves first → runs `docker compose up -d` → streams output in a log viewer panel below
- **Changed indicator:** A dot appears on the save button when content differs from saved version

---

## Images

**Path:** Click "Images" in sidebar.

### Table

| Column | Description |
|--------|-------------|
| **Repository:Tag** | Image name and tag |
| **Size** | Compressed image size |
| **Created** | When the image was built/pulled |
| **Used By** | Number of containers using this image |
| **ID** | Short image ID |

### Actions

| Action | Description |
|--------|-------------|
| **Pull** | Opens modal: enter `image:tag`, click Pull. Streams pull progress. |
| **Remove** | Remove image. Force checkbox (also remove if referenced by stopped containers). |
| **Inspect** | Full image inspect as collapsible JSON tree |
| **History** | Image layer history — each layer with command, size, and creation time |

---

## Volumes

**Path:** Click "Volumes" in sidebar.

### Table

| Column | Description |
|--------|-------------|
| **Name** | Volume name |
| **Driver** | Volume driver (local, NFS, rclone, etc.) |
| **Mountpoint** | Host path where data lives |
| **Used By** | Containers using this volume |

### Actions

| Action | Description |
|--------|-------------|
| **Create** | Opens modal: name, driver, driver options |
| **Remove** | Remove volume. Force checkbox (also remove if mounted). |
| **Prune** | Remove all unused volumes. Shows count before confirming. |
| **Inspect** | Deep volume inspection (see below) |

### Deep Volume Inspection

Click Inspect on any volume to see:

- **Driver category:** Filesystem (local), Network (NFS/CIFS), Cloud (rclone/S3), Distributed (GlusterFS), Unknown
- **Migration advice:** Transfer (local), Reconnect (network/cloud storage), Warn (unknown driver)
- **Size:** Total bytes + human-readable. Computed by mounting volume in a temporary alpine container and running `du`.
- **File count:** Number of files in the volume.
- **Last modified:** Most recent file modification time.
- **Shared:** Whether multiple containers use this volume.
- **Driver options:** Sanitized display — password/secret fields masked.
- **Labels:** Any Docker labels on the volume.

---

## Networks

**Path:** Click "Networks" in sidebar.

### Table

| Column | Description |
|--------|-------------|
| **Name** | Network name |
| **Driver** | Network driver (bridge, overlay, macvlan, host) |
| **Scope** | Local or swarm (global) |
| **Subnet** | IP subnet |
| **Gateway** | Gateway IP |
| **Containers** | Number of connected containers |

### Actions

| Action | Description |
|--------|-------------|
| **Create** | Opens modal: name, driver, subnet, gateway, IP range, labels |
| **Remove** | Remove network (must have no connected containers) |
| **Connect** | Opens modal: select container, specify IP and aliases |
| **Disconnect** | Disconnect a container from the network |
| **Prune** | Remove all unused networks. Shows count before confirming. |

---

## System

**Path:** Click "System" in sidebar.

### Info

System information table:

| Field | Source |
|-------|--------|
| Docker version | `docker version` |
| API version | `docker version` |
| OS / Architecture | `docker info` |
| Kernel version | `docker info` |
| CPU count | `docker info` |
| Total memory | `docker info` |
| Storage driver | `docker info` |
| Logging driver | `docker info` |
| Docker root directory | `docker info` |

### Prune

Resource cleanup with confirmation dialogs:

| Button | What it prunes | Shows |
|--------|---------------|-------|
| **Prune Containers** | Stopped containers | Count: "3 containers will be removed" |
| **Prune Images** | Dangling/unused images | Count + reclaimable space |
| **Prune Volumes** | Unused volumes | Count + reclaimable space |
| **Prune Networks** | Unused networks | Count |
| **Prune All** | Everything above + build cache | Total count + total reclaimable space |

Each prune button shows a confirmation dialog before executing. Result toast shows success with count and reclaimed space.

### Audit Log

(Phase 2) Scrollable log of all mutating actions performed in marionette. Shows: timestamp, action, target, admin key hash (not the key itself).

---

## Migration (Phase 2)

**Path:** Select container(s) → click "Migrate" in action bar.

The migration wizard is documented fully in the [Migration Guide](tutorial.md#migrating-a-container-between-hosts).

---

## Swarm (Phase 3)

Documentation will be added when the feature ships.

---

## Nginx Load Balancer (Phase 4)

Documentation will be added when the feature ships.
