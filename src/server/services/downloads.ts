import axios from "axios";
import { createWriteStream } from "node:fs";
import path from "node:path";
import { pipeline } from "node:stream/promises";
import fse from "fs-extra";
import { SfacgClient } from "../../client/Sfacg/api/client";
import { config } from "../config";
import type { AudioChapter, AudioInfoResponse } from "../types";
import { numberIds, readNovelDownloadMetadata, safeAudioName, safeName, writeNovelDownloadMetadata } from "./library";

type ProgressHandler = (value: number, message: string) => void;
type AudioCatalogErrorCode = "NO_AUDIO" | "AUTH_EXPIRED" | "UPSTREAM_ERROR";

export class AudioCatalogError extends Error {
  constructor(public readonly code: AudioCatalogErrorCode, message: string, public readonly httpStatus: 401 | 404 | 502) {
    super(message);
    this.name = "AudioCatalogError";
  }
}

export async function writeNovel(novelId: number, cookie: string | undefined, chapterIds: number[] | undefined, signal: AbortSignal, onProgress?: ProgressHandler) {
  const client = new SfacgClient();
  const [novel, volumes] = await Promise.all([client.novelInfo(novelId, signal), client.volumeInfos(novelId, signal)]);
  if (!novel || !volumes) throw new Error("无法读取小说信息");
  const novelName = safeName(novel.novelName);
  const novelDir = path.join(config.libraryDir, novelName);
  const imageDir = path.join(novelDir, "imgs");
  await fse.ensureDir(imageDir);
  const selected = chapterIds?.length ? new Set(chapterIds) : undefined;
  const total = volumes.reduce((sum, volume) => sum + volume.chapterList.filter((chapter) => !selected || selected.has(chapter.chapId)).length, 0);
  let finished = 0;
  const downloadClient = new SfacgClient();
  if (cookie) downloadClient.SetCookie(cookie);
  const progressFile = path.join(novelDir, ".novel-flow-progress.json");
  let savedChapters: Record<string, { volume: string; title: string; content: string }> = {};
  try {
    const saved = await fse.readJson(progressFile);
    if (saved?.novelId === novelId && saved.chapters && typeof saved.chapters === "object") savedChapters = saved.chapters;
  } catch { /* a fresh download has no progress file */ }
  const metadata = await readNovelDownloadMetadata(novelDir);
  const downloadedTextChapterIds = new Set(numberIds(metadata.downloadedTextChapterIds));
  onProgress?.(4, "正在准备书籍文件");
  const content: string[] = [];
  for (const volume of volumes) {
    const chapters: string[] = [];
    for (const chapter of volume.chapterList) {
      if (selected && !selected.has(chapter.chapId)) continue;
      if (signal.aborted) throw new Error("下载已取消");
      const saved = savedChapters[String(chapter.chapId)];
      if (saved) { finished += 1; chapters.push(saved.content); continue; }
      try {
        if (chapter.needFireMoney !== 0 && !cookie) continue;
        const raw = await downloadClient.contentInfos(chapter.chapId, signal);
        if (signal.aborted) throw new Error("下载已取消");
        finished += 1;
        onProgress?.(total ? Math.round(5 + (finished / total) * 90) : 100, `正在下载：${chapter.ntitle}`);
        if (raw) {
          const chapterContent = `## ${chapter.ntitle}\n\n${raw.replaceAll("\n", "\n\n")}`;
          savedChapters[String(chapter.chapId)] = { volume: volume.title, title: chapter.ntitle, content: chapterContent };
          downloadedTextChapterIds.add(chapter.chapId);
          chapters.push(chapterContent);
          await fse.outputFile(progressFile, JSON.stringify({ novelId, chapters: savedChapters }));
          await writeNovelDownloadMetadata(novelDir, { ...metadata, novelId, title: novel.novelName, downloadedTextChapterIds: [...downloadedTextChapterIds] });
        }
      } catch (error) {
        if (signal.aborted) throw new Error("下载已取消");
        finished += 1;
      }
    }
    if (chapters.length) content.push(`# ${volume.title}\n\n${chapters.join("\n\n")}`);
  }
  const intro = novel.expand?.intro?.split("\n").map((line: string) => `  ${line}`).join("\n") ?? "";
  const markdown = `---\ntitle: '${novel.novelName.replaceAll("'", "\\'")}'\nauthor: '${novel.authorName.replaceAll("'", "\\'")}'\nlang: 'zh-Hans'\ndescription: |-\n${intro}\n...\n\n${content.join("\n\n")}`;
  const markdownFile = `${novelName}.md`;
  await fse.outputFile(path.join(novelDir, markdownFile), markdown);
  await fse.remove(progressFile);
  await writeNovelDownloadMetadata(novelDir, { ...metadata, novelId, title: novel.novelName, downloadedTextChapterIds: [...downloadedTextChapterIds] });
  if (novel.novelCover) {
    try { await fse.outputFile(path.join(imageDir, "cover.jpeg"), await SfacgClient.image(novel.novelCover)); }
    catch { /* cover is optional */ }
  }
  onProgress?.(100, "已保存到本地书库");
  return { name: novel.novelName, folder: novelName, file: markdownFile, chapters: finished };
}

export async function getAudioChapters(novelId: number, cookie: string) {
  let data: AudioInfoResponse;
  try {
    ({ data } = await axios.get<AudioInfoResponse>("https://i.sfacg.com/ajax/ashx/Common.ashx", {
      params: { op: "getAudioInfo", nid: novelId },
      headers: { Cookie: cookie, "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/131 Safari/537.36", Accept: "application/json, text/javascript, */*; q=0.01", "X-Requested-With": "XMLHttpRequest", Referer: "https://i.sfacg.com/consume/book/" },
      timeout: 15_000,
    }));
  } catch (error) {
    if (axios.isAxiosError(error) && (error.response?.status === 401 || error.response?.status === 403)) throw new AudioCatalogError("AUTH_EXPIRED", "SF 登录会话已失效，请重新登录", 401);
    throw new AudioCatalogError("UPSTREAM_ERROR", axios.isAxiosError(error) && error.code === "ECONNABORTED" ? "SF 有声接口请求超时" : "无法连接 SF 有声接口", 502);
  }
  const upstreamStatus = Number(data.status);
  if (upstreamStatus === 401 || upstreamStatus === 403) throw new AudioCatalogError("AUTH_EXPIRED", "SF 登录会话已失效，请重新登录", 401);
  if (upstreamStatus !== 200 || !data.data) {
    if (upstreamStatus === 400 && data.msg === "参数不正确") throw new AudioCatalogError("NO_AUDIO", "该作品没有可用的有声章节", 404);
    throw new AudioCatalogError("UPSTREAM_ERROR", `SF 有声接口拒绝了请求${data.msg ? `：${data.msg}` : ""}`, 502);
  }
  const chapters: AudioChapter[] = [];
  for (const volume of data.data.VolumeSet || []) for (const audio of volume.AudioSet || []) {
    if (audio.AudioSrc) chapters.push({ id: Number(audio.AudioID || chapters.length + 1), title: audio.ChapterTitle || "未命名章节", source: audio.AudioSrc, volume: volume.VolumeName || "未分卷" });
  }
  if (!chapters.length) throw new AudioCatalogError("NO_AUDIO", "该作品没有可用的有声章节", 404);
  return { title: data.data.NovelName || `小说 ${novelId}`, chapters };
}

export async function writeAudio(novelId: number, cookie: string, chapterIds: number[] | undefined, signal: AbortSignal, onProgress?: ProgressHandler) {
  const audio = await getAudioChapters(novelId, cookie);
  const selected = chapterIds?.length ? new Set(chapterIds) : undefined;
  const chapters = selected ? audio.chapters.filter((chapter) => selected.has(chapter.id)) : audio.chapters;
  const novelName = safeName(audio.title);
  const audioDir = path.join(config.libraryDir, novelName, "audio");
  await fse.ensureDir(audioDir);
  const playlist: string[] = ["#EXTM3U"];
  const metadata = await readNovelDownloadMetadata(path.join(config.libraryDir, novelName));
  const downloadedAudioChapterIds = new Set(numberIds(metadata.downloadedAudioChapterIds));
  for (const [index, chapter] of chapters.entries()) {
    if (signal.aborted) throw new Error("下载已取消");
    const filename = `${String(index + 1).padStart(3, "0")} - ${safeAudioName(chapter.title)}.mp3`;
    const target = path.join(audioDir, filename);
    const partialTarget = `${target}.part`;
    onProgress?.(Math.round((index / chapters.length) * 96) + 2, `正在下载：${chapter.title}`);
    if (!(await fse.pathExists(target))) {
      try {
        const response = await axios.get<NodeJS.ReadableStream>(chapter.source, { responseType: "stream", signal, headers: { Cookie: cookie, "User-Agent": "Mozilla/5.0" }, timeout: 60_000 });
        await pipeline(response.data, createWriteStream(partialTarget));
        await fse.move(partialTarget, target, { overwrite: true });
      } catch (error) { await fse.remove(partialTarget); throw error; }
    }
    downloadedAudioChapterIds.add(chapter.id);
    await writeNovelDownloadMetadata(path.join(config.libraryDir, novelName), { ...metadata, novelId, title: audio.title, downloadedAudioChapterIds: [...downloadedAudioChapterIds] });
    playlist.push(`#EXTINF:-1,${chapter.volume} - ${chapter.title}`, filename);
  }
  const playlistFile = "有声目录.m3u8";
  await fse.outputFile(path.join(audioDir, playlistFile), `${playlist.join("\n")}\n`);
  await writeNovelDownloadMetadata(path.join(config.libraryDir, novelName), { ...metadata, novelId, title: audio.title, downloadedAudioChapterIds: [...downloadedAudioChapterIds] });
  onProgress?.(100, "有声内容已保存到本地书库");
  return { name: audio.title, folder: novelName, file: path.posix.join("audio", playlistFile), chapters: chapters.length };
}
