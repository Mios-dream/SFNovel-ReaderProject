# SFACG 登录与账号风控审计记录

> 记录日期：2026-09-05  
> 目的：记录可能导致 SFACG 账号出现“存在安全风险”提示的因素，供后续逐项验证和修复。  
> 范围：当前 SF Novel Flow，以及 `LanBaiCode/auto-novel`、`Oevani/Sfacg_Downloader` 的公开实现。

## 结论摘要

两个参考项目没有实现官方的“风险解除”机制。它们主要通过伪装 Android 客户端、生成 `SFSecurity` 签名、携带设备标识并调用 App API 工作。

`auto-novel` 另外在签到前调用了 `/user/androiddeviceinfos`，代码注释称这可以消除新号签到时的风险提示。该做法属于模拟客户端设备上报，不能证明对普通登录安全有效，也不应直接视为安全修复。

当前项目最值得优先验证的组合是：桌面端使用 Android/OPPO App User-Agent、固定设备令牌、App `/sessions` 登录，以及 Web Cookie 与 App API 混用。

## 当前项目已确认的信号

| 编号 | 因素 | 当前实现 | 风险判断 |
| --- | --- | --- | --- |
| C-01 | 固定设备令牌 | `src-tauri/src/sfacg.rs` 中定义了跨安装复用的设备令牌 | 多个用户或多台机器表现为同一设备，可能形成异常聚合 |
| C-02 | 平台与设备不一致 | Windows、Android 共用 Android/OPPO 风格 App User-Agent | 平台、设备型号、版本、Cookie 来源不一致，可能触发设备校验 |
| C-03 | User-Agent 伪装 | 请求声明为菠萝包 Android 客户端，而实际网络栈是 Rust/桌面应用 | 单独的“字段完整”不等于可信；字段之间不一致更可疑 |
| C-04 | App API 密码登录 | `POST /sessions` 发送用户账号密码，并取得 `session_APP` | 这是非官方桌面客户端登录路径，可能受到 App 设备风控 |
| C-05 | 登录字段与客户端版本 | 密码登录体使用 `userName`、`passWord`，并使用 5.0.36/H5 请求画像 | 字段、版本和实际客户端若不一致，可能触发上游校验 |
| C-06 | Web/App 会话混用 | 官方网页登录得到 Web Cookie 后，再用 App `/user` 验证 | `session_PC`、`session_APP` 和站点 Cookie 的权限边界可能不同；401 不代表 Cookie 捕获失败 |
| C-07 | 设备信息上报 | 密码登录成功后非阻塞调用 `/user/androiddeviceinfos` | 该接口是否影响 API 授权仍未证实；失败不会阻断登录 |
| C-08 | Nonce 生命周期 | 每个请求独立生成 UUID nonce，不跨请求复用 | 与参考客户端一致，但上游协议调整仍可能导致拒绝 |
| C-09 | 签名时间戳 | `SFSecurity` 使用 Unix 秒时间戳、当前盐值和简单 MD5 | 单位、盐值、字段顺序或 UA 不匹配可能产生 401/417/782 |
| C-10 | 请求头画像 | Basic Auth、Accept、Content-Type、App User-Agent、SFSecurity 和 Cookie 同时发送 | 头部组合若与真实客户端不匹配，可能被识别为脚本或代理请求 |
| C-11 | 登录会话验证 | 登录后立即请求 `/user`、书架和账户资料 | 登录后的连续验证会增加请求次数；若会话类型不兼容，可能造成重复登录或失败重试 |
| C-12 | 会话保存后重用 | Windows 使用加密文件保存 Cookie，Android 使用应用 WebView Cookie | 保存本身是安全措施，但跨 IP、跨设备或跨 Web/App 类型重用会成为风控信号 |

相关实现：[App API 文档](SFACG-APP-API.md)、[Rust 请求与会话](../src-tauri/src/sfacg.rs)、[TypeScript API 遗留实现](../src/client/sfacg/api/client.ts)。

## 参考项目记录

### `LanBaiCode/auto-novel`

- 使用 Android/OPPO User-Agent、设备令牌、Basic Auth 和 `SFSecurity` 签名访问 App API。
- 使用账号密码调用 `/sessions`，从响应中提取 `.SFCommunity` 和 `session_APP`。
- 签到流程先调用 `/user/androiddeviceinfos`，再调用签到接口；源码注释明确把它描述为解除新号风险提示的步骤。
- 使用虚拟短信服务注册账号，并通过 GitHub Actions 定时执行注册和签到。
- 将用户名、明文密码和 Cookie 写入 Supabase 账号表，并由定时任务批量读取。
- 下载、签到、广告奖励和多账号操作会产生比普通阅读更明显的自动化行为信号。

判断：该项目的“风险解决”是设备/客户端伪装和特定签到接口组合，不是可迁移的账号安全机制；云端保存凭据反而增加了接管风险。

### `Oevani/Sfacg_Downloader`

- 使用固定设备令牌和 Android/OPPO User-Agent。
- 通过签名预检获取 nonce，之后复用 nonce；遇到 417 时重试。
- 使用账号密码调用 `/sessions`，失效后自动重新登录并更新 Cookie。
- 将账号、密码和 Cookie 保存在工作目录的明文 `config.conf`。
- 没有发现官方 OAuth、QQ/微信授权回调或独立的账号风险处理流程。

判断：该项目主要解决的是 API 签名和会话可用性，不是“账号风险提示”；固定设备令牌和明文会话保存都不应照搬。

## 可能的风控原因清单

以下项目目前只能作为候选原因，不能仅凭代码确认。每项都需要用同一账号、同一网络和最小请求量进行对照实验。

### 设备与客户端身份

- 所有安装复用同一个设备令牌。
- 每次启动随机生成设备令牌，导致同一设备频繁变更身份。
- 设备令牌未持久化，或 Cookie 与另一设备令牌配套使用。
- User-Agent 中的系统、浏览器、客户端版本、厂商、型号与实际运行环境不一致。
- User-Agent 字段不完整，或缺少官方客户端会发送的 Accept、Accept-Encoding、Referer、X-Requested-With 等关联头。
- `package`、`abi`、`version`、`deviceId`、`deviceToken` 之间互相矛盾。
- `shuMeiId` 为空、固定复用或与设备令牌没有稳定关联。
- 桌面端直接伪装 Android，而不是使用官方 Web 登录产生的 Web 会话。
- 多个账号共享同一设备令牌、同一 Cookie、同一代理出口或同一运行环境。

“使用自己的设备令牌”和“使用完整 User-Agent”应理解为：每个安装实例拥有稳定、唯一、受保护的设备标识，并且 User-Agent 的平台、型号、版本和实际运行环境一致。仅把字符串补完整，或随意填写真实手机信息，都可能增加不一致信号。

### 会话与登录流程

- 使用 App `/sessions` 登录，而不是让用户在 SF 官方网页登录页完成认证。
- Web 登录得到的 `session_PC` 或站点 Cookie 被发送到 App API。
- App 登录得到的 `session_APP` 被发送到 Web 页面或 Web AJAX 接口。
- 登录成功后马上连续请求 `/user`、书架、余额、章节目录和资料接口。
- Cookie 失效后立刻自动用密码重新登录。
- 多次失败后无退避地重复登录、重复 nonce 预检或重复验证。
- Cookie 被复制到另一台设备、另一 IP、另一地区或另一种客户端画像。
- 未区分“Cookie 已捕获”“会话可用于 Web”“会话可用于 App API”三个状态。

### 请求行为

- 并发请求章节、图片、音频或目录。
- 短时间内连续访问大量章节，即使没有形成完整批量下载。
- 失败请求自动重试过多，尤其是 401、403、417、429 或风控响应。
- 登录后自动签到、阅读时长、分享、广告奖励、收藏、关注或购买。
- 访问与用户正常使用习惯明显不符的接口，例如新号立即执行多个任务。
- 同一账号在多个进程、定时任务和桌面应用实例中同时活动。
- 代理 IP、VPN、云主机出口、IP 地理位置与账号历史不一致。
- TLS/HTTP 网络栈、连接复用或请求顺序与官方客户端明显不同。

### 账号与外部服务

- 新注册账号、刚改密码账号、长期未登录账号或刚更换设备账号。
- 使用虚拟手机号、短信代收平台或批量注册服务。
- 多账号共享邮箱、手机号、设备指纹、IP 或代理。
- 将密码、Cookie 或账户资料上传 Supabase、GitHub Actions、日志、崩溃报告或第三方服务。
- 明文配置、源码、构建产物或 Git 历史中残留凭据。
- 共享、转发或导入其他设备导出的 Cookie。

## 需要优先验证的实验

1. 同一账号、同一网络，仅用官方浏览器登录，记录是否出现风险提示。
2. 同一账号、同一网络，仅用当前项目官方网页登录，暂不调用 App API，记录是否出现风险提示。
3. 对比当前密码登录与官方网页登录：登录接口、Cookie 名称、User-Agent、设备令牌和后续请求是否一致。
4. 将设备令牌改为每个安装唯一且持久化的值，确保不会跨安装共享，也不会每次启动变化。
5. 暂停所有自动签到、设备上报、广告、分享、收藏和购买行为，只验证登录与一次 `/user` 请求。
6. 禁止自动重试登录；对 401、403、417、429 和风险消息分别记录并停止流程。
7. 在 Windows、Android 真机和模拟器分别测试，确认 User-Agent、设备参数和 Cookie 来源没有跨平台混用。
8. 对 Web Cookie 只访问 Web 接口，对 `session_APP` 只访问已确认的 App 接口，分别验证有效性。
9. 记录登录前后的 IP、地区、时间、设备标识变化，排除网络环境本身导致的风险。
10. 使用新账号和已有正常账号分别测试，区分账号历史风险与客户端风险。

实验记录必须只保存状态码、错误码、接口类别、平台和脱敏标识；不得保存密码、完整 Cookie、完整签名或设备令牌。

## 后续修复方向

- 优先使用官方站内 Web 登录；Web 会话只用于 Web 支持的功能。
- 将 App 会话和 Web 会话分成不同的原生状态，不互相转换或猜测兼容。
- 设备标识按安装实例生成并加密持久化，避免固定共享，也避免每次启动变化。
- User-Agent、设备参数和实际平台保持一致；不要用“完整但虚构”的字段替代官方客户端身份。
- 登录失败、会话失效和风控响应分开处理，不自动循环登录。
- 默认关闭签到、设备上报、广告奖励、分享和多账号自动化，除非接口行为经过单独验证。
- 不把密码、Cookie、签名、设备令牌写入日志、渲染层、第三方数据库、CI 环境或 Git。
- 在修改前先完成上述实验，避免把参考项目的绕过行为误认为安全机制。

## 2026-09-05 已实施整改

- 网络层拆分为 `AppClient` 与 `WebClient`。`AppClient` 集中构造带 Basic Auth、`SFSecurity` 和 Android App User-Agent 的签名请求；`WebClient` 集中构造网页、网页 AJAX 和资源请求，使用平台对应的浏览器 User-Agent 与稳定的语言头。两类请求不再共用 `reqwest::Client` 或默认请求头。
- `AppClient` 与 `WebClient` 分别位于 `src-tauri/src/app_client.rs` 和 `src-tauri/src/web_client.rs`。客户端在创建时各自从原生状态读取会话快照：前者只接受 `.SFCommunity` 与 `session_APP`，后者只接受 `.SFCommunity` 与 `session_PC`。调用方不能向任一客户端传入 Cookie，因此不会将 Web Cookie 发送给 App API，或将 App Cookie 发送给网页请求。
- `WebClient` 在发出请求前仅保留 `.SFCommunity` 与 `session_PC`，拒绝 `session_APP` 和未知 `session_*`。文本正文现在网页优先，网页正文不可用时才按照 `appFallbackEnabled` 设置回退 App API；有声目录/媒体、漫画目录/章节/图片及网页封面统一经过 `WebClient`。
- App 会话和 Web 会话改为独立的原生字段。App 请求只能读取 `.SFCommunity` 与 `session_APP`，Web 请求只能读取 `.SFCommunity` 与 `session_PC`；未知的 `session_*` 不再收集或转发。
- 官方网页登录不再调用 App `/user` 验证，也不会把 Web Cookie 持久化为 App 会话。密码登录只创建 App 会话，并且 Android 不再把它写入 WebView Cookie Jar。
- `AuthStatus` 分别返回 `appAuthenticated` 与 `webAuthenticated`。账户弹窗使用 `GetLoginInfo.ashx` 验证 Web 会话并读取昵称、头像；随后从 `m.sfacg.com/my/` 读取火券、代券和月票。VIP 体系由 `pages.sfacg.com/api/User?expand=newVip` 的 `data.expand.newVip.isNewVip` 决定：`true` 时读取并展示新 VIP 的等级和名称，`false` 时读取 `pages.sfacg.com/api/common/vipInfo` 并展示旧 VIP 等级，不能通过等级数值或两个接口是否都返回数据判断。App 会话仅可选地补充金币及其账户资料，不能作为账户弹窗的前提。书架、有声、漫画和网页正文回退使用 Web 会话；App 正文仍使用 App 会话。
- 固定设备令牌被替换为每个安装唯一且稳定的 UUID v4。Windows 使用 DPAPI 加密保存；Android 使用应用私有存储保存，并使用 Android Keystore 加密 App 会话。设备令牌不会写入日志或渲染层。
- 旧版未标记会话文件不再恢复。升级后的用户必须重新登录，以消除既有混合 Cookie 与旧固定设备令牌的关联。
- Web 登录不会被误认为 App 登录；有声功能和账户资料、余额、月票及新 VIP 资料会明确要求 Web 会话。密码登录成功后仅为设备上报诊断读取一次 `accountId`，不会把诊断所需账户数据返回给渲染层。

密码登录成功后会非阻塞地尝试 `POST /user/androiddeviceinfos`，使用当前安装 UUID 作为 `deviceId`；上报失败不会撤销登录，也不能证明该接口可以授予 App API 权限。`shuMeiId`、签到、广告、分享、阅读时长和多账号自动化仍保持关闭。
