import { Router } from "express";
import { SfacgApiClient } from "../infrastructure/sfacg/client";
import { config } from "../config";
import { getAuthSession } from "../services/auth";
import { cached, sessionKey } from "../services/cache";
import { AudioCatalogError, getAudioChapters } from "../services/downloads";
import { getLocalDownloadState } from "../services/library";

export const catalogRouter = Router();

/**
 * 搜索公开作品。
 * @param req 包含 q 查询参数的 Express 请求。
 * @param res 返回作品数组或错误信息的 Express 响应。
 * @returns 搜索处理完成后的 Promise。
 */
catalogRouter.get("/search", async (req, res) => {
  const query = String(req.query.q ?? "").trim();
  if (!query) return res.json([]);
  try {
    res.json((await new SfacgApiClient().searchInfos(query)) || []);
  } catch (error) {
    res
      .status(500)
      .json({ message: error instanceof Error ? error.message : "搜索失败" });
  }
});

/**
 * 同步当前登录账号的书架，默认使用会话隔离缓存。
 * @param req 包含登录 Cookie 和可选 refresh 参数的 Express 请求。
 * @param res 返回书架分类和作品的 Express 响应。
 * @returns 书架同步完成后的 Promise。
 */
catalogRouter.get("/bookshelf", async (req, res) => {
  const session = getAuthSession(req);
  if (!session)
    return res.status(401).json({ message: "请先登录 SF 账号后查看书架" });
  try {
    const client = new SfacgApiClient();
    client.setCookie(session.cookie);
    const load = () => client.bookshelfCollection();
    const collection =
      req.query.refresh === "1"
        ? await load()
        : await cached(
            sessionKey(session.cookie, "bookshelf"),
            config.cache.bookshelfTtl,
            load,
          );
    if (!collection)
      return res
        .status(401)
        .json({ message: "SF 登录会话已失效，请重新登录后同步书架" });
    res.json(collection);
  } catch (error) {
    res.status(502).json({
      message:
        error instanceof Error
          ? `SF 书架请求失败：${error.message}`
          : "读取书架失败",
    });
  }
});

/**
 * 获取小说文本目录并标记本地已下载、已解锁章节。
 * @param req 包含小说编号和可选登录 Cookie 的 Express 请求。
 * @param res 返回分卷章节数据的 Express 响应。
 * @returns 目录读取完成后的 Promise。
 */
catalogRouter.get("/chapters/:novelId", async (req, res) => {
  const novelId = Number(req.params.novelId);
  if (!Number.isInteger(novelId) || novelId <= 0)
    return res.status(400).json({ message: "小说编号无效" });
  try {
    const session = getAuthSession(req);
    const client = new SfacgApiClient();
    if (session) client.setCookie(session.cookie);
    const volumes = await cached(
      sessionKey(session?.cookie, `chapters:${novelId}`),
      config.cache.metadataTtl,
      () => client.volumeInfos(novelId),
    );
    const localState = await getLocalDownloadState(novelId);
    if (!volumes) throw new Error("无法读取章节目录");
    const downloaded = new Set(localState.downloadedTextChapterIds),
      titles = new Set(localState.downloadedTextTitles);
    res.json(
      volumes.map((volume) => ({
        volumeId: volume.volumeId,
        title: volume.title,
        chapters: volume.chapterList.map((chapter) => {
          const isDownloaded =
            downloaded.has(chapter.chapId) || titles.has(chapter.ntitle);
          return {
            chapId: chapter.chapId,
            title: chapter.ntitle,
            needFireMoney: chapter.needFireMoney,
            isVip: chapter.isVip,
            isUnlocked: Boolean(chapter.has) || isDownloaded,
            downloaded: isDownloaded,
          };
        }),
      })),
    );
  } catch (error) {
    res.status(500).json({
      message: error instanceof Error ? error.message : "读取章节目录失败",
    });
  }
});

/**
 * 获取单部小说的详情和简介。
 * @param req 包含小说编号的 Express 请求。
 * @param res 返回小说详情的 Express 响应。
 * @returns 详情读取完成后的 Promise。
 */
catalogRouter.get("/novel/:novelId", async (req, res) => {
  const novelId = Number(req.params.novelId);
  if (!Number.isInteger(novelId) || novelId <= 0)
    return res.status(400).json({ message: "小说编号无效" });
  try {
    const novel = await cached(
      `novel:${novelId}`,
      config.cache.metadataTtl,
      () => new SfacgApiClient().novelInfo(novelId),
    );
    if (!novel) throw new Error("无法读取小说信息");
    res.json({
      novelId: novel.novelId,
      novelName: novel.novelName,
      authorName: novel.authorName,
      novelCover: novel.novelCover,
      lastUpdateTime: novel.lastUpdateTime,
      description: novel.expand?.intro || "暂无简介",
      isFinish: novel.isFinish,
      typeName: novel.expand?.typeName,
    });
  } catch (error) {
    res.status(500).json({
      message: error instanceof Error ? error.message : "读取小说信息失败",
    });
  }
});

/**
 * 获取登录账号可访问的有声章节目录。
 * @param req 包含小说编号和登录 Cookie 的 Express 请求。
 * @param res 返回有声章节或业务错误的 Express 响应。
 * @returns 有声目录读取完成后的 Promise。
 */
catalogRouter.get("/audio/:novelId", async (req, res) => {
  const session = getAuthSession(req);
  if (!session)
    return res.status(401).json({ message: "有声内容需要登录 SF 账号" });
  const novelId = Number(req.params.novelId);
  if (!Number.isInteger(novelId) || novelId <= 0)
    return res.status(400).json({ message: "小说编号无效" });
  try {
    const audio = await cached(
      sessionKey(session.cookie, `audio:${novelId}`),
      config.cache.audioTtl,
      () => getAudioChapters(novelId, session.cookie),
    );
    const downloaded = new Set(
      (await getLocalDownloadState(novelId)).downloadedAudioChapterIds,
    );
    res.json({
      title: audio.title,
      chapters: audio.chapters.map(({ id, title, volume }) => ({
        id,
        title,
        volume,
        downloaded: downloaded.has(id),
      })),
    });
  } catch (error) {
    if (error instanceof AudioCatalogError) {
      if (error.code === "NO_AUDIO")
        return res.json({
          code: error.code,
          message: error.message,
          chapters: [],
        });
      return res
        .status(error.httpStatus)
        .json({ code: error.code, message: error.message });
    }
    res.status(500).json({
      code: "AUDIO_UNKNOWN_ERROR",
      message: error instanceof Error ? error.message : "读取有声目录失败",
    });
  }
});
