# Compose-Template Migration — Full Design

**Concept:** Replace the 9-step wizard with a single-page compose template view. Source is read-only, target is fully editable. Marionette shows the diff and executes the transfer.

**Why:** The wizard is abstract — you configure "transfer methods" and "volume overrides" without seeing the actual result. A compose file IS the specification. Editing it directly is intuitive, faster, and handles cross-architecture, path remapping, image changes, and env var rewrites in one unified view.

---

## 1. Single-Page Layout

```
┌──────────────────────────────────────────────────────────────────────┐
│  Migration: my-app  (source: rpi4 → target: homelab)                 │
│  ⚠ ARM64 → AMD64   [Source] [Target]                                │
├────────────────────────────┬─────────────────────────────────────────┤
│  SOURCE COMPOSE            │  TARGET COMPOSE (editable)               │
│  (read-only, from stack)   │                                          │
│                             │                                          │
│  services:                  │  services:                              │
│    app:                     │    app:                                 │
│      image: myapp:arm64     │      image: myapp:amd64     ✏️           │
│      volumes:               │      volumes:                           │
│        - data:/var/data     │        - app-data:/opt/data  ✏️          │
│        - logs:/var/log      │        - logs:/var/log                  │
│      ports:                 │      environment:                       │
│        - "3000:3000"        │        NODE_ENV: production   ✏️          │
│      environment:           │                                          │
│        NODE_ENV: dev        │                                          │
│                             │                                          │
│  volumes:                   │  volumes:                               │
│    data:                    │    app-data:                            │
│                             │      driver_opts:                       │
│                             │        type: nfs              ✏️          │
│                             │        device: :/exports/data ✏️          │
│    logs:                    │    logs:                                │
│                             │                                          │
├─────────────────────────────┴─────────────────────────────────────────┤
│                                                                        │
│  ── Transfer Plan ─────────────────────────────────────────────────   │
│                                                                        │
│  📦 Volumes (4.2 GB total):                                           │
│     • data → app-data (/opt/data)                 2.3 GB  [transfer]  │
│     • logs → logs (same)                          1.9 GB  [transfer]  │
│                                                                        │
│  🔧 Environment changes:                                              │
│     NODE_ENV: dev → production                                        │
│                                                                        │
│  🔄 Image rebuilds needed (ARM→AMD):                                  │
│     • myapp:arm64 → myapp:amd64                                       │
│                                                                        │
│  ⚠ Port 3000 removed on target                                        │
│  ⚠ Volume 'data' renamed to 'app-data', driver changed to NFS         │
│                                                                        │
│  ┌──────────────────────────────────────────────────────────────┐    │
│  │  [Transfer Volumes]  [Deploy Compose]  [Transfer + Deploy]   │    │
│  └──────────────────────────────────────────────────────────────┘    │
│                                                                        │
│  ── Progress ─────────────────────────────────────────────────────   │
│                                                                        │
│  data → app-data  ████████████████████  2.3 GB  ✓ done                │
│  logs → logs      ████████░░░░░░░░░░░░  0.8 GB  ⏳ transferring...     │
│                                                                        │
└──────────────────────────────────────────────────────────────────────┘
```

---

## 2. Architecture Detection & Handling

```
┌──────────────────────────────────────────────────────────────────────────┐
│                         ARCHITECTURE MATRIX                              │
├──────────────┬──────────────┬────────────────────────────────────────────┤
│ Source       │ Target       │ Behavior                                   │
├──────────────┼──────────────┼────────────────────────────────────────────┤
│ ARM64        │ ARM64        │ ✅ Same — no rebuild needed                │
│ AMD64        │ AMD64        │ ✅ Same — no rebuild needed                │
│ ARM64        │ AMD64        │ ⚠ Volumes transfer fine. Images MUST       │
│              │              │   rebuild. Marionette marks each image.     │
│ AMD64        │ ARM64        │ ⚠ Same — volumes OK, images need rebuild   │
│ multi-arch   │ any          │ ✅ Image works on both — no warning         │
│ unknown      │ any          │ ⚠ Warn — can't verify compatibility        │
└──────────────┴──────────────┴────────────────────────────────────────────┘
```

Architecture detection via Docker API:
- `docker info` → `Architecture` field (aarch64, x86_64)
- `docker inspect <image>` → `Architecture` field in image manifest
- Multi-arch images: `docker manifest inspect` or check if image has multiple platform entries

---

## 3. Data Flow

```
┌─────────────┐     ┌──────────────┐     ┌─────────────┐
│ SOURCE       │     │  MARIONETTE   │     │  TARGET      │
│ Docker Host  │     │  (proxy)      │     │  Docker Host │
│              │     │               │     │              │
│ 1. Inspect   │────►│ 2. Generate   │     │              │
│    compose   │     │    compose    │     │              │
│    stack     │     │    template   │     │              │
│              │     │               │     │              │
│              │     │ 3. User edits │     │              │
│              │     │    target     │     │              │
│              │     │    compose    │     │              │
│              │     │               │     │              │
│ 4. Create    │     │               │     │              │
│    alpine    │     │               │     │              │
│    tar       │     │               │     │              │
│    container │     │               │     │              │
│              │     │               │     │              │
│ 5. Tar       │────►│ 6. Byte pipe  │────►│ 7. Alpine    │
│    stream    │     │    (bollard   │     │    tar       │
│    (stdout)  │     │     attach)   │     │    extract   │
│              │     │               │     │    (stdin)   │
│              │     │               │     │              │
│              │     │               │     │ 8. Save      │
│              │     │               │     │    compose   │
│              │     │               │     │    to target │
│              │     │               │     │    stacks    │
│              │     │               │     │              │
│              │     │               │     │ 9. Deploy    │
│              │     │               │     │    compose   │
│              │     │               │     │    up -d     │
└─────────────┘     └──────────────┘     └─────────────┘
```

---

## 4. Composer Template — Diff Detection

When the user edits the target compose, Marionette diffs against the source and categorizes changes:

| Change Type | Detected By | Action |
|-------------|-------------|--------|
| Volume renamed | `volumes:` key changed | Transfer with new name |
| Volume path changed | mount path changed inside service | Extract to new path |
| Volume driver changed | `driver:` or `driver_opts:` changed | Create with new driver on target |
| Volume added | New entry in `volumes:` | Create only, no transfer |
| Volume removed | Entry deleted from `volumes:` | Skip transfer, warn about orphaned data |
| Image changed | `image:` value differs | Warn — manual rebuild needed |
| Env var changed | `environment:` value differs | Apply on target compose |
| Port changed/removed | `ports:` differs | Apply on target compose |
| New service added | New service block | Deploy on target only |
| Service removed | Service block deleted | Warn — service won't run on target |

---

## 5. Backend API

### 5.1 GET /api/stacks/{name}/compose

Returns the raw compose YAML from the source stack.

```
GET /api/stacks/my-app/compose?endpoint=uuid-source

Response: 200 OK
Content-Type: text/yaml

services:
  app:
    image: myapp:arm64
    volumes:
      - data:/var/data
...
```

### 5.2 POST /api/migration/transfer

Executes the volume transfers via bollard direct pipe.

```json
Request:
{
  "source_endpoint": "uuid-source",
  "target_endpoint": "uuid-target",
  "transfers": [
    {
      "source_volume": "data",
      "target_volume": "app-data",
      "target_path": "/opt/data",
      "size_bytes": 2415919104
    },
    {
      "source_volume": "logs",
      "target_volume": "logs",
      "target_path": null,
      "size_bytes": 2040109465
    }
  ]
}

Response:
{
  "results": [
    {
      "source_volume": "data",
      "target_volume": "app-data",
      "target_path": "/opt/data",
      "bytes_transferred": 2415919104,
      "status": "success",
      "duration_ms": 45230
    },
    {
      "source_volume": "logs",
      "target_volume": "logs",
      "bytes_transferred": 0,
      "status": "failed",
      "error": "Volume 'logs' not found on source",
      "duration_ms": 120
    }
  ],
  "total_bytes": 2415919104,
  "status": "partial_success"
}
```

### 5.3 POST /api/stacks/deploy

Deploys the edited compose to the target endpoint.

```json
Request:
{
  "endpoint": "uuid-target",
  "stack_name": "my-app",
  "compose_yaml": "services:\n  app:\n    image: myapp:amd64\n..."
}

Response:
{
  "status": "deployed",
  "stack_name": "my-app",
  "services": ["app"]
}
```

---

## 6. Frontend Component Architecture

### New page: `MigrationCompose.jsx`

```
MigrationCompose
├── ArchitectureBanner        — "⚠ ARM64 → AMD64" warning bar
├── SourceTargetSelector      — source/target endpoint dropdowns
├── SplitPane
│   ├── ComposeEditor (source, readOnly=true)
│   │   └── YamlEditor (CodeMirror, monospace, syntax highlight)
│   └── ComposeEditor (target, readOnly=false)
│       └── YamlEditor (editable, live diff highlight)
├── DiffPanel                 — categorized change list
│   ├── VolumeChanges         — renamed, path changed, driver changed
│   ├── ImageChanges          — rebuild warnings
│   ├── EnvVarChanges         — key: old→new
│   └── StructuralChanges     — services added/removed, ports changed
├── ActionBar
│   ├── Button: Transfer Volumes
│   ├── Button: Deploy Compose
│   └── Button: Transfer + Deploy
└── TransferProgress          — per-volume progress bars
    └── VolumeProgress         — name, progress bar, bytes, status
```

### Composer Diff Algorithm (frontend)

```javascript
function diffCompose(source, target) {
  const changes = {
    volumes: [],      // {source_name, target_name, target_path, source_driver, target_driver}
    images: [],       // {service, source_image, target_image, needs_rebuild}
    envVars: [],      // {service, key, source_value, target_value}
    ports: [],        // {service, added: [], removed: [], changed: []}
    services: [],     // {added: [], removed: []}
    architecture: {source: 'aarch64', target: 'x86_64', mismatch: true}
  };

  // Compare volumes section
  for (const [name, vol] of Object.entries(target.volumes || {})) {
    const src = source.volumes?.[name];
    if (!src) { changes.volumes.push({...vol, action: 'created'}); continue; }
    if (src.driver !== vol.driver || JSON.stringify(src.driver_opts) !== JSON.stringify(vol.driver_opts)) {
      changes.volumes.push({source_name: name, target_name: name, action: 'driver_changed'});
    }
  }

  // Compare services
  for (const [svcName, svc] of Object.entries(target.services || {})) {
    const src = source.services?.[svcName];
    if (!src) { changes.services.push({name: svcName, action: 'added'}); continue; }

    // Images
    if (src.image !== svc.image) {
      changes.images.push({service: svcName, source_image: src.image, target_image: svc.image});
    }

    // Env vars
    const allKeys = new Set([...Object.keys(src.environment || {}), ...Object.keys(svc.environment || {})]);
    for (const key of allKeys) {
      if (src.environment?.[key] !== svc.environment?.[key]) {
        changes.envVars.push({service: svcName, key, source: src.environment?.[key], target: svc.environment?.[key]});
      }
    }

    // Volumes in service
    for (const v of svc.volumes || []) {
      const [srcVol, srcPath] = parseVolume(v);
      const srcMatch = src.volumes?.find(s => parseVolume(s)[0] === srcVol);
      // ... compare paths, detect renames
    }
  }

  for (const [svcName] of Object.entries(source.services || {})) {
    if (!target.services?.[svcName]) {
      changes.services.push({name: svcName, action: 'removed'});
    }
  }

  return changes;
}
```

---

## 7. Volume Transfer Engine

### `core/src/transfer.rs`

```rust
use bollard::Docker;
use bollard::container::{
    Config, CreateContainerOptions, StartContainerOptions,
    RemoveContainerOptions, WaitContainerOptions,
};
use bollard::exec::{CreateExecOptions, StartExecResults};
use futures::StreamExt;
use tokio::io::AsyncWriteExt;

pub struct TransferRequest {
    pub source_volume: String,
    pub target_volume: String,
    pub target_path: Option<String>,
}

pub struct TransferResult {
    pub source_volume: String,
    pub target_volume: String,
    pub target_path: String,
    pub bytes_transferred: u64,
    pub status: TransferStatus,
    pub error: Option<String>,
    pub duration_ms: u64,
}

pub enum TransferStatus {
    Success,
    Failed,
    Skipped,
}

pub async fn transfer_volume(
    source: &Docker,
    target: &Docker,
    req: &TransferRequest,
) -> TransferResult {
    let start = std::time::Instant::now();
    let target_path = req.target_path.as_deref().unwrap_or("/data");

    // Step 1: Create source alpine container (tar export)
    let source_container = source.create_container(
        Some(CreateContainerOptions {
            name: format!("marionette-src-{}", &req.source_volume[..12.min(req.source_volume.len())]),
            ..Default::default()
        }),
        Config {
            image: Some("alpine:latest"),
            cmd: Some(vec!["tar", "cf", "-", "-C", "/from", "."]),
            ..Default::default()
        },
        // Mount source volume at /from
        // ...
    ).await?;

    // Step 2: Create target alpine container (tar import)
    let target_container = target.create_container(
        Some(CreateContainerOptions {
            name: format!("marionette-tgt-{}", &req.target_volume[..12.min(req.target_volume.len())]),
            ..Default::default()
        }),
        Config {
            image: Some("alpine:latest"),
            cmd: Some(vec!["tar", "xf", "-", "-C", target_path]),
            attach_stdin: Some(true),
            ..Default::default()
        },
        // Mount target volume at target_path
        // ...
    ).await?;

    // Step 3: Start source, attach stdout
    let mut source_stream = source.start_container(
        &source_container.id,
        None::<StartContainerOptions<String>>,
    ).await?;

    // Step 4: Start target, attach stdin
    // Bollard exec with attached I/O
    let exec = target.create_exec(
        &target_container.id,
        CreateExecOptions {
            attach_stdin: Some(true),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            cmd: Some(vec!["tar", "xf", "-", "-C", target_path]),
            ..Default::default()
        },
    ).await?;

    let StartExecResults::Attached { output, mut input } = target.start_exec(&exec.id, None).await?;

    // Step 5: Pipe source stdout → target stdin, tracking bytes
    let mut bytes: u64 = 0;
    while let Some(chunk) = source_stream.next().await {
        if let Ok(chunk) = chunk {
            bytes += chunk.len() as u64;
            input.write_all(&chunk).await;
        }
    }
    input.shutdown().await;

    // Step 6: Cleanup containers
    let _ = source.remove_container(&source_container.id, None::<RemoveContainerOptions>).await;
    let _ = target.remove_container(&target_container.id, None::<RemoveContainerOptions>).await;

    TransferResult {
        source_volume: req.source_volume.clone(),
        target_volume: req.target_volume.clone(),
        target_path: target_path.to_string(),
        bytes_transferred: bytes,
        status: TransferStatus::Success,
        error: None,
        duration_ms: start.elapsed().as_millis() as u64,
    }
}
```

---

## 8. Route Registration

```rust
// core/src/main.rs

// Existing
.route("/migration/analyze", post(migration::analyze_migration))
.route("/migration/plan", post(migration::plan_migration))
.route("/migration/dry-run", post(migration::dry_run_migration))
.route("/migration/{id}/execute", post(migration::execute_migration))

// New
.route("/migration/transfer", post(migration::transfer_volumes))    // volume transfer via bollard pipe
.route("/stacks/{name}/compose", get(stacks::get_compose))          // get raw compose YAML
.route("/stacks/deploy", post(stacks::deploy_compose))              // deploy compose to endpoint
```

---

## 9. Implementation Phases

### Phase A: Backend Transfer Engine
- `core/src/transfer.rs` — `transfer_volume()` function
- `core/src/migration.rs` — `transfer_volumes` handler
- `core/src/main.rs` — route registration
- **Verify:** `cargo check`

### Phase B: Backend Compose & Deploy
- `core/src/routes/stacks.rs` — `get_compose()`, `deploy_compose()`
- Architecture detection via Docker API
- **Verify:** `cargo check`

### Phase C: Frontend MigrationCompose Page
- `frontend/src/pages/MigrationCompose.jsx` — split-pane compose editor
- `frontend/src/components/ComposeEditor.jsx` — YamlEditor with readOnly prop
- `frontend/src/components/DiffPanel.jsx` — categorized change list
- `frontend/src/components/TransferProgress.jsx` — per-volume progress bars
- Replace wizard Step 3+ with "Edit Target Compose" button
- **Verify:** `npm run build`

### Phase D: Integration
- Wire Transfer + Deploy buttons to API
- Architecture banner with rebuild warnings
- Cross-architecture image handling
- **Verify:** `cargo check && npm run build`

---

## 10. Migration from Old Wizard

The old 9-step wizard remains accessible (legacy path) but the default flow becomes:

1. **Select container** (Step 1) — pick source endpoint + container
2. **Edit target compose** (new) — shown as the compose template view
3. **Review diff** (new) — categorized changes panel
4. **Transfer + Deploy** (new) — single page execution with progress

The existing Steps 2-8 (analyze, strategy, credentials, fixes, target, dry-run, execute) are collapsed into the compose template view. Step 9 (verify) becomes the TransferProgress component.

---

*End of design.*
