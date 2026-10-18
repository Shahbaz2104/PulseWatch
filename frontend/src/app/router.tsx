import { createBrowserRouter, RouterProvider } from "react-router-dom";
import { AppLayout } from "@/layouts/AppLayout";
import { OverviewPage } from "@/pages/OverviewPage";
import { MonitorsPage } from "@/pages/MonitorsPage";
import { MonitorDetailPage } from "@/pages/MonitorDetailPage";
import { IncidentsPage } from "@/pages/IncidentsPage";
import { ReportsPage } from "@/pages/ReportsPage";
import { SettingsPage } from "@/pages/SettingsPage";
import { NotFoundPage } from "@/pages/NotFoundPage";

const router = createBrowserRouter([
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
