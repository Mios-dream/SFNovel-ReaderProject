import { Base } from "../../config";

export interface SfacgOption extends Base {
  sfacgAppName?: string;
  epubMake?: boolean;
  saveAccount?: boolean;
}

export interface IaccountInfo {
  userName?: string; // 用户名
  passWord?: string; // 密码
  nickName?: string; // 昵称
  avatar?: string; // 头像网址
  lastCheckIn?: string; //上次签到时间\
  vipLevel?: number; // vip等级
  fireMoneyRemain?: number; // 未使用的火币
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

// 卷列表
export interface IvolumeInfos {
  volumeId: number; // 卷ID
  title: string; // 标题
  chapterList: Ichapter[]; // 章节列表
}
[];

// 章节信息
export interface Ichapter {
  chapId: number; // 章节ID
  needFireMoney: number; // 所需火币
  charCount: number; // 字符数
  chapOrder: number; // 章节顺序
  isVip: boolean; // 是否为VIP章节
  ntitle: string; // 新标题
}

export interface IadBonusNum {
  taskId: number; // 任务ID
  requireNum: number; // 未完成看广告领奖次数
  completeNum: number; // 已完成看广告领奖次数
}
