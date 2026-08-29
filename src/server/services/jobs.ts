import type { Request } from "express";
import { bookUrl } from "./library";
import { writeAudio, writeNovel } from "./downloads";
import { getAuthSession } from "./auth";
import { getRequestPolicy } from "./requestPolicy";
import type { Job } from "../types";

const jobs = new Map<string, Job>();
const jobControllers = new Map<string, AbortController>();
const jobSessions = new Map<string, { cookie?: string; chapterIds?: number[] }>();

let activeDownloadCount = 0;
const downloadSlotWaiters: Array<{ resolve: (release: () => void) => void; reject: (error: Error) => void; signal: AbortSignal; }> = [];

function pumpDownloadSlot() {
  while (activeDownloadCount < getRequestPolicy().maxConcurrentDownloads) {
    const next = downloadSlotWaiters.shift();
    if (!next) return;
    if (next.signal.aborted) {
      next.reject(new Error("下载已取消"));
      continue;
    }
    activeDownloadCount += 1;
    let released = false;
    next.resolve(() => {
      if (released) return;
      released = true;
      activeDownloadCount -= 1;
      pumpDownloadSlot();
    });
  }
}

export function refreshDownloadSlots() { pumpDownloadSlot(); }

function acquireDownloadSlot(signal: AbortSignal): Promise<() => void> {
  return new Promise((resolve, reject) => {
    if (signal.aborted) return reject(new Error("下载已取消"));
    downloadSlotWaiters.push({ resolve, reject, signal });
    pumpDownloadSlot();
  });
}

function run(job: Job) {
  const session = jobSessions.get(job.id);
  if (!session) return;
  const controller = new AbortController();
  jobControllers.set(job.id, controller);
  void (async () => {
    let release: (() => void) | undefined;
    try {
      release = await acquireDownloadSlot(controller.signal);
      if (controller.signal.aborted) throw new Error("下载已取消");
      job.status = "downloading";
      const update = (progress: number, message: string) => Object.assign(job, { progress, message });
      if (job.kind === "audio") {
        const saved = await writeAudio(job.novelId, session.cookie || "", job.chapterIds, controller.signal, update);
        Object.assign(job, { status: "done", progress: 100, message: `已保存 ${saved.chapters} 集有声内容`, file: bookUrl(saved.folder, saved.file) });
      } else {
        const saved = await writeNovel(job.novelId, session.cookie, job.chapterIds, controller.signal, update);
        Object.assign(job, { status: "done", progress: 100, message: `已保存 ${saved.chapters} 个章节`, file: bookUrl(saved.folder, saved.file) });
      }
    } catch (error) {
      if (controller.signal.aborted && job.status === "paused") job.message = "已暂停，可继续下载";
      else if (controller.signal.aborted) Object.assign(job, { status: "cancelled", message: "下载已取消" });
      else Object.assign(job, { status: "error", message: error instanceof Error ? error.message : "下载失败" });
    } finally { release?.(); jobControllers.delete(job.id); }
  })();
}

export function validChapterIds(chapterIds: unknown) {
  return chapterIds === undefined || (Array.isArray(chapterIds) && chapterIds.every((id) => Number.isInteger(id) && id > 0));
}

export function createJob(req: Request, kind: Job["kind"], novelId: number, title: string | undefined, chapterIds: number[] | undefined) {
  const id = kind === "audio" ? `${Date.now()}-audio-${novelId}` : `${Date.now()}-${novelId}`;
  const job: Job = { id, title: title || `小说 ${novelId}`, kind, novelId, chapterIds, status: "queued", progress: 0, message: "等待开始" };
  jobs.set(id, job);
  jobSessions.set(id, { cookie: getAuthSession(req)?.cookie, chapterIds });
  run(job);
  return job;
}

export function getJobs() { return [...jobs.values()].reverse(); }
export function getJob(id: string) { return jobs.get(id); }
export function pauseJob(id: string) {
  const job = jobs.get(id);
  if (job && (job.status === "queued" || job.status === "downloading")) { job.status = "paused"; job.message = "正在暂停下载"; jobControllers.get(id)?.abort(); }
  return job;
}
export function resumeJob(id: string) {
  const job = jobs.get(id);
  if (job?.status === "paused") { job.message = "等待继续"; run(job); }
  return job;
}
export function cancelJob(id: string) {
  const job = jobs.get(id);
  if (job && !["done", "error", "cancelled"].includes(job.status)) { job.status = "cancelled"; job.message = "正在停止下载"; jobControllers.get(id)?.abort(); }
  return job;
}
export function deleteJob(id: string) {
  if (!jobs.has(id)) return false;
  jobControllers.get(id)?.abort();
  jobs.delete(id); jobControllers.delete(id); jobSessions.delete(id);
  return true;
}
