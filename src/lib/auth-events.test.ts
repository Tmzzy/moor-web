import { describe, expect, it, vi } from "vite-plus/test";

import { notifyAuthenticationRequired, subscribeAuthenticationRequired } from "./auth-events";

describe("authentication required events", () => {
  it("notifies active subscribers", () => {
    const listener = vi.fn();
    const unsubscribe = subscribeAuthenticationRequired(listener);

    notifyAuthenticationRequired();
    unsubscribe();

    expect(listener).toHaveBeenCalledOnce();
  });

  it("does not notify an unsubscribed listener", () => {
    const listener = vi.fn();
    const unsubscribe = subscribeAuthenticationRequired(listener);
    unsubscribe();

    notifyAuthenticationRequired();

    expect(listener).not.toHaveBeenCalled();
  });
});
