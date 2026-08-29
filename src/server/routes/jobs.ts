import { Router } from "express";
import { getAuthSession } from "../services/auth";
import { cancelJob, createJob, deleteJob, getJob, getJobs, pauseJob, resumeJob, validChapterIds } from "../services/jobs";

export const jobsRouter = Router();

type DownloadBody = { novelId?: number; title?: string; chapterIds?: number[] };

/**
 * 创建文本章节下载任务并将其加入后台队列。
 * @param req 包含作品和章节选择的 Express 请求。
 * @param res 返回已入队任务或校验错误的 Express 响应。
 * @returns 无返回值。
 */
jobsRouter.post("/download", (req, res) => {
  const { novelId, title, chapterIds } = req.body as DownloadBody;
  if (!novelId) return res.status(400).json({ message: "缺少小说编号" });
  if (!validChapterIds(chapterIds)) return res.status(400).json({ message: "章节选择无效" });
  res.status(202).json(createJob(req, "text", novelId, title, chapterIds));
});

/**
 * 创建有声章节下载任务，要求当前请求已登录。
 * @param req 包含作品和章节选择的 Express 请求。
 * @param res 返回已入队任务或认证、校验错误的 Express 响应。
 * @returns 无返回值。
 */
jobsRouter.post("/audio/download", (req, res) => {
  const { novelId, title, chapterIds } = req.body as DownloadBody;
  if (!getAuthSession(req)) return res.status(401).json({ message: "有声内容需要登录 SF 账号" });
  if (!novelId) return res.status(400).json({ message: "缺少小说编号" });
  if (!validChapterIds(chapterIds)) return res.status(400).json({ message: "章节选择无效" });
  res.status(202).json(createJob(req, "audio", novelId, title, chapterIds));
});

/**
 * 暂停指定下载任务。
 * @param req 包含任务 ID 的 Express 请求。
 * @param res 返回更新后任务或不存在错误的 Express 响应。
 * @returns 无返回值。
 */
jobsRouter.post("/jobs/:id/pause", (req, res) => {
  const job = pauseJob(req.params.id);
  if (!job) return res.status(404).json({ message: "下载任务不存在" });
  res.json(job);
});
/**
 * 恢复指定下载任务。
 * @param req 包含任务 ID 的 Express 请求。
 * @param res 返回更新后任务或不存在错误的 Express 响应。
 * @returns 无返回值。
 */
jobsRouter.post("/jobs/:id/resume", (req, res) => {
  const job = resumeJob(req.params.id);
  if (!job) return res.status(404).json({ message: "下载任务不存在" });
  res.json(job);
});
/**
 * 取消指定下载任务。
 * @param req 包含任务 ID 的 Express 请求。
 * @param res 返回更新后任务或不存在错误的 Express 响应。
 * @returns 无返回值。
 */
jobsRouter.post("/jobs/:id/cancel", (req, res) => {
  const job = cancelJob(req.params.id);
  if (!job) return res.status(404).json({ message: "下载任务不存在" });
  res.json(job);
});
/**
 * 删除指定任务及其内存状态。
 * @param req 包含任务 ID 的 Express 请求。
 * @param res 返回空响应或不存在错误的 Express 响应。
 * @returns 无返回值。
 */
jobsRouter.delete("/jobs/:id", (req, res) => {
  if (!deleteJob(req.params.id)) return res.status(404).json({ message: "下载任务不存在" });
  res.status(204).end();
});
/**
 * 返回当前进程中的全部下载任务。
 * @param _req 未使用的 Express 请求。
 * @param res 返回任务数组的 Express 响应。
 * @returns 无返回值。
 */
jobsRouter.get("/jobs", (_req, res) => res.json(getJobs()));
