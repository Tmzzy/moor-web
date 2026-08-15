import { useMemo, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { X } from "lucide-react";
import { ErrorBanner } from "@/components/shared/ErrorBanner";
import { Switch } from "@/components/ui/switch";
import { SegmentedControl } from "@/components/ui/segmented-control";
import { StdioConfigFields } from "@/components/servers/StdioConfigFields";
import { ProfileSelector } from "@/components/servers/ProfileSelector";
import { KeyValueEditor } from "@/components/shared/KeyValueEditor";
import { UnsavedChangesDialog } from "@/components/shared/UnsavedChangesDialog";
import {
  findDuplicateHeaderKeys,
  findDuplicateKeys,
  formToCreateInput,
  getEffectiveStdioCommand,
  type StdioLauncher,
} from "@/lib/server-form";
import type { ConnectionType, ServerCreateInput } from "@moor/types";
import { useProfiles } from "@/hooks/useProfiles";

const CONNECTION_TYPES = [
  { value: "stdio", label: "stdio" },
  { value: "http", label: "HTTP" },
] as const;

function createInitialForm() {
  return {
    name: "",
    connectionType: "stdio" as ConnectionType,
    launcher: "command" as StdioLauncher,
    command: "",
    args: "",
    workingDir: "",
    url: "",
    env: [] as Array<[string, string]>,
    headers: [] as Array<[string, string]>,
    autoStart: false,
  };
}

interface AddServerFormProps {
  onAdd: (config: ServerCreateInput) => Promise<void>;
  onClose: () => void;
}

export function AddServerForm({ onAdd, onClose }: AddServerFormProps) {
  const { profiles } = useProfiles();
  const [form, setForm] = useState(createInitialForm);
  const [profileSelection, setProfileSelection] = useState<string[]>([]);
  const [submitting, setSubmitting] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [discardOpen, setDiscardOpen] = useState(false);
  const baselineRef = useRef(createInitialForm());
  const profileSelectionDirty = profileSelection.length > 0;
  const dirty = useMemo(
    () => JSON.stringify(form) !== JSON.stringify(baselineRef.current) || profileSelectionDirty,
    [form, profileSelectionDirty],
  );
  const effectiveCommand = getEffectiveStdioCommand(form.launcher, form.command);
  const canSubmit =
    Boolean(form.name.trim()) &&
    profileSelection.length > 0 &&
    (form.connectionType === "stdio" ? Boolean(effectiveCommand) : Boolean(form.url.trim()));

  const requestClose = () => {
    if (dirty) {
      setDiscardOpen(true);
      return;
    }
    onClose();
  };

  const handleSubmit = async () => {
    if (!form.name.trim() || submitting) return;
    setFormError(null);

    if (form.connectionType === "stdio" && !effectiveCommand) {
      setFormError("Command is required.");
      return;
    }
    if (form.connectionType === "http" && !form.url.trim()) {
      setFormError("URL is required.");
      return;
    }
    if (profileSelection.length === 0) {
      setFormError("Select at least one profile.");
      return;
    }

    if (findDuplicateKeys(form.env).size > 0) {
      setFormError("Environment variable keys must be unique.");
      return;
    }
    if (form.connectionType === "http" && findDuplicateHeaderKeys(form.headers).size > 0) {
      setFormError("HTTP header keys must be unique.");
      return;
    }

    setSubmitting(true);
    try {
      await onAdd({ ...formToCreateInput(form), profileIds: profileSelection });
      onClose();
    } catch (err) {
      setFormError(err instanceof Error ? err.message : "Failed to add server");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Card className="animate-scale-in border-cursor-orange/20 shadow-[0_8px_30px_rgba(245,78,0,0.04)]">
      <UnsavedChangesDialog
        open={discardOpen}
        onCancel={() => setDiscardOpen(false)}
        onConfirm={onClose}
      />
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between">
          <CardTitle className="text-base">Add New Server</CardTitle>
          <Button
            variant="ghost"
            size="icon"
            className="text-[var(--fg-65)] hover:text-cursor-dark hover:bg-[var(--fg-08)]"
            onClick={requestClose}
            aria-label="Close add server form"
          >
            <X className="h-5 w-5" />
          </Button>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="grid gap-4 sm:grid-cols-2">
          <div className="space-y-1.5">
            <Label htmlFor="add-server-name">Name</Label>
            <Input
              id="add-server-name"
              placeholder="e.g., github"
              value={form.name}
              onChange={(e) => setForm((f) => ({ ...f, name: e.target.value }))}
            />
          </div>
          <SegmentedControl
            label="Transport"
            value={form.connectionType}
            options={CONNECTION_TYPES}
            onValueChange={(connectionType) =>
              setForm((current) => ({ ...current, connectionType }))
            }
          />
        </div>
        {form.connectionType === "stdio" ? (
          <StdioConfigFields
            idPrefix="add-server"
            launcher={form.launcher}
            command={form.command}
            args={form.args}
            workingDir={form.workingDir}
            onChange={(updates) => setForm((current) => ({ ...current, ...updates }))}
          />
        ) : (
          <>
            <div className="space-y-1.5">
              <Label htmlFor="add-server-url">URL</Label>
              <Input
                id="add-server-url"
                placeholder="e.g., http://localhost:3000/mcp"
                value={form.url}
                onChange={(e) => setForm((f) => ({ ...f, url: e.target.value }))}
              />
            </div>
            <div className="space-y-1.5">
              <Label>HTTP Headers</Label>
              <KeyValueEditor
                entries={form.headers}
                onChange={(headers) => setForm((f) => ({ ...f, headers }))}
                duplicateKeyFinder={findDuplicateHeaderKeys}
                keyLabel="Header"
                keyPlaceholder="Authorization"
                valuePlaceholder="Bearer {env:MCP_TOKEN}"
              />
            </div>
          </>
        )}
        <ProfileSelector
          profiles={profiles}
          selectedIds={profileSelection}
          onChange={setProfileSelection}
        />
        <div className="flex items-center justify-between py-2">
          <div className="space-y-0.5">
            <Label>Auto Start</Label>
            <p className="text-[11px] text-[var(--fg-40)]">
              Automatically start this server when Moor launches
            </p>
          </div>
          <Switch
            aria-label="Auto start server"
            checked={form.autoStart}
            onCheckedChange={(v) => setForm((f) => ({ ...f, autoStart: v }))}
          />
        </div>
        <div className="space-y-1.5">
          <Label>Environment Variables</Label>
          <KeyValueEditor
            entries={form.env}
            onChange={(env) => setForm((f) => ({ ...f, env }))}
            keyLabel="Variable"
            keyPlaceholder="API_KEY"
            valuePlaceholder="your-api-key"
          />
        </div>
        {formError && <ErrorBanner message={formError} />}
        <div className="flex justify-end gap-2 pt-1">
          <Button variant="outline" onClick={requestClose}>
            Cancel
          </Button>
          <Button onClick={handleSubmit} disabled={!canSubmit || submitting}>
            {submitting ? "Adding..." : "Add Server"}
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
