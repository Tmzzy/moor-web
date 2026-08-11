import { useRef, useState, type FormEvent } from "react";
import { Eye, EyeOff, Loader2, LogIn } from "lucide-react";
import { Navigate, useLocation, useNavigate } from "react-router-dom";

import { AuthLoading } from "@/components/auth/AuthLoading";
import { MoorLogo } from "@/components/icons/MoorLogo";
import { ErrorBanner } from "@/components/shared/ErrorBanner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useAuth } from "@/contexts/AuthContext";

interface LoginLocationState {
  from?: string;
}

export function safeDestination(state: unknown): string {
  const from = (state as LoginLocationState | null)?.from;
  return typeof from === "string" &&
    from.startsWith("/") &&
    !from.startsWith("//") &&
    from !== "/login"
    ? from
    : "/";
}

export function Login() {
  const { status, login } = useAuth();
  const location = useLocation();
  const navigate = useNavigate();
  const destination = safeDestination(location.state);
  const usernameRef = useRef<HTMLInputElement>(null);
  const passwordRef = useRef<HTMLInputElement>(null);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [touched, setTouched] = useState({ username: false, password: false });

  const usernameError =
    touched.username && username.trim().length === 0 ? "Enter your username." : null;
  const passwordError = touched.password && password.length === 0 ? "Enter your password." : null;

  if (status === "checking") return <AuthLoading />;
  if (status === "authenticated") return <Navigate to={destination} replace />;

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setTouched({ username: true, password: true });
    setFormError(null);

    const normalizedUsername = username.trim();
    if (!normalizedUsername) {
      usernameRef.current?.focus();
      return;
    }
    if (!password) {
      passwordRef.current?.focus();
      return;
    }

    setSubmitting(true);
    try {
      await login({ username: normalizedUsername, password });
      navigate(destination, { replace: true });
    } catch (error) {
      setFormError(error instanceof Error ? error.message : "Unable to sign in. Try again.");
      passwordRef.current?.focus();
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <main className="flex min-h-dvh items-center justify-center bg-cursor-cream px-4 py-10 sm:px-6">
      <div className="w-full max-w-[420px] animate-fade-in-up">
        <div className="mb-7 flex flex-col items-center text-center">
          <div className="mb-4 flex h-14 w-14 items-center justify-center rounded-lg bg-cursor-dark shadow-[0_12px_30px_rgba(0,0,0,0.12)]">
            <MoorLogo className="h-10 w-10 text-surface-200" />
          </div>
          <h1 className="font-headline text-3xl font-semibold text-cursor-dark">Moor</h1>
          <p className="mt-1 font-mono text-xs uppercase text-[var(--fg-45)]">
            MCP Gateway Manager
          </p>
        </div>

        <section className="rounded-lg border border-[var(--fg-10)] bg-surface-200 p-5 shadow-[0_18px_50px_rgba(0,0,0,0.07)] sm:p-7">
          <div className="mb-6">
            <h2 className="font-headline text-xl font-semibold text-cursor-dark">Sign in</h2>
            <p className="mt-1 font-body text-sm leading-relaxed text-[var(--fg-55)]">
              Access your Moor management workspace.
            </p>
          </div>

          <form className="space-y-5" onSubmit={handleSubmit} noValidate>
            <div className="space-y-2">
              <Label htmlFor="username" className="text-sm text-[var(--fg-65)]">
                Username
              </Label>
              <Input
                ref={usernameRef}
                id="username"
                name="username"
                type="text"
                autoComplete="username"
                autoCapitalize="none"
                autoCorrect="off"
                autoFocus
                value={username}
                disabled={submitting}
                aria-invalid={Boolean(usernameError)}
                aria-describedby={usernameError ? "username-error" : undefined}
                className="h-11 text-base focus-visible:ring-2 focus-visible:ring-[var(--fg-15)] sm:text-sm"
                onBlur={() => setTouched((current) => ({ ...current, username: true }))}
                onChange={(event) => setUsername(event.target.value)}
              />
              {usernameError ? (
                <p id="username-error" role="alert" className="font-body text-xs text-error-warm">
                  {usernameError}
                </p>
              ) : null}
            </div>

            <div className="space-y-2">
              <Label htmlFor="password" className="text-sm text-[var(--fg-65)]">
                Password
              </Label>
              <div className="relative">
                <Input
                  ref={passwordRef}
                  id="password"
                  name="password"
                  type={showPassword ? "text" : "password"}
                  autoComplete="current-password"
                  value={password}
                  disabled={submitting}
                  aria-invalid={Boolean(passwordError)}
                  aria-describedby={passwordError ? "password-error" : undefined}
                  className="h-11 pr-12 text-base focus-visible:ring-2 focus-visible:ring-[var(--fg-15)] sm:text-sm"
                  onBlur={() => setTouched((current) => ({ ...current, password: true }))}
                  onChange={(event) => setPassword(event.target.value)}
                />
                <button
                  type="button"
                  disabled={submitting}
                  aria-label={showPassword ? "Hide password" : "Show password"}
                  title={showPassword ? "Hide password" : "Show password"}
                  className="absolute inset-y-0 right-0 flex w-11 cursor-pointer items-center justify-center rounded-r-lg text-[var(--fg-45)] transition-colors hover:text-cursor-dark focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-[var(--fg-20)] disabled:cursor-not-allowed disabled:opacity-50"
                  onClick={() => setShowPassword((current) => !current)}
                >
                  {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                </button>
              </div>
              {passwordError ? (
                <p id="password-error" role="alert" className="font-body text-xs text-error-warm">
                  {passwordError}
                </p>
              ) : null}
            </div>

            {formError ? <ErrorBanner message={formError} /> : null}

            <Button
              type="submit"
              size="lg"
              disabled={submitting}
              className="w-full bg-cursor-dark text-surface-200 hover:border-cursor-dark hover:text-white"
            >
              {submitting ? (
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              ) : (
                <LogIn className="mr-2 h-4 w-4" />
              )}
              {submitting ? "Signing in..." : "Sign in"}
            </Button>
          </form>
        </section>

        <p className="mt-5 text-center font-mono text-[11px] text-[var(--fg-35)]">
          Moor v{__APP_VERSION__}
        </p>
      </div>
    </main>
  );
}
