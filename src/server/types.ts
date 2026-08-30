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
export type AuthSession = { cookie: string; userName: string; nonce?: string };

export type AudioChapter = {
  id: number;
  title: string;
  source: string;
  volume: string;
};

export type NovelDownloadMetadata = {
  novelId?: number;
  title?: string;
  author?: string;
  description?: string;
  downloadedTextChapterIds?: number[];
  downloadedAudioChapterIds?: number[];
};

/** 持久化的文本章节内容，用于断点续传、阅读器和导出。 */
export type StoredTextChapter = {
  id: number;
  volume: string;
  title: string;
  content: string;
  /** 在线目录中的卷序号，用于离线时保持与章节详情一致的顺序。 */
  volumeIndex: number;
  /** 卷内章节序号，用于离线时保持与章节详情一致的顺序。 */
  chapterIndex: number;
};

export type NovelChapterStore = {
  novelId: number;
  chapters: Record<string, StoredTextChapter>;
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
