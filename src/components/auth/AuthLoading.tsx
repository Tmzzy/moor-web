import { MoorLogo } from "@/components/icons/MoorLogo";

export function AuthLoading() {
  return (
    <div className="flex min-h-dvh items-center justify-center bg-cursor-cream px-6">
      <div className="flex flex-col items-center gap-4 text-center">
        <div className="flex h-12 w-12 items-center justify-center rounded-lg bg-cursor-dark">
          <MoorLogo className="h-9 w-9 text-surface-200" />
        </div>
        <div
          role="status"
          className="h-6 w-6 animate-spin rounded-full border-2 border-[var(--fg-10)] border-t-cursor-orange"
          aria-label="Checking session"
        />
      </div>
    </div>
  );
}
