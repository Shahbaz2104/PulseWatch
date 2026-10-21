/** TypeScript mirrors of the backend API DTOs (serde field names). */

export type MonitorStatus = "unknown" | "up" | "down" | "paused";
export type IncidentStatus = "open" | "resolved";
export type CheckErrorType =
  | "dns"
  | "connection"
  | "timeout"
  | "tls"
  | "http_status_mismatch"
  | "too_many_redirects"
  | "blocked_target"
  | "cancelled"
  | "internal";

export interface Monitor {
  id: string;
  name: string;
  url: string;
  enabled: boolean;
  check_interval_seconds: number;
  timeout_ms: number;
  expected_status_min: number;
  expected_status_max: number;
  consecutive_failures_threshold: number;
  consecutive_successes_threshold: number;
  retry_max_attempts: number;
  /** Alert destinations are write-only: the API reports configuration only. */
  alert_webhook_configured: boolean;
  alert_email_configured: boolean;
  current_status: MonitorStatus;
  consecutive_failures: number;
  consecutive_successes: number;
  created_at: string;
  updated_at: string;
  last_checked_at: string | null;
  next_check_at: string | null;
}

export interface MonitorSummary extends Monitor {
  latest_http_status: number | null;
  latest_response_time_ms: number | null;
  latest_success: boolean | null;
  /** `null` when the window has no checks — never coerced to 100. */
  uptime_percent: number | null;
  checks_in_window: number;
  degraded: boolean;
}

export interface Pagination {
  page: number;
  page_size: number;
  total: number;
  total_pages: number;
}

export interface Paginated<T> {
  data: T[];
  pagination: Pagination;
}

export interface CheckResult {
  id: string;
  monitor_id: string;
  checked_at: string;
  completed_at: string | null;
  http_status_code: number | null;
  response_time_ms: number | null;
  success: boolean;
  error_type: CheckErrorType | null;
  error_message: string | null;
  attempt_count: number;
  expected_status_match: boolean;
}

export interface CheckSummaryStats {
  total_checks: number;
  successful_checks: number;
  uptime_percent: number | null;
  latest_response_time_ms: number | null;
  avg_response_time_ms: number | null;
  median_response_time_ms: number | null;
  p95_response_time_ms: number | null;
  window_start: string;
  window_end: string;
}

export interface CheckResponse {
  monitor: Monitor;
  check: CheckResult;
  incident_opened: boolean;
  incident_resolved: boolean;
}

export interface Incident {
  id: string;
  monitor_id: string;
  started_at: string;
  resolved_at: string | null;
  status: IncidentStatus;
  failure_reason: string | null;
  created_at: string;
  updated_at: string;
}

export interface IncidentWithMonitor extends Incident {
  monitor_name: string;
  monitor_url: string;
  duration_seconds: number;
}

export interface ReportWindow {
  start: string;
  end: string;
}

export interface TimeBucket {
  bucket_start: string;
  checks: number;
  successes: number;
  uptime_percent: number | null;
  avg_response_time_ms: number | null;
}

export interface MonitorUptime {
  monitor_id: string;
  name: string;
  url: string;
  current_status: MonitorStatus;
  checks: number;
  successes: number;
  paused_seconds: number;
  uptime_percent: number | null;
}

export interface UptimeReport {
  window: ReportWindow;
  monitors: MonitorUptime[];
  total_checks: number;
  total_successes: number;
  total_paused_seconds: number;
  overall_uptime_percent: number | null;
}

export interface MonitorResponseTimes {
  monitor_id: string;
  name: string;
  samples: number;
  avg_response_time_ms: number | null;
  median_response_time_ms: number | null;
  p95_response_time_ms: number | null;
}

export interface ResponseTimeReport {
  window: ReportWindow;
  bucket: "hour" | "day";
  monitors: MonitorResponseTimes[];
  series: TimeBucket[];
}

export interface MonitorCounts {
  total: number;
  up: number;
  down: number;
  unknown: number;
  paused: number;
}

export interface RecentCheck {
  monitor_id: string;
  monitor_name: string;
  checked_at: string;
  success: boolean;
  http_status_code: number | null;
  response_time_ms: number | null;
}

export interface IncidentStats {
  outage_count: number;
  downtime_seconds: number;
}

export interface OverviewReport {
  window: ReportWindow;
  monitors: MonitorCounts;
  uptime_percent: number | null;
  avg_response_time_ms: number | null;
  incidents: IncidentStats;
  recent_incidents: IncidentWithMonitor[];
  recent_checks: RecentCheck[];
  uptime_series: TimeBucket[];
  response_time_series: TimeBucket[];
}

export interface IncidentReport {
  window: ReportWindow;
  stats: IncidentStats;
  incidents: IncidentWithMonitor[];
}

export interface AppSettings {
  default_check_interval_seconds: number;
  default_timeout_ms: number;
  default_retry_max_attempts: number;
  default_retry_backoff_ms: number;
  default_retry_backoff_max_ms: number;
  default_expected_status_min: number;
  default_expected_status_max: number;
  max_concurrent_checks: number;
  check_retention_days: number;
}

export interface SettingsPatch {
  default_check_interval_seconds?: number;
  default_timeout_ms?: number;
  default_retry_max_attempts?: number;
  default_retry_backoff_ms?: number;
  default_retry_backoff_max_ms?: number;
  default_expected_status_min?: number;
  default_expected_status_max?: number;
  max_concurrent_checks?: number;
  check_retention_days?: number;
}

export interface SettingsUpdateResponse {
  settings: AppSettings;
  restart_required: string[];
}

export interface CreateMonitorRequest {
  name?: string;
  url: string;
  check_interval_seconds?: number;
  timeout_ms?: number;
  expected_status_min?: number;
  expected_status_max?: number;
  consecutive_failures_threshold?: number;
  consecutive_successes_threshold?: number;
  retry_max_attempts?: number;
  /** Omit to leave unchanged; send an empty string to clear. */
  alert_webhook_url?: string;
  /** Omit to leave unchanged; send an empty string to clear. */
  alert_email?: string;
}

export type UpdateMonitorRequest = Partial<CreateMonitorRequest>;

export interface HealthResponse {
  status: "ok" | "degraded";
  service: string;
  version: string;
  database: "ok" | "unavailable";
  email_configured: boolean;
  /** Read-only public demo: auth is bypassed and mutations are disabled. */
  demo_mode: boolean;
}

export interface AuthStatus {
  authenticated: boolean;
  demo_mode: boolean;
}

/** Reporting window query parameters shared by list/report endpoints. */
export interface WindowQuery {
  window?: "24h" | "7d" | "30d" | "custom";
  from?: string;
  to?: string;
}

export function windowSearchParams(query: WindowQuery): string {
  const params = new URLSearchParams();
  if (query.window) params.set("window", query.window);
  if (query.from) params.set("from", query.from);
  if (query.to) params.set("to", query.to);
  return params.toString();
}
