import { useId } from "react";
import { cn } from "@/lib/utils";

interface SegmentedOption<T extends string> {
  value: T;
  label: string;
}

interface SegmentedControlProps<T extends string> {
  label: string;
  value: T;
  options: readonly SegmentedOption<T>[];
  onValueChange: (value: T) => void;
  className?: string;
}

export function SegmentedControl<T extends string>({
  label,
  value,
  options,
  onValueChange,
  className,
}: SegmentedControlProps<T>) {
  const generatedName = useId();

  return (
    <fieldset className={cn("min-w-0 space-y-1.5", className)}>
      <legend className="font-headline text-xs leading-none text-[var(--fg-50)]">{label}</legend>
      <div className="grid auto-cols-fr grid-flow-col gap-1 rounded-lg bg-surface-300/60 p-1">
        {options.map((option) => (
          <label key={option.value} className="min-w-0 cursor-pointer">
            <input
              type="radio"
              name={generatedName}
              value={option.value}
              checked={value === option.value}
              onChange={() => onValueChange(option.value)}
              className="peer sr-only"
            />
            <span
              className={cn(
                "flex min-h-11 items-center justify-center rounded-md px-3 font-headline text-xs transition-colors duration-150",
                "peer-focus-visible:ring-2 peer-focus-visible:ring-cursor-orange/40 peer-focus-visible:ring-offset-1",
                value === option.value
                  ? "bg-surface-100 text-cursor-dark shadow-[0_1px_3px_rgba(0,0,0,0.06)]"
                  : "text-[var(--fg-50)] hover:text-[var(--fg-70)]",
              )}
            >
              <span className="truncate">{option.label}</span>
            </span>
          </label>
        ))}
      </div>
    </fieldset>
  );
}
