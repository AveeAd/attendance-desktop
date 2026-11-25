import dotenv from "dotenv";

dotenv.config();

export interface Config {
  port: number;
  databaseUrl: string;
  apiKey: string;
  organizationName: string;
  logLevel: string;
}

export function loadConfig(env: NodeJS.ProcessEnv = process.env): Config {
  const databaseUrl = env.DATABASE_URL;
  const apiKey = env.API_KEY;
  if (!databaseUrl) throw new Error("DATABASE_URL is required");
  if (!apiKey) throw new Error("API_KEY is required");

  return {
    port: Number(env.PORT ?? 3000),
    databaseUrl,
    apiKey,
    organizationName: env.ORGANIZATION_NAME ?? "My Organization",
    logLevel: env.LOG_LEVEL ?? "info",
  };
}
