# Direct Proxy Transfer — Schematic Design

**Problem:** Current migration generates SSH commands that fail because Marionette's container has no SSH keys. The user needs to transfer volumes from source Docker host → target Docker host with custom path/name remapping.

**Solution:** Marionette proxies the transfer directly via bollard (Docker API), replacing SSH entirely. It already has Docker API connections to both source and target hosts.

---

## Architecture

```
┌─────────────────┐         ┌──────────────────┐         ┌─────────────────┐
│  Source Host     │  bollard│   Marionette      │  bollard│  Target Host     │
│  (Docker daemon) │◄────────┤   (Rust backend)   │────────►│  (Docker daemon) │
│                  │  attach │                   │  attach │                  │
│  ┌────────────┐  │  stdout │  ┌─────────────┐  │  stdin  │  ┌────────────┐  │
│  │ alpine tar │──┼────────►│  │  byte pipe   │──┼────────►│  │ alpine tar │  │
│  │ -v src:/from│ │         │  │  (in-memory) │  │         │  │ -v tgt:/to │  │
│  └────────────┘  │         │  └─────────────┘  │         │  └────────────┘  │
└─────────────────┘         └──────────────────┘         └─────────────────┘

1. Marionette creates exec session on SOURCE:  docker run alpine tar cf - -C /from .
2. Marionette reads the tar stream from source via bollard exec attach (stdout)
3. Marionette creates exec session on TARGET:  docker run -i alpine tar xf - -C /to
4. Marionette pipes the byte stream into target via bollard exec attach (stdin)
5. Done — no SSH, no temp files, no intermediate storage
```

---

## Data Flow (per volume)

```
POST /api/migration/{id}/transfer

Request body:
{
  "volume_name": "my-app-data",        // source volume name
  "target_name": "my-app-data-v2",     // optional: rename on target
  "target_path": "/opt/app/share",     // optional: custom extract path
  "source_endpoint": "uuid-123",       // source Docker host
  "target_endpoint": "uuid-456"        // target Docker host
}

Backend flow (Rust/bollard):

Step 1: Resolve Docker clients
  source_client = helpers::resolve_client(source_endpoint)
  target_client = helpers::resolve_client(target_endpoint)

Step 2: Create source container (tar export)
  source_client.create_container(CreateContainerOptions {
    image: "alpine:latest",
    cmd: ["tar", "cf", "-", "-C", "/from", "."],
    volumes: {source_volume: "/from"},
    attach_stdout: true,
    auto_remove: true,
  })

Step 3: Create target container (tar import)
  target_client.create_container(CreateContainerOptions {
    image: "alpine:latest",
    cmd: ["tar", "xf", "-", "-C", target_path],
    volumes: {target_volume: target_path},
    attach_stdin: true,
    auto_remove: true,
  })

Step 4: Start both containers with attached I/O
  source_output = source_client.start_container(..., attach_stdout)
  target_input = target_client.start_container(..., attach_stdin)

Step 5: Pipe stdout → stdin (streaming, chunked)
  while let Some(chunk) = source_output.next().await {
    target_input.write_all(&chunk).await;
  }

Step 6: Wait for both to complete, return results
  source_exit = source_client.wait_container(...)
  target_exit = target_client.wait_container(...)

Response:
{
  "volume_name": "my-app-data",
  "target_name": "my-app-data-v2",
  "target_path": "/opt/app/share",
  "bytes_transferred": 1048576000,
  "status": "success",
  "source_exit_code": 0,
  "target_exit_code": 0
}
```

---

## Frontend Flow

### Migration Wizard — Strategy Step (Step 3)

Volume overrides table (already exists from Wave 1):

| Volume | Size | Transfer | Target Name | Target Path | Skip |
|--------|------|----------|-------------|-------------|------|
| my-app-data | 2.3 GB | pipe-direct | my-app-data-v2 | /opt/app/share | ☐ |
| postgres-data | 1.1 GB | pipe-direct | [same] | [same] | ☐ |
| cache-vol | 0.5 GB | pipe-direct | [same] | [same] | ☑ skip |

- **Target Name:** Text input, placeholder = volume name. Renames volume on target.
- **Target Path:** Text input, placeholder = "Same as source". Custom extract path inside container.
- **Skip:** Checkbox, dims row. Excludes from transfer.

### Execute Step (Step 8)

Instead of SSH commands, shows the transfer progress:

```
─── Transferring volumes ───────────────────────────────────

  my-app-data → my-app-data-v2 (/opt/app/share)
  ████████████████████████ 2.3 GB / 2.3 GB  ✓ Complete

  postgres-data → postgres-data (default)
  ██████████████░░░░░░░░░░ 0.8 GB / 1.1 GB  ⏳ Transferring...

  cache-vol  ⏭ Skipped
```

---

## Backend API Changes

### New endpoint: `POST /api/migration/{id}/transfer`

**Request:**
```json
{
  "volumes": [
    {
      "name": "my-app-data",
      "target_name": "my-app-data-v2",
      "target_path": "/opt/app/share",
      "skip": false
    },
    {
      "name": "postgres-data",
      "target_name": null,
      "target_path": null,
      "skip": false
    }
  ],
  "source_endpoint": "uuid-123",
  "target_endpoint": "uuid-456"
}
```

**Response (streaming or batch):**
```json
{
  "migration_id": "abc-123",
  "results": [
    {
      "volume_name": "my-app-data",
      "target_name": "my-app-data-v2",
      "target_path": "/opt/app/share",
      "bytes_transferred": 2415919104,
      "status": "success",
      "duration_ms": 45000
    }
  ]
}
```

---

## Rust Implementation Outline

### New module: `core/src/transfer.rs`

```rust
pub struct TransferRequest {
    pub volumes: Vec<VolumeTransfer>,
    pub source_endpoint: String,
    pub target_endpoint: String,
}

pub struct VolumeTransfer {
    pub name: String,
    pub target_name: Option<String>,
    pub target_path: Option<String>,
    pub skip: bool,
}

pub struct TransferResult {
    pub volume_name: String,
    pub target_name: String,
    pub target_path: String,
    pub bytes_transferred: u64,
    pub status: String,  // "success" | "failed"
    pub error: Option<String>,
    pub duration_ms: u64,
}

pub async fn transfer_volume(
    source: &Docker,
    target: &Docker,
    vol: &VolumeTransfer,
) -> TransferResult {
    // 1. Resolve target name/path
    // 2. Create source alpine tar container
    // 3. Create target alpine tar container
    // 4. Start both, pipe stdout→stdin
    // 5. Track bytes transferred
    // 6. Wait for completion
    // 7. Return result
}
```

### Route registration (`core/src/main.rs`)
```rust
.route("/migration/{id}/transfer", post(migration::transfer_volumes))
```

---

## Changes from Current State

| Current (SSH-based) | New (Direct Proxy) |
|---------------------|-------------------|
| Generates `ssh user@host` commands | Uses bollard API connections |
| Requires SSH keys on Marionette container | No SSH needed |
| Commands run via `sh -c` | Direct Docker API calls |
| No progress tracking | Byte counting per volume |
| Target name/path ignored | Fully configurable per volume |
| Network volumes: comment only | Can recreate via Docker API |
| No error detail per volume | Per-volume status + error messages |

---

## Files Modified

| File | Change |
|------|--------|
| `core/src/transfer.rs` | **NEW** — volume transfer via bollard pipe |
| `core/src/migration.rs` | Add `transfer_volumes` handler, wire into execute flow |
| `core/src/main.rs` | Register `POST /migration/{id}/transfer` route |
| `frontend/src/pages/Migration.jsx` | Step 8: call transfer endpoint instead of execute endpoint, show progress per volume |
| `frontend/src/components/MigrationPlan.jsx` | Target Path column already exists — no change needed |

---

## Implementation Sequence

| Step | What | Risk |
|------|------|------|
| 1 | Create `core/src/transfer.rs` with `transfer_volume()` function | Medium |
| 2 | Wire into migration execute flow, add route | Low |
| 3 | Update frontend Step 8 to call transfer API with progress | Low |
| 4 | Remove SSH command generation for volume transfers | Low |

---

*End of schematic.*
