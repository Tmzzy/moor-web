import { Outlet } from "react-router-dom";
import { Sidebar } from "./Sidebar";
import { Header } from "./Header";
import { useSettings } from "@/hooks/useSettings";
import { useTheme } from "@/hooks/useTheme";
import { useState } from "react";

export function AppShell() {
  const { settings, isError, isFetched } = useSettings();
  const [mobileNavigationOpen, setMobileNavigationOpen] = useState(false);
  useTheme(isFetched && !isError ? settings.appearance.theme : null);

  return (
    <div className="flex h-screen bg-cursor-cream overflow-hidden">
      <Sidebar
        mobileOpen={mobileNavigationOpen}
        onCloseMobile={() => setMobileNavigationOpen(false)}
      />
      <div className="flex-1 flex flex-col min-w-0">
        <Header onOpenNavigation={() => setMobileNavigationOpen(true)} />
        <main className="flex-1 overflow-auto p-4 md:p-6">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
