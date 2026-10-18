/** Typed endpoint functions — thin wrappers over `api` with query strings. */

import { api } from "@/lib/api";
import type {
  AppSettings,
  CheckResponse,
  CheckResult,
  CheckSummaryStats,
  CreateMonitorRequest,
  IncidentReport,
  IncidentWithMonitor,
  Monitor,
  MonitorSummary,
  OverviewReport,
  Paginated,
  ResponseTimeReport,
  SettingsPatch,
  SettingsUpdateResponse,
  UpdateMonitorRequest,
  UptimeReport,
  WindowQuery,
} from "@/types/api";
import { windowSearchParams } from "@/types/api";

export interface MonitorListQuery extends WindowQuery {
  search?: string;
  status?: string;
  sort?: string;
  page?: number;
  page_size?: number;
}

function qs(params: Record<string, string | number | undefined>): string {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== "") search.set(key, String(value));
  }
  const text = search.toString();
  return text ? `?${text}` : "";
}

export const monitorsApi = {
  list: (query: MonitorListQuery = {}) =>
    api.get<Paginated<MonitorSummary>>(
      `/monitors${qs({
        search: query.search,
        status: query.status,
        window: query.window,
        from: query.from,
        to: query.to,
        page: query.page,
        page_size: query.page_size,
      })}`,
    ),
  get: (id: string) => api.get<MonitorSummary>(`/monitors/${id}`),
  create: (body: CreateMonitorRequest) => api.post<MonitorSummary>("/monitors", body),
  update: (id: string, body: UpdateMonitorRequest) =>
    api.put<MonitorSummary>(`/monitors/${id}`, body),
  remove: (id: string) => api.delete<void>(`/monitors/${id}`),
  checkNow: (id: string) => api.post<CheckResponse>(`/monitors/${id}/check`),
  pause: (id: string) => api.post<Monitor>(`/monitors/${id}/pause`),
  resume: (id: string) => api.post<Monitor>(`/monitors/${id}/resume`),
};

export const checksApi = {
  list: (id: string, query: WindowQuery & { result?: string; page?: number; page_size?: number }) =>
    api.get<Paginated<CheckResult>>(
      `/monitors/${id}/checks${qs({
        window: query.window,
        from: query.from,
        to: query.to,
        result: query.result,
        page: query.page,
        page_size: query.page_size,
      })}`,
    ),
  latest: (id: string) => api.get<CheckResult>(`/monitors/${id}/checks/latest`),
  summary: (id: string, query: WindowQuery = {}) =>
    api.get<CheckSummaryStats>(
      `/monitors/${id}/checks/summary${qs({
        window: query.window,
        from: query.from,
        to: query.to,
      })}`,
    ),
};

export const reportsApi = {
  overview: (query: WindowQuery = {}) =>
    api.get<OverviewReport>(
      `/reports/overview${qs({
        window: query.window,
        from: query.from,
        to: query.to,
      })}`,
    ),
  uptime: (query: WindowQuery = {}) =>
    api.get<UptimeReport>(
      `/reports/uptime${qs({
        window: query.window,
        from: query.from,
        to: query.to,
      })}`,
    ),
  responseTimes: (query: WindowQuery & { monitorId?: string } = {}) =>
    api.get<ResponseTimeReport>(
      `/reports/response-times${qs({
        window: query.window,
        from: query.from,
        to: query.to,
        monitor_id: query.monitorId,
      })}`,
    ),
  incidents: (query: WindowQuery & { monitorId?: string } = {}) =>
    api.get<IncidentReport>(
      `/reports/incidents${qs({
        window: query.window,
        from: query.from,
        to: query.to,
        monitor_id: query.monitorId,
      })}`,
    ),
};

export const incidentsApi = {
  list: (
    query: WindowQuery & {
      status?: string;
      monitorId?: string;
      page?: number;
      page_size?: number;
    } = {},
  ) =>
    api.get<Paginated<IncidentWithMonitor>>(
      `/incidents${qs({
        status: query.status,
        monitor_id: query.monitorId,
        from: query.from,
        to: query.to,
        page: query.page,
        page_size: query.page_size,
      })}`,
    ),
};

export const settingsApi = {
  get: () => api.get<AppSettings>("/settings"),
  update: (patch: SettingsPatch) => api.put<SettingsUpdateResponse>("/settings", patch),
};

export { windowSearchParams };
