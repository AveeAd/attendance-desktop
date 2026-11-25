import { randomBytes, scryptSync, timingSafeEqual } from "node:crypto";
import { randomUUID } from "node:crypto";
import type { Pool } from "pg";

const DEFAULT_ORGANIZATION_ID = "00000000-0000-0000-0000-000000000001";

export function apiKeyHint(key: string): string {
  const trimmed = key.trim();
  if (trimmed.length <= 8) return "key";
  return `${trimmed.slice(0, 4)}…${trimmed.slice(-4)}`;
}

export function hashApiKey(key: string): string {
  const salt = randomBytes(16);
  const derived = scryptSync(key.trim(), salt, 32);
  return `scrypt$${salt.toString("hex")}$${derived.toString("hex")}`;
}

export function verifyApiKey(key: string, stored: string): boolean {
  const [algorithm, saltHex, hashHex] = stored.split("$");
  if (algorithm !== "scrypt" || !saltHex || !hashHex) return false;
  try {
    const expected = Buffer.from(hashHex, "hex");
    const actual = scryptSync(key.trim(), Buffer.from(saltHex, "hex"), expected.length);
    return expected.length === actual.length && timingSafeEqual(expected, actual);
  } catch {
    return false;
  }
}

export async function seedApiKey(
  pool: Pool,
  apiKey: string,
  organizationName: string,
): Promise<void> {
  await pool.query(
    `INSERT INTO organizations(id, name)
     VALUES ($1, $2)
     ON CONFLICT (id) DO UPDATE SET name = excluded.name`,
    [DEFAULT_ORGANIZATION_ID, organizationName],
  );

  const hint = apiKeyHint(apiKey);
  const hash = hashApiKey(apiKey);
  const existing = await pool.query(
    "SELECT id FROM api_keys WHERE key_hint = $1 ORDER BY created_at LIMIT 1",
    [hint],
  );

  if (existing.rowCount === 0) {
    await pool.query(
      `INSERT INTO api_keys(id, organization_id, key_hash, key_hint, active)
       VALUES ($1, $2, $3, $4, true)`,
      [randomUUID(), DEFAULT_ORGANIZATION_ID, hash, hint],
    );
    return;
  }

  await pool.query(
    "UPDATE api_keys SET key_hash = $2, active = true WHERE id = $1",
    [existing.rows[0].id, hash],
  );
}
