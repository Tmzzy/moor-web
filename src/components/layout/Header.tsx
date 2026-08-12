import { useProfiles } from "@/hooks/useProfiles";
import { ChevronDown, Check, Loader2, LogOut, Menu } from "lucide-react";
import { useState, useRef, useEffect } from "react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { useAuth } from "@/contexts/AuthContext";

interface HeaderProps {
  onOpenNavigation: () => void;
}

export function Header({ onOpenNavigation }: HeaderProps) {
  const { profiles, activateProfile } = useProfiles();
  const { username, logout } = useAuth();
  const [open, setOpen] = useState(false);
  const [loggingOut, setLoggingOut] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  const activeProfile = profiles.find((p) => p.isActive);

  const handleLogout = async () => {
    setLoggingOut(true);
    try {
      await logout();
    } finally {
      setLoggingOut(false);
    }
  };

  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, []);

  return (
    <header className="h-14 border-b border-[var(--fg-08)] bg-surface-200/80 backdrop-blur-md flex items-center justify-between px-2 sm:px-4 md:px-6 sticky top-0 z-40">
      <div className="flex min-w-0 items-center gap-2">
        <Button
          variant="ghost"
          size="icon"
          className="shrink-0 md:hidden"
          aria-label="Open navigation"
          onClick={onOpenNavigation}
        >
          <Menu className="h-5 w-5" />
        </Button>
        <span className="hidden font-body text-sm text-[var(--fg-40)] sm:inline">
          Active Profile
        </span>
        <div className="relative" ref={ref}>
          <button
            onClick={() => setOpen(!open)}
            className={cn(
              "flex max-w-36 items-center gap-2 rounded-xl px-3 py-1.5 font-headline text-sm transition-all duration-200 sm:max-w-48",
              open
                ? "bg-surface-400 text-cursor-dark shadow-sm"
                : "bg-surface-300 hover:bg-surface-400 text-cursor-dark",
            )}
          >
            <span className="relative flex h-2 w-2">
              <span className="animate-ping-slow absolute inline-flex h-full w-full rounded-full bg-success-muted opacity-60" />
              <span className="relative inline-flex rounded-full h-2 w-2 bg-success-muted" />
            </span>
            <span className="truncate">{activeProfile?.name || "None"}</span>
            <ChevronDown
              className={cn(
                "h-3.5 w-3.5 text-[var(--fg-35)] transition-transform duration-200",
                open && "rotate-180",
              )}
            />
          </button>
          {open && (
            <div className="absolute top-full mt-1.5 left-0 z-50 w-52 rounded-xl border border-[var(--fg-10)] bg-surface-200 shadow-[rgba(0,0,0,0.14)_0px_28px_70px,rgba(0,0,0,0.1)_0px_14px_32px,oklab(0.263084_-0.00230259_0.0124794_/_0.1)_0px_0px_0px_1px] py-1.5 animate-scale-in origin-top-left">
              {profiles.map((profile) => (
                <button
                  key={profile.id}
                  onClick={() => {
                    activateProfile(profile.id);
                    setOpen(false);
                  }}
                  className={cn(
                    "w-full text-left px-3 py-2 text-sm font-headline transition-colors duration-150 flex items-center gap-2.5 rounded-lg mx-1",
                    profile.isActive
                      ? "text-cursor-dark font-medium bg-surface-300"
                      : "text-[var(--fg-55)] hover:bg-surface-300/60 hover:text-cursor-dark",
                  )}
                >
                  {profile.isActive ? (
                    <Check className="h-3.5 w-3.5 text-success-muted" />
                  ) : (
                    <span className="h-3.5 w-3.5" />
                  )}
                  {profile.name}
                </button>
              ))}
            </div>
          )}
        </div>
      </div>
      <div className="ml-2 flex min-w-0 items-center gap-1 border-l border-[var(--fg-08)] pl-2 sm:ml-4 sm:gap-2 sm:pl-4">
        <span
          className="hidden max-w-36 truncate font-headline text-xs text-[var(--fg-45)] sm:block"
          title={username ?? undefined}
        >
          {username}
        </span>
        <Button
          variant="ghost"
          size="icon"
          type="button"
          disabled={loggingOut}
          aria-label="Sign out"
          onClick={() => void handleLogout()}
        >
          {loggingOut ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <LogOut className="h-4 w-4" />
          )}
        </Button>
      </div>
    </header>
  );
}
