# 后端重构结果

对象：`src-tauri/src`，114 个文件、18,534 行（其中生产代码 16,355 行）、50 个 IPC 命令。
状态：2026-11-05 完成。改动以单条暂存变更呈现，尚未提交。
验证：`cargo clippy --all-targets -- -D warnings` 零告警、`cargo test` 105 项全通过、
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
| 测试函数 | 12 | 105 |
| 跨模块字符串错误 `Result<_, String>` | 269 处 | 0 |
| `AppError::into_message` | 70 处 | 0 |
| `eprintln!` | 29 处 | 1（仅日志宏内部） |
| IPC 命令 | 50 | 50 |
| `cargo clippy -D warnings` | 未设门禁 | 零告警 |
| 最大生产文件 | 628 行（`app/catalog.rs`） | 543 行（`engine/text.rs`） |
| VIP 正文实现 | 2 套（下载 / 阅读行为不同） | 1 套 |
| 内嵌图解析 | 3 套（规则已分叉） | 1 套 |

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

12 → 105 项，覆盖上游 JSON 解析（分卷去重保序、VIP 与解锁判定、协议相对地址补
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

### 5.2 前端从未参与重构

| 文件 | 行数 |
| --- | --- |
| `src/pages/LocalBookDetailPage.vue` | 3078 |
| `src/pages/RemoteBookDetailPage.vue` | 2397 |
| `src/pages/AudioPlayerPage.vue` | 2189 |
| `src/pages/ReaderPage.vue` | 1922 |

后端单文件上限已压到 543 行，前端同量级的页面从未拆过。拆分必须手测每条交互，
不能盲改。相对安全的起点是先把阅读器与详情页里的弹窗、设置面板抽成子组件。

### 5.3 引擎直依赖具体上游类型

`engine/*.rs` 与 `media/*.rs` 有 16 处使用 `&SfSource` 而非端口 trait，这些模块因此
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
