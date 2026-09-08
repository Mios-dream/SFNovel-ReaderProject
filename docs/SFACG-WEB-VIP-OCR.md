# SFACG 网页 VIP 正文与 OCR 技术方案

## 1. 背景与结论

当前文字目录来自匿名 App 内容接口。匿名响应可以提供章节的 `isVip`、标题和价格信息，但不能可靠提供当前用户的购买态；因此 `isVip` 不能直接转换成“禁止下载”。

网页正文也不是单一格式：

- 普通章节通常在章节 HTML 的 `#ChapterBody` 中提供可提取文本；
- 图片 VIP 或加密 VIP 可能只返回图片/GIF，文本解析器得到空内容；
- 页面能否打开、目录是否带 VIP 标识、账号是否已购买，是三个不同状态。

本方案的核心是：

1. 目录只记录销售属性和内容类型，不用匿名 `has=false` 推断未购买。
2. 使用官方 Web 会话访问网页端资源；不模拟 App 客户端，不调用 App 正文接口。
3. 对网页返回的图片/GIF，先保存原始资源，再进行本地 OCR。
4. OCR 结果必须标记为识别文本，保留原始图片和页序，不能静默覆盖原文。

实际使用仍需遵守 SFACG 的服务条款和内容授权范围。本文记录公开项目的实现方式，不记录任何账号、密码或 Cookie。

## 2. 近期项目调研

检索范围为 2025-09-07 至 2026-09-07，按 GitHub 仓库最近提交时间筛选。使用 App API（例如 `api.sfacg.com/Chaps`、`/sessions`）的项目不作为本方案参考实现。

### 2.1 主要参考项目

#### `light-nook-labs/sfacg_tools`

- 地址：[https://github.com/light-nook-labs/sfacg_tools](https://github.com/light-nook-labs/sfacg_tools)
- 最近提交：2026-07-08，`754195f`。
- 主要代码：[sfacglib/novel.py](https://github.com/light-nook-labs/sfacg_tools/blob/main/sfacglib/novel.py)
- OCR 代码：[sfacglib/ocr/engine.py](https://github.com/light-nook-labs/sfacg_tools/blob/main/sfacglib/ocr/engine.py)
- 依赖声明：[pyproject.toml](https://github.com/light-nook-labs/sfacg_tools/blob/main/pyproject.toml)

该项目是目前调研中唯一同时满足“网页 Cookie、网页 VIP 图片、GIF、OCR”的实现。它不使用 App API 正文接口。

目录解析：

- 从 PC 站 `MainIndex` HTML 解析卷、章节 ID 和标题；
- `.icn_vip` 表示 VIP；
- `.icn` 内的特定图标字符表示图片 VIP；
- VIP 且不是图片 VIP 时，标记为 `is_gif=true`，走加密 GIF 流程。

VIP 图片获取：

```text
GET https://book.sfacg.com/ajax/ashx/common.ashx
    ?op=getChapPic
    &tp=true
    &quick=true
    &cid={chapter_id}
    &nid={novel_id}
    &font=16
    &lang=
    &w=5000
```

请求使用登录后的 Web Cookie，并将 VIP 章节地址作为 `Referer`。响应先经过 GIF 格式和宽度校验；项目把无效响应视为未订阅、会话失效或上游拒绝，而不是继续解析错误页面。

OCR 过程：

1. GIF 每帧转换为 RGBA，再合成白色背景；
2. 裁剪四周空白；
3. 根据横向空白行切分文本行；
4. 去除拼音区域；
5. 对每一行使用 RapidOCR 识别，关闭检测和方向分类，只使用识别模型；
6. 按原始帧和行编号排序后合并；
7. 可选执行 OCR 文本纠错，但必须保留未纠错结果。

该项目的 OCR 依赖是 `rapidocr_onnxruntime` 和 ONNX Runtime GPU；这是桌面 Python 方案，不是可直接嵌入 Android 的移动运行时。应使用更低成本的可以在移动平台上运行的方案。

## 3. 当前项目应采用的请求分层

当前 `WebClient` 已经负责 Web Cookie 过滤和网页资源请求，但 `chapter_content()` 只处理普通章节 HTML。后续应将文字能力拆成以下几类：

```text
TextDirectoryWeb       PC 站 MainIndex 目录和 VIP 类型
TextChapterWeb         普通章节 HTML 文本
TextVipImageWeb        getChapPic 返回的图片/GIF
TextChapterOcr         本地图片/GIF OCR
```

建议的会话约束：

- `TextDirectoryWeb`、`TextChapterWeb`、`TextVipImageWeb` 只使用 `session_PC`；
- 不把 `session_PC` 转发到 App API；
- 不因网页正文失败自动调用 `/Chaps`；
- 目录请求失败、章节未购买、图片接口拒绝、OCR 失败分别报告；
- Cookie 只保存在原生会话状态，不返回 Vue，不写任务快照，不写日志。

## 4. 目录数据模型

不再用单一的 `isUnlocked: boolean` 表示所有状态。建议增加内容类型和访问结果：

```text
contentKind: text | imageVip | encryptedVip | unknown
vip: boolean
accessState: unknown | available | unavailable | sessionExpired
downloaded: boolean
```

含义：

- `vip`：章节是否属于 VIP 销售内容，只用于显示；
- `contentKind`：根据网页目录和章节页面结构判断正文形态；
- `accessState`：只有实际请求章节资源后才能从 `unknown` 变成结果；
- `downloaded`：仅表示本地是否已保存成功。

章节选择器可以显示所有目录章节。对未知购买态的 VIP，不应显示“确定未解锁”；用户选择后由对应网页资源请求给出结果。

## 5. 网页 VIP 图片获取流程

### 5.1 目录阶段

1. 使用 Web 会话请求：

   `https://book.sfacg.com/Novel/{novel_id}/MainIndex/`

2. 解析卷标题、卷 ID、章节 ID、章节标题。
3. 读取 `.icn_vip` 和图片标识，得到 `contentKind`。
4. 不把匿名 App 目录的 `has` 字段作为最终购买判断。

### 5.2 普通文本章节

继续解析：

`https://book.sfacg.com/Novel/{novel_id}/{volume_id}/{chapter_id}/`

仅在 `#ChapterBody` 中存在有效文本时保存为文本章节。若正文只有 `img`、空节点或“章节不可用”提示，转入图片 VIP 分支，而不是直接调用 App API。

### 5.3 加密 GIF 章节

1. 使用 `/vip/c/{chapter_id}/` 作为 VIP 章节 Referer。
2. 请求 `getChapPic`，携带当前小说 ID和章节 ID。
3. 检查 HTTP 状态、`Content-Type`、GIF 文件头、宽度和帧数。
4. 失败时最多进行有限重试；建议本应用默认串行或低并发，不复制参考项目的高并发设置。
5. 保存原始 GIF，再进入 OCR 队列。

响应校验不能只看 HTTP 200。错误页、登录页或提示图片也可能返回 200；至少需要验证：

- 解码格式确实为 GIF；
- 尺寸满足服务端请求的宽度要求；
- 帧数大于零；
- 图像不是明显的错误提示或空白占位图。

## 6. OCR 处理方案

### 6.1 处理流水线

```text
GIF bytes
  -> decode frames
  -> RGBA over white background
  -> crop outer whitespace
  -> grayscale / threshold
  -> detect line gaps
  -> remove pinyin strip
  -> crop individual text lines
  -> Chinese OCR recognition
  -> restore frame/line order
  -> normalize line breaks
  -> optional dictionary or manual correction
  -> save OCR text + source image metadata
```

### 6.2 图像预处理

参考项目使用以下策略：

- GIF 帧使用白底合成，避免透明像素影响 OCR；
- 以灰度图统计非白像素，裁剪外部空白；
- 按整行黑像素数量寻找空白间隔；
- 以最小间隔和最小行高过滤噪声；
- 对每行检测拼音与正文的垂直分界，将拼音区域置白；
- 再按 `pinyin_top_crop_ratio` 裁剪每行顶部，并裁剪行内左右空白，降低识别输入尺寸；

桌面 worker 默认使用 `pinyin_top_crop_ratio = 0.30`，可通过
`--pinyin-top-crop-ratio` 调整，并在 `ocr/segments/chapter-{id}/` 检查实际输入图。

这些阈值必须做成可调配置，不应写死在业务流程中。不同章节可能使用不同字体、字号、背景和压缩质量，建议保留原图并提供诊断样本。

### 6.3 OCR 模型与运行时

桌面原型可使用：

- RapidOCR；
- PP-OCR 系列中文检测/识别模型；
- ONNX Runtime CPU 或 CUDA 执行提供程序。

Android 版本建议使用独立的移动运行时：

- ONNX Runtime Mobile；
- 或经过验证的 Paddle Lite / ncnn 中文识别模型。

不要直接把 `rapidocr_onnxruntime` Python 依赖打包进 Tauri Android。移动端需要单独评估模型大小、ABI、内存峰值、线程数和许可证。

### 6.4 识别模式

初版建议采用“整页检测 + 行识别”的混合模式：

- 规则切行成功时，关闭 OCR 检测模型，仅使用识别模型，速度较快；
- 规则切行失败时，退回整页检测，避免整页漏字；
- 记录每行置信度，低置信度行进入重试或人工复核；
- 不自动使用 LLM 改写正文。若提供纠错，只能在用户确认后生成副本。

### 6.5 输出格式

每章建议保存：

```text
chapter.json       章节 ID、内容类型、帧数、OCR 版本、时间和状态
source/            原始 GIF 或分页图片
章节存储            `.novel-flow-chapters.json` 中的未纠错 OCR 正文
```

本地书库直接使用章节存储中的 OCR 正文；原始图片和 OCR 来源标记仍需保留，避免把 OCR 文本误认为官方原文。

## 7. 移动端架构建议

### 7.1 原生层职责

- 使用 Web 会话发起网页和图片请求；
- 解码 GIF、控制并发和取消；
- 将图片帧写入应用私有缓存；
- 调用 Android OCR 模块；
- 以事件形式回报章节、页、行级进度。

### 7.2 Vue 层职责

- 显示 `VIP`、`图片正文`、`OCR 中`、`OCR 完成` 等状态；
- 允许用户选择“保存原图”“生成 OCR 文本”；
- 展示低置信度或失败章节；
- 不接触 Cookie、请求签名或原始认证响应。

### 7.3 资源控制

- 默认每次处理一章；
- GIF 帧按页增量处理，处理完成即释放位图；
- OCR 线程数默认为 1，允许在设置中调整；
- 支持暂停、恢复和断点；
- 原始图片和 OCR 文本分开缓存，避免重复下载；
- 预估磁盘空间，避免把高分辨率 GIF 无限制写入书库。

## 8. 错误状态与风控边界

必须区分以下错误：

| 状态                  | 含义                          | 用户提示                               |
| --------------------- | ----------------------------- | -------------------------------------- |
| `catalog_unknown`     | 目录只有 VIP 标识，没有购买态 | “VIP 状态待验证”                       |
| `web_session_missing` | 没有官方 Web 会话             | “请先完成官方网页登录”                 |
| `not_subscribed`      | 图片接口返回无效或明确拒绝    | “网页端未提供本章内容，请确认购买状态” |
| `session_expired`     | Web 会话失效                  | “网页登录会话已失效”                   |
| `image_invalid`       | 返回不是有效图片/GIF          | “章节资源格式无效”                     |
| `ocr_failed`          | 图片已获得但 OCR 失败         | “已保存原图，OCR 失败，可稍后重试”     |

不要把 `not_subscribed`、`session_expired` 和 `ocr_failed` 合并成“VIP 不可下载”。不要为了重试 OCR 而重复请求上游图片，也不要在网页失败后自动切换 App API。

## 9. 当前桌面实现

桌面端已接入以下流程：

- PC 网页目录将普通、图片 VIP 和加密 GIF VIP 分开标记，但不会根据登录状态臆测购买态；
- `getChapPic` 只使用已过滤的 `session_PC`，验证图片文件头与最小数据长度后原子保存到书籍 `ocr/` 目录；
- 普通 HTML 正文中的图片会转换为本地 Markdown 图片；
- `imageVip` 和 `encryptedVip` 在来源图像落盘后调用随包 Python worker，通过标准输出返回未纠错文本，并将其直接保存为章节 `content`；原图路径保存在 `ocrSourcePath`；
- OCR 调用同时保存诊断中间图到 `ocr/segments/chapter-{id}/`，包括裁白帧、去拼音帧和逐行输入图，便于检查规则切行是否正确；
- OCR 失败不会删除原图，也不会对图片 VIP 自动回退到 App `/Chaps`。

桌面 OCR worker 是独立的 uv project，依赖和版本锁定在
`src-tauri/ocr-worker/pyproject.toml` 与 `uv.lock`。开发环境使用
`uv sync --project src-tauri/ocr-worker`；初版固定单 worker，并且不使用
LLM 改写 OCR 结果。发布包直接携带 PyInstaller 生成的 worker，不要求用户安装
Python 或 uv。

## 10. 后续工作

### 已完成：网页目录与内容类型

- 新增 Web PC 目录解析；
- 增加 `contentKind`；
- 移除匿名目录对 VIP 的硬禁用；
- 增加普通文本、图片 VIP、加密 VIP 的测试夹具。

### 已完成：网页图片资源

- 新增 `TextVipImageWeb` 客户端能力；
- 实现 `getChapPic` 请求和严格响应校验；
- 保存原始 GIF/图片；
- 关闭自动 App 正文回退，改为显式设置。

### 已完成：桌面 OCR 原型

- 在桌面端接入 OCR 运行时；
- 用真实的已授权章节样本验证帧数、切行、拼音去除和字数；
- 输出原始 OCR 文本；低置信度行和置信度统计待单独加入；
- 建立 10 至 20 个章节的准确率基线。

### 待评估：Android OCR

- 选择 ONNX Runtime Mobile、Paddle Lite 或 ncnn；
- 固定 Android ABI 和模型资源打包方式；
- 测试低内存、后台切换、暂停恢复和电量消耗；
- 将 OCR 从下载线程拆为可取消的独立任务。

## 11. 验收标准

- 匿名目录中的 VIP 章节不会被错误标记为“确定未购买”；
- 普通章节仍能正常保存文本；
- 图片 VIP 请求只经过 Web 客户端，不携带 App 会话；
- 无效图片不会写入“下载完成”；
- OCR 失败时原始图片仍可查看和重试；
- OCR 文本保留帧序、章节边界和来源元数据；
- 关闭 App 回退后，网页图片失败不会产生 App `/Chaps` 请求；
- 日志、Vue 状态和任务快照中不出现密码、Cookie、Basic Auth 或签名。
