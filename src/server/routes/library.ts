import { Router } from "express";
import fse from "fs-extra";
import path from "node:path";
import { config } from "../config";
import type { NovelDownloadMetadata } from "../types";
import { writeEpub } from "../services/epub";
import {
  bookUrl,
  numberIds,
  readNovelChapterStore,
  readNovelDownloadMetadata,
  safeName,
  sortStoredTextChapters,
} from "../services/library";

export const libraryRouter = Router();

type LocalAudioTrack = { title: string; href: string };

/**
 * 读取下载器写出的 M3U8 文件，并将其中的本地 MP3 转成可供网页播放器使用的轨道。
 * 播放列表只用于组织文件，浏览器实际播放的始终是单个 MP3 资源。
 */
async function readLocalAudioTracks(folder: string, dir: string): Promise<LocalAudioTrack[]> {
  const audioDir = path.join(dir, "audio");
  const playlist = path.join(audioDir, "有声目录.m3u8");
  if (!(await fse.pathExists(playlist))) return [];

  const lines = (await fse.readFile(playlist, "utf8")).split(/\r?\n/);
  const tracks: LocalAudioTrack[] = [];
  let title: string | undefined;
  for (const rawLine of lines) {
    const line = rawLine.trim();
    if (!line) continue;
    if (line.startsWith("#EXTINF:")) {
      title = line.slice(line.indexOf(",") + 1).trim() || undefined;
      continue;
    }
    if (line.startsWith("#")) continue;

    // 下载器写入的文件位于 audio 目录的第一层，拒绝清单中的路径穿越条目。
    if (path.basename(line) !== line || !line.toLowerCase().endsWith(".mp3")) {
      title = undefined;
      continue;
    }
    const file = path.join(audioDir, line);
    if (await fse.pathExists(file)) {
      tracks.push({
        title: title || path.basename(line, path.extname(line)),
        href: bookUrl(folder, path.posix.join("audio", line)),
      });
    }
    title = undefined;
  }
  return tracks;
}

/**
 * 扫描书库目录并从元数据推导前端所需的文本、有声和封面链接。
 * @param _req 未使用的 Express 请求。
 * @param res 返回本地书籍列表的 Express 响应。
 * @returns 书库扫描完成后的 Promise。
 */
libraryRouter.get("/library", async (_req, res) => {
  await fse.ensureDir(config.libraryDir);
  const folders = await fse.readdir(config.libraryDir, { withFileTypes: true });
  const books = await Promise.all(folders.filter((folder) => folder.isDirectory()).map(async (folder) => {
    const dir = path.join(config.libraryDir, folder.name);
    const files = await fse.readdir(dir);
    const markdown = files.find((file) => file.endsWith(".md"));
    const audioPlaylist = path.join(dir, "audio", "有声目录.m3u8");
    const hasAudio = await fse.pathExists(audioPlaylist);
    const epub = files.find((file) => file.endsWith(".epub"));
    if (!markdown && !hasAudio && !epub) return null;
    const updatedFile = markdown ? path.join(dir, markdown) : audioPlaylist;
    const cover = path.join(dir, "imgs", "cover.jpeg");
    let metadata: NovelDownloadMetadata = {};
    try { metadata = await fse.readJson(path.join(dir, ".novel-flow.json")); } catch { /* metadata is optional */ }
    const markdownHasChapters = markdown
      ? /^##\s+.+$/m.test(await fse.readFile(path.join(dir, markdown), "utf8"))
      : false;
    return { name: folder.name, novelId: Number.isInteger(metadata.novelId) ? metadata.novelId : undefined,
      href: markdown ? bookUrl(folder.name, markdown) : undefined,
      audioHref: hasAudio ? bookUrl(folder.name, path.posix.join("audio", "有声目录.m3u8")) : undefined,
      epubHref: epub ? bookUrl(folder.name, epub) : undefined,
      formats: { text: Boolean(epub) || numberIds(metadata.downloadedTextChapterIds).length > 0 || markdownHasChapters, audio: hasAudio, comic: false },
      cover: await fse.pathExists(cover) ? bookUrl(folder.name, path.posix.join("imgs", "cover.jpeg")) : undefined,
      updatedAt: (await fse.stat(updatedFile)).mtime };
  }));
  res.json(books.filter(Boolean));
});

function libraryFolder(folder: string) {
  const name = safeName(decodeURIComponent(folder));
  const base = path.resolve(config.libraryDir);
  const dir = path.resolve(base, name);
  if (dir === base || !dir.startsWith(`${base}${path.sep}`)) return undefined;
  return { name, dir };
}

/** 返回本地书籍目录和可阅读的已下载文本章节。 */
libraryRouter.get("/library/:folder", async (req, res) => {
  const target = libraryFolder(req.params.folder);
  if (!target || !(await fse.pathExists(target.dir)))
    return res.status(404).json({ message: "本地书籍不存在" });
  const [metadata, store] = await Promise.all([
    readNovelDownloadMetadata(target.dir),
    readNovelChapterStore(target.dir),
  ]);
  const cover = path.join(target.dir, "imgs", "cover.jpeg");
  const [audioTracks, chapters] = await Promise.all([
    readLocalAudioTracks(target.name, target.dir),
    Promise.resolve(sortStoredTextChapters(Object.values(store.chapters))),
  ]);
  const chapterVolumes = chapters.reduce<
    Array<{ volume: string; chapters: typeof chapters }>
  >((volumes, chapter) => {
    const current = volumes.at(-1);
    if (!current || current.volume !== chapter.volume)
      volumes.push({ volume: chapter.volume, chapters: [] });
    volumes.at(-1)!.chapters.push(chapter);
    return volumes;
  }, []);
  res.json({
    name: target.name,
    novelId: metadata.novelId,
    author: metadata.author || "未知作者",
    description: metadata.description || "暂无简介",
    cover: (await fse.pathExists(cover)) ? bookUrl(target.name, "imgs/cover.jpeg") : undefined,
    audioTracks,
    epubHref: (await fse.pathExists(path.join(target.dir, `${target.name}.epub`))) ? bookUrl(target.name, `${target.name}.epub`) : undefined,
    chapterVolumes: chapterVolumes.map(({ volume, chapters }) => ({
      volume,
      chapters: chapters.map(({ id, title }) => ({ id, title })),
    })),
  });
});

/** 读取本地的单章正文，阅读器不会再依赖在线接口。 */
libraryRouter.get("/library/:folder/chapters/:chapterId", async (req, res) => {
  const target = libraryFolder(req.params.folder);
  const chapterId = Number(req.params.chapterId);
  if (!target || !Number.isInteger(chapterId))
    return res.status(400).json({ message: "章节参数无效" });
  const chapter = (await readNovelChapterStore(target.dir)).chapters[String(chapterId)];
  if (!chapter) return res.status(404).json({ message: "本地未找到该章节正文" });
  res.json(chapter);
});

/** 将本地已下载的文字章节导出为 EPUB。 */
libraryRouter.post("/library/:folder/epub", async (req, res) => {
  const target = libraryFolder(req.params.folder);
  if (!target || !(await fse.pathExists(target.dir)))
    return res.status(404).json({ message: "本地书籍不存在" });
  const [metadata, store] = await Promise.all([
    readNovelDownloadMetadata(target.dir),
    readNovelChapterStore(target.dir),
  ]);
  const chapters = sortStoredTextChapters(Object.values(store.chapters));
  if (!chapters.length)
    return res.status(409).json({ message: "这本书没有可导出的已下载文字章节" });
  const filename = `${target.name}.epub`;
  await writeEpub(path.join(target.dir, filename), {
    title: metadata.title || target.name,
    author: metadata.author || "未知作者",
    description: metadata.description || "",
    chapters,
    coverPath: path.join(target.dir, "imgs", "cover.jpeg"),
    imagesDir: path.join(target.dir, "imgs"),
  });
  res.json({ href: bookUrl(target.name, filename) });
});

/**
 * 删除指定书籍目录，并验证路径未越出书库根目录。
 * @param req 包含已编码书籍目录名的 Express 请求。
 * @param res 返回删除结果或错误信息的 Express 响应。
 * @returns 删除处理完成后的 Promise。
 */
libraryRouter.delete("/library/:folder", async (req, res) => {
  const target = libraryFolder(req.params.folder);
  if (!target) return res.status(400).json({ message: "书籍目录无效" });
  if (!(await fse.pathExists(target.dir))) return res.status(404).json({ message: "本地书籍不存在" });
  await fse.remove(target.dir);
  res.status(204).end();
});
