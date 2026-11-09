# 前端重构方案

本文件是前端重构的唯一依据。改动前先读「不变量」，改动后按「验证矩阵」逐项核对。

## 1. 判据

状态归属只按一个问题判断：**这个值变化时，UI 需不需要重画**。再按生命周期与消费方收窄范围。

| 状态形态 | 存放位置 | 本项目实例 |
| --- | --- | --- |
| 无状态纯计算 | 普通模块函数 | `utils/format.ts`、`reader/chapterParts.ts`、`utils/dominantColor.ts` |
| 有状态、不驱动渲染 | 单例 class 或模块级实例 | `reader/pagedEngine.ts::PagedEngine`、`desk/navigation.ts` 的返回视图 |
| 有状态、单视图渲染 | 组件内 `ref` | 详情页 `directoryRows`、`exportingFormat` 之外的临时态 |
| 跨页面共享 | provider 提供的域上下文 | 书库、书架、下载队列、登录态、在线阅读会话 |

推论：

- 「多个页面都读」不等于「要有双向 store」。只读数据用缓存/订阅，不写回。
- desk 是**组合根**，职责只有三件：创建跨路由存活的实例、装配域模块、提供导航。它不实现业务规则。
- 域的边界按**谁拥有这份状态**划，不按页面划。跨域流程单独成层（`useWorkspace`），允许依赖全部域。

## 2. 现状问题

| 编号 | 问题 | 证据 |
| --- | --- | --- |
| F-01 | 扁平展开导致同名键互相遮蔽 | `useNovelDesk` 先 `...chapterPicker` 后 `...online`，`audioChapters`/`comicChapters`/`chapterLoading` 三个键被 online 覆盖；`AppOverlays` 把它们传给 `ChapterPickerModal`，章节选择弹窗拿到的是在线阅读的目录 |
| F-02 | 导航双真相 | `desk.active` 与 `route.path` 双向同步：`App.vue` 两个 watch 共 60 行；页面同时 `desk.navigate(x)` 和 `router.push(...)` 各跳一次 |
| F-03 | 关键流程依赖反向 watcher | `openRemoteBookDetail`、`startOnlineReading`、`startOnlineFromLibrary` 只改 `active`，由 `App.vue` 的 watch 补发 `router.push` |
| F-04 | 泛名字段污染全局命名空间 | `status`、`info`、`error`、`modalOpen`、`loading` 直接挂在 desk 根上 |
| F-05 | 非状态混入 desk | `formatDate` 是纯函数，与 desk 无关 |
| F-06 | `useNovelDesk` 职责过载 | 577 行，同时是组合根、远端会话实现（缓存/去重/收藏/媒体变体）、跨域流程 |
| F-07 | 会话状态双重记录 | `remoteDetailSessionActive` 与 `active === "remoteDetail"` 表达同一件事 |
| F-08 | 无谓的响应式包装 | `reactive(deskState)` 只为两个 watch 存在，provide 的却是原始对象 |

## 3. 第二轮问题

第一轮完成后复查到的遗留问题，均已解决：

| 编号 | 问题 | 证据 |
| --- | --- | --- |
| F-09 | 层次倒置 | `composables/useBookDiscussions.ts`、`components/BookCommentDetail.vue` 反向 import `desk/shared` 取错误文案 |
| F-10 | 设置双份真相 | `SigninModal` 自己 `get_settings`/`save_settings`，`settings` 域另有一份 `settings` ref，互不同步 |
| F-11 | 装配层写业务 | `provider` 把 online 的目录快照翻译成 `{volumes}`/`{chapters}`；picker 内部又保留一套裸 `invoke` 兜底 |
| F-12 | 聚合点过宽 | `useWorkspace` 注入 9 个域，`go()` 同时做会话清理、来源标记、加载前置、路由跳转 |
| F-13 | 业务留在组件 | 签到四个命令与自动签到设置全在 `SigninModal.vue`（878 行），`auth` 域只有开关 |
| F-14 | 导航映射两份 | `PageFrame` 自行判断 `route.path` 决定侧栏高亮，与 `navigation.currentView()` 重复 |
| F-15 | 三种媒态重复取数 | 文本/漫画/音频的「查缓存 → 复用请求 → 写缓存」各写一遍 |

第三轮（手测通过后复查）只剩两处，均已解决：

| 编号 | 问题 | 证据 |
| --- | --- | --- |
| F-16 | 错误原因被丢弃 | 13 处写 `error instanceof Error ? error.message : "…"`。`AppError` 序列化后是**字符串**，`instanceof` 恒为假，于是上游原因（含风控、缺文件、鉴权）被替换成通用兜底文案 |
| F-17 | 错误文案规则两份 | `utils/native.ts::nativeErrorMessage` 与 `composables/useToast.ts::errorMessage` 逐字重复，后者还被导出但无人引用 |
| F-18 | 无人写入的返回状态 | `returnViews.libraryDetail` 全项目只有读、没有写，`backFromLibraryDetail()` 因此恒返回 `"library"`，却仍被当作可变状态从 provider 注入 |
| F-19 | 导航正反两份映射 | `navigation.ts` 自己维护 `STATIC_PATHS`（视图→路径）与 `currentView` 的 `if` 链（路径→视图），新增视图要改两处 |
| F-20 | 同一对象构造两遍 | `workspace` 里「本地清单→远端作品条目」的两段字面量除 `lastUpdateTime` 外逐字相同 |
| F-21 | 残留死样式 | `AuthModal` 的 `.logout-all-button`（含一条整块规则）、`ChapterPickerModal` 的 `.close-button` 模板从未使用 |

## 4. 目标结构

```
src/desk/
  index.ts              对外唯一入口：各域 hook 与门面类型
  keys.ts               每个域的 InjectionKey
  provider.ts           组合根：创建域、注入依赖、provide、注册生命周期
  lifecycle.ts          应用级副作用：启动初始化、返回事件接管、复制拦截
  backLayers.ts         浮层返回栈：打开时注册、只弹最上层（模块级单例）
  navigation.ts         路由映射、返回层识别、评论页跳转、返回视图记忆
  domains/
    auth.ts              登录态、弹窗、账户资料、音频播放门禁
    library.ts           本地书库、书籍详情、章节内容、导出
    jobs.ts              下载任务、队列弹窗、下载任务创建
    bookshelf.ts         远端书架、分类、分页
    search.ts            搜索词与结果
    settings.ts          设置项、内容字典、自动签到开关（唯一真相）
    signin.ts            签到、连签奖励
    updater.ts           版本与更新
    online.ts            在线阅读会话：目录快照、三种媒态的章节取数
    picker.ts            章节选择弹窗与选择集
    remote.ts            远端详情会话、收藏、媒体变体、评论取数
    workspace.ts         跨域流程、导航与唯一返回入口 `back()`（不触达后端）

src/utils/
  native.ts             本地资源地址、原生错误文案
  date.ts               北京日历日与日期键
```

依赖方向（只允许向下）：

```
utils/*  ·  composables/useToast          无业务依赖，可被任意层引用
        ↓
domains/*  ←  workspace                  域之间互不 import；workspace 只依赖
        ↓                                  library bookshelf online picker remote
provider  →  lifecycle                   只装配与挂载，不实现业务
```

`domains/*` 之间只通过构造参数互通，禁止互相 import。`workspace` 是唯一可以横向组合多个域的域，且只组合它真正需要的 5 个；`provider` 可接触全部域，`lifecycle` 只装配启动与返回事件所需的 6 个。`backLayers.ts` 是模块级单例，可被组件与 `workspace` 同时引用，不属于任何域。

### 域职责

| 域 | 拥有的状态 | 对外 hook | 消费方 |
| --- | --- | --- | --- |
| auth | 登录态、凭证/登录/账户/签到弹窗开关、账户资料、音频播放门禁 | `useAuth` | PageFrame、AppOverlays、阅读页 |
| library | 本地书库列表、书籍详情、章节内容、导出 | `useLibrary` | LibraryPage、LocalBookDetailPage、阅读页 |
| jobs | 下载任务、队列弹窗开关、按媒态创建下载任务 | `useJobs` | AppOverlays、PageFrame、LibraryPage |
| bookshelf | 远端书架、分类、分页 | `useBookshelf` | BookshelfPage、PageFrame |
| search | 搜索词、结果、加载态 | `useSearch` | DiscoverPage |
| settings | 设置项、设置弹窗、内容字典、自动签到开关 | `useSettings` | AppOverlays、PageFrame、signin |
| signin | 签到总览、连签奖励、领取与签到动作 | `useSignin` | SigninModal |
| updater | 版本、更新状态、更新弹窗 | `useUpdater` | AppOverlays |
| online | 在线阅读会话（目录、章节内容、音轨） | `useOnline` | ReaderPage、ComicReaderPage、AudioPlayerPage、RemoteBookDetailPage |
| picker | 章节选择弹窗与选择集 | `usePicker` | AppOverlays、BookshelfPage、RemoteBookDetailPage |
| remote | 远端详情会话、收藏、媒体变体、评论/回复分页取数 | `useRemote` | RemoteBookDetailPage、RemoteBookCommentsPage、useBookDiscussions |
| workspace | 导航、侧栏入口、阅读来源、跨域流程 | `useWorkspace` | 全部页面、PageFrame |

## 5. 迁移规则

1. 页面只 import 自己消费的域 hook，一次 setup 内最多三个域。
2. 所有 `desk.<key>` 改为 `<域>.<key>`。
3. `library` 域内的书库列表命名为 `books`，避免 `library.library`。
4. 导航统一走 `await workspace.navigate(view)`：
   - 内部拆成 `prepare(view)`（会话清理、来源标记、加载前置）与 `pushView`，同路径不重复 push；
   - `libraryDetail` 由 `workspace` 从 `library.localBook.name` 推导路径。
5. 远端详情/在线阅读的进入动作自己负责导航，不依赖外部 watcher。
6. 纯函数（日期、格式化、错误文案）一律放 `utils/`，域只能自上而下引用。
7. `App.vue` 不包含任何 watch，只调用 `useDeskProvider()`。
8. 章节选择弹窗不直连后端：目录来自 `online.pickerCatalog`，详情来自 `remote.resolveDetail`，
   任务创建来自 `jobs.createDownload`。
9. 签到等业务不得留在组件：组件只保留展示派生（本周格、奖励文案），写操作与状态在域内。
10. 单视图独享的数据访问可以留在页面（如发现页 `get_discover_feed`），一旦出现第二个消费方就下沉为域。

## 6. 不变量

- 页面在 provider 之下渲染；域上下文缺失即为装配错误，直接抛错，不返回 `undefined`。
- `router.push` 只出现在 `desk/navigation.ts`；`router.replace` 只允许出现在
  `desk/navigation.ts`（评论区换层）与阅读/本地详情页的「会话失效时原地重定向」，
  后者是错误恢复，不构成导航也不参与返回链。
- `workspace.navigate` 是唯一的视图跳转入口，页面不自行调用 `router`。
- `workspace.back()` 是唯一的返回入口：Android 返回键与所有页面的返回按钮都调它，
  桌面 Esc 只调它的浮层部分（`dismissTopLayer()`）。页面不得自己拼返回目标路径。
- 浮层一律在打开时经 `useBackLayer` 入栈；`ModalShell` 自动覆盖，非 `ModalShell`
  的抽屉/面板由页面显式注册。返回链不靠任何装配层清单维护。
- 路由返回层是 `Record<RouteLayer, () => void>` 中的一行；`RouteLayer` 新增成员时
  类型检查会强制补齐所有返回动作。
- 域之间无 import 依赖；`workspace` 只依赖 5 个域，`provider`/`lifecycle` 负责剩余编排。
- 每个域的 `ref` 是「有消费者才存在」：新增字段前先确认消费方数量。
- 后端命令只在 `desk/domains/*` 内出现；`src/desk/` 之外仅允许单视图独享的取数。
- 后端契约类型（`types.ts` 中与 `invoke` 相关的部分）不因前端重构改名。

## 7. 验证矩阵

| 检查 | 命令 | 通过标准 |
| --- | --- | --- |
| 类型与模板 | `npx vue-tsc --noEmit` | 零错误 |
| 构建 | `npx vite build` | 成功产出 |
| 残留引用 | `grep -rn "desk\." src --include=*.vue --include=*.ts` | 只剩 `desk/` 目录内部 |
| 导航唯一入口 | `grep -rn "router.push" src` | 仅 `desk/navigation.ts` |
| 返回入口唯一 | `grep -rn "workspace.back()" src/pages` | 6 个页面各 1 处，页面内无第二个返回分支 |
| 浮层已入栈 | `grep -rn "useBackLayer" src` | `ModalShell` 与所有非 `ModalShell` 抽屉/面板 |
| 域内无横向 import | `grep -rn 'from "\./' src/desk/domains` | 只有同目录自身的相对引用 |
| 门面未被绕过 | `grep -rn "desk/domains/" src \| grep -v "^src/desk/"` | 无 |
| 裸 invoke | `grep -rn "invoke<\|invoke(" src --include=*.vue \| grep -v "^src/desk/"` | 仅 `DiscoverPage.vue` |
| 旧入口已删 | `grep -rn "workspace.go\|desk/shared\|useNovelDesk" src` | 无 |

人工手测（本机无法启动窗口，必须实机验证）：

1. 发现页 → 远端详情 → 章节选择弹窗（目录来自所选作品，不是在线阅读会话）。
2. 远端详情 → 开始阅读/听书/看漫画 → 返回，回到正确的上一层。
3. 书架 → 详情 → 收藏切换 → 返回书架次数正确。
4. 本地书库 → 详情 → 阅读/听书/漫画 → 返回详情 → 返回书库。
5. 签到弹窗：签到、领取连签奖励、切换自动签到；关闭设置弹窗后重开，开关状态一致。
6. Android 返回键：任意页内浮层（章节选择、目录抽屉、简介、导出、播放列表、定时菜单、书架搜索、全局弹窗）打开时，第一次返回只关浮层，第二次才回退页面；桌面 Esc 行为一致但不回退页面。
7. 评论区逐层返回：讨论详情 → 评论区 → 作品详情 → 书架/发现页；从评论区回作品详情时作品数据仍在，顶部「返回」按钮与系统返回手势结果一致。
8. 保存中/导出中的弹窗按返回不关闭、也不跳页，任务结束后再按返回才关弹窗。

## 8. 执行记录

### 第一轮：按域拆分

| 原位置 | 新位置 |
| --- | --- |
| `composables/deskShared.ts` | `desk/shared.ts` |
| `composables/useDeskAuth.ts` | `desk/domains/auth.ts`（`createAuthDesk`） |
| `composables/useDeskLibrary.ts` | `desk/domains/library.ts`（返回键 `library`→`books`） |
| `composables/useDeskJobs.ts` | `desk/domains/jobs.ts`（收编 `queueOpen`） |
| `composables/useDeskBookshelf.ts` | `desk/domains/bookshelf.ts` |
| `composables/useNovelSearch.ts` | `desk/domains/search.ts` |
| `composables/useSettings.ts` | `desk/domains/settings.ts` |
| `composables/useUpdater.ts` | `desk/domains/updater.ts`（收编 `checkFromSettings`） |
| `composables/useOnlineReading.ts` | `desk/domains/online.ts` |
| `composables/useChapterPicker.ts` | `desk/domains/picker.ts` |
| `composables/useNovelDesk.ts`（577 行） | `desk/domains/remote.ts` + `desk/domains/workspace.ts` + `desk/provider.ts` |
| `deskContext.ts` | 删除，改为 `desk/keys.ts` + `desk/index.ts` |

- 298 处 `desk.<key>` 改为按域取用，`src/desk/` 外零残留。
- `App.vue` 77 行 → 13 行，两个双向 watch 删除。
- 修复 F-01：章节选择弹窗改读 `picker` 域，不再拿到在线阅读目录。

### 第二轮：消除跨层与双源

| 改动 | 结果 |
| --- | --- |
| `desk/shared.ts` 拆分 | `utils/native.ts`（资源地址、错误文案）+ `useToast.Notify`；上层不再 import 域内部 |
| 新增 `domains/signin.ts` | 签到、领奖、自动签到全部下沉；`SigninModal.vue` 878 → 786 行，只剩展示派生 |
| `settings.setAutoSignin` + `ensureSettings` | 自动签到开关只有一份真相，签到弹窗与设置弹窗不再各显示一个答案 |
| `online.pickerCatalog` | 章节弹窗目录复用阅读会话缓存；picker 删除裸 `invoke` 兜底 |
| `jobs.createDownload` | 按媒态创建任务的参数组装归下载域，picker 只传作品与章节编号 |
| `workspace` 拆分 | `navigate()` = `prepare()` + `pushView()`；生命周期（返回键/启动/复制拦截）移入 `lifecycle.ts`；依赖 9 域 → 5 域 |
| `workspace.activeEntry()` | 侧栏入口规则回到导航层，`PageFrame` 不再自行判断路径、不再依赖 `remote` |
| `remote.fetchComments/fetchReplies` | 评论取数集中到域，`useBookDiscussions` 与 `BookCommentDetail` 不再直连命令 |
| `online.cachedChapter<T>` | 三种媒态共用「缓存 → 请求去重 → 回写」，各媒态只保留参数组装 |
| `desk/domains/` 统一 | `remote`、`workspace` 从 desk 顶层迁入 domains，命名与其余域一致 |
| 错误文案统一 | 13 处 `instanceof Error` 判断改为 `nativeErrorMessage`；`useToast::errorMessage` 删除，`reportError` 与 `nativeErrorMessage` 共用 `utils/native.ts::messageOf`（纯函数，不记日志） |
| 删除死状态 | 删 `returnViews.libraryDetail`、`createLibraryDesk` 的注入参数、`backFromLibraryDetail()`；返回一律 `navigate("library")`。`returnViews` 现在只剩两个 ref，各有唯一写入方 |
| 导航反查推导 | `currentView()` 改为遍历 `VIEW_PATHS` 求反，子路径用 `belongsTo` 判定；新增视图只改一处 |
| 条目构造合并 | `workspace::novelFromLocalWork(work, mode, lastUpdateTime?)`，续传与「打开在线版本」共用 |
| 死样式 | 删 `AuthModal::.logout-all-button` 三处选择器、`ChapterPickerModal::.close-button` 两条规则 |

### 验证

| 检查 | 结果 |
| --- | --- |
| `npx vue-tsc --noEmit` | 0 错误 |
| `npx vite build` | 成功，335.14 kB |
| `grep -rn "desk\." src` | 仅 `src/desk/` 内部 |
| `grep -rn "router.push" src` | 2 处，均在 `desk/navigation.ts` |
| 域外引用 `desk/domains/` | 0 |
| 域内横向 import | 0 |
| 裸 `invoke`（`src/desk/` 外） | 1 处：`DiscoverPage.vue::get_discover_feed` |
| `desk/` 总规模 | 3014 行 / 17 文件 |
| `error instanceof Error`（错误分支） | 0 处（仅 `utils/native.ts::messageOf` 内部的类型收窄） |

### 待人工验证

本机无法启动 Tauri 窗口，§7 的六条手测项由人工执行过一轮，未发现问题。第三轮的改动
只影响错误提示文案（失败路径），正常路径行为不变，因此手测结论仍然有效；失败提示的
文案需要下次遇到真实失败时才能确认。

### 第四轮：统一返回栈

起因是 Android 返回手势从评论区直接跳回书架：`currentView()` 按前缀把
`/remote-book/comments` 归入 `remoteDetail`，系统键据此清掉了整个详情会话；而页面
「返回」按钮却明确指向作品详情。两套返回逻辑漂移是根因，补一条旁路只能修一个点。

| 改动 | 结果 |
| --- | --- |
| 新增 `backLayers.ts` | `registerBackLayer` / `dismissTopLayer` / `useBackLayer`；按打开顺序入栈，只弹最上层，层只要还开着就继续持有返回事件 |
| `ModalShell` 自动入栈 | 8 个全局弹窗与全部页内弹窗（目录、简介、导出、讨论详情）一次性接入；Esc 从组件内移到应用级统一入口，组件只留 Tab 焦点环绕 |
| `navigation.ts` 引入 `RouteLayer` / `currentLayer()` | 评论区是返回链上的普通一层，`currentView()` 仍按前缀归属远端详情以保住侧栏高亮与会话；删掉 `isCommentsView()` / `backFromComments()` 旁路 |
| `workspace.BACK_ACTIONS` | `Record<RouteLayer, () => void>` 路由返回表，评论区只是其中一行；`back()` = 浮层出栈 → 查表回退一层，成为唯一返回入口 |
| `lifecycle.ts` 瘦身 | 删除 8 项浮层 `if` 链、评论区特判与视图判断，依赖从 9 域降到 6 域；Android 返回键直调 `back()`，桌面 Esc 只调 `dismissTopLayer()` |
| 六个页面 `goBack()` | 全部改为 `workspace.back()`；阅读页里三份重复的「在线/本地」判断收归 `leaveReader()` |
| 评论页状态 | `selectedComment` 由 `query.comment` 派生，开关只写 query；页内改用 `setCommentsQuery()`，不再直接调用 `router` |
| 非 `ModalShell` 浮层 | 播放器移动抽屉、桌面播放列表/设置面板、定时菜单、书架搜索面板显式入栈；`SettingSelect` 的 Esc 加 `stopPropagation` 以免冒泡关掉承载它的弹窗 |

新的返回链按入口分叉，每层之上都可以再叠任意浮层：远端是
**讨论详情 → 评论区 → 作品详情 → 书架/发现页**，本地是
**本地详情 → 书库**、**阅读页 → 本地详情（或在线阅读的来源页）**。
每一步恰好弹出一层，系统键与界面按钮不会再走到不同目标。

本轮验证：`vue-tsc --noEmit` 零错误，`vite build` 成功；`isCommentsView`、
`backFromComments`、页面内 `router.replace` 与 `backFromOnlineReading` 外部引用均清零。
§7 新增的第 6–8 条仍需 Android 真机确认。

## 9. 已知取舍（评估过，决定不拆）

按「文件按业务需要拆，不为对称而拆」的原则，以下四处保留现状，后续若出现第二个
消费方再动：

| 现状 | 为什么不拆 |
| --- | --- |
| `online.ts` 548 行，同一文件内含文本/漫画/音频三种章节取数 | 三种媒态共享同一份会话状态（`onlineBook`、`pendingChapterId`、目录快照、`cacheGeneration`）。物理拆分必须再造一个上下文对象把这些状态传回去，耦合不减反增。重复的取数骨架已抽成 `cachedChapter<T>` |
| `ReaderPage` 与 `ComicReaderPage` 各有一份 `openChapter` | `goBack` 已收归 `workspace.back()`；剩下的差异在翻页模型（分页重排 vs 轮播手势复位），抽成组合式函数需要把「打开后做什么」再作为回调传入，可读性下降 |
| 桌面浏览器后退不经过 `back()` | 要让 `router.back()` 成为唯一入口，得把所有层都塞进 URL / `history.state`，全局浮层状态从各域 ref 迁走且刷新后弹窗重现。Tauri 的 `onBackButtonPress` 已接管 Android 手势，桌面没有系统返回键，因此保留 vue-router 的 hash 历史，只保证「系统键 + UI 返回按钮 + 页内浮层」三者一致 |
| `mediaTitle` 在两个详情页同名不同义 | 本地页文案为「小说/有声/漫画」，远端页为「小说原著/有声小说/漫画改编」，是两套产品措辞，合并会改变界面文字 |

另一个已知的同名不同义是 `formatMetric`（详情页返回 `--`、首页返回空串、评论页取整
规则不同），合并会改变用户看到的数字，规则未定前保持原状。

## 10. 未覆盖

前端没有测试基础设施：`package.json` 无 `test` 脚本，`src/` 下无测试文件。因此上面
所有改动的正确性依赖 `vue-tsc` + 人工手测。若要补，优先级是纯函数（`utils/date`、
`utils/format`、`utils/preferences`、`reader/chapterParts`、`reader/paginate`），
它们无 DOM 与 Vue 依赖，加 `vitest` 即可，不需要改动任何现有文件。
