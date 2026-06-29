# Migration Strategy Implementation Plan — anchor-drift

**Date:** 2026-06-29
**Based on:** MIGRATION_STRATEGY_ANALYSIS.md (12 gaps identified)
**Principle:** Each wave is independently shippable and useful — no all-or-nothing dependency.

---

## Wave 1: Wire the Plumbing (Phase 1)

**Goal:** All frontend selections actually reach the backend and influence behavior. Zero behavioral change yet — just data flow.

### 1.1 Add missing fields to PlanRequest

File: `core/src/migration.rs` — PlanRequest struct (~line 132)

Add:
```rust
pub struct PlanRequest {
    pub source_endpoint: String,
    pub target_endpoint: String,
    pub container_id: String,
    #[serde(default = "default_transfer_method")]
    pub transfer_method: String,
    // NEW:
    #[serde(default)]
    pub compression: String,           // "pigz" | "zstd" | "lz4" | "none"
    #[serde(default)]
    pub post_options: PostOptions,
    #[serde(default)]
    pub volume_overrides: HashMap<String, VolumeOverride>,
    #[serde(default)]
    pub connection_resolutions: HashMap<String, ConnectionResolution>,
    #[serde(default)]
    pub target_stack_name: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PostOptions {
    pub start_on_target: bool,
    pub verify_connectivity: bool,
    pub remove_from_source: bool,
    pub rotate_credentials: bool,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VolumeOverride {
    pub transfer_method: Option<String>,
    pub custom_path: Option<String>,
    pub target_name: Option<String>,
    pub target_path: Option<String>,
    pub target_driver: Option<String>,
    pub skip: bool,
}
```

### 1.2 Add volume target fields to MigrationVolume

File: `core/src/models/migration.rs` — MigrationVolume struct

Add:
```rust
pub target_name: Option<String>,
pub target_path: Option<String>,
pub target_driver: Option<String>,
pub skip: bool,
```

### 1.3 Pass overrides through build_migration_plan

File: `core/src/migration.rs` — build_migration_plan()

- Accept `volume_overrides: &HashMap<String, VolumeOverride>` parameter
- When constructing each MigrationVolume, merge override fields
- Set `compressed` based on `compression` parameter (currently always `true`)
- Store `post_options` in MigrationPlan (add field)

### 1.4 Frontend: capture and send volume_overrides

File: `frontend/src/pages/Migration.jsx`
- handleStrategyUpdate: capture volume_overrides from MigrationPlan callback
- Add `volumeOverrides` state at page level
- Include in dry-run API call body

### 1.5 Frontend: add volume target columns

File: `frontend/src/components/MigrationPlan.jsx`
- Volume overrides table: add Target Name, Target Path, Skip columns
- Wire to handleVolumeOverride

**Verification:** cargo check ✅, npm run build ✅
**Risk:** Low — adding fields, no behavioral change
**Files:** ~5 files, ~80 lines

---

## Wave 2: Real Transfer Methods (Phase 2)

**Goal:** Each transfer method produces different, appropriate commands. The user's selection matters.

### 2.1 Refactor command generation

File: `core/src/migration.rs` — build_migration_plan() lines 332-457

Extract command generation into separate functions called by transfer_method:

```rust
match transfer_method {
    "scp" => build_scp_commands(&plan, &volumes, &has_remote_target),
    "rsync-over-ssh" => build_rsync_commands(&plan, &volumes, &has_remote_target, ssh_host),
    "pipe-direct" => build_pipe_commands(&plan, &volumes, &has_remote_target, ssh_host),
    "export-s3" => build_s3_commands(&plan, &volumes, &has_remote_target),
    _ => build_pipe_commands(...),  // default
}
```

### 2.2 SCP commands (keep current)

Current tar export + scp approach. Add pre-create: `ssh user@host "mkdir -p /tmp/marionette"`.

```
# On source:
docker run --rm -v vol:/data -v /tmp/marionette:/out alpine tar czf /out/{id}_{name}.tar.gz -C /data .
scp /tmp/marionette/{id}_{name}.tar.gz user@{host}:/tmp/marionette/

# On target:
docker run --rm -v {name}:/data -v /tmp/marionette:/in alpine tar xzf /in/{id}_{name}.tar.gz -C /data
```

### 2.3 Rsync-over-SSH commands (NEW)

Rsync for delta-transfer. No tar files — direct sync.

```
# Stop container on source first
ssh user@{host} "mkdir -p /tmp/marionette"

# Rsync volume data
docker run --rm -v {name}:/data alpine tar czf - -C /data . | \
  ssh user@{host} "docker run --rm -i -v {name}:/data alpine tar xzf - -C /data"

# Or: direct rsync if accessible
# rsync -avz --progress /var/lib/docker/volumes/{name}/_data/ user@{host}:/var/lib/docker/volumes/{name}/_data/
```

### 2.4 Pipe-direct commands (keep current)

Current docker export | ssh docker import pattern. Already works.

### 2.5 Export-to-S3 commands (NEW — admin instructions)

Marionette doesn't hold AWS credentials. Generate documented steps:

```
# === S3 EXPORT (requires aws CLI + credentials) ===
# On source:
docker run --rm -v {name}:/data -v /tmp/marionette:/out alpine tar czf /out/{id}_{name}.tar.gz -C /data .
aws s3 cp /tmp/marionette/{id}_{name}.tar.gz s3://{bucket}/marionette/{id}_{name}.tar.gz

# On target:
aws s3 cp s3://{bucket}/marionette/{id}_{name}.tar.gz /tmp/marionette/{id}_{name}.tar.gz
docker run --rm -v {name}:/data -v /tmp/marionette:/in alpine tar xzf /in/{id}_{name}.tar.gz -C /data
```

### 2.6 Per-volume transfer method

Use `vol.transfer_method` (already set per-volume) to branch command generation per volume, not per migration. A NFS volume gets export-s3 even if global is scp.

**Verification:** cargo check ✅, npm run build ✅
**Risk:** Medium — command format changes, but each method is isolated
**Files:** ~1 file (migration.rs), ~200 lines

---

## Wave 3: Volume Target Management (Phase 3)

**Goal:** Volumes can be renamed, remapped, driver-changed on target. Network volumes get proper creation commands.

### 3.1 Volume creation on target

For each volume being migrated to a remote target, generate pre-creation commands:

```
# Create volume on target (if network driver)
ssh user@{host} "docker volume create \
  --driver {target_driver or driver} \
  --opt type=nfs \
  --opt device=:/path \
  --opt o=addr={nfs_host},rw \
  {target_name or name}"
```

Uses `MigrationVolume.options` (already extracted from Docker inspect) to rebuild driver options.

### 3.2 Volume rename in commands

When `target_name` is set, use it in all target-side commands instead of `vol.name`:

```
# Without rename:
docker run --rm -v {vol.name}:/data ...

# With rename:
docker run --rm -v {vol.target_name}:/data ...
```

### 3.3 Skip volumes

When `vol.skip` is true, skip all commands for that volume. Add comment:

```
# Volume '{name}' skipped per user configuration
```

### 3.4 Same-host volume rename

For same-host migration with renamed volumes:

```
docker volume create --name {target_name}
docker run --rm -v {source_name}:/from -v {target_name}:/to alpine cp -a /from/. /to/
docker volume rm {source_name}  # optional, with warning
```

### 3.5 Network volume handling

For NFS/CIFS/cloud volumes, generate `docker volume create` with driver options instead of `tar` transfer:

```
# For NFS volume 'shared-data':
ssh user@{host} "docker volume create \
  --driver local \
  --opt type=nfs \
  --opt o=addr=192.168.1.50,rw,nfsvers=4 \
  --opt device=:/exports/data \
  shared-data"
```

**Verification:** cargo check ✅, npm run build ✅
**Risk:** Medium-High — new docker commands, need careful testing
**Files:** ~2 files, ~150 lines

---

## Wave 4: Compression + SSH Config (Phases 4-5)

### 4.1 Compression in commands

Map `compression` field to actual tar flags:

| compression | tar flag | extension |
|-------------|----------|-----------|
| `pigz` | `--use-compress-program=pigz` | `.tar.gz` |
| `zstd` | `--zstd` | `.tar.zst` |
| `lz4` | `--lz4` | `.tar.lz4` |
| `none` | (none) | `.tar` |
| default | `czf` (gzip) | `.tar.gz` |

Both source tar creation and target tar extraction must use the same flags.

### 4.2 SSH configuration

File: `core/src/models/endpoint.rs` or `core/src/migration.rs` PlanRequest

Add:
```rust
pub ssh_user: Option<String>,    // default: "root"
pub ssh_port: Option<u16>,       // default: 22
pub ssh_identity: Option<String>, // path to identity file
```

Use in command generation instead of hardcoded `user@`:
```
ssh -p {port} -i {identity} {user}@{host} "..."
```

These could be set per-endpoint (on the Endpoints page) or per-migration (in the strategy step).

### 4.3 Frontend: SSH config UI

Add to MigrationPlan or Migration.jsx strategy step:
- SSH User input (default: "root")
- SSH Port input (default: 22)

---

## Wave 5: Polish (Phases 6-7)

### 5.1 Bind mount handling

For bind mounts, generate rsync commands to transfer directory contents:

```
# Bind mount source: /host/path → target: /host/path
ssh user@{host} "mkdir -p /host/path"
rsync -avz --progress /host/path/ user@{host}:/host/path/
```

Requires the source path from Docker inspect (`mount.source`).

### 5.2 Compose file discovery

Detect compose file path from container labels or stack directory. Current code assumes `docker-compose.yml` in `~` directory on target. Better: include a "Compose file path" input in the frontend, or detect from `marionette.stack` label.

---

## Implementation Sequence

| Wave | Phases | Dependencies | Risk | Effort |
|------|--------|-------------|------|--------|
| **1** | 1 | None | Low | ~2 cycles |
| **2** | 2 | Wave 1 | Medium | ~2 cycles |
| **3** | 3 | Wave 1 | Med-High | ~3 cycles |
| **4** | 4-5 | Wave 1 | Low-Med | ~2 cycles |
| **5** | 6-7 | Wave 3 | Low | ~2 cycles |

**Recommended first move:** Execute Wave 1+2 together — wire plumbing AND implement transfer method branching. This gives immediate user-visible value: selecting rsync actually generates rsync commands.

---

## Files Affected (Total)

| File | Waves |
|------|-------|
| `core/src/models/migration.rs` | 1, 3 — add fields to MigrationVolume, MigrationPlan |
| `core/src/migration.rs` | 1, 2, 3, 4, 5 — PlanRequest, command builders, compression, SSH |
| `core/src/models/endpoint.rs` | 4 — SSH config on endpoint |
| `frontend/src/pages/Migration.jsx` | 1, 4 — capture volume_overrides, SSH UI |
| `frontend/src/components/MigrationPlan.jsx` | 1, 3, 4 — volume target columns, skip, SSH inputs |

---

*End of strategy.*
