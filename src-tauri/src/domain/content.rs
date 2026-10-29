//! 正文来源与解析结果。

use serde::Serialize;

/// 文字正文的实际来源。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TextOrigin {
    /// 正文来自 App 签名接口。
    App,
    /// 正文来自网页渠道。
    Web,
}

impl TextOrigin {
    /// 返回写入持久化记录时使用的稳定来源标识。
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::Web => "web",
        }
    }
}

/// 一次文字正文读取的结果。
pub(crate) struct TextChapter {
    /// 去除包装后的正文。
    pub(crate) content: String,
    /// 正文实际来源。
    pub(crate) origin: TextOrigin,
}

/// API 章节正文及与网页正文比对所需的标识。
pub(crate) struct ApiChapterContent {
    /// API 返回的正文。
    pub(crate) content: String,
    /// 正文所属作品编号。
    pub(crate) novel_id: i64,
    /// 正文所属分卷编号。
    pub(crate) volume_id: i64,
}

/// 已通过格式校验的远程图片。二进制内容只在原生层流转。
pub(crate) struct ImageAsset {
    /// 已通过校验的图片二进制内容。
    pub(crate) bytes: Vec<u8>,
    /// 根据 MIME 类型推导出的文件扩展名。
    pub(crate) extension: &'static str,
}

/// 在线文字章节返回给渲染进程的数据。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OnlineTextChapter {
    /// 章节编号。
    pub(crate) id: i64,
    /// 章节标题。
    pub(crate) title: String,
    /// 所属分卷名称。
    pub(crate) volume: String,
    /// Markdown 或纯文本正文。
    pub(crate) content: String,
    /// 正文中出现的图片协议地址。
    pub(crate) images: Vec<String>,
}

/// 在线漫画章节返回给渲染进程的数据。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OnlineComicChapter {
    /// 章节编号。
    pub(crate) id: i64,
    /// 章节标题。
    pub(crate) title: String,
    /// 按阅读顺序排列的页面协议地址。
    pub(crate) pages: Vec<String>,
}

/// 在线音频章节的流式播放地址。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OnlineAudioTrack {
    /// 有声章节编号。
    pub(crate) id: i64,
    /// 章节标题。
    pub(crate) title: String,
    /// 所属分卷名称。
    pub(crate) volume: String,
    /// 用于流式播放的本地协议地址。
    pub(crate) url: String,
}
