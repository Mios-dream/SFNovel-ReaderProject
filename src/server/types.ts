export type JobStatus =
  | "queued"
  | "downloading"
  | "paused"
  | "done"
  | "error"
  | "cancelled";

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
