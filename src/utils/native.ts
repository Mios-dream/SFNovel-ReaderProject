import { convertFileSrc, isTauri } from "@tauri-apps/api/core";

/**
 * 本地文件在 WebView 中的可访问地址。
 * @param path 原生绝对路径。
 * @param cacheKey 内容版本号；变化时强制刷新缓存。
 */
export function localAssetSource(path?: string, cacheKey?: number) {
  if (!path || !isTauri()) return path;
  const source = convertFileSrc(path);
  return cacheKey == null ? source : `${source}?assetVersion=${cacheKey}`;
}

/**
 * 从失败值里取出可展示的文案，不做任何副作用。
 *
 * Tauri 命令失败时 `AppError` 已经序列化成**字符串**，因此字符串分支必须优先——
 * 判 `instanceof Error` 会一路落到兜底文案，把上游原因整个丢掉。其余分支覆盖
 * 前端自身抛出的 `Error` 与带 `message` 的对象；都取不到才用调用方给的文案。
 */
export function messageOf(error: unknown, fallback: string) {
  if (typeof error === "string" && error.trim()) return error.trim();
  if (error instanceof Error && error.message.trim()) return error.message.trim();
  if (error && typeof error === "object") {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string" && message.trim()) return message.trim();
  }
  return fallback;
}

/**
 * 把命令失败转成可以直接展示的文案，并记录原始错误。
 *
 * 只用于「就地提示」的场景；需要弹 toast 时用 `useToast` 的 `reportError`，
 * 它复用 `messageOf` 但把输出交给 toast，避免同一次失败被记录两遍。
 */
export function nativeErrorMessage(error: unknown, fallback: string) {
  console.error(`[SF Novel Flow] ${fallback}`, error);
  return messageOf(error, fallback);
}
