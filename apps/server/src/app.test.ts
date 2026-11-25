import type { Pool } from "pg";
import { afterEach, describe, expect, it } from "vitest";

import { buildServer } from "./app.js";

const servers: Array<{ close: () => Promise<void> }> = [];

afterEach(async () => {
  await Promise.all(servers.splice(0).map((server) => server.close()));
});

describe("health endpoint", () => {
  it("does not require an API key", async () => {
    const app = await buildServer({ pool: {} as Pool });
    servers.push(app);
    const response = await app.inject({ method: "GET", url: "/health" });
    expect(response.statusCode).toBe(200);
    expect(response.json()).toEqual({ ok: true });
  });
});
