//! 远程作品目录、章节和账户书架模型。

use serde::{Serialize, Serializer};

/// 搜索命令返回给渲染进程的一条公开作品记录。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchNovel {
    /// 小说编号。
    pub(crate) novel_id: i64,
    /// 可选的媒体或专辑编号。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) media_id: Option<i64>,
    /// 小说名称。
    pub(crate) novel_name: String,
    /// 作者名称。
    pub(crate) author_name: String,
    /// 封面地址。
    pub(crate) novel_cover: String,
    /// 上游给出的最近更新时间。
    pub(crate) last_update_time: String,
    /// 条目所在书架的名称。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) bookshelf_name: Option<String>,
    /// 条目所在书架的原始分类标识。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) bookshelf_type: Option<String>,
    /// 网页来源路径；仅公开书架条目需要携带。
    #[serde(rename = "sourcePath", skip_serializing_if = "Option::is_none")]
    pub(crate) source_path: Option<String>,
}

/// 章节选择视图使用的公开作品详细元数据。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NovelDetail {
    /// 小说编号。
    pub(crate) novel_id: i64,
    /// 小说名称。
    pub(crate) novel_name: String,
    /// 作者名称。
    pub(crate) author_name: String,
    /// 封面地址。
    pub(crate) novel_cover: String,
    /// 最近更新时间。
    pub(crate) last_update_time: String,
    /// 作品简介。
    pub(crate) description: String,
    /// 作品是否已完结。
    pub(crate) is_finish: bool,
    /// 作品分类名称。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) type_name: Option<String>,
    /// 作品标签。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tags: Option<Vec<String>>,
    /// 作品评分。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) score: Option<f64>,
    /// 章节总数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) chapter_count: Option<i64>,
    /// 正文字数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) character_count: Option<i64>,
    /// 浏览次数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) view_count: Option<i64>,
    /// 收藏或标记次数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) mark_count: Option<i64>,
    /// 推荐点数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) point_count: Option<i64>,
    /// 收藏数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) favorite_count: Option<i64>,
    /// 月票数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ticket_count: Option<i64>,
    /// 最新章节标题。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) latest_chapter_title: Option<String>,
    /// 最新章节更新时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) latest_chapter_time: Option<String>,
    /// 上游是否允许下载。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) allow_download: Option<bool>,
    /// 网页来源路径；存在时表示该作品需通过网页客户端读取。
    #[serde(rename = "sourcePath", skip_serializing_if = "Option::is_none")]
    pub(crate) source_path: Option<String>,
}

/// SF 小说单个分卷的渲染进程安全章节集合。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChapterVolume {
    /// 分卷编号。
    pub(crate) volume_id: i64,
    /// 分卷标题。
    pub(crate) title: String,
    /// 分卷内的章节摘要，按上游顺序排列。
    pub(crate) chapters: Vec<ChapterSummary>,
}

/// 渲染进程安全的章节可用性记录。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChapterSummary {
    /// 章节编号。
    pub(crate) chap_id: i64,
    /// 章节标题。
    pub(crate) title: String,
    /// 章节是否为 VIP 内容。
    pub(crate) is_vip: bool,
    /// 目录标记的正文形态。
    pub(crate) content_kind: TextContentKind,
    /// 当前会话可确认的访问状态。
    pub(crate) access_state: TextAccessState,
    /// 该章节是否已经存在于本地书库。
    pub(crate) downloaded: bool,
}

/// 网页文字目录标记的章节正文形态。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub(crate) enum TextContentKind {
    /// 可直接读取的文字正文。
    Text,
    /// 需要登录或解锁权限的图片正文。
    ImageVip,
    /// 需要恢复字典解密的加密正文。
    EncryptedVip,
    /// 目录未给出足够信息判断正文形态。
    Unknown,
}

/// 一次网页目录读取可确认的章节访问状态。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub(crate) enum TextAccessState {
    /// 仅凭目录无法判断访问权限。
    Unknown,
    /// 当前会话可读取该章节。
    Available,
    /// 当前会话不可读取该章节。
    Unavailable,
    /// 网页会话已失效，需重新登录。
    SessionExpired,
}

/// 返回给渲染进程且不含会话数据的已登录 SF 书架。
#[derive(Clone, Debug, Serialize)]
pub(crate) struct BookshelfCollection {
    /// 书架分类名称列表。
    pub(crate) categories: Vec<String>,
    /// 已解析并去重后的书架条目。
    pub(crate) items: Vec<SearchNovel>,
}

/// 仅内部使用的有声章节记录。
pub(crate) struct AudioChapter {
    /// 有声章节编号。
    pub(crate) id: i64,
    /// 章节标题。
    pub(crate) title: String,
    /// 所属分卷名称。
    pub(crate) volume: String,
    /// 章节是否为 VIP 内容。
    pub(crate) is_vip: bool,
    /// 当前账号是否已解锁该章节。
    pub(crate) is_unlocked: bool,
}

/// 单个 SF 作品的有声目录。
#[derive(Debug, Serialize)]
pub(crate) struct AudioCatalog {
    /// 有声作品标题。
    pub(crate) title: String,
    /// 渲染进程可见的章节列表。
    pub(crate) chapters: Vec<AudioChapterSummary>,
}

/// 不含受保护上游音频地址的有声章节行。
#[derive(Debug, Serialize)]
pub(crate) struct AudioChapterSummary {
    /// 有声章节编号。
    pub(crate) id: i64,
    /// 章节标题。
    pub(crate) title: String,
    /// 所属分卷名称。
    pub(crate) volume: String,
    /// 章节是否为 VIP 内容。
    pub(crate) is_vip: bool,
    /// 当前账号是否已解锁该章节。
    pub(crate) is_unlocked: bool,
    /// 该章节是否已经下载到本地。
    pub(crate) downloaded: bool,
}

/// 漫画章节的账户拥有权状态。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ComicUnlockState {
    /// 当前账号已解锁或章节公开可用。
    Unlocked,
    /// 当前账号无法读取该章节。
    Locked,
    /// 上游未提供足够信息判断解锁状态。
    Unknown,
}

impl ComicUnlockState {
    /// 判断章节是否允许进入阅读或下载流程。
    ///
    /// `Locked` 明确不可选；`Unlocked` 和 `Unknown` 允许继续请求。
    pub(crate) fn is_selectable(self) -> bool {
        self != Self::Locked
    }
}

impl Serialize for ComicUnlockState {
    /// 按前端契约序列化为 `true`、`false` 或字符串 `"unknown"`。
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Unlocked => serializer.serialize_bool(true),
            Self::Locked => serializer.serialize_bool(false),
            Self::Unknown => serializer.serialize_str("unknown"),
        }
    }
}

/// 仅内部使用的漫画章节记录。
pub(crate) struct ComicChapter {
    /// 漫画章节编号。
    pub(crate) id: i64,
    /// 章节标题。
    pub(crate) title: String,
    /// 章节是否为 VIP 内容。
    pub(crate) is_vip: bool,
    /// 当前账号的解锁状态。
    pub(crate) is_unlocked: ComicUnlockState,
}

/// 章节选择器使用的漫画目录。
#[derive(Debug, Serialize)]
pub(crate) struct ComicCatalog {
    /// 漫画作品标题。
    pub(crate) title: String,
    /// 渲染进程可见的章节列表。
    pub(crate) chapters: Vec<ComicChapterSummary>,
}

/// 不含页面地址和来源目录信息的一条漫画章节。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ComicChapterSummary {
    /// 漫画章节编号。
    pub(crate) id: i64,
    /// 章节标题。
    pub(crate) title: String,
    /// 章节是否为 VIP 内容。
    pub(crate) is_vip: bool,
    /// 当前账号的解锁状态。
    pub(crate) is_unlocked: ComicUnlockState,
    /// 该章节是否已经下载到本地。
    pub(crate) downloaded: bool,
}

/// 从公开网页书架解析出的一条小说或漫画记录。
pub(crate) struct PublicShelfItem {
    /// 条目类型，取值为 `novel` 或 `comic`。
    pub(crate) kind: &'static str,
    /// 条目对应的作品编号。
    pub(crate) id: i64,
    /// 作品标题。
    pub(crate) title: String,
    /// 作者名称。
    pub(crate) author: String,
    /// 封面地址。
    pub(crate) cover: String,
    /// 网页来源路径。
    pub(crate) source_path: Option<String>,
}

/// 公开书架分类及解析出的条目。
pub(crate) struct PublicShelf {
    /// 公开书架页面中的分类名称。
    pub(crate) categories: Vec<String>,
    /// 分类名称与对应条目的有序集合。
    pub(crate) items: Vec<(String, PublicShelfItem)>,
}
