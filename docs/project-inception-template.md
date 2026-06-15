# Project Inception Template

> Use this template to design a new software project from concept through to implementation plan. Each section maps to a phase of the discovery process. Fill them in order. Skip nothing.

---

## 1. Concept

**Project name:** `_________________`

**One-line pitch:** `_________________________________________________`

**What problem does it solve?** (3-5 sentences)

**Who is it for?**

**Why build it now?** (What changed in the ecosystem, what gap exists)

---

## 2. Ecosystem Research

### Existing solutions

| Competitor | Tech Stack | License | Stars | What it does well | What it doesn't |
|------------|-----------|---------|-------|-------------------|-----------------|
| | | | | | |
| | | | | | |
| | | | | | |

### Positioning — where we fit

What gap exists that none of the above fill?

### Technology landscape

| What | Current version/state | Relevance to us |
|------|----------------------|-----------------|
| Key library/SDK 1 | | |
| Key library/SDK 2 | | |
| Runtime/platform | | |
| Protocol/API version | | |

---

## 3. Architecture Decision

### Proposed stack

| Layer | Choice | Why |
|-------|--------|-----|
| Backend runtime | | |
| Backend framework | | |
| Docker/API SDK | | |
| Frontend framework | | |
| CSS approach | | |
| Real-time transport | | |
| Deployment model | | |
| License | | |

### Rejected alternatives

| Option | Reason rejected |
|--------|----------------|
| | |
| | |

### Architecture diagram

```
┌──────────────────────────────────────────┐
│                                          │
│              (ASCII diagram)              │
│                                          │
└──────────────────────────────────────────┘
```

### Single container or multi-service?

- [ ] Single container (simplest)
- [ ] Sidecar (two processes, one container)
- [ ] Multi-container (docker-compose orchestrated)

### Multi-arch needed?

- [ ] amd64 only
- [ ] amd64 + arm64

---

## 4. Feature Expansion

> After initial design, run through: "What about X?" for each adjacent domain.

### Expansion checklist

| Domain | Explored? | Decision |
|--------|:---------:|----------|
| Multi-host / remote management | | |
| Orchestration (Swarm/K8s) | | |
| Load balancing / reverse proxy | | |
| Migration between hosts | | |
| Authentication / RBAC | | |
| External storage / volume drivers | | |
| Database / service discovery | | |
| Backup / restore | | |
| Monitoring / alerting | | |
| CLI / API / SDK | | |
| Mobile / responsive | | |

### Feature matrix (by phase)

| Feature | Phase 1 | Phase 2 | Phase 3 | Phase 4 |
|---------|:-------:|:-------:|:-------:|:-------:|
| | | | | |
| | | | | |
| | | | | |

---

## 5. Feature Deep-Dive (for novel/complex features)

> For the project's most novel feature, design it end-to-end.

### Feature: `_________________`

**What happens step-by-step (user flow)?**

```
1.
2.
3.
4.
5.
```

**What edge cases exist?**

| Situation | Handling |
|-----------|----------|
| | |
| | |
| | |

**What can go wrong?** (Failure modes and recovery)

**What does the admin decide vs what is automated?**

| Admin decides | Automated |
|---------------|-----------|
| | |
| | |

---

## 6. Security Review

### Auth model

- [ ] No auth (trusted network only)
- [ ] Access key / API key
- [ ] Basic auth
- [ ] JWT + login page
- [ ] OIDC / SSO
- [ ] RBAC with granular permissions

### Credential handling

| Surface | Where credentials appear | Protection |
|---------|--------------------------|------------|
| Environment variables | | |
| Config files | | |
| Command-line arguments | | |
| API responses | | |
| Logs | | |

### Attack surface

| Threat | Severity (Critical/High/Medium/Low) | Mitigation |
|--------|:------------------------------------:|------------|
| | | |
| | | |
| | | |

### Transport security

| Data in transit | Encryption | Notes |
|-----------------|:----------:|-------|
| | | |
| | | |

### Socket / API access

- What socket or API gives root-equivalent access?
- How is it protected?
- Is a socket proxy needed?

---

## 7. Performance Audit

### Through each layer

| Layer | Current design | Bottleneck? | Fix |
|-------|---------------|:-----------:|-----|
| Backend runtime | | | |
| SDK/API calls | | | |
| Gateway/proxy | | | |
| Frontend rendering | | | |
| WebSocket/real-time | | | |
| Network transfer | | | |
| Database/queries | | | |

### Caching strategy

| What | TTL | Invalidation trigger |
|------|-----|---------------------|
| | | |
| | | |

### Specific optimization opportunities

| Priority (Now/Later) | Fix | Effort | Impact |
|:---------------------|-----|--------|--------|
| | | | |
| | | | |

### Performance budget

| Metric | Target |
|--------|--------|
| Page load (cold) | |
| Page load (cached) | |
| API response (p95) | |
| Memory at idle | |
| CPU at idle | |
| Bundle size | |

---

## 8. UI/UX Design

### Design philosophy

(htop for Docker? Linear minimal? Stripe polished? Engineer-grade?)

### Pages

| Page | Contents | Phase |
|------|----------|-------|
| | | |
| | | |

### Component inventory

| Component | Reusable? | Notes |
|-----------|:---------:|-------|
| | | |
| | | |

### Color themes

- [ ] Dark only
- [ ] Light only
- [ ] Dark + Light
- [ ] Multiple themes (list: _______________)

### Density

- [ ] Dense (terminal-like, minimal whitespace)
- [ ] Comfortable (standard web app spacing)
- [ ] Spacious (Apple-like)

---

## 9. Implementation Plan

### Phased delivery

| Phase | Deliverable | Weeks (est.) | Shippable standalone? |
|-------|------------|:------------:|:---------------------:|
| 1 | | | |
| 2 | | | |
| 3 | | | |
| 4 | | | |

### Project structure

```
project/
├── Dockerfile
├── docker-compose.yml
├── Makefile
├── .env.example
├── .gitignore
├── LICENSE
├── README.md
├── docs/
│   ├── quickstart.md
│   ├── architecture.md
│   ├── security.md
│   └── api-reference.md
├── core/                          # Backend
│   └── src/
├── gateway/                       # API gateway (if separate)
│   └── src/
├── frontend/                      # Frontend
│   └── src/
├── scripts/
└── .github/workflows/
    ├── ci.yml
    └── publish.yml
```

### Key dependencies

| Dependency | Version | Purpose |
|-----------|---------|---------|
| | | |
| | | |

---

## 10. Open Questions & Decisions Pending

| Question | Status | Resolution |
|----------|--------|------------|
| | Open / Decided | |
| | | |

---

## 11. Key Design Patterns (to carry forward)

> What patterns from this project should be reused in future projects?

1.
2.
3.

---

## 12. What We Learned

> After the project ships: what would we do differently next time?

1.
2.
3.
