export type SfacgChapter = {
  chapId: number;
  needFireMoney: number;
  isVip: boolean;
  ntitle: string;
  chapOrder: number;
  volumeId: number;
  has?: boolean;
};

export type SfacgVolume = {
  volumeId: number;
  novelId: number;
  title: string;
  chapterList: SfacgChapter[];
};

export type SfacgNovel = {
  novelId: number;
  novelName: string;
  authorName: string;
  novelCover?: string;
  lastUpdateTime?: string;
  isFinish?: boolean;
  expand?: { intro?: string; typeName?: string };
};

export type SfacgSearchItem = {
  authorName: string;
  lastUpdateTime: string;
  novelCover: string;
  novelId: number;
  novelName: string;
  bookshelfName?: string;
  bookshelfType?: "novel" | "audio" | "comic";
  typeId?: number;
};

export type SfacgBookshelfCollection = {
  categories: string[];
  items: SfacgSearchItem[];
};

/** 当前已登录 SF 账号可公开展示的资料与余额。 */
export type SfacgUserProfile = {
  accountId: number;
  nickName: string;
  avatar: string;
  welfareCoin: number;
  fireMoneyRemain: number;
  couponsRemain: number;
  vipLevel: number;
};

export type UpstreamUserInfo = {
  accountId?: number;
  nickName?: string;
  avatar?: string;
  expand?: { welfareCoin?: number };
};

export type UpstreamUserMoney = {
  fireMoneyRemain?: number;
  couponsRemain?: number;
  vipLevel?: number;
};

type UpstreamBook = Partial<SfacgSearchItem> & {
  comicId?: number;
  comicName?: string;
  comicCover?: string;
  name?: string;
  coverBig?: string;
  coverMedium?: string;
  coverSmall?: string;
};

export type UpstreamSearchResponse = {
  novels?: UpstreamBook[];
  albums?: UpstreamBook[];
  comics?: UpstreamBook[];
};

export type UpstreamBookshelf = {
  name?: string;
  novelId?: number;
  expand?: { novels?: UpstreamBook[]; albums?: UpstreamBook[]; comics?: UpstreamBook[] };
} & UpstreamBook;

export type UpstreamVolume = {
  volumeId: number;
  title: string;
  chapterList: Array<Omit<SfacgChapter, "volumeId" | "has"> & { has?: boolean }>;
};
