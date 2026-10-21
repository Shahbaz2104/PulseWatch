# PulseWatch

[![CI](https://github.com/Shahbaz2104/PulseWatch/actions/workflows/ci.yml/badge.svg)](https://github.com/Shahbaz2104/PulseWatch/actions/workflows/ci.yml)
![Rust](https://img.shields.io/badge/rust-1.99-orange?logo=rust)
![React](https://img.shields.io/badge/react-19-61dafb?logo=react)
![License](https://img.shields.io/badge/license-MIT-green)

A full-stack website uptime monitoring platform written in **Rust** (axum + SQLite) with a **React** operations console.

PulseWatch checks URLs on a schedule, records response times and HTTP status, retries transient failures, opens and resolves incidents, and serves honest uptime/latency reports over a REST API — with a dark, instrument-style UI built for operators.

![Overview](docs/screenshots/overview.png)

---

## Demo

<video src="docs/linkedin-demo.mp4" poster="docs/linkedin-poster.png" controls muted playsinline width="100%"></video>

[▶ Watch the 52-second walkthrough (MP4)](docs/linkedin-demo.mp4) · [Poster frame](docs/linkedin-poster.png)

The clip covers the full loop: overview → add a monitor → live first check → per-attempt history → reports → settings.

The hosted demo runs `DEMO_MODE=true`: reads are public, every write returns `403` (the UI disables action controls), and an empty database is seeded on first boot with monitors, checks, incidents, and one paused monitor.

---

## Use cases

**Self-hosted uptime monitoring** — a UptimeRobot-style alternative you run yourself: watch APIs, homelab services, side projects, or client sites on your own schedule, with no per-monitor limits and no data leaving your machine. Alerts-grade raw data is exposed over REST and CSV.

**Portfolio / proof-of-work for engineers** — the codebase demonstrates production concerns end-to-end: a Tokio scheduler with atomic claim semantics and graceful shutdown, a single-transaction state machine for check → counters → incident, SSRF/DNS-rebinding hardening, honest metrics that never fabricate 100%, single-admin cookie authentication, OpenAPI docs, and a green CI gate over 187 tests.

**Reference implementation for common patterns** — borrow the parts you need: axum + sqlx service/repository layering, exact nearest-rank percentiles, exponential backoff with full jitter and `Retry-After`, windowed report queries, or a React 19 + TanStack Query console with command palette and light/dark theming.

**API-first integration** — everything the UI does is a REST call (`/api/v1`), so it slots into scripts, dashboards, or alerting pipelines; Swagger UI ships at `/api/v1/docs`.

---

## Screenshots

| Monitors | Monitor detail |
|---|---|
| ![Monitors](docs/screenshots/monitors.png) | ![Detail](docs/screenshots/monitor-detail.png) |

| Reports | Light theme |
|---|---|
| ![Reports](docs/screenshots/reports.png) | ![Light](docs/screenshots/overview-light.png) |

| Incidents | Settings |
|---|---|
| ![Incidents](docs/screenshots/incidents.png) | ![Settings](docs/screenshots/settings.png) |

---

## Features

- **Authentication** — single-admin Argon2id password, HMAC-signed HttpOnly session cookies (7-day expiry), read-only public demo mode; reads stay public while every mutation requires the session
- **Monitors** — CRUD with per-monitor interval, timeout, retries, expected status range, and consecutive success/failure thresholds
- **Background scheduler** — 1-second tick, atomic claim (`next_check_at`), global concurrency semaphore, graceful shutdown
- **Check engine** — pooled `reqwest` client, header-arrival timing, error taxonomy (DNS / connection / timeout / TLS / status mismatch / redirects / blocked / cancelled / internal), exponential backoff with full jitter, `Retry-After` support
- **SSRF / DNS-rebinding defense** — public-IP-only policy at create time, custom DNS resolver, re-validation on every redirect hop, private targets blocked by default
- **Incidents** — transactional open/resolve on threshold transitions; partial unique index guarantees one open incident per monitor
- **Alerting** — per-monitor webhook and email destinations, enqueued in the same transaction as the incident transition; a durable outbox worker delivers with bounded timeouts and exponential retries. Destinations are write-only: public responses expose only `alert_webhook_configured` / `alert_email_configured`
- **Reports** — uptime (null when 0 checks, never fabricated 100%), median/p95 over successful checks only, downtime from incident durations, hourly/daily series; latency stats batched in a single scan (response-times stays sub-0.5s at 100+ monitors)
- **Settings API** — defaults for new monitors (env + DB override), restart-required flag for concurrency changes
- **Frontend console** — collapsible sidebar, ⌘K command palette, live polling, charts, uptime strip, search/filter/sort, windowed dashboard feeds, incident history + recent errors per monitor, incident monitor filter, CSV export, light/dark theme (including a Settings preference), client-side pagination for 100-row tables
- **Design system** — [DESIGN.md](DESIGN.md) ("The Midnight Control Room"): status-only saturation, hairline-first elevation, tabular numerals, ambient aurora canvas, session-once boot sequence; tokens in `.impeccable/design.json`

---

## Architecture

```mermaid
flowchart LR
  subgraph Client
    UI[React + Vite\nTanStack Query]
  end

  subgraph Backend["Backend (axum)"]
    API[REST /api/v1]
    SCH[Scheduler 1s tick]
    ENG[Check engine\nreqwest + retries]
    SVC[Services\nincident / report / settings]
  end

  subgraph Storage
    DB[(SQLite WAL\nmonitors · checks · incidents · settings)]
  end

  UI -->|HTTP JSON| API
  API --> SVC
  SCH -->|claim due| DB
  SCH --> ENG
  ENG -->|HTTPS target| Internet
  ENG -->|persist result tx| DB
  SVC --> DB
  API --> DB
```

### Data model (migrations/0001–0003)

| Table | Role |
|---|---|
| `monitors` | URL, config, current status, consecutive counters, `next_check_at` |
| `check_results` | Per-attempt outcome, HTTP status, response time, error type/message |
| `incidents` | Opened/resolved transitions with `started_at` / `resolved_at` / failure reason |
| `monitor_pauses` | Paused intervals (`paused_at` / `resumed_at`), one open interval per monitor |
| `alert_deliveries` | Durable webhook/email outbox with retry state |
| `settings` | Key-value overrides merged over env defaults |

Indexes cover scheduler claims, windowed check aggregates, open-incident uniqueness, pause windows, and completed-delivery retention.

---

## Stack

| Layer | Choice |
|---|---|
| Language / runtime | Rust 1.99, Tokio |
| HTTP framework | axum 0.8 |
| Database | SQLite via sqlx 0.9 (WAL, FK on, auto-migrate) |
| HTTP client | reqwest 0.13 (custom DNS + redirect policy) |
| API docs | utoipa 6 + swagger-ui-dist at `/api/v1/docs` |
| Frontend | React 19, Vite 8, TypeScript 6, Tailwind CSS 4 |
| Client data | TanStack Query 5, React Hook Form + Zod 4 |
| Charts | Recharts + NumberFlow + Motion |

---

## Prerequisites

- Rust stable (1.99+)
- Node.js 24+ and npm
- No separate SQLite binary needed (sqlx bundles it)

---

## Setup

```bash
# 1. Backend + frontend dev deps
npm install                 # root scripts (concurrently)
npm --prefix frontend install

# 2. Configuration (auth is required outside demo mode)
cp .env.example .env
# Generate an Argon2id hash and paste it into ADMIN_PASSWORD_HASH **inside
# single quotes** (dotenvy expands unquoted `$`, which corrupts PHC strings):
printf '%s\n' 'choose-a-long-password' | cargo run --bin hash-password
openssl rand -base64 48     # paste into SESSION_SECRET

# 3. Run both (backend :3000, frontend :5173)
npm run dev
```

Open **http://localhost:5173** for the console, or **http://localhost:3000/api/v1/docs** for Swagger UI.

### Docker (full stack)

```bash
# Export ADMIN_PASSWORD_HASH and SESSION_SECRET first (or put them in .env).
# To try the read-only public demo instead: DEMO_MODE=true (seeds itself on
# first boot, rejects every write).
docker compose up --build
# App + Swagger on http://localhost:3000
# SQLite persists in the pulsewatch-data volume
```

The root `Dockerfile` is multi-stage (Node builds the frontend, Rust builds the backend) and the runtime serves both: static assets from `STATIC_DIR` with SPA fallback, while unknown `/api/*` paths stay JSON 404. Includes a `/health` healthcheck.

Email alerts locally:

```bash
docker compose --profile mailpit up --build
# Mailpit inbox: http://localhost:8025
# then run with SMTP_HOST=mailpit SMTP_PORT=1025 SMTP_FROM=alerts@pulsewatch.local
```

### Production-ish run

```bash
npm run build               # cargo build + vite build
STATIC_DIR=frontend/dist ./target/release/pulsewatch
# backend serves the API and the built SPA on :3000; no reverse proxy required
```

Fly.io deploys use `fly.toml` (read-only demo: `DEMO_MODE=true`, volume mounted at `/data`, manual `fly deploy`).

---

## Configuration

See `.env.example`. Important variables:

| Variable | Default | Notes |
|---|---|---|
| `HOST` / `PORT` | `127.0.0.1` / `3000` | API bind |
| `DATABASE_URL` | `sqlite://./data/pulsewatch.db` | Parent dirs created automatically |
| `STATIC_DIR` | unset | Built frontend dir served with SPA fallback; `/api/*` stays JSON |
| `DEFAULT_CHECK_INTERVAL_SECONDS` | `60` | New-monitor default (10–86400) |
| `DEFAULT_TIMEOUT_MS` | `5000` | Per-request timeout (100–60000) |
| `DEFAULT_RETRY_MAX_ATTEMPTS` | `2` | First try + retries (1–5) |
| `DEFAULT_RETRY_BACKOFF_MS` | `500` | Initial retry delay (0–60000) |
| `DEFAULT_RETRY_BACKOFF_MAX_MS` | `10000` | Backoff ceiling (0–300000, ≥ initial) |
| `MAX_CONCURRENT_CHECKS` | `16` | Scheduler semaphore (1–256) |
| `CHECK_RETENTION_DAYS` | `90` | Check-history retention (7–365); override in Settings |
| `ALLOWED_ORIGINS` | Vite dev origins | CORS |
| `PULSEWATCH_ALLOW_PRIVATE_TARGETS` | unset | **Tests/dev only** — allow loopback targets for wiremock |
| `ADMIN_PASSWORD_HASH` | — | Argon2id PHC hash; **required** unless `DEMO_MODE=true` |
| `SESSION_SECRET` | — | ≥32 bytes for HMAC-signed session cookies; **required** unless demo |
| `SESSION_COOKIE_SECURE` | `false` | Set `true` behind HTTPS |
| `DEMO_MODE` | `false` | Read-only public demo: disables auth and all mutations |
| `SMTP_HOST` / `SMTP_PORT` | unset / `587` | Email alerts; webhook alerts work without SMTP |
| `SMTP_FROM` | unset | Required when `SMTP_HOST` is set |
| `SMTP_USERNAME` / `SMTP_PASSWORD` | unset | Optional; must be set together |
| `SMTP_TLS` | `starttls` | `starttls`, `implicit`, or `none` |

Runtime overrides for check defaults, retry backoff, and check retention live in the **Settings** page (persisted in the `settings` table); backoff changes apply to the next check. Changing max concurrency requires a process restart. A retention worker runs at startup and every 6 hours, deleting old checks and completed alert deliveries in batches; incidents are never deleted.

---

## API overview

Base path: `/api/v1`

| Method | Path | Purpose |
|---|---|---|
| GET | `/health` (also `/api/v1/health`) | Liveness + DB status (incl. `demo_mode`) |
| GET | `/openapi.json` | OpenAPI document |
| GET | `/docs` | Swagger UI |
| POST | `/auth/login` · `/auth/logout` · GET `/auth/status` | Admin session (HMAC cookie) |
| GET/POST | `/monitors` | List (filters + pagination) / create |
| GET/PUT/DELETE | `/monitors/{id}` | Read / update / delete (cascades) |
| POST | `/monitors/{id}/check` | Run one check now (409 if paused/in-flight) |
| POST | `/monitors/{id}/pause` \| `/resume` | Pause / resume |
| GET | `/monitors/{id}/checks` | Paginated history (`window`, `result`) |
| GET | `/monitors/{id}/checks/latest` | Newest check |
| GET | `/monitors/{id}/checks/summary` | Totals, uptime, avg/median/p95 |
| GET | `/reports/overview` | Census, uptime, series, recent activity |
| GET | `/reports/uptime` | Per-monitor uptime |
| GET | `/reports/response-times` | Latency percentiles + series |
| GET | `/reports/incidents` | Windowed incident stats |
| GET | `/incidents` · `/incidents/{id}` | Incident list / detail |
| GET/PUT | `/settings` | Effective settings / partial update |

Reads are public. `POST`/`PUT`/`DELETE` on monitors, checks, and settings require the session cookie and return `401` without it (`403` in demo mode).

Errors use a consistent envelope:

```json
{
  "error": {
    "code": "validation_failed",
    "message": "…",
    "details": [{ "field": "url", "message": "…" }]
  }
}
```

### Example

```bash
# Sign in once; the cookie jar carries the HttpOnly session cookie.
curl -s -c /tmp/pw-cookies -X POST http://localhost:3000/api/v1/auth/login \
  -H 'Content-Type: application/json' -d '{"password":"your-admin-password"}'

curl -s -b /tmp/pw-cookies -X POST http://localhost:3000/api/v1/monitors \
  -H 'Content-Type: application/json' \
  -d '{"name":"Example","url":"https://example.com","check_interval_seconds":30}'

curl -s -b /tmp/pw-cookies -X POST http://localhost:3000/api/v1/monitors/<id>/check
curl -s http://localhost:3000/api/v1/reports/uptime?window=24h   # public read
```

---

## Methodology

### Scheduling
- Every second the scheduler queries due monitors (`enabled = 1 AND next_check_at <= now`).
- **Atomic claim** updates `next_check_at` so a monitor is never checked twice concurrently.
- Interval is measured **from claim time**; overdue monitors after restart are checked once, not burst-caught-up.
- A shared semaphore bounds outbound concurrency; manual checks use the same path and in-flight registry.

### Checks
- Response time is measured **until response headers arrive** (not body download).
- Transient errors retry with exponential backoff + full jitter; stable status mismatches do not retry.
- 429/503 honor `Retry-After` up to a cap.
- Check result, monitor counters, and any incident transition commit in **one SQL transaction**.

### Status & incidents
```
unknown --fail(≥threshold)--> down   [opens incident]
up      --fail(≥threshold)--> down   [opens incident]
down    --success(≥threshold)--> up  [resolves incident]
any     --pause--> paused            [records interval; resolves incident silently]
```
UI amber **Degraded** = nominally `up` but with consecutive failures (recovery threshold not yet met).

### Uptime math
- Uptime = successful checks ÷ eligible checks × 100 over a window.
- **0 checks → `null`** (UI shows “—”), never a fake 100%.
- Latency (avg / median / p95) uses **successful checks only**; failures are excluded, never counted as 0 ms.
- Median/p95 are exact **nearest-rank** percentiles; fewer than 2 samples → `null`.
- Downtime = Σ incident durations (open incidents measured to now), excluding paused overlap. Only incidents that **started** inside the window are counted.
- Paused time is reported as `paused_seconds` clipped to the window. Uptime stays **check-based**; paused time is excluded from downtime, not redefined as availability.

### SSRF / DNS rebinding
1. Create/update validation rejects non-HTTP(S), userinfo, private/loopback/link-local/reserved IP literals, and blocklisted hostnames.
2. A custom DNS resolver filters **every resolved IP** to public ranges before connect.
3. Redirect policy re-runs structure + IP checks on every hop (max 5).
4. Environment proxies are disabled so they cannot bypass the checks.

---

## Tests

```bash
# Backend — unit + integration (temp SQLite + wiremock)
cd backend && cargo test

# Frontend — Vitest + Testing Library
cd frontend && npm test

# Full workspace
npm test

# Lint / typecheck gates
npm run lint
```

**Current counts:** **187 tests** — backend **110 tests** (52 unit + 58 integration) · frontend **77 tests** · `cargo fmt` + `clippy -D warnings` clean · `oxlint` clean · production build green.

Integration tests cover CRUD, SSRF denials, manual-check outcomes, incident open/resolve, alert enqueue/delivery/retries/privacy, auth login/session/logout/demo mode, pause intervals/in-flight races, retention cleanup, scheduler recovery/overlap/shutdown, hand-computed report metrics, windowed overview feeds, and settings validation. Frontend tests cover API error mapping, monitor create/edit validation, pause/resume/delete actions, login/demo/sign-out states, dashboard metrics, window selection, loading/error/empty states, and theme preferences.

---

## Honest limitations

- **Check-based uptime**, not passive probing — uptime reflects scheduled checks only.
- **Single-process scheduler** — horizontal scale-out would need a distributed lease (not implemented).
- **Single-admin auth only** — one Argon2id-hashed password and one session secret in env, with no user accounts, roles, or password reset. Sessions are stateless for 7 days; rotate `SESSION_SECRET` to revoke all of them. Terminate TLS at a reverse proxy and set `SESSION_COOKIE_SECURE=true`.
- **Paused time is not excluded** from uptime percentages.
- **System proxies are intentionally disabled** so SSRF defenses cannot be bypassed; private targets require `PULSEWATCH_ALLOW_PRIVATE_TARGETS=true` (dev/tests only).
- SQLite is fine for a single node; multi-writer / HA would want PostgreSQL.

---

## Project layout

```
├── DESIGN.md             # design system spec ("The Midnight Control Room")
├── .impeccable/          # design tokens sidecar (component/color metadata)
├── migrations/           # SQLx migrations
├── backend/
│   ├── src/
│   │   ├── main.rs       # bootstrap, HTTP client, graceful shutdown
│   │   ├── services/     # monitoring, incident, retry, scheduler, report, settings
│   │   ├── repositories/ # data access
│   │   ├── routes/       # axum handlers + OpenAPI
│   │   └── validation/   # SSRF policy, DNS resolver, safe client
│   └── tests/            # integration suites + common harness
└── frontend/
    └── src/
        ├── app/          # router
        ├── layouts/      # shell
        ├── pages/        # Overview, Monitors, Detail, Incidents, Reports, Settings
        ├── features/     # monitor form/actions
        ├── components/   # design system + charts
        ├── hooks/        # TanStack Query hooks
        └── lib/          # API client, formatters
```

---

## License

[MIT](LICENSE) — built as a portfolio demonstration of Rust async systems + full-stack product engineering.
