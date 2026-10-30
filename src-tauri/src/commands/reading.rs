//! 在线阅读与媒体播放命令。

use crate::adapters::media::OnlineReaderAdapter;
use crate::application::reading::ReadingService;
use crate::domain::content::{OnlineAudioTrack, OnlineComicChapter, OnlineTextChapter};
use crate::domain::error::AppError;

#[tauri::command]
/// 读取在线文字章节。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `novel_id`：小说编号。
/// - `chapter_id`：章节编号。
///
/// # 错误
/// 编号非法、未登录或正文读取失败时返回错误消息。
pub(crate) async fn load_online_text_chapter(
    app: tauri::AppHandle,
    novel_id: i64,
    chapter_id: i64,
) -> Result<OnlineTextChapter, String> {
    let reader = OnlineReaderAdapter::new(app);
    ReadingService::new(&reader)
        .text(novel_id, chapter_id)
        .await
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 读取在线漫画章节。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `comic_id`：漫画编号。
/// - `chapter_id`：章节编号。
/// - `chapter_title`：章节标题，最长 300 个字符。
/// - `source_path`：可选的网页来源路径。
///
/// # 错误
/// 编号或标题非法、未登录或页面读取失败时返回错误消息。
pub(crate) async fn load_online_comic_chapter(
    app: tauri::AppHandle,
    comic_id: i64,
    chapter_id: i64,
    chapter_title: String,
    source_path: Option<String>,
) -> Result<OnlineComicChapter, String> {
    let reader = OnlineReaderAdapter::new(app);
    ReadingService::new(&reader)
        .comic(comic_id, chapter_id, chapter_title, source_path)
        .await
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 读取在线音频的流式播放地址。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `chapter_id`：音频章节编号。
/// - `chapter_title`：前端已缓存目录中的章节标题。
/// - `chapter_volume`：前端已缓存目录中的分卷名称。
///
/// # 错误
/// 章节编号或标题非法，或上游拒绝请求时返回错误消息。
pub(crate) async fn load_online_audio_track(
    app: tauri::AppHandle,
    chapter_id: i64,
    chapter_title: String,
    chapter_volume: String,
) -> Result<OnlineAudioTrack, String> {
    let reader = OnlineReaderAdapter::new(app);
    ReadingService::new(&reader)
        .audio(chapter_id, chapter_title, chapter_volume)
        .await
        .map_err(AppError::into_message)
}
