export type ViewName = "discover" | "bookshelf" | "library";
export type ChapterMode = "text" | "audio";

export type Novel = {
  novelId: number;
  novelName: string;
  authorName: string;
  novelCover: string;
  lastUpdateTime: string;
  bookshelfName?: string;
  bookshelfType?: "novel" | "audio" | "comic";
  categoryName?: string;
  typeId?: number;
  description?: string;
  isFinish?: boolean;
  typeName?: string;
};

export type Job = {
  id: string;
  title: string;
  kind: "text" | "audio";
  status: "queued" | "downloading" | "paused" | "done" | "error" | "cancelled";
  progress: number;
  message: string;
  file?: string;
  novelId?: number;
};

export type Chapter = {
  id: number;
  title: string;
  volume: string;
  needFireMoney?: number;
  isVip?: boolean;
  isUnlocked?: boolean;
  downloaded?: boolean;
};

export type ChapterVolume = {
  volumeId: number;
  title: string;
  chapters: Array<{ chapId: number; title: string; needFireMoney: number; isVip: boolean; isUnlocked: boolean; downloaded: boolean }>;
};

export type Book = {
  name: string;
  novelId?: number;
  href?: string;
  audioHref?: string;
  cover?: string;
  updatedAt: string;
};

export type AuthStatus = { authenticated: boolean; userName?: string };
export type BrowserLoginStatus = AuthStatus & { waiting?: boolean };
export type RequestPolicy = { requestIntervalMs: number; maxConcurrentDownloads: number };
