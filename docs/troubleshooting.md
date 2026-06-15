# Troubleshooting

Common errors, their causes, and how to fix them.

---

## Docker Connectivity

### "Docker unreachable" on dashboard

**Symptoms:** Dashboard shows "Docker unreachable" error. All pages show loading spinners that never resolve.

**Causes:**

1. **Docker socket not mounted.** The marionette container can't find `/var/run/docker.sock`.
   ```bash
   # Fix: ensure the socket mount is in your docker-compose.yml
   volumes:
     - /var/run/docker.sock:/var/run/docker.sock
   ```

2. **Docker daemon not running.**
   ```bash
   # Check:
   docker info
   # If not running:
   sudo systemctl start docker
   ```

3. **Permission denied.** The container user can't read the socket.
   ```bash
   # Check permissions:
   ls -la /var/run/docker.sock
   # Should be srw-rw---- docker:docker
   # Fix: add user to docker group or run container as root (default)
   ```

4. **Docker API version mismatch.** Very old Docker daemon vs new bollard.
   ```bash
   # Check versions:
   docker version
   # Marionette requires Docker Engine API 1.40+
   ```

### "Endpoint timeout" on remote host

**Symptoms:** Remote endpoint shows "Disconnected" or "Timeout."

**Causes:**

1. **Socket proxy not running on remote host.**
   ```bash
   # On remote host:
   docker ps | grep marionette-proxy
   # If not running, restart:
   docker start marionette-proxy
   ```

2. **Network unreachable.** Can marionette reach the remote host?
   ```bash
   # From marionette container:
   curl http://remote-host:2375/version
   # If not, check firewall and network routing
   ```

3. **Socket proxy bound to wrong interface.**
   ```bash
   # Check: docker inspect marionette-proxy | grep -A5 PortBindings
   # Should show 127.0.0.1:2375 or 0.0.0.0:2375
   # If bound to 127.0.0.1 and marionette is remote, change to appropriate bind
   ```

4. **TLS required but not configured.**
   ```bash
   # Error: "tls: first record does not look like a TLS handshake"
   # Fix: either disable TLS on proxy or configure TLS in marionette endpoint
   ```

---

## WebSocket Issues

### Logs tab shows "Connecting..." forever

**Symptoms:** The log viewer never connects. Spinner keeps spinning.

**Causes:**

1. **WebSocket blocked by reverse proxy.** Nginx/Apache not configured for WS.
   ```nginx
   # Required nginx config:
   location /api/ {
       proxy_pass http://marionette:8000;
       proxy_http_version 1.1;
       proxy_set_header Upgrade $http_upgrade;
       proxy_set_header Connection "upgrade";
   }
   ```

2. **Browser extension blocking WebSockets.** Some ad-blockers or privacy extensions interfere.
   ```bash
   # Test: open browser console, type:
   new WebSocket('ws://localhost:8000/api/health')
   # Should connect or show specific error
   ```

3. **Container has no logs output.** If the container produces no stdout/stderr, the WebSocket connects but sends no data.
   ```bash
   # Verify:
   docker logs <container> --tail 10
   ```

### Stats tab shows zeros

**Symptoms:** All stats values are 0.

**Causes:**

1. **Container is stopped.** Stats only available for running containers.
2. **Docker stats API disabled.** Extremely rare. Check `docker stats <container>` on host.

---

## Stack / Compose Issues

### "Deploy failed" with no details

**Symptoms:** Clicking Deploy shows "Deploy failed" in a toast.

**Causes:**

1. **Invalid compose file syntax.**
   ```bash
   # Copy the YML content and validate:
   docker compose -f /tmp/test.yml config
   # Fix syntax errors, re-save, re-deploy
   ```

2. **Port already in use.**
   ```bash
   # Check:
   docker ps --format '{{.Ports}}'
   # Change the port in your compose file
   ```

3. **Volume name conflict.**
   ```bash
   # Check:
   docker volume ls
   # Rename the volume in your compose file or remove the conflicting one
   ```

4. **Image not found.**
   ```bash
   # Pull first:
   docker pull <image>
   # Or ensure the image name is correct in your compose file
   ```

### Stack file not saving

**Symptoms:** Clicking Save does nothing, or shows "Save failed."

**Causes:**

1. **Stacks directory not mounted or not writable.**
   ```bash
   # Check mount:
   docker exec marionette ls -la /stacks
   # Should show directories with rwx permissions
   ```

2. **Disk full.**
   ```bash
   # Check:
   df -h /stacks
   ```

---

## Migration Issues (Phase 2)

### "Target disk full" during migration

**Symptoms:** Migration starts then fails with "Insufficient disk space."

**Fix:**
```bash
# Check disk space on target:
df -h

# Clean up old Docker data on target:
docker system prune -a

# Or choose a different target host with more space
```

### "SSH connection refused" during transfer

**Symptoms:** Migration fails at the transfer step.

**Causes and fixes:**

1. **SSH not running on target.**
   ```bash
   sudo systemctl start sshd
   ```

2. **Key not authorized.**
   ```bash
   # Add your public key to target:
   ssh-copy-id user@target-host
   ```

3. **Firewall blocking SSH.**
   ```bash
   # Allow port 22 on target:
   sudo ufw allow 22
   ```

4. **Use Option C instead.** If SSH is complex to set up between hosts, use marionette's command generation mode — marionette shows you the exact commands, you run them manually.

### "Volume driver mismatch"

**Symptoms:** Migration warns "Driver 'nfs' not available on target."

**Fix:**
1. Install the same volume plugin on target host:
   ```bash
   docker plugin install <driver>
   ```
2. Or choose "Treat as local" — marionette exports the data and imports as a local volume on target
3. Or skip the volume if the data is already accessible on target (NFS share)

### "Container failed to start on target"

**Symptoms:** Migration completes but the container exits immediately.

**Debug steps:**

1. Check container logs on target:
   ```
   docker logs <container>
   ```
2. Common causes:
   - Missing environment variable (not transferred)
   - Database connection still pointing to old host
   - Volume permissions mismatch (UID/GID different on target)
   - Network name collision (marionette auto-renames, but check)

3. Fix the issue and restart manually, or use the rollback option to restart on source.

---

## UI / Browser Issues

### Blank white page

**Symptoms:** Browser shows empty white page on `http://localhost:8000`.

**Causes:**

1. **React SPA files not found.** The gateway can't find the built frontend.
   ```bash
   # Check if static files exist:
   docker exec marionette ls /app/frontend/dist/index.html
   ```

2. **JavaScript error.** Open browser console (F12) → check for red errors.
   - Most common: CORS issue if accessing from a different origin
   - Fix: access marionette on the exact host:port it's running on

3. **Content Security Policy blocking scripts.** If behind a reverse proxy with strict CSP.

### Theme not persisting

**Symptoms:** Theme resets on page reload.

**Causes:**

1. **localStorage blocked.** Private/incognito mode may clear localStorage.
2. **Browser extension clearing storage.**
3. **Theme stored but CSS not applied.** Check the `<html>` element has `data-theme="dark"` attribute.

### Action buttons grayed out

**Symptoms:** Start/stop/restart buttons are disabled.

**Causes:** No container is selected. Click a row in the table to select it first.

### Table not showing all containers

**Symptoms:** Container count is 50 but only 20 rows visible.

**Causes:** Virtual scrolling. The table renders rows as you scroll. All containers are there — just scroll down.

---

## Auth Issues

### "Invalid access key" on every request

**Symptoms:** Every API call returns 401.

**Causes:**

1. **Mismatched key.** The key in the browser doesn't match `MARIONETTE_KEY` on the server.
   ```bash
   # Clear the stored key in browser:
   # localStorage.removeItem('marionette_key')
   # Refresh page, re-enter key
   ```

2. **Key contains special characters.** Shell escaping issues in docker-compose.
   ```yaml
   # Use single quotes to prevent shell expansion:
   environment:
     - MARIONETTE_KEY='my-key-with-$pecial-chars'
   ```

3. **Multiple viewers with different keys.** Each browser stores its own key. Enter the current key.

### Locked out after too many attempts

**Symptoms:** "Too many attempts. Try again in 30 seconds."

**Fix:** Wait 30 seconds. The lockout is per-IP and automatically clears.

---

## Performance Issues

### Dashboard loads slowly

**Symptoms:** Dashboard takes >5 seconds to load.

**Causes:**

1. **Many containers (>500).** Each container requires a Docker API call for stats.
   - Fix: Phase 2 event-driven updates eliminate polling

2. **Slow Docker daemon.** Check host resource usage:
   ```bash
   top
   docker system df
   ```

3. **Network latency (remote endpoints).** Each API call goes over the network.
   - Fix: keep marionette close to the Docker hosts (same network)

### Browser becomes unresponsive with large container list

**Symptoms:** Browser freezes with 500+ containers.

**Causes:** Virtual scrolling not yet enabled. Phase 1 fix: `@tanstack/react-virtual` is a must-do before shipping. See [Architecture Performance section](architecture.md#performance-design).

### Log viewer causes high memory usage

**Symptoms:** Browser memory climbs while viewing logs.

**Causes:** Unbounded log buffer. Phase 1 fix: cap at 10,000 lines. Auto-discard oldest.

---

## General Debugging Steps

When something isn't working:

1. **Check marionette health:**
   ```bash
   curl http://localhost:8000/api/health
   # Should return "ok"
   ```

2. **Check marionette logs:**
   ```bash
   docker compose logs marionette
   # Or:
   docker logs marionette
   ```

3. **Check browser console (F12):** Look for red errors in Console tab. Check Network tab for failed API calls.

4. **Try with curl:** If the UI fails but curl works, it's a frontend issue.
   ```bash
   curl -H "x-marionette-key: your-key" http://localhost:8000/api/containers
   ```

5. **Restart marionette:**
   ```bash
   docker compose restart marionette
   ```

6. **Still stuck?** [Open an issue](https://github.com/shniranjan/marionette/issues) with:
   - What you were doing
   - The exact error message
   - `docker compose logs marionette` output (last 50 lines)
   - `docker version` output
   - Browser and OS
