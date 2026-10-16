# PulseWatch — Implementation Plan & Progress Log

> This file tracks the live implementation plan for PulseWatch (spec: `prompt.md`).
> Update the "Current Status" section after every work session so the next session can resume cold.

---

## Current Status

**Stage 0 — Bootstrap: DONE**
**Stage 2 — Backend foundation: DONE** (compiles, 19 unit tests passing)
**Stage 3 — Monitoring engine: NEXT UP** (not started)
**Stages 4–8: pending**

Last verified command run: `cargo test` in `backend/` → `19 passed; 0 failed`.

---

## Decisions locked in (user-approved)

| Decision | Choice |
|---|---|
| Rust toolchain | Updated via `rustup update stable` → **rustc 1.99.1** (needed for sqlx 0.9) |
| sqlx version | **0.9** (workspace `Cargo.toml`), compiles fine on 1.99.1 |
| Git | Standalone `git init` nested inside the project folder; the outer `/home/shahbaz` mega-repo is ignored. **No commits yet** (user hasn't requested any) |
| Dev workflow | Root `package.json` with `concurrently`: `npm run dev` starts backend (`:3000`) + frontend (`:5173`) |
| Pacing | One continuous run; verification gate + short report after each stage |
| Frontend tests | Vitest + React Testing Library + jsdom |
| OpenAPI UI | `utoipa` 6 for the spec + **`swagger-ui-dist` 5.33.3** (axum routes) instead of `utoipa-swagger-ui` |

### Environment facts

- Node v24.12.0, npm 11.19.1 — available.
- **Disk is tight: ~3.9 GB free (97%).** Watch `df -h /`. `target/` profile uses `debug = "line-tables-only"` to save space. Run `cargo clean` if space runs out.
- `sqlite3` CLI is **not** installed (not needed: SQLx bundles SQLite, tests create temp-file DBs).
- `just` not installed; `make` is available (not used yet).

### ⚠️ Critical dependency discovery

The `zip` crate has had **every version from 2.0.0 through 9.0.3 mass-yanked** (supply-chain incident, only 9.0.4 is unyanked). This makes **every published version of `utoipa-swagger-ui` unresolvable** (all its releases depend on a yanked `zip` range). Workaround already in place:

- Use `utoipa` 6 to generate the OpenAPI JSON (served at `/api/v1/openapi.json`).
- Use **`swagger-ui-dist`** crate (5.33.3, axum 0.8 feature enabled by default) to serve Swagger UI assets + inline spec at **`/api/v1/docs`**. This crate embeds assets in the crate itself, no `zip` dependency.

Do **not** try to switch back to `utoipa-swagger-ui` until upstream ships a release depending on `zip ^9`.

---

## Architecture (implemented + planned)

### Scheduling semantics (Stage 4 design, decided now)
- Persisted `monitors.next_check_at`; scheduler = `tokio::time::interval` (1s tick), queries `enabled = 1 AND next_check_at <= now`.
- **Atomic claim**: `UPDATE monitors SET next_check_at = now + interval WHERE id = ? AND enabled = 1 AND next_check_at <= now`; `rows_affected == 1` means claimed → prevents overlap.
- **Interval measured from claim time.** Overdue monitors (e.g. after restart) get checked once, then rescheduled `+interval` — no catch-up burst.
- **In-flight registry** (`state::InFlightSet`, tokio mutex + `HashSet<Uuid>`, never held across other awaits) shared by scheduled + manual checks; manual check on an in-flight monitor returns a meaningful "already running" response.
- Global `tokio::sync::Semaphore` (`max_concurrent_checks`) bounds all outbound checks; per-check work is `tokio::spawn`ed so one slow site never blocks the scheduler.
- Graceful shutdown: `CancellationToken` in `AppState`; cancelled by ctrl_c / SIGTERM in `main.rs`; axum uses `with_graceful_shutdown`.

### Check engine (Stage 3 design, decided now)
- Single code path for manual + scheduled checks (`services/monitoring_service.rs`).
- Flow: validate → `Instant::now()` → `tokio::time::timeout(monitor.timeout_ms, send())` → **response time = time until response headers arrive** (fixed definition for all charts) → classify → retry loop → persist `CheckResult` + monitor counters + incident transition in **one SQL transaction**.
- Error taxonomy (`CheckErrorType`, already in models): `Dns | Connection | Timeout | Tls | HttpStatusMismatch | TooManyRedirects | BlockedTarget | Cancelled | Internal`. A 404/500 is a *received response* failing the status policy, never a transport error.
- Retries: attempts + exponential backoff (initial ×2, cap, full jitter via `rand`), `tokio::time::sleep` (non-blocking). Retry only transient classes (`is_transient()`); never retry stable status mismatches. Honor `Retry-After` for 429/503 up to a cap. Persist final `attempt_count`.
- Shared pooled `reqwest::Client` (already built in `main.rs`, needs upgrade to safe builder — see Stage 3 todo).

### SSRF / DNS-rebinding defense (Stage 3 design, decided now)
1. URL structure validation at create/update (`validation/url_validation.rs`): http/https only, no userinfo, hostname required, reject private/loopback/link-local/multicast/unspecified/reserved IPv4+IPv6 literals and blocklist hostnames (`localhost`, `*.local`, `metadata.google.internal`, `169.254.169.254`, etc.).
2. Custom reqwest DNS resolver (`.dns_resolver(...)`) that **filters every resolved IP** against the same public-IP check; empty result ⇒ connection error. Rebinding-safe because reqwest connects only to addresses the resolver returned.
3. Custom redirect policy (`.redirect(Policy::custom(...))`): max 5 hops, re-runs structure + IP-literal checks on every hop; connect-time filter covers hostname hops.
4. **`system-proxy` reqwest feature is already disabled** in workspace `Cargo.toml` (commented there) — env proxies can never bypass the checks. Also add `.no_proxy()` when building the client.
5. Test mode: env `PULSEWATCH_ALLOW_PRIVATE_TARGETS=true` (dev/tests only, default `false`) so wiremock on 127.0.0.1 works **without weakening production defaults**. Config field already exists: `config.allow_private_targets`.

### Status + incident state machine (Stage 4 design)
```
unknown ──fail(≥failure_threshold)──► down  [opens incident, transactionally]
up ──fail(≥failure_threshold)───────► down  [opens incident if none active]
down ──success(≥success_threshold)──► up    [resolves active incident]
any ──pause───────────────────────► paused  [no checks; never reported as up]
```
- Extra columns `consecutive_failures` / `consecutive_successes` on `monitors` (updated in same tx as check result). Already in schema.
- Partial unique index `idx_incidents_one_open ON incidents(monitor_id) WHERE resolved_at IS NULL` prevents duplicate open incidents. Already in schema.
- Thresholds default to 1 → transition on first confirmed result.
- UI derives amber "degraded" = `up` with `consecutive_failures > 0` (field `degraded` already on `MonitorSummary`).

### Metrics (Stage 5 design)
- Uptime = successful / eligible checks × 100 in window; **null (not 100%) when 0 samples**; check-based, not point-in-time. Paused periods excluded only if pause history is tracked (v1: document the limitation, don't fake it).
- Latency: avg over non-null `response_time_ms` only; median/p95 via SQL `ORDER BY` + index math in Rust with a sample cap.
- Downtime: `resolved_at − started_at`, else `now − started_at` (open).
- Windows: `24h | 7d | 30d | custom` via `ReportWindow::from_query` (already implemented + tested).
- Charts: SQL time-bucket aggregation (`strftime`), gaps returned as nulls.

---

## What exists on disk right now

```
PulseWatch -- Website Uptime Monitor/
├── prompt.md                     # original spec (unchanged)
├── PLAN.md                       # this file
├── README.md                     # NOT yet written (Stage 8)
├── .gitignore                    # target/, node_modules/, .env, data/, *.db
├── .env.example                  # documented config, no secrets
├── Cargo.toml                    # workspace root, [workspace.dependencies]
├── package.json                  # npm run dev (concurrently) / build / test / lint
├── migrations/
│   └── 0001_init.sql             # monitors, check_results, incidents, settings + indexes
├── backend/
│   ├── Cargo.toml                # bin "pulsewatch"
│   ├── src/
│   │   ├── main.rs               # bootstrap, tracing, graceful shutdown, SIGTERM
│   │   ├── config.rs             # Config::from_env, validated bounds
│   │   ├── db.rs                 # SQLite pool (WAL, FK on), auto-migrate
│   │   ├── error.rs              # AppError → consistent JSON envelope
│   │   ├── state.rs              # AppState + InFlightSet
│   │   ├── openapi.rs            # ApiDoc (utoipa)
│   │   ├── routes/
│   │   │   ├── mod.rs            # router, CORS (ALLOWED_ORIGINS), TraceLayer, swagger-ui-dist
│   │   │   └── health.rs         # GET /health + /api/v1/health
│   │   └── models/
│   │       ├── mod.rs            # ReportWindow (+ parsing, tested)
│   │       ├── monitor.rs        # Monitor, MonitorStatus, DTOs, queries
│   │       ├── check_result.rs   # CheckResult, CheckErrorType, CheckSummary
│   │       └── incident.rs       # Incident, IncidentStatus, ListIncidentsQuery
│   └── tests/                    # empty (integration tests land in Stage 3)
└── frontend/                     # empty (scaffolded in Stage 6)
```

### API surface so far
- `GET /health` and `GET /api/v1/health` → `{status, service, version, database}` (503 when DB down).
- `GET /api/v1/openapi.json` → utoipa spec.
- **`GET /api/v1/docs`** → Swagger UI (served by `swagger-ui-dist`, spec inlined at startup).

### Verified so far
- `cargo check` clean (only "not yet used" dead-code warnings, expected at this stage — will disappear as Stages 3–5 wire routes; **must be zero by Stage 7 clippy gate**).
- `cargo test`: 19 passing — config validation, migration apply + idempotency + FK pragma, JSON error envelope (incl. no internal leakage), status/error-type round-trips, window parsing edge cases, InFlightSet duplicate prevention.

---

## Pending work

### Stage 3 — Monitoring engine (NEXT)
1. `validation/url_validation.rs`: `SsrfPolicy { allow_private: bool }`, `is_public_ip(IpAddr)`, `validate_monitor_url(&str, &SsrfPolicy)` (structure checks), `build_safe_http_client(&Config) -> reqwest::Client` (dns_resolver filter + custom redirect policy + `.no_proxy()` + user-agent; keep pooled client shared).
   - Note: reqwest 0.13 `dns_resolver` needs a resolver implementing the `Resolve` trait; verify exact trait shape against `~/.cargo/registry/src/*/reqwest-0.13.5/` or docs.rs before writing. Resolver must do async-friendly lookup (`tokio::net::lookup_host`).
2. `services/retry.rs`: `RetryPolicy`, `compute_backoff(attempt) -> Duration` (exponential + full jitter + cap), `should_retry(error_type, attempt)`. Unit tests for backoff math + classification.
3. `services/monitoring_service.rs`: `run_check(monitor) -> CheckOutcome` — the single path. Reads monitor row, acquires semaphore permit, checks in-flight registry, executes attempts, persists `CheckResult` + updates monitor counters + incident transition in one tx (needs `repositories/`).
4. `repositories/monitor_repository.rs`, `check_repository.rs`, `incident_repository.rs`: parameterized SQL, transactions. (Incident repo needs Stage 4 transition logic but base CRUD can land here.)
5. `routes/monitors.rs`: full CRUD + `POST /{id}/check|pause|resume` wired to the service (not duplicating logic in handlers). Input validation → `AppError::Validation`. Register routes in `routes/mod.rs`, add schemas to `openapi.rs`.
6. Integration tests in `backend/tests/`: temp-file SQLite per test, **wiremock** local server — create/update/delete, manual check success/timeout/conn-refused/404-mismatch, retry limits, SSRF denials (loopback + private + redirect-to-private), pagination, invalid input, `PULSEWATCH_ALLOW_PRIVATE_TARGETS=true` for the happy-path tests.
7. Wire `build_safe_http_client` into `main.rs` (replace the plain `reqwest::Client::builder()` block).

### Stage 4 — Scheduler + incidents
1. `services/scheduler.rs`: spawn loop on `AppState.shutdown` token; tick 1s; load due monitors; atomic claim; spawn through semaphore + in-flight registry.
2. Pause/resume endpoints set `enabled`/`current_status='paused'` and clear `next_check_at`; resume recomputes `next_check_at = now + interval`.
3. Incident transition logic in `incident_service.rs` (used by both scheduler + manual checks, inside the same tx as the check result).
4. Restart recovery test (reopen DB, overdue monitor gets checked once).
5. Overlap-prevention test (slow wiremock response + trigger manual + scheduled concurrently → exactly one check in flight).

### Stage 5 — Reports API
1. `services/report_service.rs` + `routes/reports.rs`: `/reports/overview|uptime|response-times|incidents`, `/monitors/{id}/checks(/latest|/summary)` with pagination + date filters.
2. SQL-side aggregation, time buckets for charts, hand-computed test cases for uptime/median/p95/downtime.

### Stage 6 — Frontend
1. `npm create vite@latest` (React-TS) into `frontend/`, Tailwind v4, shadcn init, React Router 7, TanStack Query, RHF+zod (`standardSchemaResolver` — verify zod 4 support in @hookform/resolvers), Recharts, lucide-react, sonner.
2. App shell (collapsible sidebar: Overview/Monitors/Incidents/Reports/Settings, mobile sheet, topbar), dark navy/charcoal + emerald theme, light toggle.
3. Pages: Overview (real metrics, empty states), Monitors (table/cards, search/filter/sort/pagination, actions), Monitor detail (charts, history, incidents, config, check-now/pause/resume), Incidents, Reports (windows, CSV export optional), Settings (validated defaults, restart-required explanation).
4. Vite proxy `/api → http://localhost:3000`. Vitest + RTL tests: form validation, CRUD flows, badges, empty states, window picker, API error handling.

### Stage 7 — Verification
- `cargo fmt --check && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace`
- Frontend: lint, `vitest run`, production build.
- Manual E2E (browser): add monitor → manual check → background check appears → pause/resume → trigger incident → read report → swagger page loads.
- Zero dead-code warnings by this point.

### Stage 8 — README + delivery
- Professional `README.md`: overview, screenshots, features, stack, **Mermaid architecture diagram**, schema explanation, setup/run/test commands, OpenAPI usage, example requests, scheduling/retry/uptime/SSRF methodology, **honest limitations** (check-based uptime, single-process scheduler, no auth, proxy note, test-mode flag), future work.
- Final report: what was implemented, which tests actually ran + real results, outstanding limitations.

---

## Resume checklist for next session

1. `cd backend && cargo test` — confirm still 19 passing before starting Stage 3.
2. Read Stage 3 plan above; start with `validation/url_validation.rs` (pure functions + unit tests first, then the reqwest client builder).
3. Verify reqwest `Resolve` trait shape from the vendored source in `~/.cargo/registry/src/index.crates.io-*/reqwest-0.13.5/` before writing the custom DNS resolver.
4. Keep `df -h /` in mind; `cargo clean` if disk gets tight.
