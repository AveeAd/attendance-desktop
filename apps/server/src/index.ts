import { buildServer } from "./app.js";
import { seedApiKey } from "./auth.js";
import { loadConfig } from "./config.js";
import { createPool, migrate } from "./db.js";

async function main() {
  const config = loadConfig();
  const pool = createPool(config.databaseUrl);
  await migrate(pool);
  await seedApiKey(pool, config.apiKey, config.organizationName);
  const app = await buildServer({ pool, logger: config.logLevel === "debug" });
  await app.listen({ port: config.port, host: "0.0.0.0" });
  console.log(`Attendance API listening on port ${config.port}`);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
