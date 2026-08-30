import axios from "axios";
import { createWriteStream } from "node:fs";
import path from "node:path";
import { pipeline } from "node:stream/promises";
import fse from "fs-extra";
import {
  SfacgApiClient,
  SfacgWebContentError,
} from "../infrastructure/sfacg/client";
import { config } from "../config";
import type { AudioChapter, AudioInfoResponse, AuthSession } from "../types";
import {
  numberIds,
  buildNovelMarkdown,
  readNovelChapterStore,
  readNovelDownloadMetadata,
  safeAudioName,
  safeName,
  writeNovelChapterStore,
  writeNovelDownloadMetadata,
} from "./library";
import { throttledDownload } from "./cache";
import { decodeSfacgContent, loadSfacgContentDictionary } from "./sfacgContentDictionary";

type ProgressHandler = (value: number, message: string) => void;
type AudioCatalogErrorCode = "NO_AUDIO" | "AUTH_EXPIRED" | "UPSTREAM_ERROR";
const SF_WEB_USER_AGENT =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0";

const chapterImagePattern = /!\[([^\]]*)\]\((https?:\/\/[^\s)]+)\)/g;

function normalizeApiChapterContent(content: string) {
  return content
    .replace(/\[img=[^\]]*\](https?:\/\/[^[]+)\[\/img\]/gi, "![章节插图]($1)")
    .replace(/\r\n?/g, "\n")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

function imageExtension(url: string) {
  try {
    const extension = path.extname(new URL(url).pathname).toLowerCase();
    return [".jpeg", ".jpg", ".png", ".gif", ".webp"].includes(extension)
      ? extension
      : ".jpeg";
  } catch {
    return ".jpeg";
  }
}

/** 将网页正文中的插图保存到书库，并改写为 Markdown 本地图片链接。 */
async function downloadChapterImages(
  content: string,
  chapterId: number,
  imageDir: string,
) {
  const matches = [...content.matchAll(chapterImagePattern)];
  if (!matches.length) return content;

  let imageIndex = 0;
  let result = "";
  let lastIndex = 0;
  for (const match of matches) {
    result += content.slice(lastIndex, match.index);
    lastIndex = (match.index || 0) + match[0].length;
    const imageUrl = match[2];
    const filename = `chapter-${chapterId}-${String(imageIndex + 1).padStart(2, "0")}${imageExtension(imageUrl)}`;
    imageIndex += 1;
    try {
      await fse.outputFile(
        path.join(imageDir, filename),
        await throttledDownload(() => SfacgApiClient.image(imageUrl)),
      );
      result += `![${match[1]}](imgs/${filename})`;
    } catch {
      // 保留远程地址，避免单张失效图片阻断整章下载。
      result += match[0];
    }
  }
  return result + content.slice(lastIndex);
}

export class AudioCatalogError extends Error {
  /**
   * 创建有声目录业务错误。
   * @param code 可供 API 客户端识别的错误码。
   * @param message 面向用户的错误信息。
   * @param httpStatus 对应的 HTTP 状态码。
   */
  constructor(
    public readonly code: AudioCatalogErrorCode,
    message: string,
    public readonly httpStatus: 401 | 404 | 502,
  ) {
    super(message);
    this.name = "AudioCatalogError";
  }
}

/**
 * 下载文本章节并生成 Markdown 书籍文件。
 * @param novelId SF 小说编号。
 * @param session 可选的登录会话，用于读取已购买章节。
 * @param chapterIds 可选章节 ID 列表；未传时下载全部可用章节。
 * @param signal 用于暂停或取消任务的 AbortSignal。
 * @param onProgress 可选进度回调，接收百分比和状态消息。
 * @returns 生成文件的书名、目录、文件名和处理章节数。
 */
export async function writeNovel(
  novelId: number,
  session: AuthSession | undefined,
  chapterIds: number[] | undefined,
  signal: AbortSignal,
  onProgress?: ProgressHandler,
) {
  await loadSfacgContentDictionary();
  const cookie = session?.cookie;
  // 文本下载支持断点续传：章节内容先写入进度文件，全部完成后再生成最终 Markdown。
  const client = new SfacgApiClient();
  if (cookie) {
    client.setCookie(cookie);
    client.setNonce(session?.nonce);
  }
  const [novel, volumes] = await Promise.all([
    throttledDownload(() => client.novelInfo(novelId, signal)),
    throttledDownload(() => client.volumeInfos(novelId, signal)),
  ]);
  if (!novel || !volumes) throw new Error("无法读取小说信息");
  const novelName = safeName(novel.novelName);
  const novelDir = path.join(config.libraryDir, novelName);
  const imageDir = path.join(novelDir, "imgs");
  const markdownFile = `${novelName}.md`;
  await fse.ensureDir(imageDir);
  const selected = chapterIds?.length ? new Set(chapterIds) : undefined;
  const total = volumes.reduce(
    (sum, volume) =>
      sum +
      volume.chapterList.filter(
        (chapter) => !selected || selected.has(chapter.chapId),
      ).length,
    0,
  );
  let finished = 0;
  let contentError: Error | undefined;
  const downloadClient = new SfacgApiClient();
  if (cookie) {
    downloadClient.setCookie(cookie);
    downloadClient.setNonce(session?.nonce);
  }
  const savedStore = await readNovelChapterStore(novelDir);
  const savedChapters =
    savedStore.novelId === novelId ? savedStore.chapters : {};
  for (const [id, chapter] of Object.entries(savedChapters)) {
    if (!Number.isInteger(chapter.id)) chapter.id = Number(id);
  }
  const metadata = await readNovelDownloadMetadata(novelDir);
  const downloadedTextChapterIds = new Set(
    numberIds(metadata.downloadedTextChapterIds),
  );
  const contentSources = new Set<string>();
  onProgress?.(4, "正在准备书籍文件");
  for (const [volumeIndex, volume] of volumes.entries()) {
    for (const [chapterIndex, chapter] of volume.chapterList.entries()) {
      if (selected && !selected.has(chapter.chapId)) continue;
      if (signal.aborted) throw new Error("下载已取消");
      const saved = savedChapters[String(chapter.chapId)];
      if (saved) {
        finished += 1;
        downloadedTextChapterIds.add(chapter.chapId);
        onProgress?.(
          total ? Math.round(5 + (finished / total) * 90) : 100,
          `复用本地章节：${chapter.ntitle}`,
        );
        continue;
      }
      try {
        if (chapter.needFireMoney !== 0 && !cookie) continue;
        let raw = "";
        let source = "App API";
        try {
          onProgress?.(4, `正在通过 App API 获取：${chapter.ntitle}`);
          raw = normalizeApiChapterContent(
            decodeSfacgContent(
              await throttledDownload(() =>
                downloadClient.chapterContentFromApi(chapter.chapId, signal),
              ),
            ),
          );
        } catch {
          source = "网页解析";
          onProgress?.(4, `App API 不可用，正在通过网页解析：${chapter.ntitle}`);
          raw = await throttledDownload(() =>
            downloadClient.chapterContentFromWeb(
              novelId,
              volume.volumeId,
              chapter.chapId,
              signal,
            ),
          );
        }
        if (signal.aborted) throw new Error("下载已取消");
        finished += 1;
        console.info(`SF 正文来源：${source}（章节 ${chapter.chapId}）`);
        onProgress?.(
          total ? Math.round(5 + (finished / total) * 90) : 100,
          `已通过${source}获取：${chapter.ntitle}`,
        );
        if (raw.trim()) {
          contentSources.add(source);
          const contentWithImages = await downloadChapterImages(
            raw,
            chapter.chapId,
            imageDir,
          );
          // 网页解析器已完成段落换行规范化，这里不能再次放大换行。
          const chapterContent = `## ${chapter.ntitle}\n\n${contentWithImages}`;
          savedChapters[String(chapter.chapId)] = {
            id: chapter.chapId,
            volume: volume.title,
            title: chapter.ntitle,
            content: chapterContent,
            volumeIndex,
            chapterIndex,
          };
          downloadedTextChapterIds.add(chapter.chapId);
          await writeNovelChapterStore(novelDir, {
            novelId,
            chapters: savedChapters,
          });
          await writeNovelDownloadMetadata(novelDir, {
            ...metadata,
            novelId,
            title: novel.novelName,
            author: novel.authorName,
            description: novel.expand?.intro || "",
            downloadedTextChapterIds: [...downloadedTextChapterIds],
          });
          // 每章成功后同步 Markdown，网页不可访问的后续章节不会覆盖已下载部分。
          await fse.outputFile(
            path.join(novelDir, markdownFile),
            buildNovelMarkdown(
              novel.novelName,
              novel.authorName,
              novel.expand?.intro || "",
              volumes,
              savedChapters,
            ),
          );
        }
      } catch (error) {
        if (signal.aborted) throw new Error("下载已取消");
        if (error instanceof SfacgWebContentError) {
          contentError ||= error;
          // 正文校验失败不是章节本身的空内容，后续章节也会被同一策略拒绝。
          break;
        }
        finished += 1;
      }
    }
    if (contentError) break;
  }
  if (contentError) throw contentError;
  if (!Object.keys(savedChapters).length)
    throw new Error("未能下载任何可访问章节，原有本地文件未被覆盖");
  const markdown = buildNovelMarkdown(
    novel.novelName,
    novel.authorName,
    novel.expand?.intro || "",
    volumes,
    savedChapters,
  );
  await fse.outputFile(path.join(novelDir, markdownFile), markdown);
  await writeNovelDownloadMetadata(novelDir, {
    ...metadata,
    novelId,
    title: novel.novelName,
    author: novel.authorName,
    description: novel.expand?.intro || "",
    downloadedTextChapterIds: [...downloadedTextChapterIds],
  });
  if (novel.novelCover) {
    const coverUrl = novel.novelCover;
    try {
      await fse.outputFile(
        path.join(imageDir, "cover.jpeg"),
        await throttledDownload(() => SfacgApiClient.image(coverUrl)),
      );
    } catch {
      /* cover is optional */
    }
  }
  onProgress?.(100, "已保存到本地书库");
  return {
    name: novel.novelName,
    folder: novelName,
    file: markdownFile,
    chapters: finished,
    sources: [...contentSources],
  };
}

/**
 * 读取并解析指定小说的有声章节目录。
 * @param novelId SF 小说编号。
 * @param cookie 当前登录会话 Cookie。
 * @returns 有声作品标题及章节列表。
 * @throws AudioCatalogError 当未登录、接口失败或作品没有有声内容时抛出。
 */
export async function getAudioChapters(novelId: number, cookie: string) {
  // 有声目录使用官方网页接口，响应状态同时可能出现在 HTTP 和业务字段中。
  let data: AudioInfoResponse;
  try {
    ({ data } = await axios.get<AudioInfoResponse>(
      "https://i.sfacg.com/ajax/ashx/Common.ashx",
      {
        params: { op: "getAudioInfo", nid: novelId },
        headers: {
          Cookie: cookie,
          "User-Agent": SF_WEB_USER_AGENT,
          Accept: "application/json, text/javascript, */*; q=0.01",
          "X-Requested-With": "XMLHttpRequest",
          Referer: "https://i.sfacg.com/consume/book/",
        },
        timeout: 15_000,
      },
    ));
  } catch (error) {
    if (
      axios.isAxiosError(error) &&
      (error.response?.status === 401 || error.response?.status === 403)
    )
      throw new AudioCatalogError(
        "AUTH_EXPIRED",
        "SF 登录会话已失效，请重新登录",
        401,
      );
    throw new AudioCatalogError(
      "UPSTREAM_ERROR",
      axios.isAxiosError(error) && error.code === "ECONNABORTED"
        ? "SF 有声接口请求超时"
        : "无法连接 SF 有声接口",
      502,
    );
  }
  const upstreamStatus = Number(data.status);
  if (upstreamStatus === 401 || upstreamStatus === 403)
    throw new AudioCatalogError(
      "AUTH_EXPIRED",
      "SF 登录会话已失效，请重新登录",
      401,
    );
  if (upstreamStatus !== 200 || !data.data) {
    if (upstreamStatus === 400 && data.msg === "参数不正确")
      throw new AudioCatalogError("NO_AUDIO", "该作品没有可用的有声章节", 404);
    throw new AudioCatalogError(
      "UPSTREAM_ERROR",
      `SF 有声接口拒绝了请求${data.msg ? `：${data.msg}` : ""}`,
      502,
    );
  }
  const chapters: AudioChapter[] = [];
  for (const volume of data.data.VolumeSet || [])
    for (const audio of volume.AudioSet || []) {
      if (audio.AudioSrc)
        chapters.push({
          id: Number(audio.AudioID || chapters.length + 1),
          title: audio.ChapterTitle || "未命名章节",
          source: audio.AudioSrc,
          volume: volume.VolumeName || "未分卷",
        });
    }
  if (!chapters.length)
    throw new AudioCatalogError("NO_AUDIO", "该作品没有可用的有声章节", 404);
  return { title: data.data.NovelName || `小说 ${novelId}`, chapters };
}

/**
 * 有声目录接口只包含专辑标题和章节，缺少本地书库展示所需的作者、简介与封面。
 * 仅在本地资料不完整时补取小说详情，避免每次增量下载重复请求。
 */
async function ensureAudioBookDetails(
  novelId: number,
  cookie: string,
  novelDir: string,
  audioTitle: string,
  signal: AbortSignal,
) {
  const metadata = await readNovelDownloadMetadata(novelDir);
  const coverPath = path.join(novelDir, "imgs", "cover.jpeg");
  const hasCover = await fse.pathExists(coverPath);
  const needsDetails =
    metadata.title === undefined ||
    metadata.author === undefined ||
    metadata.description === undefined;
  if (!needsDetails && hasCover) return metadata;

  const client = new SfacgApiClient();
  client.setCookie(cookie);
  const novel = await throttledDownload(() => client.novelInfo(novelId, signal));
  if (!novel) return metadata;

  const completedMetadata = {
    ...metadata,
    novelId,
    title: metadata.title || audioTitle,
    author: novel.authorName,
    description: novel.expand?.intro || "",
  };
  await writeNovelDownloadMetadata(novelDir, completedMetadata);
  if (!hasCover && novel.novelCover) {
    try {
      await fse.outputFile(
        coverPath,
        await throttledDownload(() => SfacgApiClient.image(novel.novelCover!)),
      );
    } catch {
      /* A failed cover request must not fail an otherwise valid audio download. */
    }
  }
  return completedMetadata;
}

/**
 * 下载有声章节并生成 M3U8 播放列表。
 * @param novelId SF 小说编号。
 * @param cookie 当前登录会话 Cookie。
 * @param chapterIds 可选章节 ID 列表；未传时下载全部章节。
 * @param signal 用于暂停或取消任务的 AbortSignal。
 * @param onProgress 可选进度回调，接收百分比和状态消息。
 * @returns 生成播放列表的书名、目录、文件和章节数。
 */
export async function writeAudio(
  novelId: number,
  cookie: string,
  chapterIds: number[] | undefined,
  signal: AbortSignal,
  onProgress?: ProgressHandler,
) {
  const audio = await throttledDownload(() =>
    getAudioChapters(novelId, cookie),
  );
  const selected = chapterIds?.length ? new Set(chapterIds) : undefined;
  const chapters = selected
    ? audio.chapters.filter((chapter) => selected.has(chapter.id))
    : audio.chapters;
  const novelName = safeName(audio.title);
  const novelDir = path.join(config.libraryDir, novelName);
  const audioDir = path.join(novelDir, "audio");
  await fse.ensureDir(audioDir);
  onProgress?.(2, "正在补全作品封面和详情");
  const metadata = await ensureAudioBookDetails(
    novelId,
    cookie,
    novelDir,
    audio.title,
    signal,
  );
  const downloadedAudioChapterIds = new Set(
    numberIds(metadata.downloadedAudioChapterIds),
  );
  for (const [selectedIndex, chapter] of chapters.entries()) {
    if (signal.aborted) throw new Error("下载已取消");
    // 使用完整在线目录中的序号，增量下载时不会覆盖已有章节文件。
    const chapterIndex = audio.chapters.findIndex(
      (item) => item.id === chapter.id,
    );
    const filename = `${String(chapterIndex + 1).padStart(3, "0")} - ${safeAudioName(chapter.title)}.mp3`;
    const target = path.join(audioDir, filename);
    const partialTarget = `${target}.part`;
    onProgress?.(
      Math.round((selectedIndex / chapters.length) * 96) + 2,
      `正在下载：${chapter.title}`,
    );
    if (!(await fse.pathExists(target))) {
      try {
        const response = await throttledDownload(() =>
          axios.get<NodeJS.ReadableStream>(chapter.source, {
            responseType: "stream",
            signal,
            headers: {
              Cookie: cookie,
              "User-Agent": SF_WEB_USER_AGENT,
              Referer: "https://i.sfacg.com/consume/book/",
              Accept: "audio/mpeg,*/*;q=0.8",
            },
            timeout: 60_000,
          }),
        );
        await pipeline(response.data, createWriteStream(partialTarget));
        await fse.move(partialTarget, target, { overwrite: true });
      } catch (error) {
        await fse.remove(partialTarget);
        throw error;
      }
    }
    downloadedAudioChapterIds.add(chapter.id);
    await writeNovelDownloadMetadata(novelDir, {
      ...metadata,
      novelId,
      title: audio.title,
      downloadedAudioChapterIds: [...downloadedAudioChapterIds],
    });
  }
  // 每次下载后从完整在线目录重建清单，保留此前已下载但本次未选择的章节。
  const playlist: string[] = ["#EXTM3U"];
  for (const [chapterIndex, chapter] of audio.chapters.entries()) {
    const filename = `${String(chapterIndex + 1).padStart(3, "0")} - ${safeAudioName(chapter.title)}.mp3`;
    if (await fse.pathExists(path.join(audioDir, filename))) {
      playlist.push(
        `#EXTINF:-1,${chapter.volume} - ${chapter.title}`,
        filename,
      );
    }
  }
  const playlistFile = "有声目录.m3u8";
  await fse.outputFile(
    path.join(audioDir, playlistFile),
    `${playlist.join("\n")}\n`,
  );
  await writeNovelDownloadMetadata(novelDir, {
    ...metadata,
    novelId,
    title: audio.title,
    downloadedAudioChapterIds: [...downloadedAudioChapterIds],
  });
  onProgress?.(100, "有声内容已保存到本地书库");
  return {
    name: audio.title,
    folder: novelName,
    file: path.posix.join("audio", playlistFile),
    chapters: chapters.length,
  };
}
