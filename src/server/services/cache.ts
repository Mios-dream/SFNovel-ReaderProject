import crypto from "node:crypto";
import { getRequestPolicy } from "./requestPolicy";

type CacheEntry<T> = { expiresAt: number; value: T };

const responseCache = new Map<string, CacheEntry<unknown>>();
const pendingCacheLoads = new Map<string, Promise<unknown>>();
// 将下载请求串行化，用时间间隔控制对 SF 接口的访问频率。
let downloadQueue: Promise<void> = Promise.resolve();
let lastDownloadRequestAt = 0;

/**
 * 为缓存键附加会话哈希，避免不同账号共享个性化数据。
 * @param cookie 当前会话 Cookie；未登录时传入 undefined。
 * @param scope 缓存数据的业务范围。
 * @returns 可用于内存缓存的稳定键名。
 */
export function sessionKey(cookie: string | undefined, scope: string) {
  const identity = cookie
    ? crypto.createHash("sha256").update(cookie).digest("hex")
    : "anonymous";
  return `${scope}:${identity}`;
}

/**
 * 从内存缓存读取数据，并合并同一键的并发加载请求。
 * @param key 缓存键。
 * @param ttl 缓存有效期（毫秒）。
 * @param loader 缓存未命中时执行的异步加载函数。
 * @returns 缓存或上游加载得到的数据。
 */
export async function cached<T>(
  key: string,
  ttl: number,
  loader: () => Promise<T>,
): Promise<T> {
  // 同一 key 的并发读取共享一个 Promise，避免缓存未命中时重复请求上游。
  const existing = responseCache.get(key) as CacheEntry<T> | undefined;
  if (existing && existing.expiresAt > Date.now()) return existing.value;
  const pending = pendingCacheLoads.get(key) as Promise<T> | undefined;
  if (pending) return pending;
  const load = loader()
    .then((value) => {
      responseCache.set(key, { value, expiresAt: Date.now() + ttl });
      return value;
    })
    .finally(() => pendingCacheLoads.delete(key));
  pendingCacheLoads.set(key, load);
  return load;
}

// Serialize requests made by background download jobs. Interactive catalog
// requests intentionally bypass this queue so the details dialog stays responsive.
/**
 * 按请求间隔串行执行后台下载请求。
 * @param loader 实际发起网络请求的异步函数。
 * @param interval 两次请求之间的最小间隔，默认读取当前策略。
 * @returns 实际请求的结果。
 */
export async function throttledDownload<T>(
  loader: () => Promise<T>,
  interval = getRequestPolicy().requestIntervalMs,
): Promise<T> {
  const run = downloadQueue.then(async () => {
    const wait = Math.max(0, interval - (Date.now() - lastDownloadRequestAt));
    if (wait) await new Promise((resolve) => setTimeout(resolve, wait));
    try {
      return await loader();
    } finally {
      lastDownloadRequestAt = Date.now();
    }
  });
  downloadQueue = run.then(
    () => undefined,
    () => undefined,
  );
  return run;
}
