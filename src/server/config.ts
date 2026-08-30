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

/**
 * 判断未知值是否为非空普通对象。
 * @param value 待判断的值。
 * @returns 值为对象且不是数组时返回 true。
 */
function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * 将 JSON 中的未知值转换为带默认值的用户配置。
 * @param value 待解析的 JSON 值。
 * @returns 可供服务端使用的用户配置对象。
 */
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
    profileTtl: 120_000,
  },
  userConfigFile: path.join(rootDir, "config.json"),
  sfacgDictionaryFile: path.join(rootDir, "sfacg-content-dictionary.json"),
};

/**
 * 读取用户配置，并将相对书库路径解析为服务端使用的绝对路径。
 * @returns 当前加载后的用户配置。
 */
export async function loadUserConfig() {
  try {
    userConfig = readUserConfig(await fse.readJson(config.userConfigFile));
  } catch {
    userConfig = structuredClone(defaultUserConfig);
  }
  config.libraryDir = path.resolve(rootDir, userConfig.userSettings.libraryDir);
  return getUserConfig();
}

/**
 * 返回用户配置的副本，避免调用方直接修改内存中的配置。
 * @returns 用户配置副本。
 */
export function getUserConfig(): UserConfig {
  return structuredClone(userConfig);
}

/**
 * 只更新请求策略，保留配置文件中的其他用户设置。
 * @param requestPolicy 经校验的请求策略。
 * @returns 配置写入完成后的 Promise。
 */
export async function saveRequestPolicyConfig(requestPolicy: RequestPolicyConfig) {
  userConfig = { ...userConfig, requestPolicy: { ...requestPolicy } };
  await fse.outputJson(config.userConfigFile, userConfig, { spaces: 2 });
}
