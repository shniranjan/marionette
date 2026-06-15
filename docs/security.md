# Security

Threat model, credential handling, and hardening guide for marionette deployments.

---

## Threat Model

### What marionette protects

Marionette is a management interface for Docker. It does NOT:

- Run user workloads
- Store sensitive data (marionette is stateless)
- Handle user authentication for hosted applications

Marionette DOES:

- Control Docker containers, images, volumes, networks across multiple hosts
- Read environment variables (which may contain database passwords, API keys)
- Read volume driver options (which may contain storage credentials)
- Orchestrate data transfer between hosts during migration

### What we assume

- The Docker socket is root-equivalent. Access to it = full host control.
- Remote hosts are on a trusted network (or connected via VPN).
- The marionette container itself is not directly exposed to the internet without a reverse proxy + TLS.

---

## Access Key Authentication

### How it works

1. Admin sets `MARIONETTE_KEY` environment variable (e.g., `MARIONETTE_KEY=my-secret-key`)
2. Gateway checks `X-Marionette-Key` header on every `/api/*` request
3. If key matches → request proceeds
4. If key doesn't match → 401 Unauthorized
5. If `MARIONETTE_KEY` is empty → all requests allowed (local development only)

### Rate limiting

- 5 failed attempts from same IP → 30-second lockout
- Prevents brute-force key guessing

### Key rotation

Multiple keys are supported for rotation:

```bash
# During rotation, both old and new keys work
MARIONETTE_KEY=old-key,new-key
```

After all clients have switched to the new key, remove the old one.

### Frontend key storage

- Key stored in `localStorage` as `marionette_key`
- Auto-attached to all fetch calls via `api/client.js`
- On 401 response → key cleared, auth gate shown
- Key never appears in URL parameters

---

## Credential Handling

### Environment variables

Docker containers often have sensitive environment variables:

```
DB_PASSWORD=supersecret
AWS_SECRET_ACCESS_KEY=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY
REDIS_URL=redis://user:password@redis:6379/0
```

**Marionette's protections:**

1. **Masked by default.** All env var values matching known secret patterns are replaced with `••••••••`
2. **Reveal with confirmation.** Clicking "reveal" shows a confirmation prompt: "This value will be logged in the audit trail. Continue?"
3. **Audit logged.** Every secret reveal is logged with timestamp and admin key hash
4. **Compose file warning.** When generating a compose file during migration that contains secrets, marionette warns: "This compose file contains 3 secrets. Transfer over SSH only. Delete after migration."

### Volume driver options

Volume plugins like NFS, CIFS, rclone may have credentials in their options:

```json
{
  "Driver": "cifs",
  "Options": {
    "type": "cifs",
    "o": "username=admin,password=supersecret"
  }
}
```

**Marionette's protections:**

- Options are **sanitized before display** — password, secret, key, token fields are masked
- Sanitized options are **never transmitted between hosts** during migration
- If reconnecting a driver on the target host, admin must re-enter credentials

### SSH keys for migration

During container migration, marionette needs to transfer data between hosts.

**Option A: Ephemeral key (recommended)**
- Marionette generates a one-time SSH key pair
- Admin adds the public key to both hosts
- Key stored in memory only — destroyed when marionette restarts

**Option B: Admin-provided key**
- Admin mounts an SSH key at `/keys/id_rsa:ro`
- More convenient, but persistent

**Option C: Command generation (default, safest)**
- Marionette generates the exact commands needed
- Admin runs them manually
- Marionette never holds any SSH credentials

---

## Socket Proxy Security

### Why a proxy?

Mounting `/var/run/docker.sock` directly gives full root access. The Socket Proxy (`tecnativa/docker-socket-proxy`) acts as a gatekeeper.

### Permissions for marionette

```
ALLOW_CONTAINERS=true   # List, inspect, create, remove containers
ALLOW_START=true        # Start containers
ALLOW_STOP=true         # Stop containers
ALLOW_RESTARTS=true     # Restart containers
ALLOW_CREATE=true       # Create containers
ALLOW_DELETE=true       # Remove containers
ALLOW_IMAGES=true       # Pull, list, remove images
ALLOW_INFO=true         # System info
ALLOW_EVENTS=true       # Event stream
ALLOW_LOGS=true         # Container logs
ALLOW_EXEC=true         # Exec into containers (for connectivity tests)
ALLOW_NETWORKS=true     # List, create, remove networks
ALLOW_VOLUMES=true      # List, create, remove volumes

# Revoke dangerous endpoints
ALLOW_ATTACH=false      # Attach to container (terminal access)
ALLOW_EXEC=false        # Set true only if needed for health checks
ALLOW_SESSION=false     # Session management
ALLOW_SECRETS=false     # Swarm secrets (enable Phase 3)
ALLOW_CONFIGS=false     # Swarm configs (enable Phase 3)
```

### Network binding

- **Always** bind to `127.0.0.1:2375` or a Docker internal network
- **Never** bind to `0.0.0.0:2375` — this exposes the proxy to the internet
- If marionette and the proxy are on different networks, use TLS:

```bash
# With TLS
DOCKER_HOST=tcp://10.0.0.5:2376
DOCKER_TLS_VERIFY=1
DOCKER_CERT_PATH=/certs
```

---

## Transport Security

### Marionette ↔ Browser

In production, always put marionette behind a reverse proxy with TLS:

```nginx
server {
    listen 443 ssl;
    server_name marionette.example.com;

    ssl_certificate     /etc/letsencrypt/live/marionette.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/marionette.example.com/privkey.pem;

    location / {
        proxy_pass http://marionette:8000;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # WebSocket support
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }

    # Access key at the reverse proxy level (optional, defense-in-depth)
    # set $marionette_key "your-key";
    # if ($http_x_marionette_key != $marionette_key) { return 401; }
}
```

### Marionette ↔ Docker Socket (local)

Unix socket. No network exposure. No encryption needed.

### Marionette ↔ Socket Proxy (remote)

By default, plain HTTP on Docker internal network. If cross-network:

1. Use TLS on the proxy port
2. Configure bollard with TLS client: `Docker::connect_with_http("tcp://host:2376", 60, API_DEFAULT_VERSION, client_tls_config)`

### Host A ↔ Host B (migration)

SCP and rsync over SSH are encrypted by default. Marionette warns if admin selects plain rsync (no SSH).

---

## Audit Logging

### What is logged

Every mutating action:

| Action | Detail |
|--------|--------|
| `container.start` | Container ID, endpoint |
| `container.stop` | Container ID, endpoint, timeout |
| `container.remove` | Container ID, endpoint, force flag |
| `image.pull` | Image name, tag, endpoint |
| `volume.create` | Volume name, driver, endpoint |
| `stack.deploy` | Stack name, endpoint |
| `migration.start` | Source endpoint, target endpoint, container IDs |
| `migration.complete` | Duration, volumes transferred, bytes |
| `secret.reveal` | Which secret was revealed (not the value) |
| `auth.failed` | IP address, attempt count |

### What is NOT logged

- Environment variable values
- Volume data contents
- Container log contents
- SSH keys or file contents
- The access key itself (only its SHA-256 hash)

### Storage

- Phase 1: In-memory ring buffer (last 10,000 entries). Lost on restart.
- Phase 2: SQLite database at `/data/audit.db`. Persists across restarts.

---

## Hardening Checklist

### Production deployment

- [ ] Set a strong `MARIONETTE_KEY` (64+ random characters)
- [ ] Put marionette behind a reverse proxy with TLS (nginx + Let's Encrypt)
- [ ] Never expose marionette's port 8000 directly to the internet
- [ ] Enable firewall on marionette host
- [ ] Use Docker internal networks for socket proxy communication
- [ ] Review socket proxy permissions — revoke anything not needed
- [ ] Run marionette container as non-root (add `user: "1000:1000"` to compose)
- [ ] Mount docker socket as read-only: `:ro`
- [ ] Keep marionette updated to get security patches
- [ ] Monitor the audit log for suspicious activity
- [ ] Rotate `MARIONETTE_KEY` periodically (or after staff changes)
- [ ] Use Option C (command generation) for migration — marionette never gets SSH access

### Risk acceptance

> **Warning:** Anyone with the marionette access key can:
> - Start, stop, delete any container on any connected host
> - Read environment variables (after reveal confirmation)
> - Deploy arbitrary containers
> - Migrate containers between hosts
> - Access container logs (which may contain sensitive data)
>
> Treat the access key with the same care as a root password.
