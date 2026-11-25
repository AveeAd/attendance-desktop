import { readFileSync } from "node:fs";
import { Pool, type QueryResultRow } from "pg";

export function createPool(databaseUrl: string): Pool {
  return new Pool({ connectionString: databaseUrl });
}

export async function migrate(pool: Pool): Promise<void> {
  const sql = readFileSync(new URL("./migrations/001_init.sql", import.meta.url), "utf8");
  await pool.query(sql);
}

export async function rows<T extends QueryResultRow>(
  pool: Pool,
  sql: string,
  params: unknown[] = [],
): Promise<T[]> {
  const result = await pool.query<T>(sql, params);
  return result.rows;
}
