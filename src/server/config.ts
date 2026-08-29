import path from "node:path";
import fse from "fs-extra";

const rootDir = process.cwd();

export type RequestPolicyConfig = {
  requestIntervalMs: number;
  maxConcurrentDownloads: number;
};

type UserConfig = {
  userSettings: {
    libraryDir: string;
  };
  requestPolicy: RequestPolicyConfig;
};

const defaultUserConfig: UserConfig = {
  userSettings: {
    libraryDir: "output/菠萝包轻小说",
  },
  requestPolicy: {
    requestIntervalMs: 500,
    maxConcurrentDownloads: 1,
  },
};

let userConfig: UserConfig = structuredClone(defaultUserConfig);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function readUserConfig(value: unknown): UserConfig {
  if (!isRecord(value)) return structuredClone(defaultUserConfig);
  const settings = isRecord(value.userSettings) ? value.userSettings : {};
  const policy = isRecord(value.requestPolicy) ? value.requestPolicy : {};
  return {
    userSettings: {
      libraryDir: typeof settings.libraryDir === "string" && settings.libraryDir.trim()
        ? settings.libraryDir.trim()
        : defaultUserConfig.userSettings.libraryDir,
    },
    requestPolicy: {
      requestIntervalMs: Number(policy.requestIntervalMs),
      maxConcurrentDownloads: Number(policy.maxConcurrentDownloads),
    },
  };
}

export const config = {
  rootDir,
  libraryDir: path.join(rootDir, "output", "菠萝包轻小说"),
  webDir: path.join(rootDir, "dist"),
  port: 8787,
  host: "127.0.0.1",
  auth: {
    cookieName: "sfacg_session",
    cookieMaxAge: 30 * 24 * 60 * 60 * 1000,
    browserDebugPort: 9223,
    browserProfileDir: path.join(rootDir, ".sfacg-login-profile"),
    browserLoginUrl: "https://passport.sfacg.com/Login.aspx",
  },
  cache: {
    metadataTtl: 60_000,
    audioTtl: 120_000,
    bookshelfTtl: 120_000,
  },
  userConfigFile: path.join(rootDir, "config.json"),
};

export async function loadUserConfig() {
  try {
    userConfig = readUserConfig(await fse.readJson(config.userConfigFile));
  } catch {
    userConfig = structuredClone(defaultUserConfig);
  }
  config.libraryDir = path.resolve(rootDir, userConfig.userSettings.libraryDir);
  return getUserConfig();
}

export function getUserConfig(): UserConfig {
  return structuredClone(userConfig);
}

export async function saveRequestPolicyConfig(requestPolicy: RequestPolicyConfig) {
  userConfig = { ...userConfig, requestPolicy: { ...requestPolicy } };
  await fse.outputJson(config.userConfigFile, userConfig, { spaces: 2 });
}
