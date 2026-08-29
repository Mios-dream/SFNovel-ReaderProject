import { Router } from "express";
import fse from "fs-extra";
import path from "node:path";
import { config } from "../config";
import { bookUrl, safeName } from "../services/library";

export const libraryRouter = Router();

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
    if (!markdown && !hasAudio) return null;
    const updatedFile = markdown ? path.join(dir, markdown) : audioPlaylist;
    const cover = path.join(dir, "imgs", "cover.jpeg");
    let metadata: { novelId?: number } = {};
    try { metadata = await fse.readJson(path.join(dir, ".novel-flow.json")); } catch { /* metadata is optional */ }
    return { name: folder.name, novelId: Number.isInteger(metadata.novelId) ? metadata.novelId : undefined,
      href: markdown ? bookUrl(folder.name, markdown) : undefined,
      audioHref: hasAudio ? bookUrl(folder.name, path.posix.join("audio", "有声目录.m3u8")) : undefined,
      cover: await fse.pathExists(cover) ? bookUrl(folder.name, path.posix.join("imgs", "cover.jpeg")) : undefined,
      updatedAt: (await fse.stat(updatedFile)).mtime };
  }));
  res.json(books.filter(Boolean));
});

/**
 * 删除指定书籍目录，并验证路径未越出书库根目录。
 * @param req 包含已编码书籍目录名的 Express 请求。
 * @param res 返回删除结果或错误信息的 Express 响应。
 * @returns 删除处理完成后的 Promise。
 */
libraryRouter.delete("/library/:folder", async (req, res) => {
  const folder = safeName(decodeURIComponent(req.params.folder));
  const target = path.resolve(config.libraryDir, folder), base = path.resolve(config.libraryDir);
  if (target === base || !target.startsWith(`${base}${path.sep}`)) return res.status(400).json({ message: "书籍目录无效" });
  if (!(await fse.pathExists(target))) return res.status(404).json({ message: "本地书籍不存在" });
  await fse.remove(target);
  res.status(204).end();
});
