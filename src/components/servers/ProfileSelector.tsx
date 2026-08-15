import { useId } from "react";

import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { cn } from "@/lib/utils";
import type { Profile } from "@moor/types";

interface ProfileSelectorProps {
  profiles: Profile[];
  selectedIds: string[];
  onChange: (profileIds: string[]) => void;
  disabled?: boolean;
  className?: string;
}

export function ProfileSelector({
  profiles,
  selectedIds,
  onChange,
  disabled = false,
  className,
}: ProfileSelectorProps) {
  const labelId = useId();
  const selected = new Set(selectedIds);
  const allSelected = profiles.length > 0 && profiles.every((profile) => selected.has(profile.id));
  const someSelected = profiles.some((profile) => selected.has(profile.id));

  const toggleAll = (checked: boolean) => {
    onChange(checked ? profiles.map((profile) => profile.id) : []);
  };

  const toggleProfile = (profileId: string, checked: boolean) => {
    const next = new Set(selected);
    if (checked) {
      next.add(profileId);
    } else {
      next.delete(profileId);
    }
    onChange(profiles.filter((profile) => next.has(profile.id)).map((profile) => profile.id));
  };

  return (
    <fieldset disabled={disabled} aria-labelledby={labelId} className={cn("space-y-2", className)}>
      <div className="flex items-center justify-between gap-3">
        <Label id={labelId} asChild>
          <legend>Profiles</legend>
        </Label>
        <span className="font-body text-xs text-[var(--fg-50)]">
          {selected.size} / {profiles.length}
        </span>
      </div>

      {profiles.length === 0 ? (
        <p className="rounded-lg border border-dashed border-[var(--fg-12)] px-3 py-3 font-body text-sm text-[var(--fg-50)]">
          No profiles available
        </p>
      ) : (
        <div className="overflow-hidden rounded-lg border border-[var(--fg-10)] bg-surface-100">
          <label className="flex min-h-11 cursor-pointer items-center gap-3 border-b border-[var(--fg-08)] px-3 py-2 transition-colors duration-150 hover:bg-surface-300/60 focus-within:bg-surface-300/60">
            <Checkbox
              checked={allSelected ? true : someSelected ? "indeterminate" : false}
              onCheckedChange={(checked) => toggleAll(checked === true)}
              aria-label="Select all current profiles"
            />
            <span className="font-headline text-sm font-medium text-cursor-dark">
              All current profiles
            </span>
          </label>
          <div className="grid sm:grid-cols-2">
            {profiles.map((profile, index) => (
              <label
                key={profile.id}
                className={cn(
                  "flex min-h-11 cursor-pointer items-center gap-3 px-3 py-2 transition-colors duration-150",
                  "hover:bg-surface-300/60 focus-within:bg-surface-300/60",
                  index > 0 && "border-t border-[var(--fg-08)]",
                  index % 2 === 1 && "sm:border-l sm:border-[var(--fg-08)]",
                  index === 1 && "sm:border-t-0",
                )}
              >
                <Checkbox
                  checked={selected.has(profile.id)}
                  onCheckedChange={(checked) => toggleProfile(profile.id, checked === true)}
                  aria-label={`Use server in ${profile.name}`}
                />
                <span className="min-w-0 flex-1 truncate font-body text-sm text-cursor-dark">
                  {profile.name}
                </span>
              </label>
            ))}
          </div>
        </div>
      )}
    </fieldset>
  );
}
