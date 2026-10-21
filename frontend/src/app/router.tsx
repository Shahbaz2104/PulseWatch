import { lazy, Suspense } from "react";
import { createBrowserRouter, RouterProvider } from "react-router-dom";
import { AppLayout } from "@/layouts/AppLayout";
import { RouteFallback } from "@/components/route-fallback";
import { NotFoundPage } from "@/pages/NotFoundPage";

const OverviewPage = lazy(() =>
  import("@/pages/OverviewPage").then((module) => ({ default: module.OverviewPage })),
);
const MonitorsPage = lazy(() =>
  import("@/pages/MonitorsPage").then((module) => ({ default: module.MonitorsPage })),
);
const MonitorDetailPage = lazy(() =>
  import("@/pages/MonitorDetailPage").then((module) => ({ default: module.MonitorDetailPage })),
);
const IncidentsPage = lazy(() =>
  import("@/pages/IncidentsPage").then((module) => ({ default: module.IncidentsPage })),
);
const ReportsPage = lazy(() =>
  import("@/pages/ReportsPage").then((module) => ({ default: module.ReportsPage })),
);
const SettingsPage = lazy(() =>
  import("@/pages/SettingsPage").then((module) => ({ default: module.SettingsPage })),
);
const LoginPage = lazy(() =>
  import("@/pages/LoginPage").then((module) => ({ default: module.LoginPage })),
);

const router = createBrowserRouter([
  {
    path: "/login",
    element: (
      <Suspense fallback={<RouteFallback fullScreen />}>
        <LoginPage />
      </Suspense>
    ),
    errorElement: <NotFoundPage />,
  },
  {
    path: "/",
    element: <AppLayout />,
    errorElement: <NotFoundPage />,
    children: [
      { index: true, element: <OverviewPage /> },
      { path: "monitors", element: <MonitorsPage /> },
      { path: "monitors/:monitorId", element: <MonitorDetailPage /> },
      { path: "incidents", element: <IncidentsPage /> },
      { path: "reports", element: <ReportsPage /> },
      { path: "settings", element: <SettingsPage /> },
      { path: "*", element: <NotFoundPage /> },
    ],
  },
]);

export function AppRouter() {
  return <RouterProvider router={router} />;
}
