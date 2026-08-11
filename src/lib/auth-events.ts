type AuthenticationRequiredListener = () => void;

const listeners = new Set<AuthenticationRequiredListener>();

export function notifyAuthenticationRequired(): void {
  for (const listener of listeners) listener();
}

export function subscribeAuthenticationRequired(
  listener: AuthenticationRequiredListener,
): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
