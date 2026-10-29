//! 文字小说下载流程。

use super::assets::enrich_novel_metadata;
use super::JobContext;
use crate::adapters::downloads::registry;
use crate::adapters::downloads::DownloadState;
use crate::adapters::ocr::{recognize_image, relative_book_path};
use crate::adapters::sfacg::source::SfSource;
use crate::adapters::storage::{
    downloaded_text_chapter_ids, read_book_manifest, read_chapter_store, write_book_manifest,
    write_bytes_atomically, write_chapter_store, NativeStorage,
};
use crate::domain::catalog::TextContentKind;
use crate::domain::library::{BookManifest, ChapterIndex, StoredChapter};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Manager;

/// 下载正文中的内嵌图片并改写为本地图片协议地址。
///
/// # 参数
/// - `source`：SF 内容源。
/// - `directory`：书籍目录。
/// - `novel_id`：小说编号。
/// - `chapter_id`：章节编号。
/// - `content`：包含图片标记的正文。
///
/// # 错误
/// 图片目录创建、下载或写入失败时返回错误消息。
async fn materialize_chapter_images(
    source: &SfSource,
    directory: &Path,
    novel_id: i64,
    chapter_id: i64,
    content: &str,
) -> Result<String, String> {
    let marker = "[img=";
    if !content.contains(marker) {
        return Ok(content.to_string());
    }

    let image_directory = directory.join("imgs");
    fs::create_dir_all(&image_directory)
        .map_err(|error| format!("无法创建章节图片目录：{error}"))?;
    let referer = format!("https://book.sfacg.com/Novel/{novel_id}/");
    let mut output = String::with_capacity(content.len());
    let mut cursor = 0;
    let mut image_index = 0;

    while let Some(marker_offset) = content[cursor..].find(marker) {
        let marker_start = cursor + marker_offset;
        let Some(metadata_end_offset) = content[marker_start..].find(']') else {
            output.push_str(&content[cursor..]);
            cursor = content.len();
            break;
        };
        let url_start = marker_start + metadata_end_offset + 1;
        let Some(end_offset) = content[url_start..].find("[/img]") else {
            output.push_str(&content[cursor..]);
            cursor = content.len();
            break;
        };
        let url_end = url_start + end_offset;
        let url = content[url_start..url_end].trim();
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            output.push_str(&content[cursor..url_end + "[/img]".len()]);
            cursor = url_end + "[/img]".len();
            continue;
        }

        output.push_str(&content[cursor..marker_start]);
        image_index += 1;
        let response = source
            .asset_request(url, &referer, "image/avif,image/webp,image/*,*/*;q=0.8")
            .send()
            .await
            .map_err(|error| format!("无法下载章节图片 {image_index}：{error}"))?
            .error_for_status()
            .map_err(|error| format!("章节图片 {image_index} 下载被拒绝：{error}"))?;
        let extension = match response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .map(str::trim)
        {
            Some("image/png") => "png",
            Some("image/gif") => "gif",
            Some("image/webp") => "webp",
            Some("image/avif") => "avif",
            _ => "jpeg",
        };
        let file_name = format!("chapter-{chapter_id}-{image_index}.{extension}");
        let target = image_directory.join(&file_name);
        let payload = response
            .bytes()
            .await
            .map_err(|error| format!("无法读取章节图片 {image_index}：{error}"))?;
        write_bytes_atomically(&target, &payload, "章节图片")?;
        output.push_str(&format!("![章节插图](imgs/{file_name})"));
        cursor = url_end + "[/img]".len();
    }

    if cursor < content.len() {
        output.push_str(&content[cursor..]);
    }
    Ok(output)
}

/// 下载 VIP 图片正文并执行 OCR，返回正文和可选 OCR 源路径。
///
/// # 参数
/// - `app`：应用句柄。
/// - `source`：SF 内容源。
/// - `directory`：书籍目录。
/// - `novel_id`：小说编号。
/// - `chapter_id`：章节编号。
/// - `save_ocr_process_files`：是否保留 OCR 处理图片。
///
/// # 错误
/// 图片读取、缓存写入或 OCR 失败时返回错误消息。
async fn resolve_vip_image_content(
    app: &tauri::AppHandle,
    source: &SfSource,
    directory: &Path,
    novel_id: i64,
    chapter_id: i64,
    save_ocr_process_files: bool,
) -> Result<(String, String), String> {
    let image = source.read_vip_image_chapter(novel_id, chapter_id).await?;
    let ocr_directory = directory.join("ocr");
    fs::create_dir_all(&ocr_directory).map_err(|error| format!("无法创建 OCR 目录：{error}"))?;
    let source_path = ocr_directory.join(format!("chapter-{chapter_id}.{}", image.extension));
    write_bytes_atomically(&source_path, &image.bytes, "VIP 章节原图")?;

    let segments = save_ocr_process_files.then(|| {
        ocr_directory
            .join("segments")
            .join(format!("chapter-{chapter_id}"))
    });
    let recognized = recognize_image(app, source_path.clone(), segments).await?;
    let source_relative = relative_book_path(&source_path, directory)?;
    Ok((recognized, source_relative))
}

/// 原子写入书籍清单。
///
/// # 参数
/// - `directory`：书籍目录。
/// - `manifest`：待持久化的清单。
fn persist_manifest(directory: &Path, manifest: &BookManifest) -> Result<(), String> {
    write_book_manifest(directory, manifest)
}

/// 原子写入文字章节索引。
///
/// # 参数
/// - `directory`：书籍目录。
/// - `store`：章节索引。
fn persist_chapters(directory: &Path, store: &ChapterIndex) -> Result<(), String> {
    write_chapter_store(directory, store)
}

/// 写入文字任务的最终状态并持久化。
///
/// # 参数
/// - `context`：后台任务共享句柄。
/// - `result`：成功时返回书籍目录，失败时返回错误消息。
fn mark_finished(context: &JobContext<'_>, result: Result<String, String>) {
    let JobContext {
        app,
        state,
        job_id,
        cancelled,
    } = *context;
    match result {
        Ok(_) => registry::update(state, job_id, |job| {
            job.status = "done".to_string();
            job.progress = 100;
            job.message = "文本章节下载完成".to_string();
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

/// 执行文字下载任务并发布最终状态。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `job_id`：任务唯一标识。
/// - `novel_id`：小说编号。
/// - `title`：本地目录与展示名称。
/// - `chapter_ids`：待下载章节编号。
/// - `cancelled`：任务取消标记。
pub(super) async fn run(
    app: tauri::AppHandle,
    job_id: String,
    novel_id: i64,
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
    let result = run_inner(&context, novel_id, &title, &chapter_ids).await;
    mark_finished(&context, result);
}

/// 下载文字目录中的选中章节，并处理 VIP 图片回退。
///
/// # 参数
/// - `context`：后台任务共享句柄。
/// - `novel_id`：小说编号。
/// - `title`：本地目录名称。
/// - `chapter_ids`：待下载章节编号。
///
/// # 返回
/// 成功时返回书籍目录地址。
///
/// # 错误
/// 目录、正文、图片或文件写入失败时返回错误消息；部分 VIP 章节失败会返回汇总错误。
async fn run_inner(
    context: &JobContext<'_>,
    novel_id: i64,
    title: &str,
    chapter_ids: &[i64],
) -> Result<String, String> {
    let JobContext {
        app,
        state,
        job_id,
        cancelled,
    } = *context;
    let storage_lock = registry::text_lock(state, novel_id)?;
    let _storage_guard = storage_lock.lock().await;
    let source = SfSource::connect(app).await?;
    let storage = NativeStorage::new(app.clone());
    let policy = storage.policy().unwrap_or_default();
    let directory = storage.new_book_directory(title)?;

    let mut manifest = read_book_manifest(&directory)?;
    let novel_metadata = manifest.novel.get_or_insert_with(Default::default);
    novel_metadata.id = Some(novel_id);
    novel_metadata.title = Some(title.to_string());
    novel_metadata
        .author
        .get_or_insert_with(|| "未知作者".to_string());
    novel_metadata
        .description
        .get_or_insert_with(|| "暂无简介".to_string());
    persist_manifest(&directory, &manifest)?;
    let _ = enrich_novel_metadata(
        &source,
        &directory,
        manifest
            .novel
            .as_mut()
            .ok_or_else(|| "小说元数据初始化失败".to_string())?,
        novel_id,
    )
    .await;
    persist_manifest(&directory, &manifest)?;
    registry::update(state, job_id, |job| {
        job.status = "downloading".to_string();
        job.progress = 1;
        job.message = "作品信息已保存，准备下载".to_string();
    });
    registry::emit_job(app, state, job_id);
    registry::emit_library(app, state, job_id);

    let mut store = read_chapter_store(&directory)?;
    store.novel_id = novel_id;
    let catalog = source.text_volumes(novel_id).await?;
    let requested = chapter_ids.iter().copied().collect::<HashSet<_>>();
    let mut chapters = Vec::new();
    for (volume_index, volume) in catalog.iter().enumerate() {
        for (chapter_index, chapter) in volume.chapters.iter().enumerate() {
            if !requested.contains(&chapter.chap_id) {
                continue;
            }
            chapters.push((
                chapter.chap_id,
                volume.volume_id,
                volume.title.clone(),
                volume_index as i64,
                chapter_index,
                chapter.title.clone(),
                chapter.content_kind.clone(),
            ));
        }
    }
    if chapters.is_empty() {
        return Err("没有可下载的章节".to_string());
    }
    persist_chapters(&directory, &store)?;
    manifest.downloaded_text_chapter_ids = Some(downloaded_text_chapter_ids(&store));
    persist_manifest(&directory, &manifest)?;

    let total = chapters.len();
    let mut failed_vip_chapters = Vec::new();
    for (
        index,
        (chapter_id, volume_id, volume, volume_index, chapter_index, chapter_title, content_kind),
    ) in chapters.into_iter().enumerate()
    {
        if cancelled.load(Ordering::Relaxed) {
            return Err("下载已暂停".to_string());
        }
        registry::update(state, job_id, |job| {
            job.status = "downloading".to_string();
            job.progress = ((index * 96) / total) as u8;
            job.message = format!("正在读取：{chapter_title}");
        });
        registry::emit_job(app, state, job_id);

        let existing_vip_is_complete =
            store
                .chapters
                .get(&chapter_id.to_string())
                .is_some_and(|chapter| {
                    matches!(
                        content_kind,
                        TextContentKind::ImageVip | TextContentKind::EncryptedVip
                    ) && matches!(
                        chapter.content_source.as_deref(),
                        Some("app") | Some("webVipOcr")
                    ) && !chapter.content.trim().is_empty()
                });
        let should_write = !store.chapters.contains_key(&chapter_id.to_string())
            || (matches!(
                content_kind,
                TextContentKind::ImageVip | TextContentKind::EncryptedVip
            ) && !existing_vip_is_complete);
        if should_write {
            let (content, content_source, ocr_source_path) = match content_kind {
                TextContentKind::ImageVip | TextContentKind::EncryptedVip => {
                    let app_result = source
                        .try_direct_text_chapter(chapter_id, |raw| storage.decode_api_content(raw))
                        .await;
                    match app_result {
                        Some(Ok(content)) => (content, Some("app".to_string()), None),
                        app_result => {
                            let app_error = app_result.and_then(Result::err);
                            registry::update(state, job_id, |job| {
                                job.message = if app_error.is_some() {
                                    format!(
                                        "App 正文不可用，正在保存并识别图片正文：{chapter_title}"
                                    )
                                } else {
                                    format!("正在保存并识别图片正文：{chapter_title}")
                                };
                            });
                            registry::emit_job(app, state, job_id);
                            let vip_result = resolve_vip_image_content(
                                app,
                                &source,
                                &directory,
                                novel_id,
                                chapter_id,
                                policy.ocr_process_files_enabled,
                            )
                            .await;
                            let (content, source_path) = match vip_result {
                                Ok(value) => value,
                                Err(vip_error) => {
                                    let error = match app_error {
                                        Some(app_error) => format!(
                                            "App 正文不可用：{app_error}；网页 VIP 图片正文不可用：{vip_error}"
                                        ),
                                        None => vip_error,
                                    };
                                    failed_vip_chapters
                                        .push((chapter_title.clone(), error.clone()));
                                    registry::update(state, job_id, |job| {
                                        job.message = format!(
                                            "VIP 章节处理失败，已跳过：{chapter_title}（{error}）"
                                        );
                                    });
                                    registry::emit_job(app, state, job_id);
                                    continue;
                                }
                            };
                            (content, Some("webVipOcr".to_string()), Some(source_path))
                        }
                    }
                }
                TextContentKind::Text | TextContentKind::Unknown => {
                    let result = source
                        .read_chapter_text(novel_id, volume_id, chapter_id, |raw| {
                            storage.decode_api_content(raw)
                        })
                        .await?;
                    let content = materialize_chapter_images(
                        &source,
                        &directory,
                        novel_id,
                        chapter_id,
                        &result.content,
                    )
                    .await?;
                    (content, Some(result.origin.as_str().to_string()), None)
                }
            };
            store.chapters.insert(
                chapter_id.to_string(),
                StoredChapter {
                    id: chapter_id,
                    title: chapter_title.clone(),
                    volume,
                    content,
                    volume_index,
                    chapter_index: chapter_index as i64,
                    content_source,
                    ocr_source_path,
                },
            );
            persist_chapters(&directory, &store)?;
            manifest.downloaded_text_chapter_ids = Some(downloaded_text_chapter_ids(&store));
            persist_manifest(&directory, &manifest)?;
        }
        registry::update(state, job_id, |job| {
            job.progress = 2 + (((index + 1) * 94) / total) as u8;
            job.message = format!("已完成：{chapter_title}");
        });
        registry::emit_job(app, state, job_id);
        registry::emit_library(app, state, job_id);
        tokio::time::sleep(std::time::Duration::from_millis(policy.request_interval_ms)).await;
    }

    manifest.downloaded_text_chapter_ids = Some(downloaded_text_chapter_ids(&store));
    persist_manifest(&directory, &manifest)?;
    if failed_vip_chapters.is_empty() {
        Ok(directory.to_string_lossy().into_owned())
    } else {
        let failed_summary = failed_vip_chapters
            .iter()
            .take(3)
            .map(|(chapter_title, error)| format!("{chapter_title}（{error}）"))
            .collect::<Vec<_>>()
            .join("；");
        let remaining = failed_vip_chapters.len().saturating_sub(3);
        let suffix = (remaining > 0).then(|| format!("；其余 {remaining} 章"));
        Err(format!(
            "{} 个 VIP 章节未保存：{failed_summary}{}。已成功章节已保存，可继续下载失败章节",
            failed_vip_chapters.len(),
            suffix.unwrap_or_default()
        ))
    }
}
