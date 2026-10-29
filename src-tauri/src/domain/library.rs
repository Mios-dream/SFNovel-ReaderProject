//! 本地书库的持久化与展示模型。

use serde::{Deserialize, Serialize};

/// 一个本地存储书籍的渲染进程安全摘要。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LibraryEntry {
    /// 书库目录名，同时也是展示名称。
    pub(crate) name: String,
    /// 本地书籍内容的最后更新时间。
    pub(crate) updated_at: String,
    /// 本地封面访问路径。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) cover: Option<String>,
    /// 已经存在的媒体格式。
    pub(crate) formats: AvailableFormats,
}

/// 描述一本书已存在的本地媒体格式。
#[derive(Debug, Serialize)]
pub(crate) struct AvailableFormats {
    /// 是否存在文字版本。
    pub(crate) text: bool,
    /// 是否存在有声版本。
    pub(crate) audio: bool,
    /// 是否存在漫画版本。
    pub(crate) comic: bool,
}

/// 本地书籍详情。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BookDetail {
    /// 本地书籍名称。
    pub(crate) name: String,
    /// 文字版本详情。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) novel: Option<BookWorkDetail>,
    /// 有声版本详情。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) audio: Option<BookWorkDetail>,
    /// 漫画版本详情。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) comic: Option<BookWorkDetail>,
    /// 本地图片目录协议地址。
    pub(crate) image_directory: String,
    /// 可播放的有声轨道。
    pub(crate) audio_tracks: Vec<AudioTrackView>,
    /// 已生成的 EPUB 文件地址。
    pub(crate) epub_href: Option<String>,
    /// 按分卷组织的文字章节。
    pub(crate) chapter_volumes: Vec<LibraryChapterVolume>,
    /// 本地漫画章节。
    pub(crate) comic_chapters: Vec<LibraryComicChapter>,
}

/// 恰好一种本地媒体类型的详情元数据。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BookWorkDetail {
    /// 作品编号。
    pub(crate) id: i64,
    /// 上游目录编号。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) catalog_id: Option<i64>,
    /// 上游在线路径。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) online_path: Option<String>,
    /// 作品标题。
    pub(crate) title: String,
    /// 作者名称。
    pub(crate) author: String,
    /// 作品简介。
    pub(crate) description: String,
    /// 作品分类名称。
    pub(crate) type_name: Option<String>,
    /// 作品标签。
    pub(crate) tags: Vec<String>,
    /// 作品是否已完结。
    pub(crate) is_finished: Option<bool>,
    /// 作品评分。
    pub(crate) score: Option<f64>,
    /// 章节总数。
    pub(crate) chapter_count: Option<i64>,
    /// 正文字数。
    pub(crate) character_count: Option<i64>,
    /// 浏览次数。
    pub(crate) view_count: Option<i64>,
    /// 收藏或标记次数。
    pub(crate) mark_count: Option<i64>,
    /// 推荐点数。
    pub(crate) point_count: Option<i64>,
    /// 收藏数。
    pub(crate) favorite_count: Option<i64>,
    /// 月票数。
    pub(crate) ticket_count: Option<i64>,
    /// 上游是否允许下载。
    pub(crate) allow_download: Option<bool>,
    /// 最新章节标题。
    pub(crate) latest_chapter_title: Option<String>,
    /// 最新章节更新时间。
    pub(crate) latest_chapter_time: Option<String>,
    /// 作品最近更新时间。
    pub(crate) last_update_time: Option<String>,
    /// 本地封面地址。
    pub(crate) cover: Option<String>,
}

/// 可在本地播放的一条有声轨道。
#[derive(Debug, Serialize)]
pub(crate) struct AudioTrackView {
    /// 轨道标题。
    pub(crate) title: String,
    /// 本地媒体协议地址。
    pub(crate) href: String,
}

/// 按来源顺序分组的本地文字章节卷。
#[derive(Debug, Serialize)]
pub(crate) struct LibraryChapterVolume {
    /// 分卷名称。
    pub(crate) volume: String,
    /// 分卷内的章节摘要。
    pub(crate) chapters: Vec<LibraryChapterSummary>,
}

/// 本地章节的渲染进程安全摘要。
#[derive(Debug, Serialize)]
pub(crate) struct LibraryChapterSummary {
    /// 章节编号。
    pub(crate) id: i64,
    /// 章节标题。
    pub(crate) title: String,
}

/// 由下载器写入的书籍持久化聚合。
#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BookManifest {
    /// 文字版本元数据。
    #[serde(default)]
    pub(crate) novel: Option<WorkMetadata>,
    /// 有声版本元数据。
    #[serde(default)]
    pub(crate) audio: Option<WorkMetadata>,
    /// 漫画版本元数据。
    #[serde(default)]
    pub(crate) comic: Option<WorkMetadata>,
    /// 已下载的文字章节编号。
    #[serde(default)]
    pub(crate) downloaded_text_chapter_ids: Option<Vec<i64>>,
    /// 已下载的有声章节编号。
    #[serde(default)]
    pub(crate) downloaded_audio_chapter_ids: Option<Vec<i64>>,
    /// 已下载的漫画章节编号。
    #[serde(default)]
    pub(crate) downloaded_comic_chapter_ids: Option<Vec<i64>>,
}

/// 一种媒体类型的持久化来源元数据。
#[derive(Debug, Deserialize, Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkMetadata {
    /// 作品编号。
    pub(crate) id: Option<i64>,
    /// 上游目录编号。
    pub(crate) catalog_id: Option<i64>,
    /// 上游在线路径。
    pub(crate) online_path: Option<String>,
    /// 作品标题。
    pub(crate) title: Option<String>,
    /// 作者名称。
    pub(crate) author: Option<String>,
    /// 作品简介。
    pub(crate) description: Option<String>,
    /// 作品分类名称。
    pub(crate) type_name: Option<String>,
    /// 作品标签。
    #[serde(default)]
    pub(crate) tags: Vec<String>,
    /// 作品是否已完结。
    pub(crate) is_finished: Option<bool>,
    /// 作品评分。
    pub(crate) score: Option<f64>,
    /// 章节总数。
    pub(crate) chapter_count: Option<i64>,
    /// 正文字数。
    pub(crate) character_count: Option<i64>,
    /// 浏览次数。
    pub(crate) view_count: Option<i64>,
    /// 收藏或标记次数。
    pub(crate) mark_count: Option<i64>,
    /// 推荐点数。
    pub(crate) point_count: Option<i64>,
    /// 收藏数。
    pub(crate) favorite_count: Option<i64>,
    /// 月票数。
    pub(crate) ticket_count: Option<i64>,
    /// 上游是否允许下载。
    pub(crate) allow_download: Option<bool>,
    /// 最新章节标题。
    pub(crate) latest_chapter_title: Option<String>,
    /// 最新章节更新时间。
    pub(crate) latest_chapter_time: Option<String>,
    /// 作品最近更新时间。
    pub(crate) last_update_time: Option<String>,
}

/// 章节索引中的一条持久化文字章节。
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredChapter {
    /// 章节编号。
    pub(crate) id: i64,
    /// 章节标题。
    pub(crate) title: String,
    /// 所属分卷名称。
    pub(crate) volume: String,
    /// 章节正文。
    pub(crate) content: String,
    /// 分卷在书中的顺序。
    pub(crate) volume_index: i64,
    /// 章节在分卷中的顺序。
    pub(crate) chapter_index: i64,
    /// 正文来源标识，例如 `app` 或 `web`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) content_source: Option<String>,
    /// 正文恢复时使用的 OCR 源文件路径。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ocr_source_path: Option<String>,
}

/// 一本书的持久化章节索引。
#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChapterIndex {
    /// 章节所属的作品编号。
    pub(crate) novel_id: i64,
    /// 以章节编号字符串为键的章节索引。
    pub(crate) chapters: std::collections::HashMap<String, StoredChapter>,
}

/// 本地存储的漫画章节及其有序页面文件。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LibraryComicChapter {
    /// 漫画章节编号。
    pub(crate) id: i64,
    /// 章节标题。
    pub(crate) title: String,
    /// 章节封面地址。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) cover: Option<String>,
    /// 按阅读顺序排列的页面地址。
    pub(crate) pages: Vec<String>,
}

/// 已生成的本地导出文件。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportFile {
    /// 渲染进程可访问的文件地址。
    pub(crate) href: String,
    /// 导出文件名。
    pub(crate) file_name: String,
}
