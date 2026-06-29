# Marionette Migration Strategy — Complete Analysis

**Date:** 2026-06-29  
**Scope:** Full end-to-end analysis of migration strategies, volume handling, command generation, and data flow.  
**Analysis only — no fixes.**

---

## 1. TRANSFER METHODS

### 1.1 Frontend Definition

**File:** `frontend/src/components/MigrationPlan.jsx`, lines 3–8

Four methods defined with ID, label, description, and icon:

| ID | Label | Description |
|----|-------|-------------|
| `scp` | SCP | Secure copy over SSH. Good for single files/small volumes. |
| `rsync-over-ssh` | Rsync over SSH | Delta-transfer over SSH. Best for large volumes with incremental changes. |
| `pipe-direct` | Pipe Direct | Direct pipe via SSH (docker export \| ssh docker import). No temp files. |
| `export-s3` | Export to S3 | Archive to S3 bucket, download on target. Good for cross-region. |

### 1.2 Frontend State Management

**File:** `frontend/src/pages/Migration.jsx`, lines 139–142

```js
const [transferMethod, setTransferMethod] = useState('rsync-over-ssh');
const [compression, setCompression] = useState('pigz');
const [postOptions, setPostOptions] = useState({});
```

Default transfer method at migration page level is `rsync-over-ssh`.

**File:** `frontend/src/components/MigrationPlan.jsx`, line 18

```js
const [transferMethod, setTransferMethod] = useState(plan.transferMethod || 'rsync-over-ssh');
```

MigrationPlan component defaults to `plan.transferMethod` or `rsync-over-ssh`.

### 1.3 Backend Default

**File:** `core/src/migration.rs`, lines 140–142

```rust
fn default_transfer_method() -> String {
    "pipe-direct".to_string()
}
```

The `PlanRequest` struct defaults to `pipe-direct` if no transfer_method is provided — in conflict with the frontend's `rsync-over-ssh` default.

### 1.4 CRITICAL FINDING: Transfer Method Has No Effect on Command Generation

**File:** `core/src/migration.rs`, function `build_migration_plan()`, lines 148–493

The `transfer_method` parameter is **accepted but never used to differentiate commands**. The command generation (lines 332–457) produces IDENTICAL commands regardless of whether the user picks `scp`, `rsync-over-ssh`, `pipe-direct`, or `export-s3`.

The only place transfer_method influences behavior:
- **Line 270:** Sets `vol.transfer_method` to the global `transfer_method` for cross-host (or `"local"` for same-host).
- **Line 507:** `analyze_migration` hardcodes `"scp"` regardless.

**What commands are actually generated (all methods are identical):**
1. Tar export via `docker run alpine` → `/tmp/marionette/{id}_{name}.tar.gz`
2. SCP comment: `# === ADMIN MUST EXECUTE THE FOLLOWING scp COMMAND MANUALLY ===`
3. Pipe-direct commands: `docker run ... tar czf - | ssh user@host "docker run ... tar xzf -"`
4. Compose deploy: `ssh user@host "cd ~ && docker compose up -d"`
5. Verify + Cleanup

**`rsync-over-ssh` and `export-s3` are completely unimplemented** — they are listed in the UI but produce exactly the same tar+scp+pipe commands as every other method.

---

## 2. VOLUME HANDLING

### 2.1 Volume Detection

**File:** `core/src/migration.rs`, lines 202–303

Volumes are discovered by iterating over `info.mounts` from the Docker inspect result:

- **Named volumes** (mount_type == `"volume"`, line 214): Detected when a mount has a `.name` field.
- **Bind mounts** (mount_type == `"bind"`, line 287): Path-based mounts. Warned about but NOT included in `volumes` vector.
- **Kernel paths** (lines 113–121, 289–294): `/proc`, `/sys`, `/var/run/docker`, `/var/run`, `/etc/hostname`, `/etc/hosts`, `/etc/resolv.conf` — auto-skipped with warning.

### 2.2 Driver Classification

**File:** `core/src/docker.rs`, lines 89–99 (`classify_driver`)

| Driver(s) | Category | Advice |
|-----------|----------|--------|
| `local`, `local-persist` | `filesystem` | `transfer` |
| `nfs`, `cifs`, `smb` | `network` | `reconnect` |
| `rclone` | `cloud` | `reconnect` |
| `rexray`, `cloudstor` | `cloud_block` | `reconnect` |
| `glusterfs` | `distributed` | `reconnect` |
| `btrfs`, `zfs` | `filesystem` | `transfer` |
| `overlay`, `tmpfs` | `ephemeral` | `transfer` |
| everything else | `unknown` | `warn` |

### 2.3 Transfer Method Suggestions

**File:** `core/src/docker.rs`, lines 103–111 (`suggest_transfer_method`)

| Driver(s) | Suggested Method |
|-----------|-----------------|
| `nfs`, `cifs`, `smb` | `export-s3` |
| `btrfs`, `zfs` | `rsync-over-ssh` |
| `local`, `local-persist` | `scp` |
| `overlay`, `tmpfs` | `pipe-direct` |
| everything else | `rsync-over-ssh` |

### 2.4 MigrationVolume Struct

**File:** `core/src/models/migration.rs`, lines 24–35

```rust
pub struct MigrationVolume {
    pub name: String,                     // Volume name
    pub driver: String,                   // "local", "nfs", etc.
    pub driver_category: String,          // "filesystem", "network", "cloud", etc.
    pub size_bytes: Option<u64>,          // From Docker volume inspect
    pub shared: bool,                     // Always false (line 281)
    pub transfer_method: String,          // "local" for same-host, else global transfer_method
    pub default_transfer_method: String,  // From suggest_transfer_method()
    pub options: Option<serde_json::Value>, // Driver options + labels from volume inspect
}
```

### 2.5 Volume Size & Pre-Flight Warnings

**File:** `core/src/migration.rs`, lines 222–261

- Volume size fetched via `source_docker.inspect_volume(vol_name)`
- If cross-host and size ≥ 500 GB: `warnings.push("Volume 'X' is Y — pre-flight disk check on target required")` (lines 230–236)
- Volume options extracted: `driverOpts` map + `labels` map (lines 239–256)

### 2.6 Same-Host vs Cross-Host Volume Handling

**File:** `core/src/migration.rs`, lines 267–271

```rust
let vol_transfer_method = if same_host {
    "local".to_string()
} else {
    transfer_method.to_string()
};
```

On same-host migration, volumes keep `transfer_method = "local"` — the volumes are not transferred at all; only `docker stop` + `docker compose up -d` are issued (lines 444–456).

---

## 3. COMMAND GENERATION

### 3.1 Complete List of `commands.push()` Calls

**File:** `core/src/migration.rs`, lines 332–457

All commands.push() locations:

| Lines | What | Condition |
|-------|------|-----------|
| 372–374 | `# Export volume: {name}` comment | `driver_category == "filesystem" OR "unknown"` |
| 376–379 | `docker run --rm -v {name}:/data -v /tmp/marionette:/out alpine:latest \` | same |
| 380–383 | `tar czf /out/{id}_{name}.tar.gz -C /data .` | same |
| 384–391 | `# === ADMIN MUST EXECUTE THE FOLLOWING scp COMMAND MANUALLY ===` + scp command | same + `has_remote_target` |
| 394–398 | `# Volume '{name}' uses driver '{driver}' ({category}) — may require reconnection...` | NOT filesystem/unknown |
| 402–403 | `# === PIPE-DIRECT TRANSFER (recommended) ===` + `# On source host:` | `has_remote_target` |
| 408–411 | `docker run --rm -v {name}:/data alpine:latest tar czf - -C /data . | \` | filesystem/unknown + has_remote_target |
| 412–414 | `ssh user@{host} "docker run --rm -i -v {name}:/data alpine:latest tar xzf - -C /data"` | same |
| 420–424 | `# === COMPOSE DEPLOY on Target ===` + scp + compose up | `has_remote_target` |
| 431–434 | `# === VERIFY ===` + ssh docker ps | `has_remote_target` |
| 438–442 | `# === CLEANUP ===` + rm -rf | `has_remote_target` |
| 447 | `# === SAME-HOST MIGRATION ===` | NOT has_remote_target |
| 448 | `# Stop source container` | same-host |
| 449 | `docker stop {container_name}` | same-host |
| 451 | `# Deploy compose file` | same-host |
| 452 | `docker compose up -d` | same-host |
| 454 | `# === VERIFY ===` | same-host |
| 455 | `# docker ps --filter name={container_name}` | same-host |

### 3.2 How Volumes Are Referenced in Commands

- **Volume name** used directly as `-v {vol.name}:/data` (line 377, 409, 413)
- **Tar filename**: `/tmp/marionette/{migration_id}_{vol.name}.tar.gz` (line 371)
- **No volume creation**: Assumes volume already exists on target
- **No driver options**: Never passes `--driver` or `--opt` flags
- **SSH user hardcoded**: Always `user@` prefix (lines 389, 413, 422, 426, 433, 441)

### 3.3 Port Stripped from SSH Host

**File:** `core/src/migration.rs`, lines 356–365

```rust
let ssh_host = if has_remote_target {
    if let Some(rest) = target_conn.strip_prefix("tcp://") {
        rest.split(':').next().unwrap_or("unknown-host")
    } else {
        "unknown-host"
    }
} else { "" };
```

If target connection is `tcp://192.168.1.100:2375`, the SSH host becomes `192.168.1.100` — the Docker API port is correctly stripped, but there's no SSH port handling (`-p` flag would be needed for non-standard SSH ports).

---

## 4. DATA FLOW — END TO END

### 4.1 Route Registration

**File:** `core/src/main.rs`, lines 209–213

```
POST /migration/analyze    → migration::analyze_migration
POST /migration/plan       → migration::plan_migration
POST /migration/dry-run    → migration::dry_run_migration
GET  /migration/{id}       → migration::get_migration
POST /migration/{id}/rollback → migration::rollback_migration
POST /migration/{id}/execute  → migration::execute_migration
```

Frontend prepends `/api/` (auxgate strips this before forwarding).

### 4.2 Complete Data Flow

#### Step 1: Analyze (Frontend → Backend)

**Frontend:** `frontend/src/pages/Migration.jsx`, lines 231–234
```js
api.post('/api/migration/analyze', {
    source_endpoint: sourceEndpoint,
    container_id: selectedContainer.Id || selectedContainer.id,
});
```

**Backend Request:** `core/src/migration.rs`, lines 126–129
```rust
pub struct AnalyzeRequest {
    pub source_endpoint: String,
    pub container_id: String,
}
```

**Backend Handler:** `core/src/migration.rs`, lines 497–528
```rust
let plan = build_migration_plan(
    &state,
    &body.source_endpoint,
    None,                // No target → same_host = true
    &body.container_id,
    "scp",               // Hardcoded, ignored for command gen
    false,               // generate_commands = false → NO commands
).await?;
```

**Response:** Full `MigrationPlan` struct with volumes, warnings, db_connections, env_vars. `commands` vector is empty.

#### Step 2: Strategy (Frontend Only)

**Frontend:** `frontend/src/pages/Migration.jsx`, lines 246–251
```js
const handleStrategyUpdate = useCallback((s) => {
    setStrategy(s);
    if (s.transferMethod) setTransferMethod(s.transferMethod);
    if (s.compression) setCompression(s.compression);
    if (s.post_options) setPostOptions(s.post_options);
}, []);
```

`MigrationPlan.jsx` component calls `onUpdate` with:
```js
{
    transfer_method: transferMethod,
    compression,
    post_options: postOptions,
    volume_overrides: volumeOverrides
}
```

**VOLUME OVERRIDES ARE NOT STORED AT PAGE LEVEL** — `handleStrategyUpdate` only captures transfer_method, compression, and post_options. The `volume_overrides` from `MigrationPlan.jsx` are dropped.

#### Step 3: Dry Run (Frontend → Backend)

**Frontend:** `frontend/src/pages/Migration.jsx`, lines 321–329
```js
api.post('/api/migration/dry-run', {
    source_endpoint: sourceEndpoint,
    target_endpoint: targetEndpoint,
    container_id: selectedContainer?.Id || selectedContainer?.id,
    transfer_method: transferMethod,
    compression,                           // ← SENT but NEVER USED by backend
    post_options: postOptions,             // ← SENT but NEVER USED by backend
    connection_resolutions: connectionResolutions, // ← SENT but NEVER USED
    target_stack_name: targetStackName || undefined, // ← SENT but NEVER USED
});
```

**Backend PlanRequest struct:** `core/src/migration.rs`, lines 132–138
```rust
pub struct PlanRequest {
    pub source_endpoint: String,
    pub target_endpoint: String,
    pub container_id: String,
    #[serde(default = "default_transfer_method")]
    pub transfer_method: String,
}
```

**CRITICAL:** `PlanRequest` only has 4 fields. The following frontend fields are IGNORED:
- `compression` — not in PlanRequest, never deserialized
- `post_options` — not in PlanRequest
- `connection_resolutions` — not in PlanRequest
- `target_stack_name` — not in PlanRequest
- `volume_overrides` — never even sent

**Backend Handler:** `core/src/migration.rs`, lines 574–612
```rust
let plan = build_migration_plan(
    &state,
    &body.source_endpoint,
    Some(&body.target_endpoint),  // target → same_host computed
    &body.container_id,
    &body.transfer_method,        // The ONE field that reaches backend
    true,                          // generate_commands = true
).await?;
```

#### Step 4: Execute (Frontend → Backend)

**Frontend:** `frontend/src/pages/Migration.jsx`, line 359
```js
api.post(`/api/migration/${migrationId}/execute`, {});
```

**Backend:** `core/src/migration.rs`, lines 719–846
- Fetches plan from in-memory store
- Coalesces multi-line commands via `coalesce_commands()` (lines 679–717)
- Executes each command via `sh -c` with 120s timeout
- Returns results per command

### 4.3 Difference Between analyze, plan, and dry-run

| Endpoint | generate_commands | transfer_method | target | Stores in memory |
|----------|------------------|-----------------|--------|-----------------|
| `analyze` | `false` | hardcoded `"scp"` | None | Yes |
| `plan` | `true` | from body | from body | Yes |
| `dry-run` | `true` | from body | from body | Yes |

All three call the **same** `build_migration_plan()` function. The only difference:
- `analyze`: No commands, no target → `same_host = true`, safer warnings
- `plan`: Commands, target endpoint
- `dry-run`: Identical to `plan` but wraps result in `{"dry_run": true, "plan": ...}`

---

## 5. GAPS — What's Missing for Volume Target Management

### 5.1 Transfer Methods Not Implemented

| Method | Frontend | Backend Command Gen | Status |
|--------|----------|---------------------|--------|
| `scp` | Defined (line 4) | Always generated | ✓ Present but always generated regardless of selection |
| `rsync-over-ssh` | Defined (line 5) | **NOT generated** | ✗ No rsync commands exist |
| `pipe-direct` | Defined (line 6) | Always generated | ✓ Present but always generated regardless of selection |
| `export-s3` | Defined (line 7) | **NOT generated** | ✗ No S3 commands exist |

**Effect:** All four methods produce identical tar+scp+pipe commands. User selection is meaningless.

### 5.2 Frontend Features with No Backend Support

| Frontend Feature | Source File:Line | Backend Field | Status |
|-----------------|------------------|---------------|--------|
| Compression selection | `MigrationPlan.jsx:19` | Not in `PlanRequest` | ✗ Ignored |
| Post-options (startOnTarget, etc.) | `MigrationPlan.jsx:20-25` | Not in `PlanRequest` | ✗ Ignored |
| Per-volume transfer overrides | `MigrationPlan.jsx:67-71` | Not sent to API | ✗ Never reaches backend |
| Per-volume custom_path | `MigrationPlan.jsx:260-266` | Not in `MigrationVolume` | ✗ Field doesn't exist backend |
| Target stack name | `Migration.jsx:329` | Not in `PlanRequest` | ✗ Ignored |
| Connection resolutions | `Migration.jsx:328` | Not in `PlanRequest` | ✗ Ignored |

### 5.3 Volume Management Gaps

1. **No per-volume transfer method** — `build_migration_plan()` uses a single global `transfer_method` for ALL volumes. The `vol.transfer_method` field is set per-volume (lines 267–271) but the command loop at lines 369–400 doesn't branch on it.

2. **No volume target name/path** — Commands assume the volume has the identical name on target (line 377, 409, 413: `-v {vol.name}:/data`). No rename/remap capability.

3. **No volume creation on target** — No `docker volume create` commands anywhere. Assumes volumes pre-exist.

4. **No driver option transfer** — `MigrationVolume.options` stores driverOpts + labels from `inspect_volume()` (lines 239–256, 284) but these are NEVER used in command generation.

5. **Non-filesystem volumes get no commands** — Network/cloud drivers only get a comment (lines 394–398): `"# Volume 'X' uses driver 'Y' (Z) — may require reconnection on target"`. No mount instructions, no `docker volume create --driver nfs ...` commands.

6. **No `docker volume create` for network volumes** — For NFS/CIFS volumes, the target needs `docker volume create --driver local --opt type=nfs --opt device=...`. This is entirely missing.

7. **Bind mount handling incomplete** — Bind mounts get a warning (line 297–300) but no commands to replicate the directory structure or transfer content.

8. **`MigrationVolume.shared` always false** — Line 281: `shared: false`. Never set to true. No shared volume detection or handling.

9. **`MigrationPlan.compressed` always true** — Line 487: `compressed: true`. Frontend compression selection (pigz/zstd/lz4/none) is ignored; tar czf always uses gzip.

10. **SSH configuration incomplete** — `user@` is hardcoded. No SSH port, no identity file, no SSH config path.

11. **No pre-create on target** — Missing: `ssh user@host "mkdir -p /tmp/marionette"` before scp.

12. **No compose file discovery** — The `# scp docker-compose.yml` command (line 422) assumes a specific file. No mechanism to find, reference, or provide the compose file.

13. **Volume inspector provides unused data** — `VolumeInspector.jsx` (line 64–71) gives migration advice but this data isn't fed into the migration plan.

### 5.4 What "Volume Target Management" Would Need

To make volume target management functional, the following would need to change:

1. **`MigrationVolume` needs:** `target_name`, `target_path`, `target_driver`, `target_options`
2. **`PlanRequest` needs:** `volume_overrides`, `compression`, `post_options`
3. **`build_migration_plan()` needs:** Per-volume command branching based on `vol.transfer_method`
4. **Command generation needs:** Actual rsync commands, S3 export/import commands, volume creation commands
5. **Network volumes need:** `docker volume create --driver X --opt ...` commands for target
6. **SSH needs:** Configurable user, port, identity file
7. **Bind mounts need:** Transfer commands or clear documentation on pre-requirements

### 5.5 Security Design (Working as Intended)

**File:** `core/src/migration.rs`, lines 2–7

Marionette NEVER holds or transmits SSH keys. It generates shell commands that the admin runs manually. This is intentional and working correctly. All credential masking in env vars (lines 41–53) and volume options (`sanitize_options` in docker.rs lines 114–144) is functional.

### 5.6 Audit Trail (Working)

Every migration action (analyze, plan, dry-run, get, rollback, execute) records to the audit log. Command execution records individual command results. This is complete and well-implemented.

---

## 6. SUMMARY OF KEY FILES

| File | Lines | Role |
|------|-------|------|
| `core/src/migration.rs` | 846 | Core migration logic: `build_migration_plan()`, API handlers, command execution |
| `core/src/docker.rs` | 160 | Driver classification, transfer method suggestions, client creation |
| `core/src/models/migration.rs` | 57 | `MigrationPlan`, `MigrationVolume`, `DbConnection`, `CommandExecutionResult` structs |
| `core/src/models/endpoint.rs` | 63 | `DockerEndpoint`, `EndpointStatus`, `EndpointInfo` structs |
| `core/src/registry.rs` | 276 | `EndpointRegistry`: manages endpoint lifecycle and client caching |
| `core/src/helpers.rs` | 35 | `resolve_client()`, `resolve_endpoint_id()` |
| `core/src/main.rs` | 231 | Route registration, app startup |
| `frontend/src/pages/Migration.jsx` | 1393 | 9-step migration wizard: source, analyze, strategy, credentials, fixes, target, dry-run, execute, verify |
| `frontend/src/components/MigrationPlan.jsx` | 315 | Transfer method selection, compression, post-options, per-volume overrides |
| `frontend/src/components/VolumeInspector.jsx` | 192 | Volume detail view with driver categorization and migration advice |
| `frontend/src/api/client.js` | 89 | API client: GET/POST/PUT/PATCH/DELETE with auth key |

---

*End of analysis.*
