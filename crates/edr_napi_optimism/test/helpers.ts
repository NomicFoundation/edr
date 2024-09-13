import { toBytes } from "@nomicfoundation/ethereumjs-util";

function getEnv(key: string): string | undefined {
  const variable = process.env[key];
  if (variable === undefined || variable === "") {
    return undefined;
  }

  const trimmed = variable.trim();

  return trimmed.length === 0 ? undefined : trimmed;
}

export const ALCHEMY_URL = getEnv("ALCHEMY_URL");

export function isCI(): boolean {
  return getEnv("CI") === "true";
}

export function toBuffer(x: Parameters<typeof toBytes>[0]) {
  return Buffer.from(toBytes(x));
}
