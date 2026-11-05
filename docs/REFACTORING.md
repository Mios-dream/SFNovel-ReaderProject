# 后端重构结果

对象：`src-tauri/src`，114 个文件、18,534 行（其中生产代码 16,355 行）、50 个 IPC 命令。
状态：2026-11-05 完成，已提交为 `f0915e0 重构后端代码`。
验证：`cargo clippy --all-targets -- -D warnings` 零告警、`cargo test` 98 项全通过、
`npx vue-tsc --noEmit` 通过。

本文只写现状：改成了什么、为什么这样改、哪些地方不能动、还剩什么。改造计划与逐轮
过程已删除——计划中的编号（E-xx）与提交对照表在历史被合并后失去意义。

---

## 1. 指标

| 指标 | 重构前 | 现在 |
| --- | --- | --- |
| Rust 文件 | 84 | 114 |
| 总行数 | 15,847 | 18,534 |
| 生产代码行数 | ≈15,700 | 16,355 |
| 测试函数 | 12 | 98 |
| 跨模块字符串错误 `Result<_, String>` | 269 处 | 0 |
| `AppError::into_message` | 70 处 | 0 |
| `eprintln!` | 29 处 | 1（仅日志宏内部） |
| IPC 命令 | 50 | 50 |
| `cargo clippy -D warnings` | 未设门禁 | 零告警 |
| 最大生产文件 | 628 行（`app/catalog.rs`） | 543 行（`engine/text.rs`） |
| VIP 正文实现 | 2 套（下载 / 阅读行为不同） | 1 套 |
| 内嵌图解析 | 3 套（规则已分叉） | 1 套 |

前端（本文只列与后端契约相关的部分；前端整体设计见 `FRONTEND_REFACTORING.md`）：

| 指标 | 重构前 | 现在 |
| --- | --- | --- |
| `requireDesk()` 实现份数 | 10 | 0（已改为按域注入，见前端文档） |
| 正文拆分 / 取色 / 偏好读写位置 | 页面内联 | `reader/`、`utils/` 纯函数 |
| `ExportFormat` / `MediaType` / `DisplayMode` 定义处 | 3 / 3 / 2 | 各 1（`types.ts`） |
| `novel ↔ text` 映射实现 | 5 处（另有 `ChapterMode` 独立类型） | 0（两者已合并为 `MediaType`） |
| 未引用的 CSS 类规则 | 616 行 | 0（剩余仅 `<Transition>` 生成类与函数返回值） |
| `vue-tsc --noEmit` | 通过（靠 tsconfig 排除） | 通过（覆盖全部前端源码） |

行数上涨全部来自拆分与测试：生产代码多 508 行，测试代码约 2,179 行。

---

## 2. 现行结构

### 2.1 依赖方向

```
commands ──▶ application ──▶ domain
    │             │
    └────────▶ adapters ──▶ domain
                  （实现 application/ports）
```

- `domain`：纯模型与规则，只用 std + serde，不出现 `tauri` / `reqwest` / `tokio`。
- `application`：用例编排，只依赖 `ports.rs` 的 trait 与 `domain` 类型。
- `adapters`：实现端口，可以依赖一切外部库。
- `commands`：IPC 薄层，只做「取服务 → 调一次 → 返回」，无文件系统与网络访问。

### 2.2 目录

```
src/
  lib.rs main.rs logging.rs
  domain/       auth catalog content download error ids library settings signin time update
  application/  auth catalog downloads library reading settings signin
                ports.rs（6 个端口）state.rs
  adapters/
    sfacg/      source.rs（门面）app/（API 客户端与解析）web/（网页解析）http cookie device
    storage/    book.rs paths.rs atomic.rs settings.rs jobs.rs test_support.rs
                export/（txt epub markdown audio comic）
    downloads/  registry.rs engine/{mod,prepare,assets,text,audio,comic}.rs
    media/      reader.rs vip.rs fetch.rs inline_image.rs protocol.rs
    platform/   desktop.rs windows/ android/
    ocr/        worker.rs
  commands/     auth catalog downloads library reading settings signin diagnostics updates
```

前端与之对应的分层（`src/`）：

```
  types.ts          与后端 IPC 契约一一对应，是唯一类型来源
  utils/            纯函数，无 Vue 依赖：format 取色 偏好读写 原生错误 日期
  reader/           分页引擎：types paginate textLayout renderPage chapterParts usePagedReader
  desk/             跨页面状态：组合根、按域上下文、导航（详见 FRONTEND_REFACTORING.md）
    domains/        单个域：auth library jobs bookshelf search settings signin updater online picker remote workspace
    navigation.ts   视图与路由的唯一映射，以及返回视图记忆
    lifecycle.ts    应用级副作用：启动初始化、返回键逐层回退、复制拦截
  composables/      不依赖域的局部组合式函数：useToast useVirtualRows useBookDiscussions
  components/       可复用视图
  pages/            页面装配
```

---

## 3. 已解决的问题

### 3.1 错误分类曾经只是装饰

原来三套错误类型、四次转换（`AppApiError` → `String` → `AppError` → `String`），分类
在第一次转换就丢失；`ErrorKind` 七个变体只有一个被读过。更隐蔽的是
`impl From<String> for AppError` 一直存在——任何 `.map_err(|e| format!(...))` 都会被
静默归为 `Internal`，下游 `is_risk_control()` 因此失效。

现在：`AppError` 不实现 `From<String>` / `From<&str>`，必须显式选分类，同类退化写法
**直接编译失败**。59 处按来源补齐（文件系统 → `Storage`，上游请求 → `Upstream`）。
IPC 边界序列化为字符串，但分类在进程内部完整可用。

### 3.2 同一事实的重复实现

| 原来 | 现在 |
| --- | --- |
| VIP 正文两套：下载图存 `<book>/ocr`、遵守「保留过程图」设置、正文落库；阅读图存 `cache/vip`、忽略该设置、只返回文本 | `media/vip.rs::recognize_vip_chapter` 单一链路，阅读端另加章节级识别缓存 |
| 正文内嵌图三套解析：下载 / 阅读 / 前端 CLI，空地址、协议校验、失败降级三处规则不一致 | `media/inline_image.rs::split` 只做切分（`Text` / `Image`），写回策略留各调用点 |
| 五份「原子写入」，实为「删目标 → rename」 | `storage/atomic.rs::replace_file`（Windows 走 `MoveFileExW`） |
| 三份 `mark_finished`、三份连接装配、五份导出序列化器 | 各自收敛为一份，三个下载引擎统一 `prepare` / `plan` / `download` / `into_result` |
| 两份 `TempDir` 测试实现 | `storage/test_support.rs` |

### 3.3 声明了不可能出现的状态

`TextContentKind` 的 `EncryptedVip`、`Unknown` 与 `TextAccessState` 的 `Unknown`、
`SessionExpired` 在生产路径从未被构造，靠 `#[allow(dead_code)]` 压着，前端还照着声明
了 8 个取值。已删除 `TextContentKind`（与 `is_vip` 同源），`TextAccessState` 收窄为
`Available` / `Unavailable`，`src/types.ts` 同步。

### 3.4 分层与装配

`app/catalog.rs`（628 行）拆为 `shelf` / `album` / `search` / `discover` / `comments` /
`detail` 六个文件，最大 274 行；漫画详情解析抽出复用小说/有声的字段读取，`comic.rs`
264 → 162 行；平台层按职责切文件（`session` / `device` / `login` / `autostart` /
`storage` / `ocr` / `signin`），文件内用 `#[cfg]` 分支。

### 3.5 测试

12 → 98 项，覆盖上游 JSON 解析（分卷去重保序、VIP 与解锁判定、协议相对地址补
HTTPS）、书库读写（含 `.complete` 空白导致标题为空串的真实缺陷）、导出格式、正文
来源标识往返、内嵌图切分的畸形输入。

---

## 4. 改动前必须知道的不变量

违反其中任何一条，症状都是「静默错」，不会编译报错。

1. **错误必须带分类构造**。`AppError` 没有兜底转换，这是刻意的——兜底会让分类退化且不留痕迹。
2. **禁止 `map_err(|e| e.to_string())`**。会降级为 `Internal`，让下游 `is_risk_control()` 判断失效，直接用 `?` 上抛。
3. **VIP 正文只有一条链路**：`media/vip.rs::recognize_vip_chapter`。不要再写第二份取图 + OCR。
4. **内嵌图只有一条解析**：`media/inline_image.rs::split`。下载端失败即中止整章，阅读端降级为占位文本，这个差异由调用点各自说明，不在解析器里分支。
5. **落盘取值必须有往返入口**：`TextOrigin::as_str` 配 `TextOrigin::parse`。未知标识按「未完成」处理，宁可重下不可漏——此前 OCR 来源绕开枚举用裸字符串，一旦标识改名，VIP 章节会被无限重下。
6. **文件写入只走 `replace_file`**，不要自己 `fs::rename` 或先删后写。
7. **解析丢条目必须汇总 `log_warn!`**（缺编号的上游条目），不允许静默少数据。
8. **存储层测试用 `TempDir`（`storage/test_support.rs`）**，不要各文件自己实现。

风控判定是唯一保留文案嗅探的地方：`app/error.rs::is_account_risk_controlled` 用
`1146` 错误码，或 `422` + 文案含「风险」。原因是 422 语义过宽，仅凭状态码无法区分，
这是收窄后的必要降级，不是漏改。

---

## 5. 仍未完成

### 5.1 手测（最大空白）

本机无法启动 Tauri 窗口，以下链路全部未经人工验证：文字下载（含 VIP 章）、有声
下载、漫画下载、暂停与恢复、五种导出格式、Android 登录与 OCR、阅读器交互。

### 5.2 前端：逻辑已按后端标准整理，组件未拆

前端改动分两批，共同原则是**只做等价改写，不动渲染结构**——本机无法启动 Tauri
窗口，任何视觉变化都无法验证。

第一批（逻辑去重）：

- `requireDesk()` 在 9 个页面 + `PageFrame.vue` 各写一遍，当时统一为 `deskContext.ts::useDesk()`；该入口已在后续的前端重构中替换为按域注入（见 `docs/FRONTEND_REFACTORING.md`）。
- `trimMetric` 三处逐字相同、`formatMetric`/`scoreToneClass`/`formatSourceDate` 在两个详情页各一份，现合并到 `src/utils/format.ts`。
- 删除 3 处模板从未引用的死样式（`desktop-nav-label`、`remote-latest-*`、`volume-control`）。

第二批（结构与死代码）：

- **纯逻辑搬出组件**，与后端「domain 不依赖框架」对齐：
  - `reader/chapterParts.ts`：本地 markdown 与在线占位符两种正文拆分，原来是页面里的两个 `matchAll` 循环；本地图片地址由调用方注入，函数本身无 IO。
  - `utils/dominantColor.ts`：首页封面取色算法（缩略采样、饱和度加权、主桶选择），组件只保留缓存与竞态计数。
  - `utils/preferences.ts`：`localStorage` 读写与容错统一入口，读失败退默认值、写失败静默；校验规则仍留在各页面。
- **消除分支重复**：
  - 本地详情页「挑可用模式」原本写成 5 处嵌套三元 + 一条 7 分支 if 链，现为 `resolveAvailableChapterMode`。
  - 三种内容的文案与导出按钮原本各写一遍 `media === "xxx"`，现为 `MEDIA_KINDS` 表。
  - 桌面侧栏与移动抽屉的目录行构建原本两份近似复制（行高、key 前缀散落），现为 `buildDirectoryRows(mode, metrics)`。
  - 远端详情页「文字用 `chapId`/`accessState`、音视频用 `id`/`isUnlocked`」的字段归一原本映射两遍，现为 `directoryVolumes`。
- **契约类型收敛到 `types.ts`**：`ExportFormat`（原三个文件各写一遍）、`MediaType`（原 `Media`/`MediaType`/`RemoteMediaType` 三种别名）、`DisplayMode`（原两个阅读页各一份）。
- **`ChapterMode` 合并进 `MediaType`**：同一件事曾有两套名字（`novel`/`text`），靠 `utils/media.ts` 双向转换，8 个文件里有 11 处调用；`NovelCard` 甚至用 7 行嵌套三元把上游已经给出的 `bookshelfType` 再转一遍。合并后映射与转换全部消失。唯一保留的差异是 `Job.kind`——它是后端 `MediaKind` 的序列化名（文字为 `text`），属于边界契约，已在 `types.ts` 注明，且当前前端不读取该字段。
- **常量替代魔法数**：播放列表行高 `TRACK_ROW_HEIGHT` 原在虚拟列表与滚动定位各写一个 `54`，改一处会让定位偏移。
- **职责拆分**：有声页 `onMounted` 原混着监听注册、在线初始化、本地初始化三段，现为三个函数；`formatTime` 等箭头函数常量统一为 `function` 声明。
- **删除死样式 616 行**：`LocalBookDetailPage` 里两整套 `comic-*` 展示样式（模板只用 4 个 `comic-*` 类，其余是从远端详情页复制后的历史残留）、`drawer-tabs`、`comic-local-directory`；`RemoteBookCommentsPage` 的 `topbar-actions`/`detail-link`；`ReaderPage` 的 `directory-count`。
  判定方法：模板标识符 + 脚本字符串字面量 + `` `${x}-switch` `` 动态前缀，再人工排除 `<Transition name>` 生成的 `*-enter-*`/`*-leave-*`、`<TransitionGroup>` 的 `*-move`、`scoreToneClass()` 返回的 `score-*`。

页面规模（样式仍是主要体积）：

| 文件 | 总行数 | 模板 | 脚本 | 样式 |
| --- | --- | --- | --- | --- |
| `LocalBookDetailPage.vue` | 2479 | 524 | 499 | 1451 |
| `RemoteBookDetailPage.vue` | 2349 | 528 | 397 | 1419 |
| `AudioPlayerPage.vue` | 2196 | 465 | 701 | 1025 |
| `ReaderPage.vue` | 1861 | 364 | 572 | 920 |

**组件没有拆，这是刻意的。** scoped 样式不会穿透到子组件内部（父级 `data-v` 只落在子组件根元素），任何漏搬都表现为「样式丢失」而不是报错，而本机无法验证渲染结果。同理，漫画阅读页的指针手势（缩放、平移、翻页）也保留在页面内，抽成 composable 需要触摸设备验证。

还有一个未统一的语义分裂：`formatMetric` 仍有三套实现（详情页返回 `--`、首页返回空串、评论页取整规则不同），同名不同义，合并会改变用户看到的数字，因此保留原状待定规则。

### 5.3 引擎直依赖具体上游类型

`engine/*.rs` 与 `media/*.rs` 有 29 处引用 `SfSource`（其中 13 处是 `&SfSource` 参数）而非端口 trait，这些模块因此
无法脱离网络测试。引入 `AssetGateway` 之类的端口可行，但要动到 `prepare.rs`、
`assets.rs` 与三个引擎的签名，且同样需要手测下载链路。

### 5.4 已知但不打算动

- `sfacg/source.rs` 500 行：`RemoteGateway` 的方法九成是一行转发，拆开只让文件变短，结构不变；且要跨文件共享 `require_app` / `require_web` 这类私有前置检查。
- `app/client.rs`、`media/reader.rs`、`ocr/worker.rs`、`application/catalog.rs` 无测试 harness：需要先引入 trait 级假实现，属新增基础设施而非重构。
- Windows 与 Android 仍是两套 `#[cfg]` 文件树，不是 trait 抽象。

---

## 6. 验证

```bash
cd src-tauri
cargo clippy --all-targets -- -D warnings   # 必须零告警
cargo test                                   # 105 项

npx vue-tsc --noEmit                         # 前端契约变化时
```

`cargo fmt --check` 不是门禁：仓库存在历史格式漂移，只对新文件跑
`rustfmt --edition 2021 <file>`。
Android 目标可 `cargo check --target aarch64-linux-android`。

### 手测矩阵（待执行）

| 场景 | 关注点 |
| --- | --- |
| 密码登录 → 退出 → 再登录 | 会话状态、错误分类 |
| 未登录点下载 | 应为 `Unauthenticated` 文案，不是「内部错误」 |
| 文字下载 1 章 | 章节 JSON、manifest 的 `downloaded_text_chapter_ids` |
| 文字下载 VIP 章 | App 失败 → OCR 回退 → 成功或汇总错误 |
| 触发风控 | 任务立即 `error` 并停止后续章节，不是逐章重试 |
| 有声 / 漫画各下载 1 章 | `有声目录.m3u8`、`file` 字段；图片落盘与 `chapter.json` |
| 下载中暂停 → 继续 | `paused` → `queued`，续传不重复写 |
| 下载中杀进程 → 重启 | 恢复为 `paused`，可继续 |
| 并发 1 → 4 → 1 | 运行中任务不 panic，新任务遵守新上限 |
| 导出 TXT / EPUB / Markdown / 有声 / 漫画 | 文件非空、可打开 |
| 断网启动 | 不崩，设置仍可读 |
| 写入中断电 | 重启后原文件完好（原子写入） |
| 同名书籍（不同 `novel_id`） | 已下载标记不串 |

---

## 7. 约定

1. `Result<_, String>` 只允许出现在 adapter 内部工具函数，跨模块边界必须是 `AppResult`。
2. 不用 `message.contains(...)` 做控制流；需要区分就加 `ErrorKind` 变体。唯一的例外是风控判定（见 4 节末）。
3. 同一字符串字面量出现 3 次以上，提为常量或枚举。
4. 单函数超过 60 行或嵌套超过 3 层，先补结构体再拆。
5. 命令函数体不超过 5 行：取服务、调一次、返回。
6. 不在命令里 `new` 任何客户端；连接与状态走 `manage` 或进程级单例。
7. 校验放在类型构造处（`TryFrom`），不在调用链每层重复。
8. 编号类型：`NovelId` 仅用于纯小说语义的端口签名；收藏/评论的 `novel_id` 在漫画场景是漫画编号、`discover(type_id)` 是分类编号，必须保持 `i64`。
9. 注释只写「为什么」与失败语义，「是什么」由签名表达。
10. 文件超过 400 行时先判断是否混了两个职责，而不是继续加函数。
