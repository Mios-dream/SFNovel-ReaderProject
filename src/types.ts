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
export type ReaderSource = "local" | "online";

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
  /** Source path retained for library identity and metadata. */
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

export type DiscoverCategory = {
  typeId: number;
  typeName: string;
};

export type DiscoverFeed = {
  featured?: Novel;
  ranking: Novel[];
  recent: Novel[];
  categories: DiscoverCategory[];
};

export type BookComment = {
  commentId: number;
  title?: string;
  userName: string;
  avatar: string;
  content: string;
  createdAt: string;
  replyCount: number;
  likeCount: number;
  roleName: string;
  isStick: boolean;
  extType?: number;
  ticketNum?: number;
};

export type BookCommentPage = {
  comments: BookComment[];
  highlights?: BookComment[];
  page: number;
  hasMore: boolean;
};

export type BookCommentReply = {
  replyId: number;
  userName: string;
  avatar: string;
  content: string;
  createdAt: string;
  likeCount: number;
  repliedUserName?: string;
};

export type BookCommentReplyPage = {
  replies: BookCommentReply[];
  page: number;
  hasMore: boolean;
};

export type Job = {
  id: string;
  title: string;
  /** 后端 `MediaKind` 的序列化名：文字是 `text`，不是 `MediaType` 的 `novel`。 */
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
  /** `unknown` means the catalog only identifies a VIP chapter, not entitlement. */
  isUnlocked?: boolean | "unknown";
  downloaded?: boolean;
};

export type ChapterVolume = {
  volumeId: number;
  title: string;
  chapters: Array<{
    chapId: number;
    title: string;
    isVip: boolean;
    needFireMoney: number;
    /** 后端只产出可读与不可读两种结果，字段本身即为权限判断依据。 */
    accessState: "available" | "unavailable";
    downloaded: boolean;
  }>;
};

export type Book = {
  name: string;
  author?: string;
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

export type OnlineTextChapter = {
  id: number;
  title: string;
  volume: string;
  content: string;
  images: string[];
};

export type OnlineComicChapter = {
  id: number;
  title: string;
  pages: string[];
};

export type OnlineAudioTrack = {
  id: number;
  title: string;
  volume: string;
  /** 原生媒体协议的流式播放地址。 */
  url: string;
};

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
  vipDetailsAvailable: boolean;
  vipSystem: "new" | "legacy";
  fireMoneyRemain: number | null;
  couponsRemain: number | null;
  monthlyTicket: number | null;
  vipLevel: number;
  vipName: string;
};
export type SigninDay = {
  year: number;
  month: number;
  day: number;
};
export type SigninPrize = {
  name: string;
  num: number;
};
export type SigninContinueDays = {
  totalContinueDays: number;
  currentMonthContinueDays: number;
  maxContinueDays: number;
};
export type SigninWeekItem = {
  day: number;
  date: string;
  weekday: string;
  isSignIn: boolean;
  rewards: SigninPrize[];
};
export type SigninReward = {
  recordId: number;
  configId: number;
  typeName: string;
  day: number;
  unit: string;
  status: number;
  isFinish: boolean;
  rewards: SigninPrize[];
};
export type SigninOverview = {
  continueDays: SigninContinueDays;
  week: SigninWeekItem[];
  rewards: SigninReward[];
};
/** 自动签到最近一次结果；`pending` 表示尚未向用户提示。 */
export type SigninNotice = {
  day: string;
  success: boolean;
  message: string;
  pending: boolean;
};
/** 下载请求与过程文件设置。 */
export type DownloadSettings = {
  requestIntervalMs: number;
  maxConcurrentDownloads: number;
  ocrProcessFilesEnabled: boolean;
};
export type StorageSettings = {
  useExternalStorage: boolean;
};
export type DesktopSettings = {
  closeToTray: boolean;
  autostart: boolean;
};
export type SigninSettings = {
  autoSigninEnabled: boolean;
};
export type UpdateSettings = {
  autoCheck: boolean;
};
/** 应用设置按功能分类，持久化到单个 settings.json。 */
export type AppSettings = {
  download: DownloadSettings;
  storage: StorageSettings;
  desktop: DesktopSettings;
  signin: SigninSettings;
  updates: UpdateSettings;
};

/**
 * 导出格式，与后端 `export_book` 命令的 `format` 参数一一对应。
 *
 * `epub` / `markdown` / `txt` 只适用于文字作品，`audio` / `comic` 分别对应
 * 有声与漫画作品的打包导出。
 */
export type ExportFormat = "epub" | "markdown" | "txt" | "audio" | "comic";

/** 作品的内容形态：文字小说、有声、漫画。 */
export type MediaType = "novel" | "audio" | "comic";

/** 阅读器的排版方式：纵向滚动或分页。 */
export type DisplayMode = "vertical" | "paged";
