import { describe, expect, it } from "vitest";

import { apiKeyHint, hashApiKey, verifyApiKey } from "./auth.js";

describe("api key hashing", () => {
  it("verifies a matching key and rejects a different one", () => {
    const key = "ak_live_1234567890abcdef";
    const hash = hashApiKey(key);
    expect(hash).not.toContain(key);
    expect(verifyApiKey(key, hash)).toBe(true);
    expect(verifyApiKey("wrong-key", hash)).toBe(false);
  });

  it("hints do not expose the whole key", () => {
    const hint = apiKeyHint("ak_live_1234567890abcdef");
    expect(hint.length).toBeLessThan(12);
    expect(hint).not.toBe("ak_live_1234567890abcdef");
  });
});
