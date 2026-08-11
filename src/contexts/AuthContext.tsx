import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import type { AuthSession, LoginInput } from "@moor/types";

import { api, apiPost } from "@/lib/api/client";
import { routes } from "@/lib/api-routes";
import { queryClient } from "@/lib/query-client";
import { subscribeAuthenticationRequired } from "@/lib/auth-events";

type AuthStatus = "checking" | "authenticated" | "unauthenticated";

interface AuthContextValue {
  status: AuthStatus;
  username: string | null;
  login: (credentials: LoginInput) => Promise<void>;
  logout: () => Promise<void>;
}

const AuthContext = createContext<AuthContextValue | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [status, setStatus] = useState<AuthStatus>("checking");
  const [username, setUsername] = useState<string | null>(null);

  const clearAuthentication = useCallback(() => {
    queryClient.clear();
    setUsername(null);
    setStatus("unauthenticated");
  }, []);

  useEffect(() => subscribeAuthenticationRequired(clearAuthentication), [clearAuthentication]);

  useEffect(() => {
    const controller = new AbortController();
    void api<AuthSession>(routes.auth.session(), { signal: controller.signal })
      .then((session) => {
        if (session.authenticated && session.username) {
          setUsername(session.username);
          setStatus("authenticated");
        } else {
          clearAuthentication();
        }
      })
      .catch((error: unknown) => {
        if (error instanceof DOMException && error.name === "AbortError") return;
        clearAuthentication();
      });
    return () => controller.abort();
  }, [clearAuthentication]);

  const login = useCallback(async (credentials: LoginInput) => {
    const session = await apiPost<AuthSession>(routes.auth.login(), credentials);
    if (!session.authenticated || !session.username) {
      throw new Error("The server did not create an authenticated session.");
    }
    setUsername(session.username);
    setStatus("authenticated");
  }, []);

  const logout = useCallback(async () => {
    try {
      await api<void>(routes.auth.logout(), { method: "POST" });
    } catch (error) {
      console.warn("Server logout failed; cleared the local session state.", error);
    } finally {
      clearAuthentication();
    }
  }, [clearAuthentication]);

  const value = useMemo(
    () => ({ status, username, login, logout }),
    [login, logout, status, username],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth(): AuthContextValue {
  const context = useContext(AuthContext);
  if (!context) throw new Error("useAuth must be used within AuthProvider");
  return context;
}
