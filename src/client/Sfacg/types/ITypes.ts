export interface Option {
  saveAccount?: boolean; // 是否保存账号
  orderedChaps?: boolean; // 是否购买未购章节
  epubMake?: boolean; // 是否打包成EPUB
}

export interface IaccountInfo {
  userName: string; // 用户名
  passWord?: string; // 密码
  nickName?: string; // 昵称
  lastCheckIn?: string; //上次签到时间
  avatar?: string; // 头像网址
  fireMoneyRemain?: number; // 未使用的火币
  vipLevel?: number; // vip等级
  couponsRemain?: number; // 未使用的代币
}

export interface Itag {
  id: number; // 标签序号
  name: string; // 标签名
}
[];

export interface IcontentInfos {
  title: string; // 标题
  content: string | Buffer; // 内容
}

export interface InovelInfo {
  authorId: number; // 作者ID
  lastUpdateTime: string; // 最后更新时间
  novelCover: string; // 小说封面
  bgBanner: string; // 背景横幅
  novelName: string; // 小说名称
  isFinish: boolean; // 是否已完结
  authorName: string; // 作者名称
  charCount: number; // 字符数
  intro: string; // 简介
  tags: string[]; //标签
}

// 小说目录详情
export interface volumeInfos {
  novelId: number; // 小说ID
  lastUpdateTime: string; // 最后更新时间
  volumeList: VolumeList[]; // 卷列表
}

// 卷列表
export interface VolumeList {
  volumeId: number; // 卷ID
  title: string; // 标题
  sno: number; // 序列号
  chapterList: chapter[]; // 章节列表
}

// 章节信息
export interface chapter {
  chapId: number; // 章节ID
  novelId: number; // 小说ID
  volumeId: number; // 卷ID
  needFireMoney: number; // 所需火币
  originNeedFireMoney: number; // 原始所需火币
  chapterOriginFireMoney: number; // 章节原始火币
  charCount: number; // 字符数
  rowNum: number; // 行数
  chapOrder: number; // 章节顺序
  title: string; // 标题
  content: any; // 内容
  sno: number; // 序列号
  isVip: boolean; // 是否为VIP章节
  AddTime: string; // 添加时间
  updateTime: any; // 更新时间
  canUnlockWithAd: boolean; // 是否可以通过广告解锁
  ntitle: string; // 新标题
  isRubbish: boolean; // 是否为废弃章节
  auditStatus: number; // 审核状态
}
