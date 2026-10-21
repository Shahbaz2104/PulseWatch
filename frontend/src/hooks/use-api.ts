/** TanStack Query hooks. Polling cadence: overview/lists 30s, detail 15s —
 * paused automatically when the tab is hidden (Query default behaviour). */

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  authApi,
  checksApi,
  healthApi,
  incidentsApi,
  monitorsApi,
  reportsApi,
  settingsApi,
} from "@/lib/endpoints";
import type { CreateMonitorRequest, UpdateMonitorRequest, WindowQuery } from "@/types/api";

export const queryKeys = {
  monitors: (params: Record<string, unknown>) => ["monitors", params] as const,
  monitor: (id: string) => ["monitor", id] as const,
  checks: (id: string, params: Record<string, unknown>) => ["checks", id, params] as const,
  checkSummary: (id: string, window: WindowQuery) => ["check-summary", id, window] as const,
  overview: (window: WindowQuery) => ["overview", window] as const,
  uptime: (window: WindowQuery) => ["uptime", window] as const,
  responseTimes: (window: WindowQuery, monitorId?: string) =>
    ["response-times", window, monitorId] as const,
  incidentReport: (window: WindowQuery) => ["incident-report", window] as const,
  incidents: (params: Record<string, unknown>) => ["incidents", params] as const,
  settings: ["settings"] as const,
  health: ["health"] as const,
  auth: ["auth"] as const,
};

const LIST_POLL_MS = 30_000;
const DETAIL_POLL_MS = 15_000;

export function useMonitors(query: Parameters<typeof monitorsApi.list>[0] = {}) {
  return useQuery({
    queryKey: queryKeys.monitors(query as Record<string, unknown>),
    queryFn: () => monitorsApi.list(query),
    refetchInterval: LIST_POLL_MS,
  });
}

export function useMonitor(id: string) {
  return useQuery({
    queryKey: queryKeys.monitor(id),
    queryFn: () => monitorsApi.get(id),
    refetchInterval: DETAIL_POLL_MS,
  });
}

export function useChecks(id: string, query: Parameters<typeof checksApi.list>[1]) {
  return useQuery({
    queryKey: queryKeys.checks(id, query as Record<string, unknown>),
    queryFn: () => checksApi.list(id, query),
    refetchInterval: DETAIL_POLL_MS,
  });
}

export function useCheckSummary(id: string, window: WindowQuery) {
  return useQuery({
    queryKey: queryKeys.checkSummary(id, window),
    queryFn: () => checksApi.summary(id, window),
    refetchInterval: DETAIL_POLL_MS,
  });
}

export function useOverview(window: WindowQuery = {}) {
  return useQuery({
    queryKey: queryKeys.overview(window),
    queryFn: () => reportsApi.overview(window),
    refetchInterval: LIST_POLL_MS,
  });
}

export function useUptimeReport(window: WindowQuery = {}) {
  return useQuery({
    queryKey: queryKeys.uptime(window),
    queryFn: () => reportsApi.uptime(window),
    refetchInterval: LIST_POLL_MS,
  });
}

export function useResponseTimes(window: WindowQuery = {}, monitorId?: string) {
  return useQuery({
    queryKey: queryKeys.responseTimes(window, monitorId),
    queryFn: () => reportsApi.responseTimes({ ...window, monitorId }),
    refetchInterval: LIST_POLL_MS,
  });
}

export function useIncidentReport(window: WindowQuery = {}) {
  return useQuery({
    queryKey: queryKeys.incidentReport(window),
    queryFn: () => reportsApi.incidents(window),
    refetchInterval: LIST_POLL_MS,
  });
}

export function useIncidents(query: Parameters<typeof incidentsApi.list>[0] = {}) {
  return useQuery({
    queryKey: queryKeys.incidents(query as Record<string, unknown>),
    queryFn: () => incidentsApi.list(query),
    refetchInterval: LIST_POLL_MS,
  });
}

export function useSettings() {
  return useQuery({
    queryKey: queryKeys.settings,
    queryFn: settingsApi.get,
    staleTime: 60_000,
  });
}

/** True when the backend runs as a read-only public demo. */
export function useDemoMode(): boolean {
  return useAuthStatus().data?.demo_mode ?? false;
}

/** Public session state used to render sign-in/out controls. */
export function useAuthStatus() {
  return useQuery({
    queryKey: queryKeys.auth,
    queryFn: authApi.status,
    staleTime: 30_000,
    retry: false,
  });
}

export function useLogin() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: authApi.login,
    onSuccess: () => {
      queryClient.setQueryData<{ authenticated: boolean; demo_mode: boolean }>(
        queryKeys.auth,
        { authenticated: true, demo_mode: false },
      );
      void queryClient.invalidateQueries();
    },
  });
}

export function useLogout() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: authApi.logout,
    onSuccess: () => {
      queryClient.setQueryData<{ authenticated: boolean; demo_mode: boolean }>(
        queryKeys.auth,
        (current) => ({ authenticated: false, demo_mode: current?.demo_mode ?? false }),
      );
      void queryClient.invalidateQueries();
    },
  });
}

/** Server capabilities needed for UI hints (never fetches secrets). */
export function useHealth() {
  return useQuery({
    queryKey: queryKeys.health,
    queryFn: healthApi.get,
    staleTime: 60_000,
    retry: false,
  });
}

/** Shared invalidation after mutations that change check/monitor state. */
export function useInvalidateMonitorData() {
  const queryClient = useQueryClient();
  return () => {
    void queryClient.invalidateQueries({ queryKey: ["monitors"] });
    void queryClient.invalidateQueries({ queryKey: ["monitor"] });
    void queryClient.invalidateQueries({ queryKey: ["checks"] });
    void queryClient.invalidateQueries({ queryKey: ["check-summary"] });
    void queryClient.invalidateQueries({ queryKey: ["overview"] });
    void queryClient.invalidateQueries({ queryKey: ["uptime"] });
    void queryClient.invalidateQueries({ queryKey: ["response-times"] });
    void queryClient.invalidateQueries({ queryKey: ["incident-report"] });
    void queryClient.invalidateQueries({ queryKey: ["incidents"] });
  };
}

export function useCreateMonitor() {
  const invalidate = useInvalidateMonitorData();
  return useMutation({
    mutationFn: (body: CreateMonitorRequest) => monitorsApi.create(body),
    onSuccess: invalidate,
  });
}

export function useUpdateMonitor(id: string) {
  const invalidate = useInvalidateMonitorData();
  return useMutation({
    mutationFn: (body: UpdateMonitorRequest) => monitorsApi.update(id, body),
    onSuccess: invalidate,
  });
}

export function useDeleteMonitor() {
  const invalidate = useInvalidateMonitorData();
  return useMutation({
    mutationFn: (id: string) => monitorsApi.remove(id),
    onSuccess: invalidate,
  });
}

export function useCheckNow() {
  const invalidate = useInvalidateMonitorData();
  return useMutation({
    mutationFn: (id: string) => monitorsApi.checkNow(id),
    onSuccess: invalidate,
  });
}

export function usePauseMonitor() {
  const invalidate = useInvalidateMonitorData();
  return useMutation({
    mutationFn: (id: string) => monitorsApi.pause(id),
    onSuccess: invalidate,
  });
}

export function useResumeMonitor() {
  const invalidate = useInvalidateMonitorData();
  return useMutation({
    mutationFn: (id: string) => monitorsApi.resume(id),
    onSuccess: invalidate,
  });
}

export function useUpdateSettings() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: settingsApi.update,
    onSuccess: (data) => {
      queryClient.setQueryData(queryKeys.settings, data.settings);
      void queryClient.invalidateQueries({ queryKey: queryKeys.settings });
    },
  });
}
