import { convertFileSrc, isTauri } from "@tauri-apps/api/core";

export type Notify = (message: string) => void;

export function localAssetSource(path?: string, cacheKey?: number) {
  if (!path || !isTauri()) return path;
  const source = convertFileSrc(path);
  return cacheKey == null ? source : `${source}?assetVersion=${cacheKey}`;
}

/** Converts native command failures into a UI-safe message. */
export function nativeErrorMessage(error: unknown, fallback: string) {
  console.error(`[SF Novel Flow] ${fallback}`, error);
  return typeof error === "string"
    ? error
    : error instanceof Error && error.message
      ? error.message
      : fallback;
}
