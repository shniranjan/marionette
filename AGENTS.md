# Marionette AI Agent Instructions

> Marionette: Docker management UI — **Rust backend** (Axum + bollard), **TypeScript gateway** (Fastify), **React 19 SPA**, TypeScript frontend, optional relay-agent sidecar.
>
> **⚡ ACTIVE BUILD** — `EXECUTION_PLAN.md` is the build script (phases, gates, hard stops).
> **Vault docs** (`.hermes/vault/tunnel-loom/`) are the design authority — specs, protocol, architecture.
> **Build approach:** Docker socket-proxy (no Rust on host). `python3 → Docker API → rust:1.96-alpine`.
>
> Build phases 0-9. Verify at every gate. Phases 0-7 complete. Phases 8-9 remain.

---

## ⚠️ Golden Rules — READ FIRST

### Code-Level Rules

1. **ALWAYS update existing code in place.** Never create a new function, handler, or service when one already exists. Find the existing code and modify it.
2. **NEVER create stub, shim, or pass-through helper functions.** Call bollard directly from handlers. Call `crate::ws_relay::send_relay_command()` directly from routes. Do not write a thin wrapper that just forwards.
3. **NEVER duplicate functionality.** Before writing anything, search `core/src/`, `crates/`, `frontend/src/api/`, and `frontend/src/components/` for existing implementations.
4. **NEVER create a new API standard.** Use existing Axum + JSON patterns (axum::Json, extractors). Do not introduce actix-web, warp, raw hyper handlers, or untyped patterns.
5. **NEVER mix types across crate boundaries.** Shared types go in `crates/relay-protocol/`. Core types stay in `core/src/models/`. Frontend types go in `frontend/src/api/` or inline in components.

### Design & Planning Rules

6. **Protocol before implementation.** Define shared types and message formats BEFORE writing any handler. Payload structs, error codes, and operation codes go into `crates/relay-protocol/` first. Rust's type system is your first line of defense — roundtrip serialize/deserialize tests per struct verify correctness at compile time.

7. **Wave-based task decomposition.** Every feature is broken into numbered, sequential, independently-verifiable waves. Each wave produces a working incremental state. Never plan a monolithic implementation — waves make delegation possible and containment of failure cheap.

8. **Fallback-first architecture.** Every feature that introduces a new capability (relay agent) MUST preserve the existing code path as fallback. Pattern: try relay → fall back to bollard-direct. This is *graceful degradation* — the new thing is an enhancement, never a hard dependency that breaks existing functionality.

9. **Strict DAG, zero circular dependencies.** The dependency graph is a strict DAG: Tier 4 (Features) → Tier 3 (Operations) → Tier 2 (Core Runtime) → Tier 1 (Foundation). Components depend ONLY downward. This is enforced both in code (no circular crate dependencies) and in planning (build order follows the DAG).

10. **Delegation is the default execution model.** Complex multi-wave plans (latch-pipe: 39 tasks, stitch-crate: 4 waves, pulse-pipe: 3 waves) are executed via `delegate_task` — each wave is an isolated subagent with its own context. Subagents get: the specific task file to modify, the exact pattern to follow, and a verification command. Never try to do all waves yourself — context caps at 200K tokens.

### Verification & Quality Rules

11. **Verification commands are mandatory on every task.** Every implementation task MUST end with a concrete `cargo check`, `cargo test`, or `cargo build` verification. "It compiles" is the minimum bar. No verification = task is not complete.

12. **Acceptance criteria before implementation.** Every plan documents what "done" looks like BEFORE coding starts. For latch-pipe: "transfer a volume between two endpoints." For stitch-crate: "list .17 stacks, create a test stack, verify it appears on .17."

13. **Structural integrity audits.** Periodically audit design documents against actual code. The tunnel-loom audit found: ghost components (05-relay-tunnel.md referenced but missing), tier misdeclarations (6 files wrong), stale overview documents contradicting the blueprint. Fix the docs — stale design docs are worse than no docs.

14. **Debugging designed alongside the system.** Observability is not retrofitted. Every new operation gets: structured JSON logging, per-operation statistics, debug state introspection, event streaming, and replay/dry-run capability. The debugging strategy cataloged 49 existing primitives and closed 12 gaps BEFORE writing production code.

15. **Integration tests simulate reality.** The test harness uses Docker-in-Docker with 4 containers on a bridge network: marionette, relay-a (with docker:dind daemon), relay-b (with docker:dind daemon), test-runner (pytest + fault injection). 16 test scenarios ordered by risk. Fault injection: network latency/loss/disconnect, process SIGSTOP/SIGKILL/SIGTERM/OOM, data corruption/tampering.

---

## 🔄 Development Lifecycle

Marionette development follows a strict 5-phase pipeline derived from the latch-pipe, tunnel-loom, and stitch-crate plans. This is the meta-process — the rules for how we plan and build.

### Phase 0: Design (Vault)

Design documents go in `.hermes/vault/tunnel-loom/`. They follow the tiered architecture:

```
Tier 1 — Foundation:  protocol (01), auth (02)
Tier 2 — Core:        agent (03), api (04), tunnel (05), bridge (06)
Tier 3 — Operations:  deployment (07), security (08)
Tier 4 — Features:    migration (09), transfer (10), cluster (11)
```

Every design doc MUST declare its tier, dependencies, and consumers in its header. The MASTER_BLUEPRINT is the single source of truth — all other docs reference it.

**What goes in design docs:**
- Problem statement + why now
- Failure mode catalog (what breaks, why, how fixed)
- Architectural principles (numbered P1, P2, ...)
- API specification (every message subtype with request/response examples)
- Concrete mapping to the user's homelab (.17, .59, localhost)

### Phase 1: Plan (`.hermes/plans/`)

Plans get a codename + date: `2026-07-04_stitch-crate.md`. They answer:

- **Scope:** What files change, how many handlers, how much new code
- **Waves:** Numbered, named, with task IDs (e.g., `fodulek`, `sugirab`)
- **Risk:** Per-wave risk assessment (Low/Medium/High)
- **Verification:** Per-wave and end-to-end verification commands
- **Dependencies:** What must be built first (DAG order)

Plans reference vault docs for design rationale but are executable — they're task lists, not prose.

### Phase 2: Implement (Delegation)

Waves are executed by subagents via `delegate_task`. Each subagent gets:
1. The exact file(s) to modify
2. A working example of the pattern to follow
3. A verification command to run when done
4. Context about what other waves have already built

Waves run sequentially within a stage, parallel across stages (Stage 1a/1b can run simultaneously).

### Phase 3: Verify

```bash
# Per-wave: the subagent runs this before returning
cargo check -p relay-agent
cargo check -p marionette-core
cargo test -p relay-protocol

# End-to-end after all waves:
cargo build --workspace          # Everything compiles
cargo test --workspace           # All tests pass
```

### Phase 4: Audit (Structural Integrity)

After implementation, re-read the design docs against the code:
- Does every referenced file exist?
- Do dependency declarations match reality?
- Are tier assignments correct?
- Does the MASTER_BLUEPRINT still describe the system?

This is what the STRUCTURAL_INTEGRITY_AUDIT did — found 3 CRITICAL and 3 HIGH issues. Run this periodically.

### Phase 5: Test (Integration)

The integration test framework (`docker-compose.test.yml`) simulates the full homelab:
- 4 containers on a bridge network (172.28.0.0/16)
- Marionette + 2 relay agents (each with docker:dind daemon) + test runner
- 16 test scenarios ordered by risk (P0–P5)
- Fault injection: network latency/loss/disconnect, SIGSTOP/SIGKILL/OOM, data corruption

Tests run with `pytest -v --timeout=120` and produce JUnit XML output.

---

## 📋 Planning Template

When creating a new plan, follow this structure:

```markdown
# codename — One-line summary

## Summary
[2-3 sentences: what changes, why, how many files/handlers]

## Scope
- **Files:** list every file that changes (create/modify)
- **New code:** estimated lines
- **Risk:** Low | Medium | High (with reason)

## Waves

### Wave 1 `taskid` — Wave name
- **Agent task:** [specific instructions for the subagent]
- **Files:** [exact paths]
- **Pattern to follow:** [reference existing code]
- **Verification:** `cargo check -p <crate>`

### Wave 2 `taskid` — Wave name
[repeat]

## Acceptance Criteria
- [ ] Concrete, verifiable outcome
- [ ] End-to-end test that proves it works

## Risks
- **[Severity]:** Risk description — mitigation
```

---

## 🏗️ Architecture Overview

**Tech Stack:** Rust (Axum + bollard) + TypeScript (Fastify gateway) + React 19 SPA + SQLite

```
frontend/          React 19 SPA (Vite, Pico CSS, Recharts)
    ↓ REST/WS (8443)
gateway/           Fastify proxy + auth (X-Marionette-Key)
    ↓ localhost:9119
core/              Rust backend — all Docker operations
    ↓ WebSocket (WSS)
crates/relay-agent/  Optional sidecar — runs on remote Docker hosts
crates/relay-protocol/  Shared message types between core ↔ agent
```

### Core (`core/src/`) — Rust backend

```
main.rs              # Entrypoint — Axum router, state init, listen on 9119
ws_relay.rs          # Relay WebSocket handler + send_relay_command / send_relay_command_streaming
routes/              # Thin route handlers → call ws_relay or bollard directly
  stacks.rs          # Stack CRUD, deploy, env, compose operations
  containers.rs      # Container list/inspect/start/stop/restart/remove
  images.rs          # Image list/pull/inspect/remove
  volumes.rs         # Volume list/create/remove/prune/inspect
  networks.rs         # Network list/create/connect/disconnect
  system.rs          # Docker info, version, prune, events, audit
  templates.rs       # Template CRUD + deploy
  endpoints.rs       # Multi-host Docker endpoint management
  nginx.rs           # Nginx upstream config generation
  routes_config.rs   # AuxGate route table CRUD
  swarm.rs           # Swarm init/join/leave/nodes/services/secrets/configs
  users.rs           # User listing
ws/                  # WebSocket streaming handlers
  exec.rs            # Interactive container exec
  logs.rs            # Container log streaming
  stats.rs           # Container stats streaming (CPU/Memory/Network)
  merged_logs.rs     # Multi-container merged logs
  progress.rs        # Migration progress streaming
models/              # Domain structs — Container, Stack, DockerEndpoint, Template, etc.
relay/               # Relay auth, session management, signed messages
  auth.rs            # Registration token validation
  session.rs         # HMAC session key management
  signed.rs          # SignedMessage wrapper for wire protocol
  mod.rs
compose.rs           # Compose file parsing, diff, validation
compose_diff.rs      # Source↔target compose diff for migration
migration.rs         # Container + compose-template migration engine
switchover.rs        # Compose template switchover with health-check + rollback
transfer.rs          # Direct pipe volume transfer via Docker API
docker.rs            # bollard client factory, build_initial_endpoints
db.rs                # SQLite via rusqlite — endpoints, users, routes, audit, tokens
registry.rs          # EndpointRegistry — CRUD for Docker endpoints
audit.rs             # Audit log recording
helpers.rs           # Shared utilities
```

**Key patterns:**

- **Axum** is the HTTP router. Handlers use extractors: `Json`, `Query`, `Path`, `State<Arc<AppState>>`.
- **Routes are thin:** extract typed input → call `ws_relay::send_relay_command()` or bollard → return `Json` response.
- **Global state:** `AppState` holds `EndpointRegistry`, `AuditLog`, `stacks_dir`.
- **bollard** is the Docker client — `Docker::connect_with_unix()` or `Docker::connect_with_http()`. Called directly from routes and handlers.
- **Logging:** `tracing` with `tracing-subscriber` (JSON output, env filter).
- **Errors:** `axum::http::StatusCode` + `axum::Json` error body. Use `(StatusCode, Json<Value>)` error tuple.

**Never wrap `ws_relay` helpers — call directly:**
- Relay: `crate::ws_relay::send_relay_command(hostname, msg).await`
- Streaming: `crate::ws_relay::send_relay_command_streaming(hostname, msg).await`
- Status: `crate::ws_relay::get_all_relay_status().await`
- Lookup: `crate::ws_relay::get_relay_for_endpoint(endpoint_id).await`

### Gateway (`gateway/src/`) — TypeScript proxy

```
index.ts       # Fastify server — HTTPS, proxy to marionette-core:9119, serve SPA
auth.ts        # X-Marionette-Key header validation, dev mode support
proxy.ts       # API proxy + WebSocket upgrade passthrough
```

- **Auth:** All HTTP requests require `X-Marionette-Key` header (unless in dev mode). WebSocket upgrades skip auth (browsers can't set custom headers).
- **Ports:** 8000 (HTTP → 301 redirect), 8443 (HTTPS app).
- **TLS:** Auto-generated self-signed cert on first run, persists in `/app/certs/`.

### Frontend (`frontend/src/`) — React 19 SPA

```
api/              # API client + types (fetch against /api/* endpoints)
components/       # Reusable React components
context/          # React context providers (theme, auth, etc.)
hooks/            # Custom React hooks
pages/            # Route pages (dashboard, containers, stacks, etc.)
styles/           # CSS + Pico CSS overrides
```

**Frontend rules:**
- **React 19** — use functional components, hooks (`useState`, `useEffect`, `useCallback`).
- **Pico CSS** is the design system — use Pico class names, extend with custom CSS in `styles/`.
- **Recharts** for charts (CPU/Memory/Network history in container stats).
- **xterm.js** for interactive terminal (container exec).
- **API calls** via `fetch()` with `X-Marionette-Key` header stored in context.
- **No hardcoded API paths** — use `window.location.origin` or relative paths.

### Relay Protocol (`crates/relay-protocol/`)

Shared types between core and relay-agent:

```
message.rs       # Message wire envelope (Request/Response/Event)
payloads.rs      # Typed payloads for every operation
errors.rs        # Error codes and messages
validate.rs      # Message validation
operations.rs    # Operation code constants
```

**Message format:**
```json
{
  "id": "uuid-v4",
  "type": "request | response | event",
  "subtype": "ping | docker.ps | compose.up | ...",
  "payload": { ... },
  "timestamp": "ISO8601 (optional)",
  "seq": 42 (optional)
}
```

### Relay Agent (`crates/relay-agent/`)

Optional sidecar — runs on remote Docker hosts to proxy marionette commands:

```
main.rs          # Entrypoint — init docker client, connect to marionette via WSS
ws.rs            # WebSocket connect loop, registration, heartbeat, dispatch
handlers.rs      # 22 handler functions — all Docker + compose + filesystem ops
config.rs        # Config from env vars (MARIONETTE_URL, RELAY_TOKEN, DOCKER_HOST)
auth.rs          # HMAC session key management (x25519 key exchange)
signed.rs        # SignedMessage wrapper
```

**Handler subtypes:** ping, docker.ps, docker.inspect, docker.stop, docker.start, docker.restart, docker.exec, docker.logs, docker.stats, compose.up, compose.down, compose.stop, compose.logs, compose.config, image.ensure, volume.transfer_out, volume.transfer_in, host.info, fs.list, fs.read, fs.write, relay.debug.*

---

## 🔧 Build & Run

### Development

```bash
# Build everything (Docker multi-stage)
docker build -t marionette:dev .

# Or build individual components:
cd core && cargo build --release                           # Rust backend
cd frontend && npm install && npm run dev                  # React dev server (Vite HMR)
cd gateway && npm install && npm run dev                   # Fastify dev server

# Relay agent
cd crates/relay-agent && cargo build --release
docker compose -f crates/relay-agent/docker-compose.relay.yml up -d
```

### Docker

```bash
docker compose up -d                                       # HTTPS on :8443
docker compose -f docker-compose.yml up -d --build         # build from source
```

### Tests

```bash
cargo test --workspace                                     # Rust unit tests
cd frontend && npx tsc --noEmit                            # TypeScript type check
```

---

## 🧩 Critical Patterns

### Axum Handler Pattern

```rust
// Route: .route("/containers", get(list_containers))
async fn list_containers(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ContainerListParams>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // 1. Extract hostname from params (or use "localhost" for local Docker)
    let hostname = params.hostname.as_deref().unwrap_or("localhost");

    // 2. Build relay message
    let msg = Message::new_request(id, "docker.ps", json!({ "all": true }));

    // 3. Send via relay
    let response = crate::ws_relay::send_relay_command(hostname, msg).await
        .map_err(|e| (StatusCode::BAD_GATEWAY, Json(json!({"error": e}))))?;

    Ok(Json(serde_json::to_value(response).unwrap()))
}
```

### Docker Enum Pattern (Frontend)

```tsx
// Container states, image tags, network drivers — use string unions, not enums
type ContainerState = 'running' | 'stopped' | 'paused' | 'restarting' | 'dead';
```

### SQLite Pattern (rusqlite)

```rust
// db.rs — always use parameterized queries
let mut stmt = conn.prepare("SELECT id, name FROM endpoints WHERE id = ?1")?;
let endpoint = stmt.query_row(params![id], |row| { ... })?;
```

### WebSocket Streaming Pattern

```rust
// ws/exec.rs, ws/logs.rs, ws/stats.rs — all follow the same pattern:
// 1. Axum WS upgrade
// 2. Send relay command with streaming
// 3. Drain mpsc receiver, forward each Event to browser
// 4. Send final Response to browser, close
```

---

## 🔐 Auth Architecture

Two separate auth layers — keep them distinct:

### HTTP API Auth (Gateway)

- `X-Marionette-Key` header validated by Fastify gateway
- Dev mode: no key required (for local development)
- Production: key checked against `MARIONETTE_KEY` env var (comma-separated for multiple keys)
- WebSocket upgrades skip auth (browsers can't set custom headers on WS)

### Relay Agent Auth (Registration Tokens)

- Relay agent sends `register` message with `token` + `host_info`
- Core validates token against `registration_tokens` table
- On success: creates session with x25519 key exchange → HMAC session key
- All subsequent messages signed with HMAC-SHA256 + nonce for replay protection

---

## 🔄 Relay Architecture

```
Marionette Core                        Relay Agent
┌──────────────────┐                  ┌──────────────────┐
│ RELAYS HashMap   │     WSS          │ ws::connect_loop │
│ hostname→cmd_tx  │◄═══════════════► │ dispatch() →     │
│                  │  SignedMessage   │ handlers::*      │
│ pending HashMap  │  (HMAC-SHA256)   │                  │
│ msg_id→resp_tx   │                  │ DOCKER OnceLock   │
└──────────────────┘                  │ → bollard Docker │
                                       │ → /var/run/docker.sock
                                       └──────────────────┘
```

- **send_relay_command:** Sends a `RelayCommand` via `cmd_tx` channel, blocks up to 30s waiting for `response_tx` oneshot.
- **send_relay_command_streaming:** Same but returns an `mpsc::UnboundedReceiver<Message>` — drains Events then final Response.
- **Registration flow:** connect → register message with token → register_ok with session_id + session_key → HMAC signing for all subsequent messages.
- **Heartbeat:** 5s interval ping/pong. Pings carry host info (hostname, docker version, arch, os). Unauthenticated relays bound to endpoints on first ping.

---

## 🐳 Container Image

Single Docker image with supervisor managing 3 processes:

```
supervisord.conf
├── marionette-core    # Rust binary listening on 127.0.0.1:9119
├── gateway            # Node.js Fastify listening on 0.0.0.0:8443
└── nginx              # nginx for upstream LB (optional)
```

**Health check:** `curl -sf http://localhost:9119/health`

---

## 🚫 Anti-Patterns

| ❌ Don't | ✅ Do |
|----------|------|
| Business logic in route handlers | Thin handlers → bollard or ws_relay |
| New router framework (actix, warp) | Extend existing Axum patterns |
| New API response shape | Use `Result<Json<Value>, (StatusCode, Json<Value>)>` |
| Hardcoded `localhost` Docker calls | Use endpoints + relay for remote hosts |
| Wrapping `send_relay_command` | Call it directly from routes |
| Raw SQLite queries with string interpolation | Use `params![]` for parameterized queries |
| Skipping relay for local Docker | Always go through relay pattern (local Docker is relay host "localhost") |
| New message types outside relay-protocol | All wire types go in `crates/relay-protocol/` |
| Writing relay handlers without `event_tx` parameter | All handlers take `&mpsc::UnboundedSender<Message>` |
| TypeScript `any` in frontend | Proper TypeScript types |
| Skipping auth in production endpoints | Gateway enforces `X-Marionette-Key` for HTTP; relay enforces HMAC for WS |
