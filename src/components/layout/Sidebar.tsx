import { NavLink } from "react-router-dom";
import { cn } from "@/lib/utils";
import { MoorLogo } from "@/components/icons/MoorLogo";
import {
  LayoutDashboard,
  Server,
  FolderOpen,
  FileText,
  Braces,
  HelpCircle,
  Cog,
  X,
} from "lucide-react";
import { useEffect } from "react";
import { Button } from "@/components/ui/button";

const navItems = [
  { to: "/", label: "Dashboard", icon: LayoutDashboard },
  { to: "/servers", label: "Servers", icon: Server },
  { to: "/profiles", label: "Profiles", icon: FolderOpen },
  { to: "/logs", label: "Audit Logs", icon: FileText },
  { to: "/config", label: "Client Config", icon: Braces },
];

interface SidebarContentProps {
  onNavigate?: () => void;
  showClose?: boolean;
}

function SidebarContent({ onNavigate, showClose = false }: SidebarContentProps) {
  return (
    <>
      <div className="flex items-center gap-2.5 px-5 py-4">
        <div className="h-9 w-9 rounded-xl bg-cursor-dark flex items-center justify-center">
          <MoorLogo className="h-7 w-7 text-surface-200" />
        </div>
        <div className="-space-y-0.5">
          <span className="font-headline text-lg font-semibold tracking-tight text-cursor-dark leading-tight block">
            Moor
          </span>
          <span className="font-mono text-[10px] text-[var(--fg-40)] tracking-wider uppercase leading-none">
            MCP Manager
          </span>
        </div>
        {showClose ? (
          <Button
            variant="ghost"
            size="icon"
            className="ml-auto"
            aria-label="Close navigation"
            onClick={onNavigate}
          >
            <X className="h-5 w-5" />
          </Button>
        ) : null}
      </div>

      <nav className="flex-1 px-3 py-2 space-y-0.5">
        {navItems.map(({ to, label, icon: Icon }) => (
          <NavLink
            key={to}
            to={to}
            onClick={onNavigate}
            className={({ isActive }) =>
              cn(
                "flex items-center gap-3 px-3 py-2.5 rounded-xl font-headline text-sm transition-all duration-200 relative group",
                isActive
                  ? "bg-surface-400 text-cursor-dark font-medium"
                  : "text-[var(--fg-55)] hover:bg-[var(--fg-06)] hover:text-cursor-dark",
              )
            }
          >
            {({ isActive }) => (
              <>
                <span
                  className={cn(
                    "absolute left-0 top-1/2 -translate-y-1/2 w-[3px] h-5 rounded-r-full bg-cursor-orange transition-all duration-200",
                    isActive ? "opacity-100" : "opacity-0",
                  )}
                />
                <Icon
                  className={cn("h-4 w-4 transition-colors", isActive && "text-cursor-orange")}
                />
                {label}
              </>
            )}
          </NavLink>
        ))}
      </nav>

      <div className="p-3 mt-auto">
        <a
          href="https://github.com/modelcontextprotocol"
          target="_blank"
          rel="noopener noreferrer"
          className="flex items-center gap-3 px-3 py-2.5 rounded-xl font-headline text-sm text-[var(--fg-45)] hover:bg-[var(--fg-06)] hover:text-cursor-dark transition-all duration-200"
        >
          <HelpCircle className="h-4 w-4" />
          Documentation
        </a>

        <div className="my-2 border-t border-[var(--fg-08)]" />

        <NavLink
          to="/settings"
          onClick={onNavigate}
          className={({ isActive }) =>
            cn(
              "flex items-center gap-3 px-3 py-2.5 rounded-xl font-headline text-sm transition-all duration-200 relative group",
              isActive
                ? "bg-surface-400 text-cursor-dark font-medium"
                : "text-[var(--fg-45)] hover:bg-[var(--fg-06)] hover:text-cursor-dark",
            )
          }
        >
          {({ isActive }) => (
            <>
              <span
                className={cn(
                  "absolute left-0 top-1/2 -translate-y-1/2 w-[3px] h-5 rounded-r-full bg-cursor-orange transition-all duration-200",
                  isActive ? "opacity-100" : "opacity-0",
                )}
              />
              <Cog className={cn("h-4 w-4 transition-colors", isActive && "text-cursor-orange")} />
              Settings
            </>
          )}
        </NavLink>
      </div>

      <div className="px-5 py-3 border-t border-[var(--fg-08)]">
        <p className="font-mono text-[10px] text-[var(--fg-30)]">{`Moor v${__APP_VERSION__}`}</p>
      </div>
    </>
  );
}

interface SidebarProps {
  mobileOpen: boolean;
  onCloseMobile: () => void;
}

export function Sidebar({ mobileOpen, onCloseMobile }: SidebarProps) {
  useEffect(() => {
    if (!mobileOpen) return;
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") onCloseMobile();
    };
    document.addEventListener("keydown", closeOnEscape);
    return () => document.removeEventListener("keydown", closeOnEscape);
  }, [mobileOpen, onCloseMobile]);

  return (
    <>
      <aside className="hidden w-[220px] shrink-0 flex-col border-r border-[var(--fg-10)] bg-surface-300 md:flex">
        <SidebarContent />
      </aside>
      {mobileOpen ? (
        <div className="fixed inset-0 z-50 md:hidden">
          <button
            type="button"
            aria-label="Close navigation"
            className="absolute inset-0 bg-black/35"
            onClick={onCloseMobile}
          />
          <aside
            role="dialog"
            aria-modal="true"
            aria-label="Main navigation"
            className="relative flex h-full w-[min(280px,85vw)] flex-col border-r border-[var(--fg-10)] bg-surface-300 shadow-[0_20px_60px_rgba(0,0,0,0.2)]"
          >
            <SidebarContent onNavigate={onCloseMobile} showClose />
          </aside>
        </div>
      ) : null}
    </>
  );
}
