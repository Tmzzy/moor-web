import { lazy, Suspense, type ReactNode } from "react";
import {
  Navigate,
  Outlet,
  RouterProvider,
  createBrowserRouter,
  useLocation,
} from "react-router-dom";
import { Toaster } from "sonner";

import { AuthLoading } from "@/components/auth/AuthLoading";
import { AppShell } from "@/components/layout/AppShell";
import { ErrorBoundary } from "@/components/shared/ErrorBoundary";
import { PageLoading } from "@/components/shared/PageLoading";
import { SSEProvider } from "@/contexts/SSEContext";
import { useAuth } from "@/contexts/AuthContext";

const Dashboard = lazy(() =>
  import("@/pages/Dashboard").then((module) => ({ default: module.Dashboard })),
);
const Servers = lazy(() =>
  import("@/pages/Servers").then((module) => ({ default: module.Servers })),
);
const ServerDetail = lazy(() =>
  import("@/pages/ServerDetail").then((module) => ({ default: module.ServerDetail })),
);
const Profiles = lazy(() =>
  import("@/pages/Profiles").then((module) => ({ default: module.Profiles })),
);
const ProfileDetail = lazy(() =>
  import("@/pages/ProfileDetail").then((module) => ({ default: module.ProfileDetail })),
);
const AuditLogs = lazy(() =>
  import("@/pages/AuditLogs").then((module) => ({ default: module.AuditLogs })),
);
const ClientConfig = lazy(() =>
  import("@/pages/ClientConfig").then((module) => ({ default: module.ClientConfig })),
);
const SettingsPage = lazy(() =>
  import("@/pages/Settings").then((module) => ({ default: module.SettingsPage })),
);
const Login = lazy(() => import("@/pages/Login").then((module) => ({ default: module.Login })));
const NotFound = lazy(() =>
  import("@/pages/NotFound").then((module) => ({ default: module.NotFound })),
);

function page(element: ReactNode) {
  return <Suspense fallback={<PageLoading message="Loading page..." />}>{element}</Suspense>;
}

function RootLayout() {
  return (
    <>
      <Toaster
        position="top-right"
        closeButton
        toastOptions={{
          classNames: {
            toast: "font-headline rounded-lg",
            title: "text-sm font-medium",
            description: "text-xs text-[var(--fg-55)]",
            closeButton:
              "!top-2 !right-2 !left-auto !rounded-lg !border-0 !bg-transparent !text-[var(--fg-40)] hover:!bg-[var(--fg-08)] hover:!text-cursor-dark",
          },
        }}
      />
      <ErrorBoundary>
        <Outlet />
      </ErrorBoundary>
    </>
  );
}

function ProtectedApp() {
  const { status } = useAuth();
  const location = useLocation();

  if (status === "checking") return <AuthLoading />;
  if (status === "unauthenticated") {
    const from = `${location.pathname}${location.search}${location.hash}`;
    return <Navigate to="/login" replace state={{ from }} />;
  }

  return (
    <SSEProvider>
      <AppShell />
    </SSEProvider>
  );
}

const router = createBrowserRouter([
  {
    element: <RootLayout />,
    children: [
      {
        path: "/login",
        element: (
          <Suspense fallback={<AuthLoading />}>
            <Login />
          </Suspense>
        ),
      },
      {
        element: <ProtectedApp />,
        children: [
          { path: "/", element: page(<Dashboard />) },
          { path: "/servers", element: page(<Servers />) },
          { path: "/servers/:id", element: page(<ServerDetail />) },
          { path: "/profiles", element: page(<Profiles />) },
          { path: "/profiles/:id", element: page(<ProfileDetail />) },
          { path: "/logs", element: page(<AuditLogs />) },
          { path: "/config", element: page(<ClientConfig />) },
          { path: "/settings", element: page(<SettingsPage />) },
          { path: "*", element: page(<NotFound />) },
        ],
      },
    ],
  },
]);

export default function App() {
  return <RouterProvider router={router} />;
}
