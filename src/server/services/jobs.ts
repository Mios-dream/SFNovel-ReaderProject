import type { Request } from "express";
import { bookUrl } from "./library";
import { writeAudio, writeNovel } from "./downloads";
import { getAuthSession } from "./auth";
import { getRequestPolicy } from "./requestPolicy";
import type { Job } from "../types";

const jobs = new Map<string, Job>();
const jobControllers = new Map<string, AbortController>();
const jobSessions = new Map<
  string,
  { cookie?: string; chapterIds?: number[] }
>();

// 任务状态保存在内存中；服务重启后未完成任务不会自动恢复。
let activeDownloadCount = 0;
const downloadSlotWaiters: Array<{
  resolve: (release: () => void) => void;
  reject: (error: Error) => void;
  signal: AbortSignal;
}> = [];

/**
 * 唤醒等待中的任务，直到达到当前并发下载上限。
 * @returns 无返回值。
 */
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

/**
 * 在并发设置变更后重新分配等待中的下载槽位。
 * @returns 无返回值。
 */
export function refreshDownloadSlots() {
  pumpDownloadSlot();
}

/**
 * 等待获得一个后台下载槽位。
 * @param signal 任务取消信号。
 * @returns 释放槽位的函数。
 */
function acquireDownloadSlot(signal: AbortSignal): Promise<() => void> {
  return new Promise((resolve, reject) => {
    if (signal.aborted) return reject(new Error("下载已取消"));
    downloadSlotWaiters.push({ resolve, reject, signal });
    pumpDownloadSlot();
  });
}

/**
 * 启动或恢复单个下载任务，并负责更新其状态和错误信息。
 * @param job 要执行或恢复的任务。
 * @returns 无返回值；任务结果通过 job 对象异步更新。
 */
function run(job: Job) {
  const session = jobSessions.get(job.id);
  if (!session) return;
  const controller = new AbortController();
  jobControllers.set(job.id, controller);
  // 每次恢复任务都创建新的 AbortController，暂停后可安全重新开始。
  void (async () => {
    let release: (() => void) | undefined;
    try {
      release = await acquireDownloadSlot(controller.signal);
      if (controller.signal.aborted) throw new Error("下载已取消");
      job.status = "downloading";
      const update = (progress: number, message: string) =>
        Object.assign(job, { progress, message });
      if (job.kind === "audio") {
        const saved = await writeAudio(
          job.novelId,
          session.cookie || "",
          job.chapterIds,
          controller.signal,
          update,
        );
        Object.assign(job, {
          status: "done",
          progress: 100,
          message: `已保存 ${saved.chapters} 集有声内容`,
          file: bookUrl(saved.folder, saved.file),
        });
      } else {
        const saved = await writeNovel(
          job.novelId,
          session.cookie,
          job.chapterIds,
          controller.signal,
          update,
        );
        Object.assign(job, {
          status: "done",
          progress: 100,
          message: `已保存 ${saved.chapters} 个章节`,
          file: bookUrl(saved.folder, saved.file),
        });
      }
    } catch (error) {
      if (controller.signal.aborted && job.status === "paused")
        job.message = "已暂停，可继续下载";
      else if (controller.signal.aborted)
        Object.assign(job, { status: "cancelled", message: "下载已取消" });
      else
        Object.assign(job, {
          status: "error",
          message: error instanceof Error ? error.message : "下载失败",
        });
    } finally {
      release?.();
      jobControllers.delete(job.id);
    }
  })();
}

/**
 * 校验章节 ID 参数是否为正整数数组。
 * @param chapterIds 待校验的请求参数。
 * @returns 参数合法时返回 true。
 */
export function validChapterIds(chapterIds: unknown) {
  return (
    chapterIds === undefined ||
    (Array.isArray(chapterIds) &&
      chapterIds.every((id) => Number.isInteger(id) && id > 0))
  );
}

/**
 * 创建并立即调度一个后台下载任务。
 * @param req Express 请求，用于读取当前登录会话。
 * @param kind 下载类型（文本或有声）。
 * @param novelId SF 小说编号。
 * @param title 前端显示的作品标题。
 * @param chapterIds 要下载的章节 ID；未传时表示全部章节。
 * @returns 新建的任务对象。
 */
export function createJob(
  req: Request,
  kind: Job["kind"],
  novelId: number,
  title: string | undefined,
  chapterIds: number[] | undefined,
) {
  const id =
    kind === "audio"
      ? `${Date.now()}-audio-${novelId}`
      : `${Date.now()}-${novelId}`;
  const job: Job = {
    id,
    title: title || `小说 ${novelId}`,
    kind,
    novelId,
    chapterIds,
    status: "queued",
    progress: 0,
    message: "等待开始",
  };
  jobs.set(id, job);
  jobSessions.set(id, { cookie: getAuthSession(req)?.cookie, chapterIds });
  run(job);
  return job;
}

/**
 * 获取按创建时间倒序排列的全部内存任务。
 * @returns 全部任务数组。
 */
export function getJobs() {
  return [...jobs.values()].reverse();
}
/**
 * 按 ID 获取任务。
 * @param id 任务 ID。
 * @returns 对应任务，不存在时返回 undefined。
 */
export function getJob(id: string) {
  return jobs.get(id);
}
/**
 * 暂停等待中或正在执行的任务。
 * @param id 要暂停的任务 ID。
 * @returns 更新后的任务，不存在时返回 undefined。
 */
export function pauseJob(id: string) {
  const job = jobs.get(id);
  if (job && (job.status === "queued" || job.status === "downloading")) {
    job.status = "paused";
    job.message = "正在暂停下载";
    jobControllers.get(id)?.abort();
  }
  return job;
}
/**
 * 恢复已暂停的任务。
 * @param id 要恢复的任务 ID。
 * @returns 更新后的任务，不存在时返回 undefined。
 */
export function resumeJob(id: string) {
  const job = jobs.get(id);
  if (job?.status === "paused") {
    job.message = "等待继续";
    run(job);
  }
  return job;
}
/**
 * 取消尚未结束的任务。
 * @param id 要取消的任务 ID。
 * @returns 更新后的任务，不存在时返回 undefined。
 */
export function cancelJob(id: string) {
  const job = jobs.get(id);
  if (job && !["done", "error", "cancelled"].includes(job.status)) {
    job.status = "cancelled";
    job.message = "正在停止下载";
    jobControllers.get(id)?.abort();
  }
  return job;
}
/**
 * 删除任务及其内存状态。
 * @param id 要删除的任务 ID。
 * @returns 找到并删除任务时返回 true。
 */
export function deleteJob(id: string) {
  if (!jobs.has(id)) return false;
  jobControllers.get(id)?.abort();
  jobs.delete(id);
  jobControllers.delete(id);
  jobSessions.delete(id);
  return true;
}
