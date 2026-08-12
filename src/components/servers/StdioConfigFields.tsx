import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { SegmentedControl } from "@/components/ui/segmented-control";
import { Textarea } from "@/components/ui/textarea";
import { getStdioLauncherUpdates, type StdioLauncher } from "@/lib/server-form";

const LAUNCHERS = [
  { value: "command", label: "Command" },
  { value: "npx", label: "npx" },
  { value: "uvx", label: "uvx" },
] as const;

export interface StdioFieldsValue {
  launcher: StdioLauncher;
  command: string;
  args: string;
  workingDir: string;
}

interface StdioConfigFieldsProps extends StdioFieldsValue {
  idPrefix: string;
  onChange: (updates: Partial<StdioFieldsValue>) => void;
}

function argsPlaceholder(launcher: StdioLauncher): string {
  if (launcher === "npx") {
    return "@modelcontextprotocol/server-filesystem\n/data";
  }
  if (launcher === "uvx") return "--with\nmcp<2\nmcp-server-fetch";
  return "--stdio";
}

export function StdioConfigFields({
  idPrefix,
  launcher,
  command,
  args,
  workingDir,
  onChange,
}: StdioConfigFieldsProps) {
  const commandId = `${idPrefix}-command`;
  const argsId = `${idPrefix}-args`;
  const workingDirId = `${idPrefix}-working-dir`;

  const changeLauncher = (nextLauncher: StdioLauncher) => {
    onChange(getStdioLauncherUpdates(nextLauncher, args));
  };

  return (
    <>
      <SegmentedControl
        label="Launcher"
        value={launcher}
        options={LAUNCHERS}
        onValueChange={changeLauncher}
      />
      {launcher === "command" ? (
        <div className="space-y-1.5">
          <Label htmlFor={commandId}>Command</Label>
          <Input
            id={commandId}
            placeholder="e.g., node"
            value={command}
            onChange={(event) => onChange({ command: event.target.value })}
          />
        </div>
      ) : null}
      <div className="space-y-1.5">
        <Label htmlFor={argsId}>Arguments (one per line)</Label>
        <Textarea
          id={argsId}
          placeholder={argsPlaceholder(launcher)}
          value={args}
          onChange={(event) => onChange({ args: event.target.value })}
          className="min-h-[80px] font-mono text-xs"
        />
      </div>
      <div className="space-y-1.5">
        <Label htmlFor={workingDirId}>Working Directory</Label>
        <Input
          id={workingDirId}
          placeholder="e.g., /data/project"
          value={workingDir}
          onChange={(event) => onChange({ workingDir: event.target.value })}
        />
      </div>
    </>
  );
}
