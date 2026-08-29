import {
  getUserConfig,
  loadUserConfig,
  saveRequestPolicyConfig,
} from "../config";

export type RequestPolicy = {
  requestIntervalMs: number;
  maxConcurrentDownloads: number;
};

const defaultPolicy: RequestPolicy = {
  requestIntervalMs: 500,
  maxConcurrentDownloads: 1,
};

let currentPolicy: RequestPolicy = { ...defaultPolicy };

/**
 * 校验并规范化下载请求策略。
 * @param value 待校验的部分请求策略。
 * @returns 通过范围校验的完整请求策略。
 * @throws 当任一数值不符合服务端限制时抛出错误。
 */
function normalizePolicy(value: Partial<RequestPolicy>): RequestPolicy {
  const requestIntervalMs = Number(value.requestIntervalMs);
  const maxConcurrentDownloads = Number(value.maxConcurrentDownloads);
  if (
    !Number.isInteger(requestIntervalMs) ||
    requestIntervalMs < 250 ||
    requestIntervalMs > 10_000
  ) {
    throw new Error("请求间隔需为 250 到 10000 毫秒之间的整数");
  }
  if (
    !Number.isInteger(maxConcurrentDownloads) ||
    maxConcurrentDownloads < 1 ||
    maxConcurrentDownloads > 3
  ) {
    throw new Error("下载并发数需为 1 到 3 之间的整数");
  }
  return { requestIntervalMs, maxConcurrentDownloads };
}

/**
 * 从 config.json 加载请求策略，并在配置缺失或无效时写入默认值。
 * @returns 当前生效的请求策略。
 */
export async function loadRequestPolicy() {
  try {
    await loadUserConfig();
    currentPolicy = normalizePolicy(getUserConfig().requestPolicy);
  } catch {
    currentPolicy = { ...defaultPolicy };
    await saveRequestPolicyConfig(currentPolicy);
  }
  return getRequestPolicy();
}

/**
 * 获取当前进程内生效的请求策略。
 * @returns 请求间隔和最大并发数。
 */
export function getRequestPolicy(): RequestPolicy {
  return { ...currentPolicy };
}

/**
 * 校验并保存新的请求策略。
 * @param value 待保存的部分请求策略。
 * @returns 保存后的完整请求策略。
 */
export async function updateRequestPolicy(value: Partial<RequestPolicy>) {
  currentPolicy = normalizePolicy(value);
  await saveRequestPolicyConfig(currentPolicy);
  return getRequestPolicy();
}
