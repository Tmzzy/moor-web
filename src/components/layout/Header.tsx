import { Loader2, LogOut, Menu } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { useAuth } from "@/contexts/AuthContext";

interface HeaderProps {
  onOpenNavigation: () => void;
}

export function Header({ onOpenNavigation }: HeaderProps) {
  const { username, logout } = useAuth();
  const [loggingOut, setLoggingOut] = useState(false);

  const handleLogout = async () => {
    setLoggingOut(true);
    try {
      await logout();
    } finally {
      setLoggingOut(false);
    }
  };

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
