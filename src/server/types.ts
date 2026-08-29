/** 后台下载任务的生命周期状态。 */
export type JobStatus =
  | "queued"
  | "downloading"
  | "paused"
  | "done"
  | "error"
  | "cancelled";

/** 返回给前端的下载任务及进度信息。 */
export type Job = {
  id: string;
  title: string;
  status: JobStatus;
  progress: number;
  message: string;
  file?: string;
  kind: "text" | "audio";
  novelId: number;
  chapterIds?: number[];
};

/** 通过 HttpOnly 会话 Cookie 传递的 SF 登录信息。 */
export type AuthSession = { cookie: string; userName: string };

export type AudioChapter = {
  id: number;
  title: string;
  source: string;
  volume: string;
};

export type NovelDownloadMetadata = {
  novelId?: number;
  title?: string;
  downloadedTextChapterIds?: number[];
  downloadedAudioChapterIds?: number[];
};

export type AudioInfoResponse = {
  status?: number;
  msg?: string;
  data?: {
    NovelName?: string;
    VolumeSet?: Array<{
      VolumeName?: string;
      AudioSet?: Array<{
        AudioID?: number;
        ChapterTitle?: string;
        AudioSrc?: string;
      }>;
    }>;
  };
};
