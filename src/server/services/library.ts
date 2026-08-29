import fse from "fs-extra";
import path from "node:path";
import { config } from "../config";
import type { NovelChapterStore, NovelDownloadMetadata, StoredTextChapter } from "../types";

const chapterStoreName = ".novel-flow-chapters.json";

function isCurrentChapterStore(value: unknown): value is NovelChapterStore {
  if (!value || typeof value !== "object") return false;
  const store = value as Partial<NovelChapterStore>;
  if (!Number.isInteger(store.novelId) || !store.chapters || typeof store.chapters !== "object")
    return false;
  return Object.values(store.chapters).every(
    (chapter) =>
      chapter &&
      Number.isInteger(chapter.id) &&
      typeof chapter.volume === "string" &&
      typeof chapter.title === "string" &&
      typeof chapter.content === "string" &&
      Number.isInteger(chapter.volumeIndex) &&
      Number.isInteger(chapter.chapterIndex),
  );
}

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

/** 读取可恢复的文本章节内容。 */
export async function readNovelChapterStore(dir: string): Promise<NovelChapterStore> {
  try {
    const value = (await fse.readJson(
      path.join(dir, chapterStoreName),
    )) as NovelChapterStore;
    if (isCurrentChapterStore(value))
      return value;
  } catch {
    /* no current chapter store is available */
  }
  return { novelId: 0, chapters: {} };
}

/** 将每章正文独立持久化，使暂停、继续和增量下载不会覆盖已有章节。 */
export async function writeNovelChapterStore(dir: string, store: NovelChapterStore) {
  await fse.outputJson(path.join(dir, chapterStoreName), store);
}

/** 按在线目录的卷和卷内位置排序；旧书库回退到章节 ID。 */
export function sortStoredTextChapters(chapters: StoredTextChapter[]) {
  return [...chapters].sort((left, right) =>
    left.volumeIndex - right.volumeIndex ||
    left.chapterIndex - right.chapterIndex ||
    left.id - right.id,
  );
}

/** 从章节库生成稳定的 Markdown 正文，按在线目录而非本次选择的章节排序。 */
export function buildNovelMarkdown(
  title: string,
  author: string,
  description: string,
  volumes: Array<{ title: string; chapterList: Array<{ chapId: number }> }>,
  chapters: Record<string, StoredTextChapter>,
) {
  const content = volumes
    .map((volume) => {
      const items = volume.chapterList
        .map((chapter) => chapters[String(chapter.chapId)]?.content)
        .filter((item): item is string => Boolean(item));
      return items.length ? `# ${volume.title}\n\n${items.join("\n\n")}` : "";
    })
    .filter(Boolean)
    .join("\n\n");
  const intro = description
    .split("\n")
    .map((line) => `  ${line}`)
    .join("\n");
  return `---\ntitle: '${title.replaceAll("'", "\\\\'")}'\nauthor: '${author.replaceAll("'", "\\\\'")}'\nlang: 'zh-Hans'\ndescription: |-\n${intro}\n...\n\n${content}`;
}

/**
 * 汇总指定小说在本地书库中的文本和有声下载状态。
 * @param novelId SF 小说编号。
 * @returns 已下载章节 ID。
 */
export async function getLocalDownloadState(novelId: number) {
  await fse.ensureDir(config.libraryDir);
  const folders = await fse.readdir(config.libraryDir, { withFileTypes: true });
  const states = await Promise.all(
    folders
      .filter((folder) => folder.isDirectory())
      .map(async (folder) => {
        const dir = path.join(config.libraryDir, folder.name);
        const metadata = await readNovelDownloadMetadata(dir);
        if (metadata.novelId !== novelId)
          return { metadata, downloadedTextChapterIds: [] as number[] };
        const store = await readNovelChapterStore(dir);
        const storeIds = store.novelId === novelId ? Object.keys(store.chapters).map(Number).filter((id) => Number.isInteger(id) && id > 0) : [];
        const metadataIds = numberIds(metadata.downloadedTextChapterIds);
        return {
          metadata,
          downloadedTextChapterIds: [...new Set([...metadataIds, ...storeIds])],
        };
      }),
  );
  const matching = states.filter(
    ({ metadata }) => metadata.novelId === novelId,
  );
  return {
    downloadedTextChapterIds: [
      ...new Set(
        matching.flatMap(({ metadata, downloadedTextChapterIds }) => [
          ...numberIds(metadata.downloadedTextChapterIds),
          ...downloadedTextChapterIds,
        ]),
      ),
    ],
    downloadedTextTitles: [],
    downloadedAudioChapterIds: [
      ...new Set(
        matching.flatMap(({ metadata }) =>
          numberIds(metadata.downloadedAudioChapterIds),
        ),
      ),
    ],
  };
}
