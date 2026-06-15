# API Reference

All endpoints exposed by the marionette Rust core on `127.0.0.1:9119`. The Node gateway proxies `/api/*` to these — use `/api/health` from the browser, which proxies to `/health` internally.

All endpoints accept `?endpoint=<id>` query parameter (defaults to "local"). Phase 2 adds multi-endpoint support.

**Auth:** The gateway handles authentication before proxying. The Rust core itself has no auth — it listens on `127.0.0.1` only.

---

## Health

### `GET /health`

Check if marionette-core is running and connected to Docker.

**Response 200:**
```json
"ok"
```

**Response 503:**
```json
"docker-unreachable"
```

---

## Containers

### `GET /containers`

List containers.

**Query params:** `all=true|false` (default: true)

**Response 200:**
```json
[
  {
    "id": "abc123def456",
    "name": "hermes-app",
    "image": "hermes:latest",
    "status": "Up 3 hours",
    "state": "running",
    "ports": ["0.0.0.0:8000→8000/tcp"],
    "created": 1718400000,
    "cpu_percent": 2.1,
    "mem_usage": 134217728,
    "mem_limit": 1073741824
  }
]
```

### `GET /containers/:id`

Inspect a container. Returns full `docker inspect` output.

**Response 200:** Standard Docker container inspect JSON.

### `GET /containers/:id/logs`

Get container logs.

**Query params:**
- `tail=N` — last N lines (default: 500)
- `since=ISO8601` — since timestamp
- `timestamps=true|false` — include timestamps (default: false)
- `stdout=true|false` — include stdout (default: true)
- `stderr=true|false` — include stderr (default: true)

**Response 200:** Array of log entries.

### `GET /containers/:id/stats`

Get a single stats snapshot.

**Response 200:** Standard Docker stats JSON.

### `POST /containers/:id/start`

Start a container.

**Response 204:** No content.

### `POST /containers/:id/stop`

Stop a container.

**Query params:** `timeout=N` — seconds to wait before kill (default: 10)

**Response 204:** No content.

### `POST /containers/:id/restart`

Restart a container.

**Response 204:** No content.

### `POST /containers/:id/kill`

Kill a container.

**Query params:** `signal=SIGNAL` — signal to send (default: SIGKILL)

**Response 204:** No content.

### `POST /containers/:id/pause`

Pause a container.

**Response 204:** No content.

### `POST /containers/:id/unpause`

Unpause a container.

**Response 204:** No content.

### `DELETE /containers/:id`

Remove a container.

**Query params:** `force=true|false` — force remove even if running (default: false)

**Response 204:** No content.

### `PATCH /containers/:id/rename`

Rename a container.

**Request body:**
```json
{ "name": "new-name" }
```

**Response 200:** Updated container summary.

### `WS /containers/:id/logs/stream`

WebSocket. Streams log lines as JSON frames.

**Opening:** Connect with WebSocket. No query params needed — streams all logs with follow.

**Messages:**
```json
{"stream": "stdout", "text": "Server listening on port 3000\n", "timestamp": "2026-06-15T10:30:00Z"}
{"stream": "stderr", "text": "Warning: deprecated flag\n", "timestamp": "2026-06-15T10:30:01Z"}
```

Connection stays open until client disconnects.

### `WS /containers/:id/stats/stream`

WebSocket. Streams decoded stats dicts every ~2 seconds.

**Messages:** Standard Docker stats JSON with decoded fields.

---

## Images

### `GET /images`

List images.

**Query params:** `dangling=true|false` — only dangling images (default: false)

### `GET /images/:id`

Inspect an image.

### `POST /images/pull`

Pull an image.

**Request body:**
```json
{ "image": "nginx", "tag": "alpine" }
```

**Response 200 (streaming):** Pull progress events as they arrive. Server-sent or chunked response.

### `DELETE /images/:id`

Remove an image.

**Query params:** `force=true|false`

### `GET /images/:id/history`

Image layer history.

---

## Volumes

### `GET /volumes`

List volumes.

### `GET /volumes/:name`

Inspect a volume.

### `GET /volumes/:name/deep`

Deep volume inspection. Returns size, file count, driver classification, migration advice.

**Response 200:**
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
  "used_by": ["hermes-app"],
  "shared": false,
  "options": {},
  "options_sanitized": {},
  "labels": {}
}
```

### `POST /volumes/create`

Create a volume.

**Request body:**
```json
{ "name": "myvol", "driver": "local", "options": {} }
```

### `DELETE /volumes/:name`

Remove a volume. Query: `force=true|false`

### `POST /volumes/prune`

Prune unused volumes.

---

## Networks

### `GET /networks`

List networks.

### `GET /networks/:id`

Inspect a network.

### `POST /networks/create`

Create a network.

**Request body:**
```json
{ "name": "mynet", "driver": "bridge" }
```

### `DELETE /networks/:id`

Remove a network.

### `POST /networks/:id/connect`

Connect a container to a network.

**Request body:**
```json
{ "container": "hermes-app", "ip": "172.20.0.10", "aliases": ["hermes"] }
```

### `POST /networks/:id/disconnect`

Disconnect a container from a network.

**Request body:**
```json
{ "container": "hermes-app" }
```

### `POST /networks/prune`

Prune unused networks.

---

## Stacks

### `GET /stacks`

List docker-compose stacks in `/stacks/`. Returns stack names with service status.

### `GET /stacks/:name`

Read a stack's `docker-compose.yml` content.

**Response 200:**
```json
{
  "name": "hermes-web",
  "content": "version: \"3.8\"\nservices:\n  ...",
  "services": [
    {"name": "nginx", "status": "running"},
    {"name": "hermes", "status": "running"}
  ]
}
```

### `PUT /stacks/:name`

Save/overwrite `docker-compose.yml`.

**Request body:**
```json
{ "content": "version: \"3.8\"\nservices:\n  nginx:\n    image: nginx:alpine" }
```

### `POST /stacks/:name/deploy`

Run `docker compose -f /stacks/<name>/docker-compose.yml up -d`.

**Response 202:** Accepted. Deploy output available via WebSocket.

### `WS /stacks/:name/deploy/stream`

WebSocket. Streams `docker compose up -d` output in real time. Closes when deploy completes.

### `POST /stacks/:name/stop`

Run `docker compose stop`.

### `POST /stacks/:name/down`

Run `docker compose down`. Query: `volumes=true|false`, `rmi=true|false`

### `POST /stacks/:name/restart`

Run `docker compose restart`.

### `GET /stacks/:name/logs`

Get aggregated logs. Query: `tail=N`

### `DELETE /stacks/:name`

Remove stack directory + `docker compose down` first.

---

## System

### `GET /system/info`

Docker system info (`docker info` equivalent).

### `GET /system/version`

Docker version info (`docker version` equivalent).

### `GET /system/events`

SSE stream. Docker events pushed as they happen.

**Event format:**
```
event: container
data: {"type": "container", "action": "start", "id": "abc123", "name": "hermes-app", "time": 1718400000}
```

### `POST /system/prune`

Prune unused resources.

**Request body:**
```json
{ "containers": true, "images": true, "volumes": false, "networks": false, "all": false }
```

**Response 200:**
```json
{
  "containers_deleted": 3,
  "images_deleted": 7,
  "volumes_deleted": 0,
  "networks_deleted": 0,
  "space_reclaimed_bytes": 2415919104
}
```

---

## Endpoints (Phase 2)

### `GET /endpoints`

List configured Docker endpoints.

### `POST /endpoints`

Add an endpoint.

**Request body:**
```json
{ "name": "production-us", "connection": "tcp://10.0.0.5:2375", "tags": ["production", "us-east"] }
```

### `GET /endpoints/:id`

Get endpoint detail + connectivity status.

### `PATCH /endpoints/:id`

Update endpoint configuration.

### `DELETE /endpoints/:id`

Remove an endpoint (disconnects client).

### `POST /endpoints/:id/test`

Test connectivity to an endpoint. Returns success or error message.

---

## Swarm (Phase 3)

Documentation will be added when the feature ships.

---

## Error Responses

All errors follow this format:

```json
{
  "error": "Human-readable error message",
  "detail": "Additional technical detail (optional)",
  "code": "ERROR_CODE"
}
```

| Status | Code | Meaning |
|--------|------|---------|
| 400 | `BAD_REQUEST` | Invalid request body or parameters |
| 401 | `UNAUTHORIZED` | Invalid or missing access key (gateway level) |
| 404 | `NOT_FOUND` | Resource not found (container, image, etc.) |
| 409 | `CONFLICT` | Operation conflicts with current state (e.g., container already running) |
| 500 | `INTERNAL_ERROR` | Unexpected error |
| 503 | `DOCKER_UNREACHABLE` | Docker daemon not responding |
| 504 | `ENDPOINT_TIMEOUT` | Endpoint connection timeout (5s) |

---

## Rate Limiting

The gateway applies rate limiting to protect against brute-force key guessing:

- 5 failed auth attempts per IP → 30-second lockout
- Lockout applies to all `/api/*` requests from that IP
- Successful auth resets the counter
- No rate limiting when `MARIONETTE_KEY` is not set (local dev mode)
