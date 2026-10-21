# PulseWatch Portfolio Readiness Plan

Status: approved and in progress.

## Session handoff (2026-10-21)

**Phase 5 (demo + deployment) is complete.** Phases 1–4 remain complete and verified.

Phase 5 shipped:

- `STATIC_DIR` config: axum serves the built frontend with SPA fallback (200), while unknown
  `/api/*` paths keep their JSON 404 envelope; invalid dirs fail startup.
- Demo mode seeds six monitors, 124 checks, two incidents (one open), and one open pause
  interval only when the monitors table is empty; restarts never duplicate data.
- Root multi-stage `Dockerfile` (Node frontend + Rust backend, one runtime process);
  Compose runs one full-stack service with an optional `mailpit` profile for email alerts.
- Frontend disables Add/Edit/Check/Pause/Resume controls when `/auth/status` reports
  `demo_mode`, on top of the existing read-only badge and login explanation.
- `fly.toml` (read-only demo, `/data` volume, manual deploys) plus README deployment docs.
- Tests: SPA fallback + JSON API 404s, static-less 404s, seed idempotence, and demo 403s.

Phase 4 shipped:

- All pages are route-level `React.lazy` chunks behind an accessible Suspense fallback;
  the login route has its own full-screen fallback.
- Initial vendor splitting groups React, Radix/UI, motion, TanStack Query, and remaining
  initial dependencies; charts stay in their own lazy chunk.
- `vite.config.ts` uses `import.meta.dirname`; the build is warning-free with
  `chunkSizeWarningLimit` set above the intentionally lazy Three.js boot chunk.
- App entry chunk: 1,425.66 kB → 31.56 kB (shared vendor chunks load alongside). Clicked through `/`, `/monitors`, `/reports`,
  `/settings`, and `/login` in a real browser; every lazy chunk resolved.

Phase 3 shipped:

- `migrations/0003_monitor_pauses.sql`: pause intervals with a partial unique index
  enforcing one open interval per monitor.
- Pause/resume are transactional; pausing records the interval and resolves any open
  incident at the same instant without enqueuing a recovery alert.
- In-flight checks re-read the monitor inside their persistence transaction, so a pause
  racing a check cannot reopen an incident or overwrite `paused`.
- Reports return window-clipped `paused_seconds` per monitor and in total; downtime
  subtracts paused overlap. The Reports UI shows "Paused (excluded)".
- `check_retention_days` (default 90, range 7–365) in env + Settings; a cleanup worker
  runs at startup and every 6 hours, deleting old checks and delivered/failed alert
  deliveries in batches while never deleting incidents.

Phase 2 shipped:

- Argon2id password verification via `ADMIN_PASSWORD_HASH`; `hash-password` binary reads
  the password from stdin and prints the PHC hash.
- `SESSION_SECRET` (≥32 bytes) signs stateless HMAC-SHA256 session tokens; the
  `pulsewatch_session` cookie is HttpOnly, SameSite=Strict, `Path=/`, 7-day `Max-Age`,
  and adds `Secure` when `SESSION_COOKIE_SECURE=true`.
- Startup fails outside demo mode unless both credentials are valid.
- `POST /api/v1/auth/login`, `POST /api/v1/auth/logout`, `GET /api/v1/auth/status`.
- `AdminSession` extractor protects every monitor mutation, manual check, pause/resume,
  and settings update; all reads stay public. Demo mode returns `403` for writes and
  disables login.
- Health reports `demo_mode`; OpenAPI documents the cookie security scheme and 401/403s.
- Frontend: login page (`/login`), auth hooks, header sign-in/out, read-only demo badge,
  and automatic redirect after login/logout. API requests send cookies and CORS allows
  credentials for configured origins.
- `.env.example`, README, Docker Compose, and the local `.env` now document/configure auth.

Phase 1 details worth keeping:

- Monitor serialization is hand-written: public responses expose only
  `alert_webhook_configured` / `alert_email_configured`; raw destinations stay internal.
- `render_email` subjects are `[PulseWatch] DOWN: <name>` / `RECOVERED: <name>`.
- Update semantics: omitted leaves destinations unchanged, `""` clears them.
- `fdf/` and the duplicate root `src/` were backed up to
  `~/pulsewatch-extras-backup-2026-10-21.tar.gz` and removed from the tree.

Verification: `cargo fmt`, clippy `-D warnings`, backend `cargo test` → **110 passed**
(52 unit + 58 integration); frontend `npm test` → **77 passed**, `npm run lint` and
`npm run build` green.

Next session: start Phase 6 (presentation): regenerate screenshots against the live demo,
re-record the walkthrough, refresh README highlights, and verify CI + the live deployment.

Still pending from Phase 1: exercise alert fields through the browser smoke test
(create monitor → webhook/email → incident → recovery) once SMTP or Mailpit is available.

Additional decisions locked:

- Auth is mandatory outside demo mode: startup fails unless the admin password hash and
  session secret are configured.
- Pausing a monitor resolves any open incident silently (no recovery alert).
- Reporting stays check-based; add `paused_seconds` rather than rewriting uptime as
  wall-clock availability.

Portfolio decisions:

- Host the live demo on **Fly.io** (single app, SQLite volume, always-running machine).
- The public demo is **read-only** with seeded realistic data and a visible banner.
- Alerting supports **webhooks and email**, backed by a reliable outbox worker.
- Deployments are **manual** (`fly deploy`), not automated from CI.
- Include the previously deferred product work: **single-admin auth, retention, and paused-time correctness**.
- Authentication is a single-admin cookie session. The password is stored as an
  Argon2id hash in env; sessions are HMAC-signed, HttpOnly, SameSite=Strict cookies.
  Public read endpoints stay open; mutations and settings require login.
- SMTP settings live in env, never in SQLite. Email is configured per monitor.
- Demo mode disables authentication and all mutations; the scheduler keeps producing
  live check data.

## Phase 0 — Release the current work

- Push `47f3a94`, confirm CI is green.
- Update GitHub About: description, topics, and homepage after deployment.

## Phase 1 — Alerting: webhooks + email

Migration `0002_alerting.sql`:

- `monitors.alert_webhook_url TEXT NULL`
- `monitors.alert_email TEXT NULL`
- `alert_deliveries` outbox with monitor/incident/event/channel/destination/payload,
  status, attempts, next attempt, last error, and unique `(incident_id, event, channel)`.

Backend:

- Add alert fields to the monitor model, create/update DTOs, repository SQL, and
  validation. Webhook URLs use the existing SSRF-safe validator; empty clears.
- Enqueue opened/resolved deliveries inside the check transaction for each channel.
- Add an alert worker: claim due rows, deliver webhook POSTs or SMTP email, retry with
  exponential backoff, cap attempts, log failures, and cancel on shutdown.
- Add SMTP configuration to `config.rs` and validate it at startup.
- Report `email_configured` and `demo_mode` from the health endpoint.
- Tests: transition enqueue, wiremock webhook delivery, retries/failure, duplicate
  protection, invalid/private webhooks, and email rendering via a fake sender.

Frontend:

- Add optional alert webhook and alert email fields to monitor create/edit.
- Show alert destinations on monitor detail and warn when email is unavailable.
- Test schema and payload mapping.

## Phase 2 — Authentication — DONE (2026-10-21)

- Add a `hash-password` helper binary and env-based admin credentials.
- Add login/logout/status endpoints with signed HttpOnly cookies.
- Protect mutations and settings with an `AdminSession` extractor; keep reads public.
- Add a frontend login page, auth-status hook, and header sign-in/out actions.
- Demo mode bypasses auth but remains read-only.
- Tests: valid/invalid login, cookie requirement, public reads, protected writes, logout.
- Document the single-admin scope and the reverse-proxy recommendation.

## Phase 3 — Retention and paused-time correctness — DONE (2026-10-21)

Migration `0003_monitor_pauses.sql`:

- `monitor_pauses(id, monitor_id, paused_at, resumed_at)` plus one-open-pause unique index.

Paused time:

- Make pause transactional: record an interval and resolve any open incident at pause time.
- Resume closes the interval; checks stay blocked while paused.
- Report paused seconds clipped to the selected window and show "Paused (excluded)".
- Tests: incident closes on pause, interval opens/closes once, downtime excludes paused time.

Retention:

- Add `check_retention_days` to settings (default 90, range 7–365) and the Settings UI.
- Add a cleanup worker (startup + every 6 hours) with batched deletes. Prune delivered and
  failed alert deliveries; never delete incidents.
- Tests: old checks removed, fresh checks retained, setting validation.

## Phase 4 — Performance polish — DONE (2026-10-21)

- Lazy-load routes with `React.lazy` and a Suspense fallback.
- Split Overview/Reports chart chunks; add vendor chunking if the main bundle remains >500 kB.
- Replace `__dirname` with `import.meta.dirname`.
- Verify build output and route tests.

## Phase 5 — Demo, full-stack Docker, Fly deployment — DONE (2026-10-21)

- Serve `STATIC_DIR` from axum with SPA fallback; unknown `/api/*` paths stay JSON 404.
- Multi-stage Docker build (Node + Rust), non-root runtime, copied static assets.
- Compose becomes one full-stack service; optional Mailpit profile for local email testing.
- Demo mode: seed monitors/checks/incidents when empty, block mutations, expose the demo
  flag, show a banner, and disable action controls.
- Add `fly.toml` with a volume, port 3000, one always-running machine, static dir, and
  demo mode enabled.
- Manual deploy: `fly volumes create`, `fly deploy`, then smoke-test the URL, health,
  API docs, and write blocking.
- Tests: static fallback, demo 403s, and seed idempotence.

## Phase 6 — Presentation

- Regenerate screenshots from the live demo: Overview dark/light, Monitors, Monitor detail
  with alerts, Incidents filter, Reports with paused time, and Settings.
- Re-record the 45–60 second walkthrough.
- README: live demo, alert payload example, SMTP/auth setup, demo mode, new schema,
  updated test counts, and engineering highlights.
- Update PLAN/PRODUCT, push, verify CI and the live deployment.

## Verification gates

- `cargo fmt --check`, clippy `-D warnings`, `cargo test`
- `npm run lint`, `npm test`, `npm run build`
- Docker build and compose smoke test
- Browser smoke: login, demo read-only, alert fields, pause/resume, incident filter,
  retention setting, lazy routes, mobile layout
- Live Fly smoke test and green CI
