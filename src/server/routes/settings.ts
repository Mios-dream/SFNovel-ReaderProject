import { Router } from "express";
import { refreshDownloadSlots } from "../services/jobs";
import { getRequestPolicy, updateRequestPolicy } from "../services/requestPolicy";

export const settingsRouter = Router();

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
