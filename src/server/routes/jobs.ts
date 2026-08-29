import { Router } from "express";
import { getAuthSession } from "../services/auth";
import { cancelJob, createJob, deleteJob, getJob, getJobs, pauseJob, resumeJob, validChapterIds } from "../services/jobs";

export const jobsRouter = Router();

type DownloadBody = { novelId?: number; title?: string; chapterIds?: number[] };

jobsRouter.post("/download", (req, res) => {
  const { novelId, title, chapterIds } = req.body as DownloadBody;
  if (!novelId) return res.status(400).json({ message: "缺少小说编号" });
  if (!validChapterIds(chapterIds)) return res.status(400).json({ message: "章节选择无效" });
  res.status(202).json(createJob(req, "text", novelId, title, chapterIds));
});

jobsRouter.post("/audio/download", (req, res) => {
  const { novelId, title, chapterIds } = req.body as DownloadBody;
  if (!getAuthSession(req)) return res.status(401).json({ message: "有声内容需要登录 SF 账号" });
  if (!novelId) return res.status(400).json({ message: "缺少小说编号" });
  if (!validChapterIds(chapterIds)) return res.status(400).json({ message: "章节选择无效" });
  res.status(202).json(createJob(req, "audio", novelId, title, chapterIds));
});

jobsRouter.post("/jobs/:id/pause", (req, res) => {
  const job = pauseJob(req.params.id);
  if (!job) return res.status(404).json({ message: "下载任务不存在" });
  res.json(job);
});
jobsRouter.post("/jobs/:id/resume", (req, res) => {
  const job = resumeJob(req.params.id);
  if (!job) return res.status(404).json({ message: "下载任务不存在" });
  res.json(job);
});
jobsRouter.post("/jobs/:id/cancel", (req, res) => {
  const job = cancelJob(req.params.id);
  if (!job) return res.status(404).json({ message: "下载任务不存在" });
  res.json(job);
});
jobsRouter.delete("/jobs/:id", (req, res) => {
  if (!deleteJob(req.params.id)) return res.status(404).json({ message: "下载任务不存在" });
  res.status(204).end();
});
jobsRouter.get("/jobs", (_req, res) => res.json(getJobs()));
