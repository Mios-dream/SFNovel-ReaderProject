/** 主界面导航项。 */
export type ViewName =
  | "discover"
  | "bookshelf"
  | "remoteDetail"
  | "library"
  | "libraryDetail"
  | "reader"
  | "comicReader"
  | "audioPlayer";
export type ChapterMode = "text" | "audio" | "comic";

export type Novel = {
  novelId: number;
  /** Source-specific ID: an album ID for audio and comic ID for comics. */
  mediaId?: number;
  novelName: string;
  authorName: string;
  novelCover: string;
  lastUpdateTime: string;
  bookshelfName?: string;
  bookshelfType?: "novel" | "audio" | "comic";
  /** Public web path used to open a comic without App identity lookup. */
  sourcePath?: string;
  categoryName?: string;
  typeId?: number;
  description?: string;
  isFinish?: boolean;
  typeName?: string;
  tags?: string[];
  score?: number;
  chapterCount?: number;
  characterCount?: number;
  viewCount?: number;
  markCount?: number;
  pointCount?: number;
  favoriteCount?: number;
  ticketCount?: number;
  latestChapterTitle?: string;
  latestChapterTime?: string;
};

export type Job = {
  id: string;
  title: string;
  kind: "text" | "audio" | "comic";
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
    isVip: boolean;
    /** Web-visible text shape; this is not an entitlement result. */
    contentKind: "text" | "imageVip" | "encryptedVip" | "unknown";
    /** Only chapter-resource requests can change this from unknown. */
    accessState: "unknown" | "available" | "unavailable" | "sessionExpired";
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
export type LocalComicChapter = {
  id: number;
  title: string;
  cover?: string;
  pages: string[];
};
export type LocalAudioTrack = { title: string; href: string };
export type LocalWorkMetadata = {
  id: number;
  catalogId?: number;
  onlinePath?: string;
  title: string;
  author: string;
  description: string;
  typeName?: string;
  tags: string[];
  isFinished?: boolean;
  score?: number;
  chapterCount?: number;
  characterCount?: number;
  viewCount?: number;
  markCount?: number;
  pointCount?: number;
  favoriteCount?: number;
  ticketCount?: number;
  allowDownload?: boolean;
  latestChapterTitle?: string;
  latestChapterTime?: string;
  lastUpdateTime?: string;
  cover?: string;
};
export type LocalBookDetail = {
  name: string;
  novel?: LocalWorkMetadata;
  audio?: LocalWorkMetadata;
  comic?: LocalWorkMetadata;
  imageDirectory: string;
  audioTracks: LocalAudioTrack[];
  epubHref?: string;
  chapterVolumes: LocalChapterVolume[];
  comicChapters: LocalComicChapter[];
};
export type LocalChapterContent = LocalChapter & { content: string };

export type AuthStatus = {
  authenticated: boolean;
  appAuthenticated: boolean;
  webAuthenticated: boolean;
  userName?: string;
};
export type BrowserLoginStatus = AuthStatus & { waiting?: boolean };
export type UserProfile = {
  accountId: number;
  nickName: string;
  avatar: string;
  appDetailsAvailable: boolean;
  webDetailsAvailable: boolean;
  vipDetailsAvailable: boolean;
  vipSystem: "new" | "legacy";
  welfareCoin: number;
  fireMoneyRemain: number;
  couponsRemain: number;
  monthlyTicket: number;
  vipLevel: number;
  vipName: string;
};
/** 下载请求限流设置，与服务端 config.json 的 requestPolicy 对应。 */
export type RequestPolicy = {
  requestIntervalMs: number;
  maxConcurrentDownloads: number;
  appFallbackEnabled: boolean;
  androidDeviceReportEnabled: boolean;
};
