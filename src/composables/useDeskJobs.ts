import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import type { Job } from "../types";
import type { Notify } from "./deskShared";

type UseDeskJobsOptions = {
  notify: Notify;
  refreshLibrary: () => Promise<void>;
};

export function useDeskJobs({ notify, refreshLibrary }: UseDeskJobsOptions) {
  const jobs = ref<Job[]>([]);
  let stopJobListener: (() => void) | undefined;
  let disposed = false;

  const runningJobs = computed(() =>
    jobs.value.filter((job) =>
      ["downloading", "queued", "paused"].includes(job.status),
    ),
  );
  const libraryJobs = computed(() =>
    jobs.value.filter((job) =>
      ["queued", "downloading", "paused"].includes(job.status),
    ),
  );

  function upsertJob(job: Job) {
    const index = jobs.value.findIndex((item) => item.id === job.id);
    if (index < 0) jobs.value.unshift(job);
    else jobs.value[index] = job;
  }

  async function refreshJobs() {
    try {
      jobs.value = await invoke<Job[]>("list_download_jobs");
      if (jobs.value.some((job) => job.status === "done")) void refreshLibrary();
    } catch {
      /* Native runtime may be restarting during development. */
    }
  }

  async function pauseJob(job: Job) {
    try {
      await invoke<Job>("pause_download_job", { jobId: job.id });
      job.status = "paused";
      job.message = "已暂停，可继续下载";
    } catch (error) {
      notify(error instanceof Error ? error.message : "暂停下载失败");
    }
  }

  async function resumeJob(job: Job) {
    try {
      await invoke<Job>("resume_download_job", { jobId: job.id });
      job.status = "downloading";
      job.message = "正在继续下载";
      void refreshJobs();
    } catch (error) {
      notify(error instanceof Error ? error.message : "继续下载失败");
    }
  }

  async function deleteJob(job: Job) {
    try {
      await invoke<void>("delete_download_job", { jobId: job.id });
      jobs.value = jobs.value.filter((item) => item.id !== job.id);
    } catch (error) {
      notify(error instanceof Error ? error.message : "删除任务失败");
    }
  }

  onMounted(() => {
    void listen<Job>("download-progress", (event) => {
      upsertJob(event.payload);
      if (event.payload.status === "done") void refreshLibrary();
    }).then((unlisten) => {
      if (disposed) unlisten();
      else stopJobListener = unlisten;
    }).catch(() => {
      // The native event bridge is unavailable in browser-only previews.
    });
    void refreshJobs();
  });

  onBeforeUnmount(() => {
    disposed = true;
    stopJobListener?.();
  });

  return {
    jobs,
    runningJobs,
    libraryJobs,
    upsertJob,
    refreshJobs,
    pauseJob,
    resumeJob,
    deleteJob,
  };
}
