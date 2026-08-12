// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

import { useCallback, useEffect, useState } from "react";
import { Cog, ExternalLink, Palette, Wrench } from "lucide-react";
import type { GeneralSettings, SettingsGroup } from "@moor/types";
import { cn, getErrorMessage } from "@/lib/utils";
import { PageHeader } from "@/components/shared/PageHeader";
import { ErrorBanner } from "@/components/shared/ErrorBanner";
import { useSettings } from "@/hooks/useSettings";
import { Switch } from "@/components/ui/switch";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Card, CardContent } from "@/components/ui/card";
import { Tabs } from "@/components/ui/tabs";
import { getSettingsPageLoadState, parseTimeoutSecondsInput } from "./settings-state";

declare const __APP_VERSION__: string;

interface SettingRowProps {
  label: string;
  description?: string;
  children: React.ReactNode;
}

function SettingRow({ label, description, children }: SettingRowProps) {
  return (
    <div className="flex items-center justify-between gap-4 px-4 py-3.5">
      <div className="min-w-0 flex-1">
        <p className="font-headline text-sm text-cursor-dark">{label}</p>
        {description && (
          <p className="mt-0.5 font-body text-xs text-[var(--fg-45)]">{description}</p>
        )}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

interface GroupNavItemProps {
  icon: React.ElementType;
  label: string;
  active: boolean;
  onClick: () => void;
}

function GroupNavItem({ icon: Icon, label, active, onClick }: GroupNavItemProps) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "flex w-full items-center gap-2.5 rounded-lg px-3 py-2 font-headline text-sm transition-all duration-200",
        active
          ? "bg-surface-400 font-medium text-cursor-dark"
          : "text-[var(--fg-55)] hover:bg-[var(--fg-06)] hover:text-cursor-dark",
      )}
    >
      <Icon className="h-4 w-4" />
      {label}
    </button>
  );
}

function GeneralSection({ onError }: { onError: (message: string | null) => void }) {
  const { settings, updateSettings } = useSettings();

  const handleSwitch = useCallback(
    async (key: keyof GeneralSettings, value: boolean) => {
      try {
        onError(null);
        await updateSettings({ general: { [key]: value } });
      } catch (error) {
        onError(getErrorMessage(error, "Failed to update server startup settings"));
      }
    },
    [onError, updateSettings],
  );

  return (
    <Card>
      <CardContent className="divide-y divide-[var(--fg-06)] p-2">
        <SettingRow
          label="Auto-start Servers"
          description="Start enabled auto-start servers when the Moor container starts"
        >
          <Switch
            checked={settings.general.autoStartServersOnLaunch}
            onCheckedChange={(value) => void handleSwitch("autoStartServersOnLaunch", value)}
          />
        </SettingRow>
      </CardContent>
    </Card>
  );
}

function AppearanceSection({ onError }: { onError: (message: string | null) => void }) {
  const { settings, updateSettings } = useSettings();

  const handleThemeChange = useCallback(
    async (value: string) => {
      try {
        onError(null);
        await updateSettings({ appearance: { theme: value as "light" | "dark" | "system" } });
      } catch (error) {
        onError(getErrorMessage(error, "Failed to update theme"));
      }
    },
    [onError, updateSettings],
  );

  return (
    <Card>
      <CardContent className="p-2">
        <SettingRow label="Theme" description="Choose the application appearance">
          <Tabs
            value={settings.appearance.theme}
            onValueChange={(value) => void handleThemeChange(value)}
            tabs={[
              { value: "light", label: "Light" },
              { value: "dark", label: "Dark" },
              { value: "system", label: "System" },
            ]}
          />
        </SettingRow>
      </CardContent>
    </Card>
  );
}

function AdvancedSection({ onError }: { onError: (message: string | null) => void }) {
  const { settings, updateSettings } = useSettings();
  const [localRetention, setLocalRetention] = useState(String(settings.advanced.logRetentionDays));
  const [localRequestTimeout, setLocalRequestTimeout] = useState(
    String(settings.advanced.mcpRequestTimeoutMs / 1000),
  );
  const [localStartTimeout, setLocalStartTimeout] = useState(
    String(settings.advanced.mcpServerStartTimeoutMs / 1000),
  );
  const requestTimeoutState = parseTimeoutSecondsInput(localRequestTimeout);
  const startTimeoutState = parseTimeoutSecondsInput(localStartTimeout);
  const requestTimeoutErrorId = "request-timeout-error";
  const startTimeoutErrorId = "server-start-timeout-error";

  useEffect(() => {
    setLocalRetention(String(settings.advanced.logRetentionDays));
    setLocalRequestTimeout(String(settings.advanced.mcpRequestTimeoutMs / 1000));
    setLocalStartTimeout(String(settings.advanced.mcpServerStartTimeoutMs / 1000));
  }, [
    settings.advanced.logRetentionDays,
    settings.advanced.mcpRequestTimeoutMs,
    settings.advanced.mcpServerStartTimeoutMs,
  ]);

  const applyRetention = async () => {
    const retention = Number(localRetention);
    if (!Number.isInteger(retention) || retention < 0 || retention > 365) {
      onError("Log retention must be a whole number between 0 and 365 days.");
      return;
    }
    try {
      onError(null);
      await updateSettings({ advanced: { logRetentionDays: retention } });
    } catch (error) {
      onError(getErrorMessage(error, "Failed to update log retention"));
    }
  };

  const updateAuditLogging = async (enabled: boolean) => {
    try {
      onError(null);
      await updateSettings({ advanced: { enableAuditLogging: enabled } });
    } catch (error) {
      onError(getErrorMessage(error, "Failed to update audit logging"));
    }
  };

  type TimeoutKey = "mcpRequestTimeoutMs" | "mcpServerStartTimeoutMs";

  const applyTimeout = async (
    key: TimeoutKey,
    parsed: typeof requestTimeoutState,
    label: string,
  ) => {
    try {
      onError(null);
      if (!parsed.valid) {
        onError(parsed.message);
        return;
      }
      await updateSettings({ advanced: { [key]: parsed.milliseconds } });
    } catch (error) {
      onError(getErrorMessage(error, `Failed to update ${label}`));
    }
  };

  return (
    <div className="space-y-4">
      <Card>
        <CardContent className="divide-y divide-[var(--fg-06)] p-2">
          <SettingRow
            label="Log Retention"
            description="Number of days to keep audit logs (0 for unlimited)"
          >
            <div className="flex items-center gap-2">
              <Input
                type="number"
                min={0}
                max={365}
                value={localRetention}
                onChange={(event) => setLocalRetention(event.target.value)}
                className="h-8 w-20 text-center text-xs"
              />
              <Button variant="secondary" size="sm" onClick={() => void applyRetention()}>
                Apply
              </Button>
            </div>
          </SettingRow>
          <SettingRow label="Audit Logging" description="Record tool calls in the audit log">
            <Switch
              checked={settings.advanced.enableAuditLogging}
              onCheckedChange={(value) => void updateAuditLogging(value)}
            />
          </SettingRow>
          <SettingRow
            label="Request Timeout"
            description="Timeout for MCP JSON-RPC requests in seconds (5-300)"
          >
            <div className="flex flex-col items-end gap-1">
              <div className="flex items-center gap-2">
                <Input
                  type="number"
                  min={5}
                  max={300}
                  step={1}
                  value={localRequestTimeout}
                  aria-invalid={!requestTimeoutState.valid}
                  aria-describedby={requestTimeoutState.valid ? undefined : requestTimeoutErrorId}
                  onChange={(event) => setLocalRequestTimeout(event.target.value)}
                  className="h-8 w-20 text-center text-xs"
                />
                <Button
                  variant="secondary"
                  size="sm"
                  disabled={!requestTimeoutState.valid}
                  onClick={() =>
                    void applyTimeout("mcpRequestTimeoutMs", requestTimeoutState, "request timeout")
                  }
                >
                  Apply
                </Button>
              </div>
              {!requestTimeoutState.valid && (
                <p id={requestTimeoutErrorId} className="font-body text-[11px] text-error-warm">
                  {requestTimeoutState.message}
                </p>
              )}
            </div>
          </SettingRow>
          <SettingRow
            label="Server Start Timeout"
            description="Startup wait for MCP servers in seconds (5-300)"
          >
            <div className="flex flex-col items-end gap-1">
              <div className="flex items-center gap-2">
                <Input
                  type="number"
                  min={5}
                  max={300}
                  step={1}
                  value={localStartTimeout}
                  aria-invalid={!startTimeoutState.valid}
                  aria-describedby={startTimeoutState.valid ? undefined : startTimeoutErrorId}
                  onChange={(event) => setLocalStartTimeout(event.target.value)}
                  className="h-8 w-20 text-center text-xs"
                />
                <Button
                  variant="secondary"
                  size="sm"
                  disabled={!startTimeoutState.valid}
                  onClick={() =>
                    void applyTimeout(
                      "mcpServerStartTimeoutMs",
                      startTimeoutState,
                      "server start timeout",
                    )
                  }
                >
                  Apply
                </Button>
              </div>
              {!startTimeoutState.valid && (
                <p id={startTimeoutErrorId} className="font-body text-[11px] text-error-warm">
                  {startTimeoutState.message}
                </p>
              )}
            </div>
          </SettingRow>
        </CardContent>
      </Card>

      <Card>
        <CardContent className="flex items-center justify-between gap-4 p-4">
          <div>
            <p className="font-headline text-sm text-cursor-dark">Moor v{__APP_VERSION__}</p>
            <p className="font-body text-xs text-[var(--fg-40)]">
              Apache-2.0, based on the original Moor by varandrew
            </p>
          </div>
          <div className="flex items-center gap-2">
            <a
              href="https://github.com/Tmzzy/moor-web"
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-1.5 rounded-md px-2 py-1.5 font-headline text-xs text-[var(--fg-55)] hover:bg-[var(--fg-06)] hover:text-cursor-dark"
            >
              Source <ExternalLink className="h-3.5 w-3.5" />
            </a>
            <a
              href="https://github.com/varandrew/moor"
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-1.5 rounded-md px-2 py-1.5 font-headline text-xs text-[var(--fg-55)] hover:bg-[var(--fg-06)] hover:text-cursor-dark"
            >
              Upstream <ExternalLink className="h-3.5 w-3.5" />
            </a>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

const groups: { key: SettingsGroup; label: string; icon: React.ElementType }[] = [
  { key: "general", label: "General", icon: Cog },
  { key: "appearance", label: "Appearance", icon: Palette },
  { key: "advanced", label: "Advanced", icon: Wrench },
];

export function SettingsPage() {
  const [activeGroup, setActiveGroup] = useState<SettingsGroup>("general");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const { isLoading, isError, error, resetSettings } = useSettings();
  const loadState = getSettingsPageLoadState({ isLoading, isError, error });

  const handleReset = async () => {
    if (!window.confirm("Reset all settings to their default values?")) return;
    try {
      setErrorMessage(null);
      await resetSettings();
    } catch (resetError) {
      setErrorMessage(getErrorMessage(resetError, "Failed to reset settings"));
    }
  };

  if (loadState.kind === "loading") {
    return (
      <div className="flex h-64 items-center justify-center">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-surface-300 border-t-cursor-orange" />
      </div>
    );
  }

  return (
    <div className="space-y-6 animate-fade-in-up">
      <PageHeader
        title="Settings"
        subtitle="Configure Moor to your preferences"
        action={
          loadState.canRenderControls ? (
            <Button variant="outline" size="sm" onClick={() => void handleReset()}>
              Reset to Defaults
            </Button>
          ) : undefined
        }
      />

      {loadState.kind === "error" && <ErrorBanner message={loadState.message} />}
      {errorMessage && <ErrorBanner message={errorMessage} />}

      {loadState.canRenderControls && (
        <div className="flex flex-col gap-4 md:flex-row md:gap-6">
          <nav className="grid shrink-0 grid-cols-3 gap-1 md:w-44 md:self-start md:grid-cols-1">
            {groups.map(({ key, label, icon }) => (
              <GroupNavItem
                key={key}
                icon={icon}
                label={label}
                active={activeGroup === key}
                onClick={() => setActiveGroup(key)}
              />
            ))}
          </nav>

          <div className="min-w-0 flex-1">
            {activeGroup === "general" && <GeneralSection onError={setErrorMessage} />}
            {activeGroup === "appearance" && <AppearanceSection onError={setErrorMessage} />}
            {activeGroup === "advanced" && <AdvancedSection onError={setErrorMessage} />}
          </div>
        </div>
      )}
    </div>
  );
}
