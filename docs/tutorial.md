# Tutorial

Guided walkthroughs for common marionette workflows.

---

## Tutorial 1: Deploying Your First Stack

This tutorial walks through deploying a simple web application stack using marionette's built-in YML editor.

**Prerequisites:** Marionette running with `/stacks` mounted. See [Quickstart](quickstart.md).

### Step 1: Create a new stack

1. Click **Stacks** in the sidebar
2. Click **+ New Stack**
3. Enter name: `hello-world`
4. Click **Create**

A skeleton `docker-compose.yml` opens in the editor.

### Step 2: Write the compose file

Replace the skeleton with:

```yaml
version: "3.8"

services:
  hello:
    image: nginx:alpine
    ports:
      - "8080:80"
    volumes:
      - hello_html:/usr/share/nginx/html

volumes:
  hello_html:
```

### Step 3: Add custom HTML

We need to populate the volume with content. Let's do this through marionette:

1. Click **Save** (or `Ctrl+S`)
2. Click **Deploy**
3. Watch the deploy output stream — you'll see `Creating network... Creating volume... Creating container... Starting... Done.`

### Step 4: Verify

```bash
curl http://localhost:8080
# Should show the default nginx welcome page
```

### Step 5: Update the stack

1. Edit the compose file — change port `8080` to `9090`
2. Click **Save** then **Deploy**
3. The stack updates: nginx container is recreated with the new port

### Step 6: View logs

1. In the stacks list, find `hello-world`
2. Click **Logs** in the action bar
3. You'll see nginx access and error logs

### Step 7: Stop and remove

1. Select `hello-world` in the stacks list
2. Click **Down** → check "Remove volumes" → Confirm
3. The stack, its containers, and volumes are removed

---

## Tutorial 2: Inspecting a Troubled Container

When a container isn't behaving, marionette gives you all the tools to diagnose it.

### Step 1: Identify the container

1. Go to **Containers**
2. Scan the status column — look for ✕ (error), ◌ (stopped), or high CPU/memory
3. Click the container row to select it

### Step 2: Check logs

1. Double-click the container to open detail view
2. Go to **Logs** tab
3. If the container died, logs show what happened before exit
4. Use the **Filter** input to search for "error" or "panic"
5. Toggle **Timestamps** to correlate with other events

### Step 3: Check stats

1. Go to **Stats** tab
2. If CPU is pegged at 100% or memory is growing (leak), you have a resource issue
3. If everything is zero, the container may have exited

### Step 4: Check config

1. Go to **Config** tab
2. Verify the image tag is correct (no accidental `latest` breakage)
3. Check environment variables — are required env vars set?
4. Verify ports are mapped correctly
5. Check volume mounts point to real paths

### Step 5: Check network

1. Go to **Network** tab
2. Is the container on the expected network?
3. Does it have an IP?
4. Can it reach its database? (Phase 2 adds connectivity testing)

### Step 6: Take action

Based on your findings:
- **Wrong config:** Fix the compose file in Stacks and redeploy
- **Missing env var:** Add it and restart
- **Resource issue:** Add CPU/memory limits, restart
- **Image problem:** Pull a newer/older tag

---

## Tutorial 3: Migrating a Container Between Hosts (Phase 2)

This tutorial walks through the full migration workflow — marionette's signature feature.

**Prerequisites:** Two hosts connected to marionette. At least one running container on the source host.

### Step 1: Select the container

1. Switch host in sidebar to the source host
2. Go to **Containers**
3. Select the container(s) to migrate
4. Click **Migrate** in the action bar

### Step 2: Review the analysis

Marionette inspects the container and shows:

- **Stack detection:** If the container is part of a compose stack (has `com.docker.compose.project` label), marionette recommends migrating the entire stack
- **Volumes found:** Each volume classified by driver type with migration advice
- **Database connections detected:** Environment variables matching known DB patterns (DB_HOST, REDIS_URL, etc.)

### Step 3: Choose migration strategy

Marionette pre-selects the safest strategy based on analysis:

| Strategy | When to use |
|----------|------------|
| **Stack migration** | Container is part of a compose stack (recommended) |
| **Full snapshot** | Container has named volumes only, no bind mounts |
| **Recreate from compose** | Container has bind mounts or complex config |
| **Clone + decommission** | Need minimal downtime, have load balancer |
| **CRIU checkpoint** | Experimental live migration (may not work) |

Click any strategy to select it. The default is usually correct.

### Step 4: Select target host

1. Choose target host from the dropdown
2. Marionette shows available disk space and container count
3. Enter target stack name if migrating a stack (defaults to same name)

### Step 5: Review database connections

Marionette shows every detected database connection:

```
DB_HOST=postgres → container 'postgres' on host-a ⚠ Will break after migration
```

For each connection, choose:
- **Migrate together:** Move the DB container too
- **Replace with hostname:** Point to host-a:5432 (DB stays on source)
- **Custom:** Enter your own connection string
- **Leave as-is:** Admin will handle manually

### Step 6: Review volume sync plan

For each volume, marionette shows the default sync plan. Admin can override:

- **Transfer method:** SCP, rsync, intermediate location, skip
- **Compression:** gzip, pigz, zstd, lz4
- **Custom source/target paths:** For bind mounts
- **Exclude patterns:** Skip cache, logs, temp files

### Step 7: Dry run

Click **Dry Run** to see the exact commands marionette will execute:

```
✓ Stop hermes-app
✓ Export hermes_config (12MB) → tar + pigz
✓ Export hermes_data (2.3GB) → tar + pigz
[host-a] scp /tmp/marionette/*.tar.gz user@host-b:/tmp/marionette/
[host-b] docker volume create hermes_config
[host-b] docker volume create hermes_data
[host-b] docker compose up -d
[host-b] docker exec hermes-app pg_isready
```

Review carefully. This is your last chance to catch issues.

### Step 8: Execute

Click **Execute Migration**.

Marionette shows progress for each step:

```
▸ Exporting hermes_data (1.8GB / 2.3GB) — 78%
✓ Stop hermes-app
✓ Export hermes_config
▸ Export hermes_data
○ Transfer files
○ Import volumes
○ Start container
○ Verify
```

### Step 9: Verify

After migration completes:

1. Marionette runs health checks (container status, DB connectivity)
2. If all green: migration successful
3. If any red: check the specific failure and fix

### Step 10: Cleanup

- **Remove from source:** Deletes the old container/stack and volumes
- **Restart on source (rollback):** If target failed, restart the stopped source container
- **Done:** Keep both (source stopped, target running)

---

## Tutorial 4: Managing Volumes

### View volume details

1. Go to **Volumes**
2. Find the volume you want to inspect
3. Click **Inspect** in the action bar
4. The deep inspection panel shows:
   - Driver type and category
   - Size and file count
   - Which containers use it
   - Whether it's shared
   - Migration advice

### Create a volume

1. Click **Create**
2. Enter name: `backup-data`
3. Choose driver: `local`
4. (Optional) Add labels
5. Click **Create**

The volume appears in the table. Now mount it in a container via the Stacks YML editor.

### Prune unused volumes

1. Click **Prune**
2. Marionette shows: "3 unused volumes will be removed. 1.2GB will be reclaimed."
3. Click **Confirm**

---

## Tutorial 5: Setting Up Remote Host Access (Phase 2)

### On the remote host

Run the Socket Proxy:

```bash
docker run -d --name marionette-proxy \
  --restart unless-stopped \
  -v /var/run/docker.sock:/var/run/docker.sock:ro \
  -p 127.0.0.1:2375:2375 \
  -e ALLOW_CONTAINERS=true \
  -e ALLOW_START=true \
  -e ALLOW_STOP=true \
  -e ALLOW_RESTARTS=true \
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

### In marionette

1. Go to **Endpoints**
2. Click **Add Endpoint**
3. Name: `homelab-nas`
4. Connection: `tcp://192.168.1.100:2375`
5. Tags: `homelab`, `nas`
6. Click **Test Connection** → should show ✓ Connected
7. Click **Save**

### Switch to the remote host

1. Use the endpoint switcher in the sidebar
2. Select `homelab-nas`
3. All pages now show data from the remote host
4. Container actions, image pulls, stack deploys — all work on the remote host

---

## Tutorial 6: Setting Up Nginx Load Balancing (Phase 4)

### Label your containers

In your docker-compose.yml:

```yaml
services:
  myapp:
    image: myapp:latest
    labels:
      marionette.lb.enabled: "true"
      marionette.lb.domain: "myapp.example.com"
      marionette.lb.port: "3000"
      marionette.lb.path: "/"
      marionette.lb.ssl: "true"
    deploy:
      replicas: 3
```

### Deploy and watch

1. Deploy the stack
2. Go to **Nginx** in marionette
3. You'll see `myapp.example.com` appear with 3 upstream servers
4. Click **View Config** to see the generated nginx configuration

### (Optional) Force regenerate

If you add a new replica or remove one, the config auto-updates via Docker events. To force a manual refresh, click **Regenerate**.

---

## Common Patterns

### Restart a crashed container

1. Find the container (✕ status) in the table
2. Select it
3. Click **Start**
4. If it crashes again, check **Logs** before retrying

### Roll back a bad deploy

1. Go to **Stacks**
2. Click **Edit** on the stack
3. Revert your changes (or paste the previous version)
4. Click **Save** then **Deploy**

### Check if a container is using too much memory

1. Go to container detail → **Stats** tab
2. Watch the memory bar over 30 seconds
3. If it's steadily growing, you have a memory leak
4. Set `mem_limit` in your compose file and redeploy

### Find which containers use an image

1. Go to **Images**
2. The "Used By" column shows container count
3. Click **Inspect** on an image → see repo digests and layer info

### Quick container restart (keyboard only)

1. Press `/` to focus filter
2. Type container name prefix → filtered to one row
3. Press `Enter` to select it
4. Press `R` (future: keyboard shortcut for restart)
5. Toast: "Container restarted"
