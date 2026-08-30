import { Router } from "express";
import { refreshDownloadSlots } from "../services/jobs";
import { getRequestPolicy, updateRequestPolicy } from "../services/requestPolicy";
import { getAuthSession } from "../services/auth";
import { SfacgApiClient } from "../infrastructure/sfacg/client";
import {
  getSfacgContentDictionarySize,
  loadSfacgContentDictionary,
  updateSfacgContentDictionary,
} from "../services/sfacgContentDictionary";

export const settingsRouter = Router();

settingsRouter.get("/content-dictionary", async (_req, res) => {
  await loadSfacgContentDictionary();
  res.json({ size: getSfacgContentDictionarySize() });
});

/** 用指定的公开章节对齐 API 混淆字与网页正常字。 */
settingsRouter.post("/content-dictionary/update", async (req, res) => {
  const chapterId = Number(req.body?.chapterId);
  if (!Number.isInteger(chapterId) || chapterId <= 0)
    return res.status(400).json({ message: "章节编号无效" });
  try {
    const session = getAuthSession(req);
    const client = new SfacgApiClient();
    if (session) {
      client.setCookie(session.cookie);
      client.setNonce(session.nonce);
    }
    res.json(await updateSfacgContentDictionary(chapterId, client));
  } catch (error) {
    res.status(502).json({
      message: error instanceof Error ? error.message : "正文恢复字典更新失败",
    });
  }
});

/**
 * 获取当前下载请求策略。
 * @param _req 未使用的 Express 请求。
 * @param res 返回请求策略的 Express 响应。
 * @returns 无返回值。
 */
settingsRouter.get("/request-policy", (_req, res) => res.json(getRequestPolicy()));

/**
 * 校验并更新下载请求策略，同时唤醒等待中的任务。
 * @param req 包含请求策略 JSON 的 Express 请求。
 * @param res 返回更新后策略或校验错误的 Express 响应。
 * @returns 保存处理完成后的 Promise。
 */
settingsRouter.put("/request-policy", async (req, res) => {
  try {
    const policy = await updateRequestPolicy(req.body || {});
    refreshDownloadSlots();
    res.json(policy);
  } catch (error) {
    res.status(400).json({ message: error instanceof Error ? error.message : "请求策略无效" });
  }
});
