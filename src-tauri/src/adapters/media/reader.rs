//! 在线浏览用例的原生适配器。

use super::inline_image::{split as split_inline_images, InlineSegment};
use super::protocol::validate_sfacg_asset_url;
use super::{online_media_url, OnlineMediaResource, OnlineMediaState};
use crate::adapters::sfacg::source::SfSource;
use crate::adapters::storage::paths::online_cache_directory;
use crate::adapters::storage::NativeStorage;
use crate::application::ports::OnlineReader;
use crate::application::ports::RemoteGateway;
use crate::domain::content::{OnlineAudioTrack, OnlineComicChapter, OnlineTextChapter};
use crate::domain::error::{AppError, AppResult};
use crate::domain::ids::NovelId;
use async_trait::async_trait;
use std::fs;
use std::path::Path;
use tauri::Manager;

/// 使用 SF 内容源和本地缓存读取在线正文、漫画与音频的适配器。
pub(crate) struct OnlineReaderAdapter {
    /// 访问应用状态、缓存目录和媒体令牌所需的句柄。
    app: tauri::AppHandle,
}

impl OnlineReaderAdapter {
    /// 创建在线阅读适配器。
    ///
    /// # 参数
    /// - `app`：Tauri 应用句柄。
    pub(crate) fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

#[async_trait]
impl OnlineReader for OnlineReaderAdapter {
    /// 读取在线文字章节，并缓存正文图片。
    ///
    /// # 参数
    /// - `novel_id`：小说编号。
    /// - `chapter_id`：章节编号。
    ///
    /// # 错误
    /// 目录、正文或图片读取失败时返回上游错误。
    async fn load_text(&self, novel_id: NovelId, chapter_id: i64) -> AppResult<OnlineTextChapter> {
        load_text(&self.app, novel_id, chapter_id).await
    }

    /// 读取在线漫画章节，并为页面登记媒体令牌。
    ///
    /// # 参数
    /// - `comic_id`：漫画编号。
    /// - `chapter_id`：章节编号。
    /// - `chapter_title`：渲染进程展示的章节标题。
    /// - `_source_path`：保留的来源路径，漫画已改用 App API 读取。
    ///
    /// # 错误
    /// 目录或页面列表无法读取时返回上游错误。
    async fn load_comic(
        &self,
        comic_id: i64,
        chapter_id: i64,
        chapter_title: &str,
        _source_path: Option<&str>,
    ) -> AppResult<OnlineComicChapter> {
        load_comic(&self.app, comic_id, chapter_id, chapter_title).await
    }

    /// 读取在线音频并登记本地流式播放令牌。
    ///
    /// # 参数
    /// - `chapter_id`：音频章节编号。
    /// - `chapter_title`：前端缓存目录中的章节标题。
    /// - `chapter_volume`：前端缓存目录中的分卷名称。
    ///
    /// # 错误
    /// 章节或音频地址无法解析时返回上游错误。
    async fn load_audio(
        &self,
        chapter_id: i64,
        chapter_title: &str,
        chapter_volume: &str,
    ) -> AppResult<OnlineAudioTrack> {
        load_audio(&self.app, chapter_id, chapter_title, chapter_volume).await
    }
}

/// 实现文字章节读取、VIP 图片回退和正文图片缓存。
///
/// # 参数
/// - `app`：应用句柄。
/// - `novel_id`：小说编号。
/// - `chapter_id`：章节编号。
///
/// # 错误
/// 章节不在目录中、正文读取失败或缓存写入失败时返回错误消息。
async fn load_text(
    app: &tauri::AppHandle,
    novel_id: NovelId,
    chapter_id: i64,
) -> AppResult<OnlineTextChapter> {
    let source = SfSource::connect(app).await?;
    let catalog = source.text_volumes(novel_id).await?;
    // 后续步骤只用于拼 URL、做缓存目录名和日志，统一回到基础整数编号。
    let novel_id = novel_id.get();
    let mut selected = None;
    for volume in &catalog {
        if let Some(chapter) = volume
            .chapters
            .iter()
            .find(|chapter| chapter.chap_id == chapter_id)
        {
            selected = Some((volume, chapter));
            break;
        }
    }
    let (volume, chapter) = selected.ok_or_else(|| AppError::not_found("该章节不在当前作品目录中"))?;
    let cache_root = online_cache_directory(app)?;
    let image_directory = cache_root.join("text").join(novel_id.to_string());
    let referer = format!(
        "https://book.sfacg.com/Novel/{novel_id}/{}/{chapter_id}/",
        volume.volume_id
    );
    let storage = NativeStorage::new(app.clone());
    // 读取设置失败不应阻断阅读，退化为不保留 OCR 中间文件。
    let keep_process_files = storage
        .settings()
        .map(|settings| settings.download.ocr_process_files_enabled)
        .unwrap_or(false);
    let vip_cache = cache_root.join("vip");
    let mut content = if chapter.is_vip {
        match source
            .try_direct_text_chapter(chapter_id, |raw, encrypted| {
                storage.decode_api_content(raw, encrypted)
            })
            .await
        {
            Some(Ok(content)) => content,
            Some(Err(error)) => return Err(error),
            None => {
                load_vip_text_content(
                    app,
                    &source,
                    &vip_cache,
                    novel_id,
                    chapter_id,
                    keep_process_files,
                )
                .await?
            }
        }
    } else {
        match source
            .read_chapter_text(novel_id, volume.volume_id, chapter_id, |raw, encrypted| {
                storage.decode_api_content(raw, encrypted)
            })
            .await
        {
            Ok(result) => result.content,
            Err(error) if source.has_app_session() || error.is_risk_control() => return Err(error),
            Err(error) => load_vip_text_content(
                app,
                &source,
                &vip_cache,
                novel_id,
                chapter_id,
                keep_process_files,
            )
            .await
            .map_err(|vip_error| {
                AppError::upstream(format!("{error}；VIP 图片正文不可用：{vip_error}"))
            })?,
        }
    };
    let images = if content.contains("[img=") {
        let (next, images) = cache_text_images(
            &source,
            &image_directory,
            novel_id,
            chapter_id,
            &referer,
            &content,
        )
        .await?;
        content = next;
        images
    } else {
        Vec::new()
    };
    Ok(OnlineTextChapter {
        id: chapter.chap_id,
        title: chapter.title.clone(),
        volume: volume.title.clone(),
        content,
        images,
    })
}

/// 读取漫画页面地址并转换为本地媒体协议令牌。
///
/// # 参数
/// - `app`：应用句柄。
/// - `comic_id`：漫画编号。
/// - `chapter_id`：章节编号。
/// - `chapter_title`：章节标题。
/// # 错误
/// 缓存目录创建、目录查找或页面地址读取失败时返回错误消息。
async fn load_comic(
    app: &tauri::AppHandle,
    comic_id: i64,
    chapter_id: i64,
    chapter_title: &str,
) -> AppResult<OnlineComicChapter> {
    let directory = online_cache_directory(app)?
        .join("comics")
        .join(comic_id.to_string())
        .join(chapter_id.to_string());
    fs::create_dir_all(&directory).map_err(|error| AppError::storage(format!("无法创建漫画在线缓存目录：{error}")))?;
    let client = SfSource::connect(app).await?;
    let catalog = client
        .comic_catalog(comic_id)
        .await?;
    let path_name = catalog
        .iter()
        .find(|chapter| chapter.id == chapter_id)
        .map(|chapter| chapter.path_name.as_str())
        .ok_or_else(|| AppError::not_found("该章节不在当前漫画目录中"))?;
    let referer = format!("https://m.sfacg.com/comic/{comic_id}/");
    let image_urls = client.comic_images(comic_id, path_name).await?;
    if image_urls.is_empty() {
        return Err(AppError::not_found("该漫画章节没有可读取的页面"));
    }
    let media = app.state::<OnlineMediaState>();
    let mut pages = Vec::with_capacity(image_urls.len());
    for (index, url) in image_urls.iter().enumerate() {
        let source = validate_sfacg_asset_url(url)?;
        let token = media.register(OnlineMediaResource::ComicPage {
            source: source.to_string(),
            referer: referer.clone(),
            cache_path: directory.join(format!("{:03}.image", index + 1)),
        })?;
        pages.push(online_media_url("comic", &token));
    }
    Ok(OnlineComicChapter {
        id: chapter_id,
        title: chapter_title.to_string(),
        pages,
    })
}

/// 解析有声专辑、章节及音频地址，并登记流式播放令牌。
///
/// # 参数
/// - `app`：应用句柄。
/// - `chapter_id`：音频章节编号。
/// - `chapter_title`：前端缓存目录中的章节标题。
/// - `chapter_volume`：前端缓存目录中的分卷名称。
///
/// # 错误
/// 音频地址非法或媒体状态不可用时返回错误消息。
async fn load_audio(
    app: &tauri::AppHandle,
    chapter_id: i64,
    chapter_title: &str,
    chapter_volume: &str,
) -> AppResult<OnlineAudioTrack> {
    let client = SfSource::connect(app).await?;
    let source = client.audio_source(chapter_id).await?;
    let source = validate_sfacg_asset_url(&source)?;
    let media = app.state::<OnlineMediaState>();
    let token = media.register(OnlineMediaResource::Audio {
        source: source.to_string(),
        referer: format!("https://m.sfacg.com/a/{chapter_id}/"),
    })?;
    Ok(OnlineAudioTrack {
        id: chapter_id,
        title: chapter_title.to_string(),
        volume: chapter_volume.to_string(),
        url: online_media_url("audio", &token),
    })
}

/// 下载并缓存一张受保护的上游图片。
///
/// # 参数
/// - `client`：已认证的 SF 内容源。
/// - `url`：上游图片地址。
/// - `referer`：请求图片所需的 Referer。
/// - `target_without_extension`：不含扩展名的缓存目标路径。
///
/// # 返回
/// 实际写入的缓存文件路径。
///
/// # 错误
/// 地址非法、请求失败、内容为空或写入失败时返回错误消息。
async fn cache_remote_image(
    client: &SfSource,
    url: &str,
    referer: &str,
    target_without_extension: &Path,
) -> AppResult<String> {
    let url = validate_sfacg_asset_url(url)?;
    let image = super::fetch::fetch_remote_image(client, url.as_str(), referer, None).await?;
    let extension = super::fetch::image_extension(&image.content_type, &image.bytes);
    let target = target_without_extension.with_extension(extension);
    crate::adapters::storage::replace_file(&target, &image.bytes, "在线浏览缓存")?;
    Ok(target.to_string_lossy().to_string())
}

/// 将正文中的图片标记替换为缓存图片占位符。
///
/// 标记解析由 `super::inline_image::split` 完成，与下载侧共用同一套规则；
/// 这里只决定"取图成功写占位符、失败写占位文本"的降级策略。
///
/// # 参数
/// - `client`：已认证的 SF 内容源。
/// - `directory`：本章节图片缓存目录。
/// - `novel_id`：小说编号，用于日志。
/// - `chapter_id`：章节编号，用于文件名和日志。
/// - `referer`：请求图片所需的 Referer。
/// - `content`：包含 `[img=...]...[/img]` 标记的正文。
///
/// # 返回
/// 替换后的正文和成功缓存的图片路径。
///
/// # 错误
/// 缓存目录无法创建时返回错误；单张图片失败会记录日志并保留占位文本。
async fn cache_text_images(
    client: &SfSource,
    directory: &Path,
    novel_id: i64,
    chapter_id: i64,
    referer: &str,
    content: &str,
) -> AppResult<(String, Vec<String>)> {
    fs::create_dir_all(directory).map_err(|error| AppError::storage(format!("无法创建正文图片缓存目录：{error}")))?;
    let mut output = String::with_capacity(content.len());
    let mut images = Vec::new();

    for segment in split_inline_images(content) {
        let url = match segment {
            InlineSegment::Text(text) => {
                output.push_str(text);
                continue;
            }
            InlineSegment::Image(url) => url,
        };

        let index = images.len();
        let target = directory.join(format!("chapter-{chapter_id}-image-{index}"));
        match cache_remote_image(client, url, referer, &target).await {
            Ok(path) => {
                output.push_str(&format!("{{{{image:{index}}}}}"));
                images.push(path);
            }
            Err(error) => {
                if error.is_risk_control() {
                    return Err(error);
                }
                log_warn!(
                    "[sfacg] online text image skipped: novel_id={novel_id}, chapter_id={chapter_id}, error={error}"
                );
                output.push_str("\n[插图暂时无法加载]\n");
            }
        }
    }

    Ok((output, images))
}

/// 下载 VIP 图片正文，执行 OCR 并返回识别文本。
///
/// 与下载任务共用 `super::vip::recognize_vip_chapter`，因此「是否保留 OCR 中间图」
/// 这类设置对在线阅读同样生效，不会出现同一章因入口不同而结果不同。
///
/// # 参数
/// - `app`：应用句柄。
/// - `source`：已认证的 SF 内容源。
/// - `cache_directory`：VIP 图片缓存目录。
/// - `novel_id`：小说编号。
/// - `chapter_id`：章节编号。
/// - `keep_process_files`：是否保留 OCR 中间切片。
///
/// # 错误
/// 图片读取、缓存写入或 OCR 失败时返回错误消息。
async fn load_vip_text_content(
    app: &tauri::AppHandle,
    source: &SfSource,
    cache_directory: &Path,
    novel_id: i64,
    chapter_id: i64,
    keep_process_files: bool,
) -> AppResult<String> {
    // 识别结果按章节缓存：同一章的图片正文不会变，重复打开时不必再跑一次 OCR。
    let cached = cache_directory.join(format!("chapter-{chapter_id}.txt"));
    if let Ok(content) = fs::read_to_string(&cached) {
        if !content.trim().is_empty() {
            return Ok(content);
        }
    }
    let recognized = super::vip::recognize_vip_chapter(
        app,
        source,
        cache_directory,
        novel_id,
        chapter_id,
        keep_process_files,
    )
    .await?;
    if !recognized.content.trim().is_empty() {
        crate::adapters::storage::replace_file(
            &cached,
            recognized.content.as_bytes(),
            "VIP 正文识别缓存",
        )?;
    }
    Ok(recognized.content)
}
