# PulseWatch — Implementation Plan & Progress Log

> This file tracks the live implementation plan for PulseWatch (spec: `prompt.md`).
> Update the "Current Status" section after every work session so the next session can resume cold.

---

## Current Status

**Stage 0 — Bootstrap: DONE**
**Stage 2 — Backend foundation: DONE** (compiles, 19 unit tests passing)
**Stage 3 — Monitoring engine: DONE** (51 tests passing: 39 unit + 12 integration; fmt + clippy `-D warnings` clean)
**Stage 4 — Scheduler + incidents: DONE** (55 tests passing: 39 unit + 16 integration; scheduler loop wired into `main.rs`)
**Stage 5 — Reports API: DONE** (72 tests passing: 45 unit + 27 integration; fmt + clippy `-D warnings` clean)
**Stage 6 — Settings API (M0a): DONE** (80 backend tests: 47 unit + 33 integration)
**Stage 6 — Frontend (M0b–M5): DONE** (build green, oxlint clean, 65 Vitest tests; live browser E2E + screenshots dark/light)
**Stage 7 — Verification: DONE** (backend 80 + fmt/clippy clean; frontend lint/65 tests/build clean)
**Stage 8 — README + delivery: DONE**
**Status: DELIVERY COMPLETE** (+ post-delivery completeness pass: sorting, incident history, recent errors, overview window, theme preference, page-flow tests, retry backoff settings, incident monitor filter, windowed overview feeds)
**Post-delivery — Phase 1 alerting: DONE (2026-10-21)** — webhook + email destinations, transactional outbox, retrying delivery worker, SMTP config, write-only destination privacy, frontend fields and SMTP warning.
**Post-delivery — Phase 2 authentication: DONE (2026-10-21)** — Argon2id admin password, HMAC-signed HttpOnly session cookies, login/logout/status endpoints, `AdminSession` protection on all mutations, read-only demo mode, login page + header auth controls. See `docs/PORTFOLIO_PLAN.md` for the active roadmap.
**Post-delivery — Phase 3 retention + paused-time: DONE (2026-10-21)** — `monitor_pauses` intervals, transactional pause/resume with silent incident resolution, in-flight check guard, window-clipped `paused_seconds`, downtime pause exclusion, configurable retention (90d default, 7–365), and a batched startup/6-hour cleanup worker.
**Post-delivery — Phase 5 demo + deployment: DONE (2026-10-21)** — axum serves `STATIC_DIR` with SPA fallback while unknown `/api/*` paths stay JSON 404; demo mode seeds monitors/checks/incidents/one pause when empty (idempotent); full-stack multi-stage `Dockerfile` + one-service Compose with an optional Mailpit profile; frontend disables mutation controls in demo mode; `fly.toml` for the read-only Fly demo.
**Post-delivery — Phase 4 performance polish: DONE (2026-10-21)** — every route/lazy page is a separate chunk behind an accessible Suspense fallback; React, UI, motion, query, and initial-vendor groups split the entry bundle; `import.meta.dirname` replaces `__dirname`; build is warning-free. The app entry chunk fell from 1.43 MB to 31.56 kB (shared vendor chunks load alongside), with charts (386 kB) and the first-visit Three.js boot orb (736 kB, 187 kB gzip) loaded only on demand.

Last verified: backend `cargo test` → **110 passed** (52 unit + 58 integration, including static/SPA-fallback and demo-seed suites); frontend `npm test` → **77 passed**; fmt/clippy `-D warnings`, `oxlint`, and `npm run build` green; browser smoke-tested `/`, `/monitors`, `/reports`, `/settings`, and `/login` with every lazy chunk resolving. E2E screenshots in `/tmp/opencode/pulsewatch-shots/` are stale until Phase 6.

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
any ──pause───────────────────────► paused  [records interval; resolves incident silently]
```
- Extra columns `consecutive_failures` / `consecutive_successes` on `monitors` (updated in same tx as check result). Already in schema.
- Partial unique index `idx_incidents_one_open ON incidents(monitor_id) WHERE resolved_at IS NULL` prevents duplicate open incidents. Already in schema.
- Thresholds default to 1 → transition on first confirmed result.
- UI derives amber "degraded" = `up` with `consecutive_failures > 0` (field `degraded` already on `MonitorSummary`).

### Metrics (Stage 5 — implemented + tested)
- Uptime = successful / eligible checks × 100 in window; **null (not 100%) when 0 samples**; check-based, not point-in-time. Paused time is reported separately (`paused_seconds`) and excluded from downtime, not added to uptime.
- Latency: avg/median/p95 computed ONLY over **successful** checks with non-null `response_time_ms`; failures never contribute (never counted as 0). Median/p95 = exact **nearest-rank** via SQL `ORDER BY response_time_ms LIMIT 1 OFFSET k` (`k = (n-1)/2` and `ceil(p·n)-1`); `null` when fewer than 2 samples.
- Downtime: `resolved_at − started_at`, else `now − started_at` (open); counts only incidents that **started** inside the window; one open incident max per monitor is guaranteed by the partial unique index.
- Windows: `24h | 7d | 30d | custom` via `ReportWindow::from_query` (RFC3339 `from`/`to`, inversion + format validated → 422).
- Chart buckets: SQL-side `strftime` (hourly ≤24h windows, daily otherwise, UTC-aligned). Overview sparklines (`uptime_series`, `response_time_series`) share a **fixed 24h hourly** base so the payload is time-base consistent; windowed series live on `/reports/response-times`.

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
│   ├── Cargo.toml                # lib + bin "pulsewatch"; dev-deps: wiremock, tempfile, tower
│   ├── src/
│   │   ├── lib.rs                # library root (shared by bin + integration tests)
│   │   ├── main.rs               # bootstrap, tracing, safe HTTP client, graceful shutdown
│   │   ├── auth.rs               # Argon2id verify, HMAC session tokens, AdminSession
│   │   ├── config.rs             # Config::from_env, validated bounds
│   │   ├── db.rs                 # SQLite pool (WAL, FK on), auto-migrate
│   │   ├── error.rs              # AppError → consistent JSON envelope
│   │   ├── state.rs              # AppState + InFlightSet
│   │   ├── openapi.rs            # ApiDoc (utoipa) incl. monitors paths/schemas
│   │   ├── validation/
│   │   │   ├── mod.rs            # re-exports
│   │   │   └── url_validation.rs # SSRF policy, is_public_ip, DNS resolver, redirect policy, safe client
│   │   ├── services/
│   │   │   ├── mod.rs
│   │   │   ├── monitoring_service.rs  # single check: SSRF DNS layer, retries, timing (tested)
│   │   │   ├── incident_service.rs    # plan_transition + persist_check_outcome (tested)
│   │   │   ├── retry.rs               # RetryPolicy (backoff + jitter + Retry-After, tested)
│   │   │   ├── scheduler.rs           # 1s tick loop, atomic claim, spawn via run_check
│   │   │   └── report_service.rs      # uptime/latency/downtime math + report assembly (unit-tested)
│   │   ├── repositories/
│   │   │   ├── mod.rs
│   │   │   ├── monitor_repository.rs  # CRUD, claim_due, window_stats, latest_checks, list, status_counts, find_all
│   │   │   ├── check_repository.rs    # insert, paginated history, window_counts, latency_stats, activity_series, recent_global
│   │   │   └── incident_repository.rs # find_open / open / resolve_open, list_with_monitor, window_incidents, recent, find_with_monitor
│   │   ├── routes/
│   │   │   ├── mod.rs            # router, CORS, TraceLayer, swagger-ui-dist
│   │   │   ├── auth.rs           # login/logout/status + cookie issuance
│   │   │   ├── health.rs         # GET /health + /api/v1/health
│   │   │   ├── monitors.rs       # CRUD + check/pause/resume + pagination
│   │   │   ├── checks.rs         # history (paginated + result filter), latest, summary
│   │   │   ├── reports.rs        # overview / uptime / response-times / incidents
│   │   │   └── incidents.rs      # incident list (filters + pagination) + detail
│   │   └── models/
│   │       ├── monitor.rs, check_result.rs, incident.rs, mod.rs (ReportWindow, parse_rfc3339)
│   │       └── reports.rs        # Stage 5 report DTOs + ReportQuery
│   └── tests/
│       ├── common/mod.rs         # TestApp harness (temp DB, wiremock-friendly config)
│       ├── monitors_api.rs       # 12 integration tests
│       ├── reports_api.rs        # 11 integration tests (hand-computed metric assertions)
│       ├── auth_api.rs           # 6 auth/demo integration tests
│       └── scheduler.rs          # 4 scheduler integration tests
└── frontend/                     # empty (scaffolded in Stage 6)
```

### API surface so far
- `GET /health` and `GET /api/v1/health` → `{status, service, version, database}` (503 when DB down).
- `GET /api/v1/openapi.json` → utoipa spec.
- **`GET /api/v1/docs`** → Swagger UI (served by `swagger-ui-dist`, spec inlined at startup).
- **Monitors (Stage 3):**
  - `GET /api/v1/monitors` → paginated `MonitorSummary[]` (search/filter/sort + windowed uptime enrichment).
  - `POST /api/v1/monitors` → create (201); first check due immediately.
  - `GET/PUT/DELETE /api/v1/monitors/{id}` (delete cascades checks + incidents).
  - `POST /api/v1/monitors/{id}/check` → run one check now (409 when paused/already in flight).
  - `POST /api/v1/monitors/{id}/pause|resume` (resume → `unknown`, first check due immediately).
- **Check history (Stage 5):**
  - `GET /api/v1/monitors/{id}/checks` → paginated `CheckSummary[]` (window, `result=success|failure`, page clamps 1..=200).
  - `GET /api/v1/monitors/{id}/checks/latest` → newest check row (404 when the monitor has none).
  - `GET /api/v1/monitors/{id}/checks/summary` → totals, uptime %, avg/median/p95, latest response time (nulls when no checks).
- **Reports (Stage 5):**
  - `GET /api/v1/reports/overview` → monitor census, uptime %, avg latency, incident stats, recent incidents/checks, fixed-24h sparklines.
  - `GET /api/v1/reports/uptime` → per-monitor uptime + overall (null on zero checks).
  - `GET /api/v1/reports/response-times` → per-monitor avg/median/p95 + time-bucketed series (optional `monitor_id`, 404 on unknown).
  - `GET /api/v1/reports/incidents` → windowed incident stats + incidents (newest first).
- **Incidents (Stage 5):**
  - `GET /api/v1/incidents` → paginated `IncidentWithMonitor[]` (status/monitor/date filters, durations computed).
  - `GET /api/v1/incidents/{id}` → incident + monitor + duration (404 when unknown).

### Verified so far
- `cargo fmt --check` clean; `cargo clippy --all-targets --all-features -- -D warnings` clean.
- `cargo test`: **72 passing** — 45 unit (config validation, migrations, error envelope, status/error-type round-trips, window parsing, InFlightSet, SSRF validation matrix + DNS resolver, retry policy math, incident state-machine transitions, report metric math: nearest-rank offsets, uptime null/exact, bucket granularity, bucket parse, status filter) + 27 integration (temp-file SQLite + wiremock + in-process router): CRUD round-trip + pagination, invalid-input 422s, SSRF create-denials, manual check success/404-mismatch/timeout/retry-exhaustion/DNS-block, incident open+resolve across checks, pause/resume flow, host-default naming; scheduler: restart recovery (checked once, no burst), future/paused no-claim, manual-vs-scheduled overlap → single probe + 409, loop stops on shutdown; reports: hand-computed uptime 80.0%, avg 27.5/median 25/p95 45 over 8 success samples (999-ms failures excluded), history pagination + result filter, latest, 404s, incident durations (resolved exact, open elapsed), overview census/counts, invalid windows 422, empty-installation nulls (never fabricated 100%).

### Stage 3 implementation notes (gotchas hit)
- **sqlx 0.9 `QueryBuilder`**: `push_values` emits SQLite `IN (VALUES ...)` which fails to prepare on our SQLite build; `?` characters inside `push()` strings are emitted **literally** — placeholders must come from `push_bind` interleaved between `push` calls. All dynamic SQL follows this pattern now (`push_id_list` helper in monitor_repository).
- **sqlx 0.9 SQLite encodes `Uuid` as 16-byte BLOB** — raw SQL in tests must bind `Uuid`, never `.to_string()`.
- **reqwest 0.13**: `dns_resolver` accepts any `Resolve` impl (blanket `IntoResolve`); error classification survives via `source()` chain markers (`BlockedTarget`, `TooManyRedirects`).
- Crate is now a **lib + bin** (`src/lib.rs`) so `backend/tests/` integration tests can import the app; `main.rs` uses `pulsewatch_backend::…`.
- Redirect-to-private SSRF hop is covered by unit tests of the redirect policy; the integration suite covers create-time denial + request-time DNS-layer blocking (a loopback-only wiremock cannot isolate the redirect layer when the DNS layer blocks loopback first).

### Stage 5 implementation notes (gotchas hit)
- **`sqlx::query_as::<_, (SomeStruct, …)>` does not work** — a derived-`FromRow` struct cannot be decoded inside a tuple; use a flat column tuple and construct the struct manually (`IncidentJoinRow` + `join_row_to_with_monitor` in incident_repository).
- **`chrono::DateTime::to_rfc3339()` produces `+00:00`** — the `+` decodes as a space in query strings. Tests and any client URLs must use `to_rfc3339_opts(SecondsFormat::Secs, true)` (`Z` suffix) or percent-encode.
- **`f64` has no `From<i64>`** — `.map(f64::from)` on `Option<i64>` fails; use `.map(|v| v as f64)` (latency medians are i64 from SQL, API serializes f64).
- **`#[serde(flatten)]` on `IncidentWithMonitor.incident`** — incident fields appear at the top level of each array element (`data[0]["status"]`, not `data[0]["incident"]["status"]`); keep in mind for frontend shaping too.
- **Minute-relative test seeding straddles hour buckets depending on wall-clock minute** — seed relative to `hour_start = now.with_minute(0)…` instead; also note overview sparklines are fixed-24h so checks older than the query window still appear there.
- `Pagination::new` had to become `pub` for reuse by checks/incidents routes.

### Stage 6 implementation notes (gotchas hit)
- **RHF + `zodResolver` + zod 4 `z.coerce`**: input ≠ output types break `useForm<z.infer<…>>`. Fix: type the form as `z.output<typeof schema>` and cast `zodResolver(schema) as Resolver<FormValues>`.
- **TS 6 deprecates `baseUrl`** — `paths` works without it; remove `baseUrl` from `tsconfig.app.json`.
- **shadcn CLI** writes components into a literal `./@/components/ui/` directory — `mv` them under `src/`, re-point `cn` imports to `@/lib/utils`, uninstall the accidental `cn` npm package the CLI installs.
- **NAV_ITEMS as const** with a union of objects: destructuring `{ end }` fails on members that omit the key — read `end` with `"end" in item ? item.end : false`.
- **`AppSettings` field names are `default_*`** (`default_check_interval_seconds`, …) plus bare `max_concurrent_checks`; `SettingsUpdateResponse.restart_required` is `string[]`, not a boolean.
- Design tokens, StatusPill, MetricCard (animated via `@number-flow/react` + `motion`), UptimeStrip, SeriesChart, PageTransition, and the ⌘K CommandPalette live under `frontend/src/components/`. Polling cadence is 30 s (lists) / 15 s (detail).

---

## Pending work

### Stage 3 — Monitoring engine: DONE
All seven items landed: SSRF validation + safe client (`validation/url_validation.rs`), retry policy (`services/retry.rs`), single check path (`services/monitoring_service.rs`), repositories + tx-persisted incident transitions, monitors routes + OpenAPI, 12 integration tests (`backend/tests/monitors_api.rs` + `common/mod.rs` harness), client wired into `main.rs`.

### Stage 4 — Scheduler + incidents: DONE
1. `services/scheduler.rs`: 1s tick loop on the shutdown token (`MissedTickBehavior::Delay`); `run_due_once` claims due monitors via `claim_due` (atomic SQL claim, `next_check_at = now + interval`, no catch-up burst) and spawns one task per monitor through `monitoring_service::run_check` (semaphore + in-flight registry live inside that path). Wired into `main.rs` right after state construction.
2. ~~Pause/resume endpoints~~ — DONE in Stage 3 (`routes/monitors.rs`).
3. ~~Incident transition logic~~ — DONE in Stage 3 (`services/incident_service.rs`, same tx as check insert).
4. Restart recovery test — `tests/scheduler.rs::overdue_monitor_is_checked_exactly_once_after_restart`.
5. Overlap prevention test — `tests/scheduler.rs::scheduled_and_manual_checks_never_overlap` (manual → 409 while scheduled probe in flight; exactly one HTTP request + one check row; slot released afterwards). Plus future/paused no-claim + clean-shutdown loop tests.

### Stage 5 — Reports API: DONE
All items landed:
1. `models/reports.rs` (DTOs + `ReportQuery` with `IntoParams`), `services/report_service.rs` (uptime/nearest-rank/series/incident math + overview assembly, 6 unit tests), repositories extended (`window_counts`, `latency_stats`, `activity_series`, `recent_global`, `list_with_monitor`, `window_incidents`, `recent`, `find_with_monitor`, `status_counts`, `find_all`).
2. Routes: `routes/checks.rs` (history/latest/summary), `routes/reports.rs` (overview/uptime/response-times/incidents), `routes/incidents.rs` (list/detail); registered in `routes/mod.rs` + `openapi.rs`.
3. `tests/reports_api.rs`: 11 integration tests with hand-computed metric values (hour-aligned seeded checks, 999-ms failures excluded from latency, old out-of-window check, null-not-100% on empty, 404s, 422 window/filter validation, incident durations).

### Stage 6 — Frontend: DONE (M0a–M5)
All pages and the settings slice landed:
1. **M0a — Settings API**: `services/settings_service.rs` (`AppSettings::load` env→DB, `update` with field validation + `restart_required`), `repositories/settings_repository.rs`, `routes/settings.rs` (GET/PUT), wired into OpenAPI; create/update monitor handlers now load `AppSettings::load` first so new monitors inherit defaults; `tests/settings_api.rs` (4 tests).
2. **M0b — Scaffold**: Vite React-TS, Tailwind v4 (`@theme inline` navy/emerald tokens, light+dark), Geist Sans/Mono self-hosted, shadcn (button, card, dialog, dropdown-menu, form, input, label, popover, select, separator, sheet, skeleton, switch, table, tabs, tooltip, sonner, alert, textarea, alert-dialog), Vite proxy `/api` → `:3000`.
3. **M1 — Shell**: collapsible sidebar (motion `layoutId` active indicator), mobile sheet, sticky blurred header with ⌘K search, ThemeToggle (light/dark/system), CommandPalette (monitor search + nav), PageTransition (200 ms rise), StatusPill (live ring-pulse), MetricCard (NumberFlow animated KPIs + stagger), EmptyState, UptimeStrip (staggered bars), SeriesChart (Recharts, CSS-var colors), typed `lib/api.ts` + `hooks/use-api.ts` (TanStack Query, 30 s/15 s polling, full CRUD mutations).
4. **M2–M4 — Pages**: Overview (KPI grid, uptime strip, response chart, recent incidents/checks, teaching empty states), Monitors (search/filter/window/pagination, desktop table + mobile cards, create/edit dialog via `?new=1`, row actions check/pause/resume/delete), MonitorDetail (24h/7d/30d uptime KPIs, median/p95, response chart, uptime strip, paginated check history with result filter, config panel, edit dialog), Incidents (status + window filters, table + mobile cards), Reports (window picker, uptime table, latency percentiles with null-on-low-samples, response series, client-side CSV export), Settings (validated form, restart banner, server field-error mapping), NotFound.
5. **M5 — Tests**: `npm test` → **65/65 passing** (format helpers, StatusPill, EmptyState, MetricCard, WindowPicker, monitor zod schema, API error mapping/query params, Monitors/Overview/MonitorDetail/Settings pages, monitor create/edit form, pause/resume/delete actions).

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

1. **Delivery is complete** through Stage 8. No open blockers.
2. Re-run gates if touching code:
   - Backend: `cd backend && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test` (expect 80).
   - Frontend: `cd frontend && npm run lint && npm test && npm run build` (expect 65 tests).
3. Dev: `npm install && npm --prefix frontend install && cp .env.example .env && npm run dev` → UI :5173, API :3000, Swagger `/api/v1/docs`.
4. Gotchas: incident JSON is flattened; settings fields use `default_*` prefix; workspace path has a **trailing space**; `PULSEWATCH_ALLOW_PRIVATE_TARGETS=true` only for wiremock tests.
5. Screenshots from E2E live under `/tmp/opencode/pulsewatch-shots/` (not committed).
