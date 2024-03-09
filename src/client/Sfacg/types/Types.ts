// 小说章节内容信息
export interface contentInfos {
  chapId: number; // 章节ID
  novelId: number; // 小说ID
  volumeId: number; // 卷ID
  charCount: number; // 字符数
  rowNum: number; // 行数
  chapOrder: number; // 章节顺序
  title: string; // 标题
  addTime: string; // 添加时间
  updateTime: string; // 更新时间
  sno: number; // 序列号
  isVip: boolean; // 是否为VIP章节
  expand: contentInfos_Expand; // 扩展信息
  ntitle: string; // 新标题
  isRubbish: boolean; // 是否为废弃内容
  auditStatus: number; // 审核状态
}

// 小说章节内容的扩展信息
export interface contentInfos_Expand {
  needFireMoney: number; // 所需火币
  originNeedFireMoney: number; // 原始所需火币
  content: string; // 内容
  tsukkomi: any; // 吐槽信息
  chatLines: any; // 聊天线
  paragraphs: any; // 段落
  volume: any; // 卷信息
  authorTalk: any; // 作者说
  isContentEncrypted: boolean; // 内容是否加密
  isBranch: boolean; // 是否为分支章节
}

// 小说信息
export interface novelInfo {
  authorId: number; // 作者ID
  lastUpdateTime: string; // 最后更新时间
  markCount: number; // 标记数
  novelCover: string; // 小说封面
  bgBanner: string; // 背景横幅
  novelId: number; // 小说ID
  novelName: string; // 小说名称
  point: number; // 分数
  isFinish: boolean; // 是否已完结
  authorName: string; // 作者名称
  charCount: number; // 字符数
  viewTimes: number; // 查看次数
  typeId: number; // 类型ID
  allowDown: boolean; // 是否允许下载
  addTime: string; // 添加时间
  isSensitive: boolean; // 是否敏感
  signStatus: string; // 签名状态
  categoryId: number; // 类别ID
  expand: novelInfo_Expand; // 扩展信息
}

// 小说信息的扩展信息
export interface novelInfo_Expand {
  intro: string; // 简介
  typeName: string; // 类型名称
  sysTags: SysTags[]; // 系统标签
}

// 系统标签
export interface SysTags {
  sysTagId: number; // 系统标签ID
  tagName: string; // 标签名称
}

// 用户信息
export interface userInfo {
  userName: string; // 用户名
  countryCode: number; // 国家代码
  nickName: string; // 昵称
  email: string; // 邮箱
  accountId: number; // 账户ID
  roleName: string; // 角色名称
  fireCoin: number; // 火币数量
  avatar: string; // 头像
  isAuthor: boolean; // 是否作者
  phoneNum: string; // 电话号码
  registerDate: string; // 注册日期
}

// 卷信息
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
// 书架信息
export interface bookshelfInfos {
  accountId: number; // 账号ID
  pocketId: number; // 口袋ID
  name: string; // 书架名称
  typeId: number; // 类型ID
  createTime: string; // 创建时间
  isFull: boolean; // 是否已满
  canModify: boolean; // 是否可以修改
  expand: _Expand; // 扩展信息
}

// 分类信息
export interface categories {
  accountId: number; // 账号ID
  pocketId: number; // 口袋ID
  name: string; // 分类名称
  typeId: number; // 类型ID
  createTime: string; // 创建时间
  isFull: boolean; // 是否已满
  canModify: boolean; // 是否可以修改
  expand: _Expand; // 扩展信息
}

// _开头表示被categories和bookshelfInfos共享的扩展信息，
export interface _Expand {
  albums: _album[]; // 专辑数组
  novels: _novel[]; // 小说数组
}

// 专辑信息
interface _album {
  albumId: number; // 专辑ID
  novelId: number; // 小说ID
  authorId: number; // 作者ID
  latestChapterId: number; // 最新章节ID
  visitTimes: number; // 访问次数
  name: string; // 专辑名称
  lastUpdateTime: string; // 最后更新时间
  coverBig: string; // 大封面
  coverSmall: string; // 小封面
  coverMedium: string; // 中封面
  isFinished: boolean; // 是否完结
  expand: any; // 更多扩展信息
  isSticky: boolean; // 是否置顶
  stickyDateTime: any; // 置顶时间
  markDateTime: string; // 标记时间
}

// 小说信息
export interface _novel {
  authorId: number; // 作者ID
  lastUpdateTime: string; // 最后更新时间
  markCount: number; // 标记次数
  novelCover: string; // 小说封面
  bgBanner: string; // 背景横幅
  novelId: number; // 小说ID
  novelName: string; // 小说名称
  point: number; // 分数
  isFinish: boolean; // 是否完结
  authorName: string; // 作者名称
  charCount: number; // 字符数
  viewTimes: number; // 浏览次数
  typeId: number; // 类型ID
  allowDown: boolean; // 是否允许下载
  signStatus: string; // 签名状态
  categoryId: number; // 类别ID
  isSensitive: boolean; // 是否敏感内容
  expand: any; // 更多扩展信息
  isSticky: boolean; // 是否置顶
  stickyDateTime: any; // 置顶时间
  markDateTime: string; // 标记时间
}
















