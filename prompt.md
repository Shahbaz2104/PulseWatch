# Build PulseWatch — A Full-Stack Website Uptime Monitoring Platform in Rust

## 1. Your Role

Act as a senior Rust backend engineer, asynchronous systems engineer, database architect, frontend developer, UI/UX designer, and software testing engineer.

Build a complete, working, portfolio-quality application called **PulseWatch**.

PulseWatch is a website uptime monitoring platform that allows users to register website URLs, check their availability automatically, track response times, detect outages, retry failed checks, and review historical uptime reports.

This project is intended for an intermediate Rust developer who wants to demonstrate backend engineering skills through a practical application.

Do not deliver only a plan, tutorial, static dashboard, or incomplete prototype. Implement the real backend, persistent database, asynchronous monitoring engine, frontend, tests, and documentation.

Inspect the existing repository before modifying it. Preserve useful existing work.

## 2. Core Objectives

The completed application must support:

1. Adding and managing monitored websites.
2. Scheduling recurring HTTP health checks.
3. Measuring response times and HTTP status codes.
4. Running checks concurrently with controlled resource usage.
5. Handling timeouts and retrying transient failures.
6. Persisting every relevant check result.
7. Calculating uptime percentages and downtime durations.
8. Displaying historical performance charts.
9. Detecting status transitions and recording incidents.
10. Providing a modern, responsive dashboard.
11. Running reliably with a single documented local development workflow.
12. Demonstrating idiomatic Rust, async/await, Tokio, testing, and database design.

## 3. Technology Stack

### Rust backend

Use:

- Rust stable.
- Axum for REST API routing.
- Tokio for asynchronous execution and scheduling.
- Reqwest for outbound HTTP requests.
- SQLx with SQLite for persistent storage.
- Serde and serde_json for serialization.
- Chrono or another suitable date/time library.
- UUID for resource identifiers.
- `thiserror` for typed errors.
- `tracing` and `tracing-subscriber` for structured logging.
- `tower-http` for CORS, tracing, and HTTP middleware.
- `utoipa` and Swagger UI, or an equivalent OpenAPI solution.
- `csv` only if CSV export is implemented.
- `tokio-util` or equivalent cancellation support if needed.

Use compatible stable crate versions and verify the actual APIs for the versions selected.

### Frontend

Use:

- React.
- TypeScript with strict mode.
- Vite.
- Tailwind CSS.
- shadcn/ui.
- Lucide React icons.
- Recharts for charts.
- TanStack Query for API requests and caching.
- React Router if multiple pages require routing.
- React Hook Form and Zod for forms and client-side validation.

Use shadcn/ui components where appropriate. Create a cohesive design system rather than styling each component independently.

### Database

Use SQLite as the primary database for the first version.

Requirements:

- SQLx migrations.
- Foreign-key enforcement.
- Indexes for frequently queried fields.
- Parameterized SQL queries.
- Database transactions for multi-step updates.
- A documented database path.
- No automatic deletion or replacement of existing data.

Do not introduce Redis, Kafka, Kubernetes, or a distributed microservice architecture in the initial implementation.

## 4. UI/UX Design

Create a premium monitoring dashboard inspired by modern developer tools and observability platforms.

### Design direction

- Modern, professional, minimalist interface.
- Dark navy or charcoal theme with emerald-green accents.
- Optional light theme if practical.
- Subtle borders, restrained shadows, consistent spacing.
- Clear typography and accessible contrast.
- Green for operational status, red for outages, amber for degraded states, and neutral gray for unknown states.
- Responsive layouts for desktop, tablet, and mobile.
- Smooth but restrained transitions.
- Skeleton loading states and useful empty states.
- Toast notifications for success and failure.

Avoid excessive gradients, unnecessary animations, oversized headings, and decorative charts with fabricated values.

### Application shell

Implement:

- Collapsible desktop sidebar.
- Mobile navigation.
- Top navigation bar.
- Product logo and PulseWatch branding.
- Page title and contextual actions.
- Consistent content containers.
- A theme-aware component system.

Sidebar navigation:

1. Overview
2. Monitors
3. Incidents
4. Reports
5. Settings

### Overview dashboard

Display data obtained from the real backend:

- Total monitors.
- Currently operational monitors.
- Monitors currently experiencing outages.
- Monitors with unknown status.
- Overall uptime for the selected reporting window.
- Average response time.
- Recent incidents.
- Recent monitoring activity.
- Response-time history chart.
- Uptime summary over the last 24 hours.
- A quick-add monitor action.

Provide useful empty states for new installations. Never fabricate uptime percentages or historical monitoring data to make the dashboard look populated.

### Monitors page

Create a table or responsive card layout containing:

- Monitor name.
- URL.
- Current status.
- Latest HTTP status code, when available.
- Latest response time.
- Uptime percentage for the selected period.
- Check interval.
- Last checked timestamp.
- Next scheduled check when it can be calculated reliably.
- Actions to view, edit, pause, resume, check now, and delete.

Add:

- Search by name or URL.
- Status filters.
- Sorting.
- Pagination if required.
- Date-range controls for relevant statistics.
- Confirmation dialogs for deletion.
- Clear pause/resume indicators.

### Monitor details page

For each monitor, display:

- Monitor name and URL.
- Current operational status.
- Latest response time.
- Latest HTTP status code.
- Uptime percentage over 24 hours, 7 days, and 30 days, where sufficient data exists.
- Response-time line chart.
- Historical check results.
- Incident history.
- Recent errors.
- Monitor configuration.
- Manual check action.
- Pause/resume controls.

Use real check records and explain insufficient history instead of inventing metrics.

### Incidents page

Display:

- Incident title.
- Affected monitor.
- Start time.
- End time, when resolved.
- Duration.
- Current incident state.
- Error or status information.
- Incident history filters.

Record an incident when a monitor transitions from operational to down. Resolve it when the configured recovery rule is satisfied.

Avoid creating a new incident for every failed retry. Use a state-transition-based incident model.

### Reports page

Provide:

- Uptime percentages by monitor.
- Average response time.
- Median and p95 response time where enough observations exist.
- Outage count.
- Total recorded downtime.
- Response-time history.
- Daily or hourly aggregation where useful.
- Configurable reporting windows.
- Optional CSV export.

Use backend aggregation where practical.

### Settings page

Include:

- Default monitoring interval.
- Default request timeout.
- Default retry policy.
- Default expected HTTP status range.
- Maximum concurrent checks.
- Theme preference.
- Data retention configuration if implemented.

Validate all configuration values. Explain when a setting requires restarting the scheduler.

## 5. Monitor Data Model

Create normalized SQL migrations for the following entities.

### Monitor

Suggested fields:

- `id`
- `name`
- `url`
- `enabled`
- `check_interval_seconds`
- `timeout_ms`
- `expected_status_min`
- `expected_status_max`
- `consecutive_failures_threshold`
- `consecutive_successes_threshold`
- `current_status`
- `created_at`
- `updated_at`
- `last_checked_at`
- `next_check_at`, if persisted by the scheduling design

Define reasonable limits for configurable intervals and timeouts.

The supported status model should include:

- `unknown`
- `up`
- `down`
- `paused`

Treat paused status carefully: a paused monitor should not be reported as operational merely because its most recent check succeeded.

### CheckResult

Suggested fields:

- `id`
- `monitor_id`
- `checked_at`
- `completed_at`, if needed
- `http_status_code`, nullable
- `response_time_ms`, nullable
- `success`
- `error_type`, nullable
- `error_message`, nullable and sanitized
- `attempt_count`
- `expected_status_match`

Record the final result of each scheduled check.

For richer diagnostics, optionally store attempt-level records in a separate table. Do not store unbounded response bodies or sensitive headers.

Distinguish HTTP response failures from transport failures, timeouts, and DNS errors.

### Incident

Suggested fields:

- `id`
- `monitor_id`
- `started_at`
- `resolved_at`, nullable
- `status`
- `failure_reason`, nullable
- `created_at`
- `updated_at`

Use a transaction when recording status changes and opening or resolving incidents.

Prevent duplicate active incidents for the same monitor.

### Indexes and constraints

- Index check results by monitor and timestamp.
- Index monitors by enabled state and next scheduled check where applicable.
- Index incidents by monitor and start time.
- Use foreign keys.
- Add appropriate uniqueness and validity constraints.
- Ensure paused or deleted monitors cannot accidentally continue receiving scheduled checks.

Use migrations rather than creating tables informally during every startup.

## 6. HTTP Monitoring Engine

This is the core of the project. Implement a real monitoring engine, not just a button that sends an HTTP request.

### HTTP requests

Use Reqwest with a reusable HTTP client.

For each check:

1. Validate the monitor configuration.
2. Record the start time.
3. Send a GET request to the configured URL.
4. Measure elapsed wall-clock duration using `std::time::Instant`.
5. Capture the HTTP status code when a response is received.
6. Evaluate whether the response matches the monitor's expected status policy.
7. Classify timeouts and transport errors.
8. Apply retry rules when appropriate.
9. Persist the final result.
10. Update monitor state and incident records if a transition occurs.

Do not treat every HTTP response as a transport failure. For example, a 500 response is a received HTTP response, but it may fail the monitor's expected-status policy.

Do not follow redirects indefinitely. Use Reqwest's safe redirect defaults or configure a bounded redirect policy and document it.

Record response time consistently. Define whether it represents the duration until response headers arrive or another specific milestone. Use the same definition in charts and reports.

### Timeouts

Support:

- Per-monitor request timeout.
- A global maximum timeout.
- Cancellation of timed-out requests.
- Useful error classification.
- Protection against tasks that never finish.

Use appropriate timeout mechanisms from Tokio and Reqwest.

### Retries

Implement a bounded retry policy:

- Configurable maximum attempts.
- Configurable initial backoff.
- Exponential backoff.
- Optional jitter.
- Maximum backoff duration.
- Retry only errors considered transient by the policy.

Do not retry every HTTP status indiscriminately.

For example, connection resets and timeouts may be retried, while a stable 404 response should normally be classified as a failed health check without repeated retries unless explicitly configured.

Respect `Retry-After` when appropriate and safe.

Store the total number of attempts in the final result.

Ensure retry delays do not block Tokio worker threads.

### Concurrency

Run multiple independent monitor checks concurrently.

Use Tokio tasks, a bounded worker pool, or a semaphore to limit concurrency.

Requirements:

- A configurable maximum number of concurrent checks.
- No unbounded spawning of tasks.
- Isolation so one slow website does not block unrelated checks.
- Controlled database access.
- Useful structured logs.
- Graceful cancellation and shutdown.
- Avoid overlapping scheduled checks for the same monitor.

Use async-aware synchronization and avoid blocking mutexes across `.await` points.

### Scheduling

Implement a background scheduler that periodically finds monitors due for checking.

The scheduler should:

- Load enabled monitors.
- Respect each monitor's interval.
- Trigger checks independently of incoming frontend requests.
- Avoid scheduling paused monitors.
- Avoid overlapping checks for the same monitor.
- Recover sensibly after a restart.
- Stop cleanly when the application shuts down.

Do not implement a busy loop. Use Tokio intervals, sleep-based scheduling, or another appropriate approach.

Document scheduling semantics, including whether an interval is measured from the previous scheduled time or from the completion of the previous check.

For the first version, a single Rust backend process is sufficient. If multiple processes are later supported, add distributed coordination before claiming duplicate-free scheduling across replicas.

### Manual checks

Provide a "Check now" action.

Manual checks must:

- Use the same validation and checking service as scheduled checks.
- Respect global concurrency limits.
- Avoid conflicting with an already-running check for the same monitor.
- Return a meaningful response to the frontend.
- Persist results consistently.

Avoid duplicating monitoring logic inside HTTP handlers.

## 7. Uptime and Performance Calculations

Implement all metrics with clearly documented definitions.

### Availability

Calculate:

uptime percentage = successful completed checks / all eligible completed checks × 100

Use the selected reporting window and the monitor's applicable status policy.

Document that this is check-based availability, not proof that every instant in the window was available.

Exclude paused periods from the denominator when reliable pause history is available. If pause history is not implemented, clearly document the limitation.

When no eligible checks exist, return a null or an explicit unavailable state rather than claiming 100% uptime.

### Response time

Calculate:

- Latest response time.
- Average response time.
- Median response time.
- p95 response time where the sample size supports a useful interpretation.

Document how failed requests without response headers are represented.

Avoid treating missing response times as zero.

### Downtime

Use incident start and resolution timestamps to calculate incident duration.

For unresolved incidents, calculate the elapsed duration at query time.

Do not double-count overlapping incidents.

### Time windows

Support:

- Last 24 hours.
- Last 7 days.
- Last 30 days.
- Custom start and end timestamps, with validation.

Use UTC timestamps internally and convert to the user's local time only for display.

### Aggregation

Perform database-side aggregation where practical.

For long history windows, aggregate into sensible time buckets rather than sending every raw check to the frontend.

Define behavior for gaps, sparse data, newly created monitors, and changed check intervals.

## 8. REST API Design

Use a consistent API prefix: `/api/v1`.

### Health

- `GET /health`
- `GET /api/v1/health`

Return service health without leaking secrets or sensitive configuration.

### Monitors

- `GET /api/v1/monitors`
- `GET /api/v1/monitors/{id}`
- `POST /api/v1/monitors`
- `PUT /api/v1/monitors/{id}`
- `DELETE /api/v1/monitors/{id}`
- `POST /api/v1/monitors/{id}/check`
- `POST /api/v1/monitors/{id}/pause`
- `POST /api/v1/monitors/{id}/resume`

Validate URLs and monitor configuration before persistence.

### Check history

- `GET /api/v1/monitors/{id}/checks`
- `GET /api/v1/monitors/{id}/checks/latest`
- `GET /api/v1/monitors/{id}/checks/summary`

Support pagination and date filters.

### Reports

- `GET /api/v1/reports/overview`
- `GET /api/v1/reports/uptime`
- `GET /api/v1/reports/response-times`
- `GET /api/v1/reports/incidents`

### Incidents

- `GET /api/v1/incidents`
- `GET /api/v1/incidents/{id}`

Support status, monitor, and date filters.

### Validation

Validate:

- URL scheme.
- Hostname presence.
- Supported URL formats.
- Check intervals.
- Timeout bounds.
- Retry configuration.
- Expected status-code range.
- Pagination limits.
- Date ranges.

Return a consistent JSON error structure and appropriate HTTP status codes.

Document the endpoints with OpenAPI and provide example requests and responses.

## 9. URL Safety and SSRF Protection

The monitor accepts user-provided URLs, so implement deliberate outbound-request protections.

- Allow only HTTP and HTTPS.
- Reject embedded credentials in URLs.
- Reject localhost and loopback destinations.
- Block private, link-local, multicast, unspecified, and other prohibited IP ranges for public monitoring.
- Validate resolved IPv4 and IPv6 addresses.
- Revalidate destination addresses when redirects occur.
- Prevent redirects from bypassing destination restrictions.
- Do not allow users to access cloud metadata endpoints or internal infrastructure.
- Set reasonable response and connection timeouts.
- Do not expose internal request headers or secrets in errors.

A simple hostname string check is not sufficient.

Account for DNS rebinding: validating DNS once and then letting the HTTP client resolve the hostname independently can be insufficient. Implement a carefully designed DNS and connection policy or use a well-tested mechanism that validates the actual destination IP used for connections. If full protection is not implemented, document the limitation and restrict the service to trusted local use rather than claiming it is safe for public multi-user deployment.

For local development and testing, use explicitly configured test fixtures or a controlled test mode. Do not weaken production URL restrictions globally just to make localhost tests work.

## 10. Error Handling and Observability

Use typed application errors and consistent HTTP responses.

Classify failures such as:

- DNS resolution failure.
- Connection failure.
- Timeout.
- TLS error.
- HTTP status mismatch.
- Invalid configuration.
- Database error.
- Cancelled operation.

Do not expose sensitive internal details to the public API.

Use `tracing` to record:

- Monitor identifier.
- Check start and completion.
- Attempt number.
- Elapsed time.
- Result classification.
- Scheduler events.
- Incident transitions.
- Unexpected errors.

Avoid logging secrets, sensitive headers, or unnecessarily large URLs with embedded query parameters.

Use structured logs and configure useful log levels.

## 11. Testing Strategy

Write real tests using deterministic fixtures.

### Unit tests

Test:

- Uptime calculations.
- Response-time summaries.
- Percentile calculations.
- Retry classification.
- Backoff calculation.
- Timeout classification.
- Status transitions.
- Incident creation and resolution.
- URL validation.
- IPv4 and IPv6 destination restrictions.
- Reporting window validation.

### Integration tests

Test:

- Creating and retrieving monitors.
- Updating monitor configuration.
- Pausing and resuming monitors.
- Manual checks.
- Scheduled checks.
- Successful HTTP responses.
- HTTP status mismatches.
- Timeouts.
- Connection failures.
- Retry limits.
- Database persistence.
- Incident creation and recovery.
- Report endpoints.
- Pagination.
- Invalid requests.
- Deleting monitors safely.

Use a local test HTTP server with controllable responses and delays. Do not depend on external websites or public DNS for automated tests.

Make tests deterministic and isolate their databases.

Test concurrency limits and verify that one slow request does not block all other monitors.

### Frontend tests

Test:

- Monitor creation and validation.
- Edit and delete flows.
- Pause/resume behavior.
- Loading and error states.
- Status badges.
- Dashboard metrics.
- Time-window selection.
- Empty states.
- API error handling.

## 12. Repository Structure

Use a maintainable workspace, for example:

pulsewatch/
  README.md
  .gitignore
  .env.example
  Cargo.toml
  Cargo.lock
  migrations/
  backend/
    Cargo.toml
    src/
      main.rs
      config.rs
      state.rs
      error.rs
      routes/
        mod.rs
        health.rs
        monitors.rs
        checks.rs
        reports.rs
        incidents.rs
      models/
        mod.rs
        monitor.rs
        check_result.rs
        incident.rs
      services/
        mod.rs
        monitoring_service.rs
        scheduler.rs
        retry.rs
        report_service.rs
        incident_service.rs
      repositories/
        mod.rs
        monitor_repository.rs
        check_repository.rs
        incident_repository.rs
      validation/
        mod.rs
        url_validation.rs
  frontend/
    package.json
    src/
      api/
      components/
      features/
      hooks/
      layouts/
      pages/
      types/
      lib/
  tests/

Adjust the structure to suit the actual workspace. Avoid empty modules and unnecessary abstraction layers.

## 13. Configuration and Startup

Provide an `.env.example` containing documented non-secret settings such as:

- `HOST`
- `PORT`
- `DATABASE_URL`
- `RUST_LOG`
- Default check interval.
- Default timeout.
- Maximum concurrency.
- Default retry count.
- Default backoff configuration.

Use appropriate configuration names and types.

Validate configuration at startup. Never commit real secrets or user data.

Create the SQLite parent directory when appropriate.

The backend should start its scheduler automatically and shut down gracefully when terminated.

Document whether database migrations run automatically or through a separate command. Never destroy existing data during startup.

Provide a simple developer workflow for running the frontend and backend concurrently.

## 14. Performance and Reliability

- Reuse a configured Reqwest client rather than creating one for every check.
- Bound concurrent requests.
- Keep database transactions short.
- Avoid blocking operations in asynchronous handlers.
- Use pagination for historical check data.
- Index common queries.
- Avoid unbounded in-memory history.
- Configure HTTP response-size limits where appropriate.
- Make cancellation and shutdown behavior predictable.
- Use sensible retention policies for large check histories if implemented.
- Avoid running multiple simultaneous checks for the same monitor unintentionally.
- Avoid blocking the entire scheduler while one retry is sleeping.

Document trade-offs and avoid premature optimization.

## 15. GitHub README

Create a professional README with:

- Project overview.
- Screenshots.
- Feature list.
- Technology stack.
- Architecture diagram using Mermaid.
- Database schema explanation.
- Installation instructions.
- Environment setup.
- Database migration commands.
- Backend and frontend startup commands.
- API documentation instructions.
- Example monitor creation request.
- Testing commands.
- Scheduling and retry design.
- Uptime calculation methodology.
- Security and SSRF considerations.
- Limitations and future improvements.

Never claim that a feature, test, benchmark, or security control exists unless it has actually been implemented and verified.

## 16. Implementation Process

Follow these stages.

### Stage 1: Inspect and plan

Inspect the current repository and available Rust toolchain.

Produce a concise implementation plan and identify dependency compatibility issues. Preserve existing useful files.

### Stage 2: Backend foundation

Set up the Rust workspace, Axum, configuration, SQLite, migrations, error handling, tracing, and health endpoints.

Compile and test before continuing.

### Stage 3: Monitoring engine

Implement URL validation, HTTP checks, response-time measurement, timeout handling, retry policies, bounded concurrency, and database persistence.

Write tests using a local mock HTTP server.

### Stage 4: Scheduler and incidents

Implement recurring checks, pause/resume behavior, duplicate-check prevention, graceful shutdown, status transitions, and incident tracking.

Verify behavior after restarting the backend.

### Stage 5: Reporting API

Implement uptime summaries, latency statistics, time-window filtering, and historical check pagination.

Verify calculations against hand-computed test cases.

### Stage 6: Frontend

Implement the responsive dashboard, monitor management, monitor details, incidents, reports, and settings.

Connect every data-driven component to the actual API.

### Stage 7: Verification

Run:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`, adapting feature flags if necessary.
- `cargo test`
- Frontend lint and tests.
- Frontend production build.
- Manual end-to-end checks.

Fix root causes rather than hiding failures.

### Stage 8: Documentation and delivery

Complete the README and provide verified startup commands.

Report what was implemented, which tests were actually executed, their real results, and any outstanding limitations.

## 17. Non-Negotiable Rules

- Do not stop after writing a plan.
- Do not build a static-only dashboard.
- Do not simulate monitoring results in the finished application.
- Do not use external websites as test dependencies.
- Do not leave critical API handlers unimplemented.
- Do not use `unwrap()` indiscriminately in production paths.
- Do not disable tests to make the build pass.
- Do not invent successful test results.
- Do not expose internal network resources through arbitrary URL monitoring.
- Do not claim SSRF protection without addressing DNS resolution and redirect behavior.
- Do not overwrite existing files without inspecting them.
- Keep the implementation understandable to an intermediate Rust learner.
- Make reasonable decisions independently and document significant trade-offs.

## Definition of Done

A user must be able to start PulseWatch locally, add a website, configure its interval and timeout, run a manual check, observe automatic background checks, see HTTP status and response time, inspect historical results, pause and resume monitoring, view incidents, and obtain accurate uptime reports.

The application must persist data in SQLite, respect concurrency limits, handle transient failures and timeouts, pass its important automated tests, and include a complete README.

