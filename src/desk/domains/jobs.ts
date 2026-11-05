import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import type { Job, MediaType, Novel } from "../../types";
import type { Notify } from "../../composables/useToast";
import { nativeErrorMessage } from "../../utils/native";

type UseDeskJobsOptions = {
  notify: Notify;
  refreshLibrary: () => Promise<void>;
  refreshLocalBook: () => Promise<void>;
};

const libraryRefreshStatuses = new Set<Job["status"]>([
  "done",
  "error",
  "paused",
  "cancelled",
]);

export function createJobsDesk({
  notify,
  refreshLibrary,
  refreshLocalBook,
}: UseDeskJobsOptions) {
  const jobs = ref<Job[]>([]);
  const queueOpen = ref(false);
  let stopJobListener: (() => void) | undefined;
  let stopLibraryListener: (() => void) | undefined;
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

  function refreshLibraryViews() {
    void refreshLibrary();
    void refreshLocalBook();
  }

  /**
   * 按内容形态创建下载任务并入队。
   *
   * 三种形态的上游命令参数不同，但「创建 → 入队 → 刷新」是同一段流程，调用方
   * （章节选择弹窗）只需要给出作品与章节编号。
   */
  async function createDownload(
    mode: MediaType,
    novel: Novel,
    chapterIds: number[],
  ) {
    const job =
      mode === "novel"
        ? await invoke<Job>("create_text_download", {
            novelId: novel.novelId,
            title: novel.novelName,
            chapterIds,
          })
        : mode === "audio"
          ? await invoke<Job>("create_audio_download", {
              novelId: novel.novelId,
              albumId: novel.mediaId,
              title: novel.novelName,
              chapterIds,
            })
          : await invoke<Job>("create_comic_download", {
              comicId: novel.novelId,
              sourcePath: novel.sourcePath,
              title: novel.novelName,
              chapterIds,
            });
    upsertJob(job);
    notify(`已将《${novel.novelName}》加入下载队列`);
    void refreshJobs();
    return job;
  }

  async function refreshJobs() {
    try {
      jobs.value = await invoke<Job[]>("list_download_jobs");
      if (jobs.value.some((job) => libraryRefreshStatuses.has(job.status)))
        refreshLibraryViews();
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
      notify(nativeErrorMessage(error, "暂停下载失败"), "error");
    }
  }

  async function resumeJob(job: Job) {
    try {
      await invoke<Job>("resume_download_job", { jobId: job.id });
      job.status = "downloading";
      job.message = "正在继续下载";
      void refreshJobs();
    } catch (error) {
      notify(nativeErrorMessage(error, "继续下载失败"), "error");
    }
  }

  async function deleteJob(job: Job) {
    try {
      await invoke<void>("delete_download_job", { jobId: job.id });
      jobs.value = jobs.value.filter((item) => item.id !== job.id);
    } catch (error) {
      notify(nativeErrorMessage(error, "删除任务失败"), "error");
    }
  }

  onMounted(() => {
    void listen<Job>("download-progress", (event) => {
      upsertJob(event.payload);
      if (libraryRefreshStatuses.has(event.payload.status)) refreshLibraryViews();
    }).then((unlisten) => {
      if (disposed) unlisten();
      else stopJobListener = unlisten;
    }).catch(() => {
      // The native event bridge is unavailable in browser-only previews.
    });
    void listen<Job>("download-library-update", (event) => {
      upsertJob(event.payload);
      refreshLibraryViews();
    }).then((unlisten) => {
      if (disposed) unlisten();
      else stopLibraryListener = unlisten;
    }).catch(() => {
      // The native event bridge is unavailable in browser-only previews.
    });
    void refreshJobs();
  });

  onBeforeUnmount(() => {
    disposed = true;
    stopJobListener?.();
    stopLibraryListener?.();
  });

  return {
    jobs,
    queueOpen,
    runningJobs,
    libraryJobs,
    upsertJob,
    createDownload,
    refreshJobs,
    pauseJob,
    resumeJob,
    deleteJob,
  };
}

/** 该域对外暴露的完整上下文；页面只应通过 `useJobs` 取用。 */
export type JobsDesk = ReturnType<typeof createJobsDesk>;
