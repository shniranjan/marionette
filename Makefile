.PHONY: dev dev-core dev-gateway dev-frontend setup setup-core setup-gateway setup-frontend build up down logs clean

# ── Development ──────────────────────────────────────────────

dev-core:
	cd core && cargo run

dev-gateway:
	cd gateway && npx tsx src/index.ts

dev-frontend:
	cd frontend && npm run dev

dev:
	@echo "Start all three in separate terminals:"
	@echo "  make dev-core"
	@echo "  make dev-gateway"
	@echo "  make dev-frontend"

# ── Setup ────────────────────────────────────────────────────

setup-core:
	cd core && cargo build

setup-gateway:
	cd gateway && npm install

setup-frontend:
	cd frontend && npm install

setup: setup-core setup-gateway setup-frontend

# ── Docker ───────────────────────────────────────────────────

build:
	docker compose build

up:
	docker compose up -d

down:
	docker compose down

logs:
	docker compose logs -f

# ── Maintenance ──────────────────────────────────────────────

clean:
	cargo clean --manifest-path core/Cargo.toml
	rm -rf gateway/node_modules frontend/node_modules frontend/dist
