import fse from "fs-extra";
import path from "node:path";
import { config } from "../config";
import type { NovelDownloadMetadata } from "../types";

export const safeName = (name: string) =>
  name.replace(/[<>:"/\\|?*]/g, "_").trim();
export const safeAudioName = (name: string) =>
  safeName(name).replace(/[. ]+$/g, "") || "未命名章节";

export function bookUrl(folder: string, file: string) {
  return `/library/${encodeURIComponent(folder)}/${encodeURIComponent(file)}`;
}

export function numberIds(value: unknown) {
  return Array.isArray(value)
    ? value.filter((id): id is number => Number.isInteger(id) && id > 0)
    : [];
}

export async function readNovelDownloadMetadata(
  dir: string,
): Promise<NovelDownloadMetadata> {
  try {
    const metadata = (await fse.readJson(
      path.join(dir, ".novel-flow.json"),
    )) as NovelDownloadMetadata;
    return metadata && typeof metadata === "object" ? metadata : {};
  } catch {
    return {};
  }
}

export async function writeNovelDownloadMetadata(
  dir: string,
  metadata: NovelDownloadMetadata,
) {
  await fse.outputFile(
    path.join(dir, ".novel-flow.json"),
    JSON.stringify({
      ...metadata,
      downloadedTextChapterIds: numberIds(metadata.downloadedTextChapterIds),
      downloadedAudioChapterIds: numberIds(metadata.downloadedAudioChapterIds),
    }),
  );
}

export async function getLocalDownloadState(novelId: number) {
  await fse.ensureDir(config.libraryDir);
  const folders = await fse.readdir(config.libraryDir, { withFileTypes: true });
  const states = await Promise.all(
    folders.filter((folder) => folder.isDirectory()).map(async (folder) => {
      const dir = path.join(config.libraryDir, folder.name);
      const metadata = await readNovelDownloadMetadata(dir);
      if (metadata.novelId !== novelId || numberIds(metadata.downloadedTextChapterIds).length) {
        return { metadata, downloadedTextTitles: [] as string[] };
      }
      const markdown = (await fse.readdir(dir)).find((file) => file.endsWith(".md"));
      if (!markdown) return { metadata, downloadedTextTitles: [] as string[] };
      try {
        const content = await fse.readFile(path.join(dir, markdown), "utf8");
        return { metadata, downloadedTextTitles: [...content.matchAll(/^##\s+(.+)$/gm)].map((match) => match[1].trim()) };
      } catch {
        return { metadata, downloadedTextTitles: [] as string[] };
      }
    }),
  );
  const matching = states.filter(({ metadata }) => metadata.novelId === novelId);
  return {
    downloadedTextChapterIds: [...new Set(matching.flatMap(({ metadata }) => numberIds(metadata.downloadedTextChapterIds)))],
    downloadedTextTitles: [...new Set(matching.flatMap(({ downloadedTextTitles }) => downloadedTextTitles))],
    downloadedAudioChapterIds: [...new Set(matching.flatMap(({ metadata }) => numberIds(metadata.downloadedAudioChapterIds)))],
  };
}
