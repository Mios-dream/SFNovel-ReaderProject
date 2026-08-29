import fse from "fs-extra";
import path from "node:path";
import { config } from "../config";
import type { NovelDownloadMetadata } from "../types";

/**
 * 将名称中的文件系统保留字符替换为下划线。
 * @param name 原始名称。
 * @returns 可安全用作目录或文件名的名称。
 */
export const safeName = (name: string) =>
  name.replace(/[<>:"/\\|?*]/g, "_").trim();
/**
 * 清洗有声章节文件名并处理尾部句点或空格。
 * @param name 原始章节名称。
 * @returns 可安全保存的音频文件名。
 */
export const safeAudioName = (name: string) =>
  safeName(name).replace(/[. ]+$/g, "") || "未命名章节";

/**
 * 将本地书库路径转换为前端可访问的 URL，并统一编码中文文件名。
 * @param folder 书籍目录名。
 * @param file 书籍文件相对路径。
 * @returns 编码后的书库资源 URL。
 */
export function bookUrl(folder: string, file: string) {
  return `/library/${encodeURIComponent(folder)}/${encodeURIComponent(file)}`;
}

/**
 * 从未知输入中筛选出正整数章节 ID。
 * @param value 待处理的任意值。
 * @returns 去除非法值后的章节 ID 数组。
 */
export function numberIds(value: unknown) {
  return Array.isArray(value)
    ? value.filter((id): id is number => Number.isInteger(id) && id > 0)
    : [];
}

/**
 * 读取指定书籍的下载元数据；文件不存在时返回空对象。
 * @param dir 书籍目录路径。
 * @returns 下载元数据或空元数据对象。
 */
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

/**
 * 写入书籍下载元数据，并清洗章节 ID 数组。
 * @param dir 书籍目录路径。
 * @param metadata 待保存的下载元数据。
 * @returns 文件写入完成后的 Promise。
 */
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

/**
 * 汇总指定小说在本地书库中的文本和有声下载状态。
 * @param novelId SF 小说编号。
 * @returns 已下载章节 ID 及兼容旧文件得到的章节标题集合。
 */
export async function getLocalDownloadState(novelId: number) {
  // 通过下载元数据和旧版 Markdown 标题双重识别已下载章节，兼容历史文件。
  await fse.ensureDir(config.libraryDir);
  const folders = await fse.readdir(config.libraryDir, { withFileTypes: true });
  const states = await Promise.all(
    folders
      .filter((folder) => folder.isDirectory())
      .map(async (folder) => {
        const dir = path.join(config.libraryDir, folder.name);
        const metadata = await readNovelDownloadMetadata(dir);
        if (
          metadata.novelId !== novelId ||
          numberIds(metadata.downloadedTextChapterIds).length
        ) {
          return { metadata, downloadedTextTitles: [] as string[] };
        }
        const markdown = (await fse.readdir(dir)).find((file) =>
          file.endsWith(".md"),
        );
        if (!markdown)
          return { metadata, downloadedTextTitles: [] as string[] };
        try {
          const content = await fse.readFile(path.join(dir, markdown), "utf8");
          return {
            metadata,
            downloadedTextTitles: [...content.matchAll(/^##\s+(.+)$/gm)].map(
              (match) => match[1].trim(),
            ),
          };
        } catch {
          return { metadata, downloadedTextTitles: [] as string[] };
        }
      }),
  );
  const matching = states.filter(
    ({ metadata }) => metadata.novelId === novelId,
  );
  return {
    downloadedTextChapterIds: [
      ...new Set(
        matching.flatMap(({ metadata }) =>
          numberIds(metadata.downloadedTextChapterIds),
        ),
      ),
    ],
    downloadedTextTitles: [
      ...new Set(
        matching.flatMap(({ downloadedTextTitles }) => downloadedTextTitles),
      ),
    ],
    downloadedAudioChapterIds: [
      ...new Set(
        matching.flatMap(({ metadata }) =>
          numberIds(metadata.downloadedAudioChapterIds),
        ),
      ),
    ],
  };
}
