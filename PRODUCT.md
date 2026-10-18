# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Primary: operators of small fleets — homelabers, indie developers, small teams — who run a self-hosted tool and need to know within minutes when a URL breaks.

Confirmed secondary (user answered "both 1 and 2"): recruiters and engineers evaluating the codebase as portfolio proof of Rust async systems and full-stack craft. The showpiece ambition is real, but daily operation wins ties.

## Product Purpose

Self-hosted website uptime monitoring: check URLs on a schedule, record latency and HTTP status, open and resolve incidents on thresholds, and serve uptime/latency reports over a REST API. Success means an operator trusts the numbers enough to alert on them, and a reader of the code sees production-grade decisions.

## Positioning

An uptime monitor you own end-to-end: honest metrics (0 checks → null, never a fabricated 100%; percentiles over successes only), an SSRF/DNS-rebinding-hardened check engine, and a single-transaction incident state machine — API-first on Rust, no per-monitor limits, no data leaving the machine.

## Operating Context

- Run locally or self-hosted (`npm run dev`, Docker API, static frontend); single-node SQLite in WAL mode.
- Daily workflow: scan fleet status, drill into a monitor's check history, review reports across 24h/7d/30d windows, adjust check defaults; API consumers use Swagger UI at `/api/v1/docs`.
- Evaluation happens through the GitHub README, a 52-second demo video, screenshots, and a CI gate on every push.

## Capabilities and Constraints

- Monitors CRUD with pause/resume/manual check; 1-second-tick scheduler with atomic claim and concurrency semaphore; retries with exponential backoff; incidents; reports; settings; CSV export; ⌘K command palette; light/dark theme.
- Constraints: no authentication (put a reverse proxy in front for shared deploys); single-process scheduler; check-based uptime only; paused time is not excluded from uptime percentages; private targets are blocked by default (dev-only override).
- Terminology to preserve: monitor, check, incident, Degraded (amber: nominally up with consecutive failures), missing data shown as "—".

## Brand Commitments

- Name: PulseWatch. Voice: operator-honest, no hype ("real check data, never fabricated").
- Binding visual constraint, recorded without expansion: the project's prompt.md brief pins a cyberpunk-infused premium observability direction — layered midnight grounds, glass surfaces, emerald/rose/amber status semantics, tabular numerals. The user additionally pinned Phosphor as the icon set and an effect intensity of "data calm, chrome loud".

## Evidence on Hand

- Working app: backend 79 tests, frontend 36 tests, CI green; `README.md` with use cases; `docs/linkedin-demo.mp4` + `docs/linkedin-poster.png`; five screenshots in `docs/screenshots/`; `.env.example`; OpenAPI at `/api/v1/docs`.
- Absent and never to be fabricated: customers, testimonials, benchmarks, pricing.

## Product Principles

1. Honest numbers or no numbers — never fabricate uptime.
2. The operator's scan path beats decoration; data stays calm while chrome may be loud.
3. Safety by default (SSRF, DNS-rebinding) is a product feature, not an afterthought.
4. The repository is the portfolio — every visible decision should survive close reading.
5. Single-node simplicity; no distributed-system cosplay.
