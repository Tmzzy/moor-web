import { useState } from "react";
import { Copy, Eye, EyeOff, KeyRound, Loader2, RefreshCw } from "lucide-react";
import { toast } from "sonner";
import type { McpTokenResponse } from "@moor/types";

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ErrorBanner } from "@/components/shared/ErrorBanner";
import { api } from "@/lib/api/client";
import { routes } from "@/lib/api-routes";

type BusyAction = "reveal" | "copy" | "rotate" | null;

export function McpTokenPanel() {
  const [token, setToken] = useState<string | null>(null);
  const [visible, setVisible] = useState(false);
  const [busy, setBusy] = useState<BusyAction>(null);
  const [error, setError] = useState<string | null>(null);
  const [rotateOpen, setRotateOpen] = useState(false);

  const loadToken = async (): Promise<string> => {
    if (token) return token;
    const response = await api<McpTokenResponse>(routes.security.mcpToken());
    setToken(response.token);
    return response.token;
  };

  const handleReveal = async () => {
    if (visible) {
      setVisible(false);
      return;
    }
    setBusy("reveal");
    setError(null);
    try {
      await loadToken();
      setVisible(true);
    } catch (requestError) {
      setError(requestError instanceof Error ? requestError.message : "Unable to load the token.");
    } finally {
      setBusy(null);
    }
  };

  const handleCopy = async () => {
    setBusy("copy");
    setError(null);
    try {
      const currentToken = await loadToken();
      await navigator.clipboard.writeText(currentToken);
      toast.success("MCP token copied");
    } catch (requestError) {
      setError(requestError instanceof Error ? requestError.message : "Unable to copy the token.");
    } finally {
      setBusy(null);
    }
  };

  const handleRotate = async () => {
    setBusy("rotate");
    setError(null);
    try {
      const response = await api<McpTokenResponse>(routes.security.rotateMcpToken(), {
        method: "POST",
      });
      setToken(response.token);
      setVisible(true);
      setRotateOpen(false);
      toast.success("MCP token rotated", {
        description: "Update connected clients before their next request.",
      });
    } catch (requestError) {
      setError(
        requestError instanceof Error ? requestError.message : "Unable to rotate the token.",
      );
      setRotateOpen(false);
    } finally {
      setBusy(null);
    }
  };

  return (
    <>
      <section className="rounded-lg border border-[var(--fg-10)] bg-surface-200 p-5">
        <div className="flex flex-col gap-5 lg:flex-row lg:items-end lg:justify-between">
          <div className="min-w-0 flex-1">
            <div className="mb-3 flex items-center gap-3">
              <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-[var(--fg-08)] bg-surface-300">
                <KeyRound className="h-4 w-4 text-cursor-orange" />
              </div>
              <div className="min-w-0">
                <h2 className="font-headline text-sm font-semibold text-cursor-dark">
                  MCP Access Token
                </h2>
                <p className="font-body text-xs text-[var(--fg-50)]">
                  Used by clients that connect to the public MCP endpoint.
                </p>
              </div>
            </div>

            <div className="flex max-w-2xl items-center gap-2">
              <Input
                readOnly
                type={visible ? "text" : "password"}
                value={token ?? ""}
                placeholder="Token hidden until requested"
                aria-label="MCP access token"
                className="h-11 min-w-0 font-mono text-xs"
              />
              <Button
                type="button"
                variant="outline"
                size="icon"
                disabled={busy !== null}
                aria-label={visible ? "Hide MCP token" : "Show MCP token"}
                title={visible ? "Hide token" : "Show token"}
                onClick={() => void handleReveal()}
              >
                {busy === "reveal" ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : visible ? (
                  <EyeOff className="h-4 w-4" />
                ) : (
                  <Eye className="h-4 w-4" />
                )}
              </Button>
              <Button
                type="button"
                variant="outline"
                size="icon"
                disabled={busy !== null}
                aria-label="Copy MCP token"
                title="Copy token"
                onClick={() => void handleCopy()}
              >
                {busy === "copy" ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Copy className="h-4 w-4" />
                )}
              </Button>
            </div>
          </div>

          <Button
            type="button"
            variant="outline"
            disabled={busy !== null}
            className="w-full shrink-0 lg:w-auto"
            onClick={() => setRotateOpen(true)}
          >
            <RefreshCw className="mr-2 h-4 w-4" />
            Rotate token
          </Button>
        </div>

        {error ? <ErrorBanner message={error} className="mt-4 max-w-2xl" /> : null}
      </section>

      <AlertDialog open={rotateOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Rotate MCP access token?</AlertDialogTitle>
            <AlertDialogDescription>
              The current token will stop working immediately. Every connected client must be
              updated with the new value.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={busy === "rotate"} onClick={() => setRotateOpen(false)}>
              Cancel
            </AlertDialogCancel>
            <AlertDialogAction disabled={busy === "rotate"} onClick={() => void handleRotate()}>
              {busy === "rotate" ? <Loader2 className="mr-2 h-4 w-4 animate-spin" /> : null}
              Rotate token
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
