# Contributing

How to set up marionette for development and submit changes.

---

## Code of Conduct

Be respectful. Be constructive. Assume good faith.

---

## Development Setup

### Prerequisites

- **Rust:** 1.85+ (stable)
- **Node.js:** 22+
- **Docker:** 24+ (for integration testing)
- **OpenSSL** (for Rust TLS support)

### Clone and install

```bash
git clone https://github.com/shniranjan/marionette.git
cd marionette

# Install Rust dependencies + build
cd core
cargo build

# Install Node dependencies
cd ../gateway
npm install

cd ../frontend
npm install
```

### Run in development mode

You need three terminals:

**Terminal 1 — Rust core:**
```bash
cd marionette/core
cargo run
# Listening on 127.0.0.1:9119
```

**Terminal 2 — Node gateway:**
```bash
cd marionette/gateway
MARIONETTE_KEY=dev npm run dev
# Listening on :8000, proxying to :9119
```

**Terminal 3 — React frontend (optional, for hot reload):**
```bash
cd marionette/frontend
npm run dev
# Listening on :5173 with HMR
# Proxies /api to :8000
```

Visit `http://localhost:5173` for development (with hot reload) or `http://localhost:8000` for production-like setup.

---

## Project Structure

```
marionette/
├── Dockerfile              # Multi-stage build
├── docker-compose.yml      # Local dev deployment
├── supervisord.conf        # Process manager config
├── core/                   # Rust backend
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs         # Entry point, router, AppState
│       ├── docker.rs       # Docker client factory
│       ├── compose.rs      # docker compose shell wrapper
│       ├── routes/         # Route handlers (one file per resource)
│       ├── ws/             # WebSocket handlers
│       └── models.rs       # Serde types
├── gateway/                # Node/TS gateway
│   ├── package.json
│   └── src/
│       ├── index.ts        # Fastify entry
│       ├── auth.ts         # Access key middleware
│       └── proxy.ts        # Reverse proxy config
├── frontend/               # React 19 SPA
│   └── src/
│       ├── api/client.js   # Fetch wrapper
│       ├── pages/          # One file per page
│       ├── components/     # Reusable components
│       ├── context/        # ThemeContext
│       └── styles/         # CSS by theme
└── docs/                   # Documentation
```

---

## Conventions

### Commit messages

```
type: description

Where type is one of:
  feat     — new feature
  fix      — bug fix
  docs     — documentation
  refactor — code restructuring (no behavior change)
  perf     — performance improvement
  test     — adding or updating tests
  chore    — build, CI, dependencies

Examples:
  feat: add container migration wizard
  fix: handle empty volume list in migration
  docs: add troubleshooting guide for SSH issues
  perf: parallelize Docker API calls in dashboard
```

### Rust style

- Follow `cargo fmt` and `cargo clippy`
- Use `anyhow` or `thiserror` for error types
- Route handlers return `Result<Json<T>, AppError>`
- All Docker interactions go through `AppState::get_client(endpoint_id)`
- Avoid `unwrap()` — use `?` or proper error handling

### TypeScript style

- Follow `eslint` and `prettier` defaults
- Use `async/await` over raw promises
- API client exports named functions, not a class
- Components are React functional components with hooks

### Frontend style

- CSS custom properties for all colors — no hardcoded hex values in components
- Three data attributes: `[data-theme="dark"]`, `[data-theme="light"]`, `[data-theme="sepia"]`
- State-based routing — no react-router. `page` state in App.jsx
- Tables use `@tanstack/react-virtual` for >100 items

---

## Testing

### Rust

```bash
cd core
cargo test                   # Unit tests
cargo test -- --ignored      # Integration tests (need Docker running)
```

### Frontend

```bash
cd frontend
npm run build                # Verify build succeeds (CI check)
# Manual testing via browser at localhost:5173
```

### Integration

```bash
# Build and run full stack
docker compose up -d --build

# Check health
curl http://localhost:8000/api/health

# Test with auth
curl -H "x-marionette-key: dev" http://localhost:8000/api/containers
```

---

## Pull Request Process

1. **Fork the repository**
2. **Create a feature branch:** `git checkout -b feat/my-feature`
3. **Make changes.** Follow conventions above.
4. **Test locally.** Run unit tests + manual integration test.
5. **Push and open a PR.** Include:
   - Clear description of what changed and why
   - Screenshots for UI changes
   - Link to any related issues
6. **CI must pass.** GitHub Actions runs: `cargo check` + `cargo test` + `npm run build`
7. **Review.** At least one maintainer reviews. Address feedback.
8. **Merge.** Squash merge to `main`.

---

## Adding a New Feature

### To Rust core:

1. Add route handler in `core/src/routes/<name>.rs`
2. Add types in `core/src/models.rs`
3. Register route in `core/src/routes/mod.rs` → wire in `main.rs`
4. Test with `curl localhost:9119/<endpoint>`

### To frontend:

1. Create page in `frontend/src/pages/<Name>.jsx`
2. Add API functions in `frontend/src/api/client.js`
3. Add page to `App.jsx` routing
4. Add sidebar entry in `Sidebar.jsx`
5. Test at `localhost:5173`

### To documentation:

1. If adding a user-facing feature, update `docs/user-manual.md`
2. If adding an endpoint, update `docs/api-reference.md`
3. If changing behavior, update `docs/tutorial.md` if relevant
4. If changing architecture, update `docs/architecture.md`

---

## Adding a Phase 2+ Feature

Phase 1 ships as a single-host Docker manager. Phase 2+ features should be designed with these considerations:

- **Multi-client ready:** All new Rust routes accept `?endpoint=` param
- **Feature flags:** Wrap Phase 2+ UI in feature detection (check if endpoint list > 1)
- **Graceful degradation:** If a feature isn't available (e.g., Swarm not initialized), show a clear message, not a crash
- **Documentation:** Mark Phase 2+ features clearly in docs

---

## Release Process

1. Update version in `core/Cargo.toml` and `gateway/package.json`
2. Update CHANGELOG.md
3. Tag: `git tag v0.1.0`
4. Push tag: `git push origin v0.1.0`
5. GitHub Actions builds multi-arch Docker image and pushes to GHCR
6. Update README badges if needed

---

## Getting Help

- **Questions:** Open a [Discussion](https://github.com/shniranjan/marionette/discussions)
- **Bugs:** Open an [Issue](https://github.com/shniranjan/marionette/issues) with reproduction steps
- **Feature requests:** Open an [Issue](https://github.com/shniranjan/marionette/issues) with use case description

## License

By contributing, you agree that your contributions will be licensed under the AGPL v3 license.
