//! SFACG 对外能力与会话边界注册表。
//!
//! 本模块是上游接口选择的唯一入口：它登记每项用户可见能力使用的传输协议、
//! 对应的上游路由模板、是否需要指定登录凭证，以及正文的可选回退关系。
//!
//! `AppClient` 与 `WebClient` 只负责各自协议的请求细节。本模块不读取、存储或
//! 返回 Cookie；它只在构造客户端时检查该能力是否具备所需会话，并给出面向用户
//! 的、可操作的错误信息。

use crate::app_client::AppClient;
use crate::web_client::WebClient;

/// 用户可见的 SFACG 远程能力。
///
/// 枚举项以业务能力命名，而不是以前端页面或某次请求 URL 命名，避免调用方自行
/// 推断该使用 App 还是网页凭证。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EndpointCapability {
    /// 通过 App 协议提交账号密码并换取 App 会话。
    AppPasswordLogin,
    /// 搜索小说、有声与漫画。
    Search,
    /// 读取小说详情。
    NovelDetail,
    /// 读取有声专辑详情。
    AudioDetail,
    /// 读取文字小说目录。
    TextDirectory,
    /// 从网页读取文字小说目录及 VIP 正文类型。
    TextDirectoryWeb,
    /// 从网页读取文字正文。
    TextChapterWeb,
    /// 从网页 AJAX 获取已授权 VIP 章节的图片或 GIF 正文。
    TextVipImageWeb,
    /// 从 App API 读取文字正文。
    TextChapterApp,
    /// 读取有声目录及后续媒体资源。
    Audio,
    /// 读取漫画的 App 公开元数据。
    ComicIdentity,
    /// 读取漫画网页目录。
    ComicCatalog,
    /// 读取漫画网页章节、图片列表与图片资源。
    ComicPages,
    /// 读取公开网页火袋中的小说与漫画。
    WebBookshelf,
    /// 验证 Web 登录会话并读取网页可提供的基础账户资料。
    WebAccountProfile,
    /// 读取 App 账户资料与余额。
    AccountProfile,
    /// 在登录后提交实验性设备信息。
    AndroidDeviceReport,
}

/// 一个能力所使用的原生网络客户端。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClientKind {
    /// 带签名的 `api.sfacg.com` App 协议。
    App,
    /// 官方网页、网页 AJAX 与静态资源协议。
    Web,
}

/// 能力是否要求对应客户端拥有登录会话。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SessionRequirement {
    /// 上游公开内容可匿名访问，已连接时仍会使用本端会话。
    Optional,
    /// 缺少本端会话时不发起请求，直接返回引导信息。
    Required,
}

/// 可审计的对外接口登记项。
///
/// `route_template` 是路由族的文档化模板，不含用户数据或认证材料。动态 ID 由
/// 客户端实现负责填充；调用方不得绕开本表自行选择会话。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EndpointPolicy {
    /// 供日志与诊断使用的稳定能力名。
    pub(crate) name: &'static str,
    /// 当前能力使用的原生客户端类型。
    pub(crate) client: ClientKind,
    /// 对外请求的域名与路由模板。
    pub(crate) route_template: &'static str,
    /// 发起此能力所需的登录状态。
    pub(crate) session: SessionRequirement,
}

impl EndpointCapability {
    /// 返回本能力唯一对应的接口登记项。
    pub(crate) const fn policy(self) -> EndpointPolicy {
        use ClientKind::{App, Web};
        use EndpointCapability::*;
        use SessionRequirement::{Optional, Required};

        match self {
            AppPasswordLogin => EndpointPolicy {
                name: "App 登录",
                client: App,
                route_template: "POST api.sfacg.com/sessions",
                session: Optional,
            },
            Search => EndpointPolicy {
                name: "搜索",
                client: App,
                route_template: "GET api.sfacg.com/search/novels/result/new",
                session: Optional,
            },
            NovelDetail => EndpointPolicy {
                name: "小说详情",
                client: App,
                route_template: "GET api.sfacg.com/novels/{novelId}",
                session: Optional,
            },
            AudioDetail => EndpointPolicy {
                name: "有声专辑详情",
                client: App,
                route_template: "GET api.sfacg.com/albums/{albumId}",
                session: Optional,
            },
            TextDirectory => EndpointPolicy {
                name: "文字目录",
                client: App,
                route_template: "GET api.sfacg.com/novels/{novelId}/dirs",
                session: Optional,
            },
            TextDirectoryWeb => EndpointPolicy {
                name: "网页文字目录",
                client: Web,
                route_template: "GET book.sfacg.com/Novel/{novelId}/MainIndex/",
                session: Required,
            },
            TextChapterWeb => EndpointPolicy {
                name: "网页正文",
                client: Web,
                route_template: "GET book.sfacg.com/Novel/{novelId}/{volumeId}/{chapterId}/",
                session: Optional,
            },
            TextVipImageWeb => EndpointPolicy {
                name: "网页 VIP 图片正文",
                client: Web,
                route_template: "GET book.sfacg.com/ajax/ashx/common.ashx?op=getChapPic",
                session: Required,
            },
            TextChapterApp => EndpointPolicy {
                name: "App 正文",
                client: App,
                route_template: "GET api.sfacg.com/Chaps/{chapterId}",
                session: Required,
            },
            Audio => EndpointPolicy {
                name: "有声目录与媒体",
                client: Web,
                route_template: "GET i.sfacg.com/ajax/ashx/Common.ashx?op=getAudioInfo",
                session: Required,
            },
            ComicIdentity => EndpointPolicy {
                name: "漫画详情",
                client: App,
                route_template: "GET api.sfacg.com/comics/{comicId}",
                session: Optional,
            },
            ComicCatalog => EndpointPolicy {
                name: "漫画目录",
                client: Web,
                route_template: "GET manhua.sfacg.com/mh/{folder}/",
                session: Optional,
            },
            ComicPages => EndpointPolicy {
                name: "漫画章节与图片",
                client: Web,
                route_template:
                    "GET manhua.sfacg.com/mh/{folder}/{chapterId}/; GET ajax/Common.ashx?op=getPics",
                session: Optional,
            },
            WebBookshelf => EndpointPolicy {
                name: "网页书架",
                client: Web,
                route_template:
                    "GET passport.sfacg.com/Ajax/GetLoginInfo.ashx; GET p.sfacg.com/u/{name}/",
                session: Required,
            },
            WebAccountProfile => EndpointPolicy {
                name: "网页账户资料、余额与 VIP",
                client: Web,
                route_template: "GET passport.sfacg.com/Ajax/GetLoginInfo.ashx; GET m.sfacg.com/my/; GET pages.sfacg.com/api/User?expand=newVip; GET pages.sfacg.com/api/common/vipInfo",
                session: Required,
            },
            AccountProfile => EndpointPolicy {
                name: "账户资料与余额",
                client: App,
                route_template: "GET api.sfacg.com/user; GET api.sfacg.com/user/money",
                session: Required,
            },
            AndroidDeviceReport => EndpointPolicy {
                name: "设备信息实验",
                client: App,
                route_template: "POST api.sfacg.com/user/androiddeviceinfos",
                session: Required,
            },
        }
    }

    /// 返回网页正文失败时可展示的完整后续操作说明。
    pub(crate) fn unavailable_message(self, source_error: &str) -> String {
        let policy = self.policy();
        match self {
            Self::TextChapterWeb => format!(
                "{}不可用：{source_error}。请先确认章节可在官方网站访问；如该章节仅支持 App，可连接 App 凭证并在请求设置中开启 App 正文回退后重试。",
                policy.name
            ),
            Self::Audio => format!(
                "{}不可用：{source_error}。该能力仅使用官方网站登录凭证，请完成网站登录后重试。",
                policy.name
            ),
            Self::ComicCatalog | Self::ComicPages => format!(
                "{}不可用：{source_error}。请确认官方网站页面可访问；受限章节还需要上游目录明确标记为已解锁。",
                policy.name
            ),
            _ => format!("{}不可用：{source_error}", policy.name),
        }
    }
}

/// 为 App 能力构造经过登记项校验的客户端。
///
/// # 错误
/// 当能力未登记为 App、App 会话缺失，或客户端初始化失败时返回错误。缺少会话时
/// 不会向上游发起请求。
pub(crate) fn app_endpoint_client(
    app: &tauri::AppHandle,
    capability: EndpointCapability,
) -> Result<AppClient, String> {
    let policy = capability.policy();
    if policy.client != ClientKind::App {
        return Err(format!(
            "内部接口映射错误：{} 不属于 App 客户端",
            policy.name
        ));
    }
    let client = AppClient::new(app)?;
    if policy.session == SessionRequirement::Required && !client.has_session() {
        return Err(format!(
            "{}需要连接 App 凭证，请先完成 App 登录。",
            policy.name
        ));
    }
    Ok(client)
}

/// 为网页能力构造经过登记项校验的客户端。
///
/// # 错误
/// 当能力未登记为网页、网站会话缺失，或客户端初始化失败时返回完整操作提示。公开
/// 能力即使没有 `session_PC` 也允许继续请求。
pub(crate) fn web_endpoint_client(
    app: &tauri::AppHandle,
    capability: EndpointCapability,
) -> Result<WebClient, String> {
    let policy = capability.policy();
    if policy.client != ClientKind::Web {
        return Err(format!(
            "内部接口映射错误：{} 不属于网页客户端",
            policy.name
        ));
    }
    let client = WebClient::new(app)?;
    if policy.session == SessionRequirement::Required && !client.has_session() {
        return Err(capability.unavailable_message("尚未连接网站凭证"));
    }
    Ok(client)
}
