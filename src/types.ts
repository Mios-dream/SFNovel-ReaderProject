/** 主界面导航项。 */
export type ViewName =
  | "discover"
  | "bookshelf"
  | "library"
  | "libraryDetail"
  | "reader"
  | "audioPlayer";
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
  chapters: Array<{
    chapId: number;
    title: string;
    needFireMoney: number;
    isVip: boolean;
    isUnlocked: boolean;
    downloaded: boolean;
  }>;
};

export type Book = {
  name: string;
  novelId?: number;
  href?: string;
  audioHref?: string;
  epubHref?: string;
  cover?: string;
  updatedAt: string;
  formats: { text: boolean; audio: boolean; comic: boolean };
};

export type LocalChapter = { id: number; title: string; volume: string };
export type LocalChapterVolume = {
  volume: string;
  chapters: Array<Omit<LocalChapter, "volume">>;
};
export type LocalAudioTrack = { title: string; href: string };
export type LocalBookDetail = {
  name: string;
  novelId?: number;
  author: string;
  description: string;
  cover?: string;
  audioTracks: LocalAudioTrack[];
  epubHref?: string;
  chapterVolumes: LocalChapterVolume[];
};
export type LocalChapterContent = LocalChapter & { content: string };

export type AuthStatus = { authenticated: boolean; userName?: string };
export type BrowserLoginStatus = AuthStatus & { waiting?: boolean };
export type UserProfile = {
  accountId: number;
  nickName: string;
  avatar: string;
  welfareCoin: number;
  fireMoneyRemain: number;
  couponsRemain: number;
  vipLevel: number;
};
/** 下载请求限流设置，与服务端 config.json 的 requestPolicy 对应。 */
export type RequestPolicy = {
  requestIntervalMs: number;
  maxConcurrentDownloads: number;
};
