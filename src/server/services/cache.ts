import crypto from "node:crypto";
import { getRequestPolicy } from "./requestPolicy";

type CacheEntry<T> = { expiresAt: number; value: T };

const responseCache = new Map<string, CacheEntry<unknown>>();
const pendingCacheLoads = new Map<string, Promise<unknown>>();
let downloadQueue: Promise<void> = Promise.resolve();
let lastDownloadRequestAt = 0;

export function sessionKey(cookie: string | undefined, scope: string) {
  const identity = cookie
    ? crypto.createHash("sha256").update(cookie).digest("hex")
    : "anonymous";
  return `${scope}:${identity}`;
}

export async function cached<T>(
  key: string,
  ttl: number,
  loader: () => Promise<T>,
): Promise<T> {
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
