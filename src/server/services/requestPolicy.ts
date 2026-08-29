import { getUserConfig, loadUserConfig, saveRequestPolicyConfig } from "../config";

export type RequestPolicy = {
  requestIntervalMs: number;
  maxConcurrentDownloads: number;
};

const defaultPolicy: RequestPolicy = {
  requestIntervalMs: 500,
  maxConcurrentDownloads: 1,
};

let currentPolicy: RequestPolicy = { ...defaultPolicy };

function normalizePolicy(value: Partial<RequestPolicy>): RequestPolicy {
  const requestIntervalMs = Number(value.requestIntervalMs);
  const maxConcurrentDownloads = Number(value.maxConcurrentDownloads);
  if (!Number.isInteger(requestIntervalMs) || requestIntervalMs < 250 || requestIntervalMs > 10_000) {
    throw new Error("请求间隔需为 250 到 10000 毫秒之间的整数");
  }
  if (!Number.isInteger(maxConcurrentDownloads) || maxConcurrentDownloads < 1 || maxConcurrentDownloads > 3) {
    throw new Error("下载并发数需为 1 到 3 之间的整数");
  }
  return { requestIntervalMs, maxConcurrentDownloads };
}

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

export function getRequestPolicy(): RequestPolicy {
  return { ...currentPolicy };
}

export async function updateRequestPolicy(value: Partial<RequestPolicy>) {
  currentPolicy = normalizePolicy(value);
  await saveRequestPolicyConfig(currentPolicy);
  return getRequestPolicy();
}
