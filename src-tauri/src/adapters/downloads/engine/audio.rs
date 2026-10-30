//! 有声章节下载流程。

use super::assets::enrich_audio_metadata;
use super::JobContext;
use crate::adapters::downloads::registry;
use crate::adapters::downloads::DownloadState;
use crate::adapters::sfacg::source::SfSource;
use crate::adapters::storage::{
    read_book_manifest, safe_library_name, write_book_manifest, write_bytes_atomically,
    NativeStorage,
};
use crate::domain::catalog::AudioChapter;
use crate::domain::ids::validate_novel_id;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Manager;

/// 清理章节标题并限制为可安全用于文件名的长度。
///
/// # 参数
/// - `value`：上游章节标题。
fn safe_audio_name(value: &str) -> String {
    safe_library_name(value).chars().take(100).collect()
}

/// 使用 App 凭证下载有声章节。
async fn fetch_audio(source: &SfSource, url: &str, referer: &str) -> Result<Vec<u8>, String> {
    source
        .audio_asset_request(url, referer, "audio/mpeg,*/*;q=0.8")
        .send()
        .await
        .map_err(|error| format!("无法下载有声章节：{error}"))?
        .error_for_status()
        .map_err(|error| format!("有声章节下载被拒绝：{error}"))?
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|error| format!("无法读取有声章节：{error}"))
}

/// 根据已存在的音频文件生成 M3U 播放列表。
///
/// # 参数
/// - `audio_directory`：有声文件目录。
/// - `catalog`：有声目录，用于保持播放顺序。
///
/// # 错误
/// 播放列表写入失败时返回错误消息。
fn persist_audio_playlist(audio_directory: &Path, catalog: &[AudioChapter]) -> Result<(), String> {
    let mut playlist = vec!["#EXTM3U".to_string()];
    for (index, chapter) in catalog.iter().enumerate() {
        let file_name = format!("{:03} - {}.mp3", index + 1, safe_audio_name(&chapter.title));
        if audio_directory.join(&file_name).is_file() {
            playlist.push(format!("#EXTINF:-1,{} - {}", chapter.volume, chapter.title));
            playlist.push(file_name);
        }
    }
    fs::write(
        audio_directory.join("有声目录.m3u8"),
        format!("{}\n", playlist.join("\n")),
    )
    .map_err(|error| format!("无法写入有声播放列表：{error}"))
}

/// 写入有声任务的最终状态并持久化。
///
/// # 参数
/// - `context`：后台任务共享句柄。
/// - `result`：成功时返回播放列表地址，失败时返回错误消息。
fn mark_finished(context: &JobContext<'_>, result: Result<String, String>) {
    let JobContext {
        app,
        state,
        job_id,
        cancelled,
    } = *context;
    match result {
        Ok(file) => registry::update(state, job_id, |job| {
            job.status = "done".to_string();
            job.progress = 100;
            job.message = "有声章节下载完成".to_string();
            job.file = Some(file);
        }),
        Err(_) if cancelled.load(Ordering::Relaxed) => registry::update(state, job_id, |job| {
            job.status = "paused".to_string();
            job.message = "已暂停，可继续下载".to_string();
        }),
        Err(error) => registry::update(state, job_id, |job| {
            job.status = "error".to_string();
            job.message = error;
        }),
    }
    registry::emit_job(app, state, job_id);
    let _ = registry::persist(app, state);
}

/// 执行有声下载任务并发布最终状态。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `job_id`：任务唯一标识。
/// - `novel_id`：小说编号。
/// - `album_id`：可选的有声专辑编号。
/// - `title`：本地目录与展示名称。
/// - `chapter_ids`：待下载章节编号。
/// - `cancelled`：任务取消标记。
pub(super) async fn run(
    app: tauri::AppHandle,
    job_id: String,
    novel_id: i64,
    album_id: Option<i64>,
    title: String,
    chapter_ids: Vec<i64>,
    cancelled: Arc<AtomicBool>,
) {
    let state = app.state::<DownloadState>();
    let context = JobContext {
        app: &app,
        state: state.inner(),
        job_id: &job_id,
        cancelled: cancelled.as_ref(),
    };
    let result = run_inner(&context, novel_id, album_id, &title, &chapter_ids).await;
    mark_finished(&context, result);
}

/// 下载有声目录中的选中章节并生成播放列表。
///
/// # 参数
/// - `context`：后台任务共享句柄。
/// - `novel_id`：小说编号。
/// - `album_id`：可选的有声专辑编号。
/// - `title`：本地目录名称。
/// - `chapter_ids`：待下载章节编号。
///
/// # 返回
/// 成功时返回播放列表相对地址。
///
/// # 错误
/// 目录、音频地址、文件写入失败，或全部章节不可下载时返回错误消息。
async fn run_inner(
    context: &JobContext<'_>,
    novel_id: i64,
    album_id: Option<i64>,
    title: &str,
    chapter_ids: &[i64],
) -> Result<String, String> {
    let JobContext {
        app,
        state,
        job_id,
        cancelled,
    } = *context;
    let source = SfSource::connect(app).await?;
    let album_id = match album_id {
        Some(album_id) => {
            validate_novel_id(album_id)?;
            album_id
        }
        None => source.audio_album_id(novel_id, Some(title)).await?,
    };
    let storage = NativeStorage::new(app.clone());
    let policy = storage.policy().unwrap_or_default();
    let directory = storage.new_book_directory(title)?;
    let mut manifest = read_book_manifest(&directory)?;
    {
        let audio_metadata = manifest.audio.get_or_insert_with(Default::default);
        audio_metadata.id = Some(album_id);
        audio_metadata.catalog_id = Some(novel_id);
        audio_metadata.title = Some(title.to_string());
        audio_metadata
            .author
            .get_or_insert_with(|| "未知作者".to_string());
        audio_metadata
            .description
            .get_or_insert_with(|| "暂无简介".to_string());
    }
    write_book_manifest(&directory, &manifest)?;
    let _ = enrich_audio_metadata(
        &source,
        &directory,
        manifest
            .audio
            .as_mut()
            .ok_or_else(|| "有声元数据初始化失败".to_string())?,
        novel_id,
        album_id,
    )
    .await;
    write_book_manifest(&directory, &manifest)?;
    registry::update(state, job_id, |job| {
        job.status = "downloading".to_string();
        job.progress = 1;
        job.message = "作品信息已保存，准备下载".to_string();
    });
    registry::emit_job(app, state, job_id);
    registry::emit_library(app, state, job_id);

    let (_catalog_title, catalog) = source.audio_catalog(album_id, Some(title)).await?;
    let selected = chapter_ids.iter().copied().collect::<HashSet<_>>();
    let chapters = catalog
        .iter()
        .filter(|chapter| selected.contains(&chapter.id))
        .collect::<Vec<_>>();
    if chapters.is_empty() {
        return Err("没有可下载的有声章节".to_string());
    }
    let audio_directory = directory.join("audio");
    fs::create_dir_all(&audio_directory).map_err(|error| format!("无法创建有声目录：{error}"))?;
    let mut downloaded = manifest
        .downloaded_audio_chapter_ids
        .take()
        .unwrap_or_default()
        .into_iter()
        .collect::<HashSet<_>>();
    let total = chapters.len();
    let mut skipped_chapters: Vec<(String, String)> = Vec::new();
    'chapters: for (selected_index, chapter) in chapters.iter().enumerate() {
        if cancelled.load(Ordering::Relaxed) {
            return Err("下载已暂停".to_string());
        }
        let catalog_index = catalog
            .iter()
            .position(|item| item.id == chapter.id)
            .ok_or_else(|| "有声目录状态已变化".to_string())?;
        let file_name = format!(
            "{:03} - {}.mp3",
            catalog_index + 1,
            safe_audio_name(&chapter.title)
        );
        let target = audio_directory.join(&file_name);
        registry::update(state, job_id, |job| {
            job.status = "downloading".to_string();
            job.progress = 2 + ((selected_index * 94) / total) as u8;
            job.message = format!("正在下载：{}", chapter.title);
        });
        registry::emit_job(app, state, job_id);
        if !target.is_file() {
            let audio_url = match source.audio_source(chapter.id).await {
                Ok(url) => url,
                Err(error) => {
                    eprintln!(
                        "[sfacg] audio chapter skipped: chapter_id={}, title={}, error={error}",
                        chapter.id, chapter.title
                    );
                    skipped_chapters.push((chapter.title.clone(), error));
                    registry::update(state, job_id, |job| {
                        job.message = format!("章节不可下载，已跳过：{}", chapter.title);
                    });
                    registry::emit_job(app, state, job_id);
                    continue 'chapters;
                }
            };
            let payload = fetch_audio(
                &source,
                &audio_url,
                &format!("https://m.sfacg.com/a/{}/", chapter.id),
            )
            .await;
            let payload = match payload {
                Ok(payload) => payload,
                Err(error) => {
                    eprintln!(
                        "[sfacg] audio chapter skipped: chapter_id={}, title={}, error={error}",
                        chapter.id, chapter.title
                    );
                    skipped_chapters.push((chapter.title.clone(), error));
                    registry::update(state, job_id, |job| {
                        job.message = format!("章节音频下载失败，已跳过：{}", chapter.title);
                    });
                    registry::emit_job(app, state, job_id);
                    continue 'chapters;
                }
            };
            write_bytes_atomically(&target, &payload, "有声章节")?;
        }
        downloaded.insert(chapter.id);
        let mut downloaded_ids = downloaded.iter().copied().collect::<Vec<_>>();
        downloaded_ids.sort_unstable();
        manifest.downloaded_audio_chapter_ids = Some(downloaded_ids);
        write_book_manifest(&directory, &manifest)?;
        persist_audio_playlist(&audio_directory, &catalog)?;
        registry::update(state, job_id, |job| {
            job.progress = 2 + (((selected_index + 1) * 94) / total) as u8;
            job.message = format!("已完成：{}", chapter.title);
        });
        registry::emit_job(app, state, job_id);
        registry::emit_library(app, state, job_id);
        tokio::time::sleep(std::time::Duration::from_millis(policy.request_interval_ms)).await;
    }
    persist_audio_playlist(&audio_directory, &catalog)?;
    let mut downloaded_ids = downloaded.into_iter().collect::<Vec<_>>();
    downloaded_ids.sort_unstable();
    manifest.downloaded_audio_chapter_ids = Some(downloaded_ids);
    write_book_manifest(&directory, &manifest)?;
    if skipped_chapters.len() == chapters.len() {
        let (chapter_title, error) = &skipped_chapters[0];
        return Err(format!(
            "所有有声章节都不可下载：{chapter_title}（{error}）"
        ));
    }
    if skipped_chapters.is_empty() {
        Ok("audio/有声目录.m3u8".to_string())
    } else {
        let summary = skipped_chapters
            .iter()
            .take(3)
            .map(|(chapter_title, error)| format!("{chapter_title}（{error}）"))
            .collect::<Vec<_>>()
            .join("；");
        let remaining = skipped_chapters.len().saturating_sub(3);
        let suffix = (remaining > 0).then(|| format!("；其余 {remaining} 章"));
        Err(format!(
            "{} 个有声章节未保存：{summary}{}。已成功章节已保存，可继续下载失败章节",
            skipped_chapters.len(),
            suffix.unwrap_or_default()
        ))
    }
}
