//! 漫画章节下载流程。

use super::assets::{enrich_comic_metadata, fetch_asset};
use super::JobContext;
use crate::adapters::downloads::registry;
use crate::adapters::downloads::DownloadState;
use crate::adapters::sfacg::app::is_risk_control_failure;
use crate::adapters::sfacg::source::SfSource;
use crate::adapters::storage::{
    read_book_manifest, write_book_manifest, write_bytes_atomically, NativeStorage,
};
use crate::domain::catalog::ComicChapter;
use std::collections::HashSet;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Manager;

/// 根据漫画页面 URL 选择保存扩展名。
///
/// # 参数
/// - `url`：上游页面地址。
fn comic_image_extension(url: &str) -> &'static str {
    let path = url.split('?').next().unwrap_or(url).to_ascii_lowercase();
    if path.ends_with(".png") {
        "png"
    } else if path.ends_with(".webp") {
        "webp"
    } else if path.ends_with(".jpeg") {
        "jpeg"
    } else {
        "jpg"
    }
}

/// 写入漫画任务的最终状态并持久化。
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
        Ok(file) => registry::update(state, job_id, |job| {
            job.status = "done".to_string();
            job.progress = 100;
            job.message = "漫画章节下载完成".to_string();
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

/// 执行漫画下载任务并发布最终状态。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `job_id`：任务唯一标识。
/// - `comic_id`：漫画编号。
/// - `source_path`：可选的网页来源路径。
/// - `title`：本地目录与展示名称。
/// - `chapter_ids`：待下载章节编号。
/// - `cancelled`：任务取消标记。
pub(super) async fn run(
    app: tauri::AppHandle,
    job_id: String,
    comic_id: i64,
    source_path: Option<String>,
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
    let result = run_inner(
        &context,
        comic_id,
        source_path.as_deref(),
        &title,
        &chapter_ids,
    )
    .await;
    mark_finished(&context, result);
}

/// 下载漫画目录中的选中章节及页面图片。
///
/// # 参数
/// - `context`：后台任务共享句柄。
/// - `comic_id`：漫画编号。
/// - `source_path`：可选的网页来源路径。
/// - `title`：本地目录名称。
/// - `chapter_ids`：待下载章节编号。
///
/// # 返回
/// 成功时返回书籍目录地址。
///
/// # 错误
/// 来源、目录、页面下载或文件写入失败时返回错误消息。
async fn run_inner(
    context: &JobContext<'_>,
    comic_id: i64,
    source_path: Option<&str>,
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
    let (_, folder) = source
        .comic_source(comic_id, source_path, Some(title))
        .await?;
    let storage = NativeStorage::new(app.clone());
    let policy = storage.policy().unwrap_or_default();
    let directory = storage.new_book_directory(title)?;
    let mut manifest = read_book_manifest(&directory)?;
    {
        let comic_metadata = manifest.comic.get_or_insert_with(Default::default);
        comic_metadata.id = Some(comic_id);
        comic_metadata.title = Some(title.to_string());
        comic_metadata
            .author
            .get_or_insert_with(|| "未知作者".to_string());
        comic_metadata
            .description
            .get_or_insert_with(|| "暂无简介".to_string());
        comic_metadata.online_path.get_or_insert(folder.clone());
    }
    write_book_manifest(&directory, &manifest)?;
    if let Err(error) = enrich_comic_metadata(
        &source,
        &directory,
        manifest
            .comic
            .as_mut()
            .ok_or_else(|| "漫画元数据初始化失败".to_string())?,
        comic_id,
    )
    .await
    {
        if is_risk_control_failure(&error) {
            return Err(error);
        }
    }
    write_book_manifest(&directory, &manifest)?;
    registry::update(state, job_id, |job| {
        job.status = "downloading".to_string();
        job.progress = 1;
        job.message = "作品信息已保存，准备下载".to_string();
    });
    registry::emit_job(app, state, job_id);
    registry::emit_library(app, state, job_id);

    let catalog = source.comic_catalog(comic_id).await?;
    let requested = chapter_ids.iter().copied().collect::<HashSet<_>>();
    let chapters = catalog
        .into_iter()
        .filter(|chapter: &ComicChapter| {
            requested.contains(&chapter.id) && chapter.is_unlocked.is_selectable()
        })
        .collect::<Vec<_>>();
    if chapters.is_empty() {
        return Err("所选漫画章节不在当前目录中".to_string());
    }
    let comic_directory = directory.join("comic");
    fs::create_dir_all(&comic_directory).map_err(|error| format!("无法创建漫画目录：{error}"))?;
    let mut downloaded = manifest
        .downloaded_comic_chapter_ids
        .take()
        .unwrap_or_default()
        .into_iter()
        .collect::<HashSet<_>>();
    let total = chapters.len();
    let mut skipped_chapters = Vec::new();
    'chapters: for (chapter_index, chapter) in chapters.into_iter().enumerate() {
        if cancelled.load(Ordering::Relaxed) {
            return Err("下载已暂停".to_string());
        }
        registry::update(state, job_id, |job| {
            job.status = "downloading".to_string();
            job.progress = 2 + ((chapter_index * 94) / total) as u8;
            job.message = format!("正在读取：{}", chapter.title);
        });
        registry::emit_job(app, state, job_id);
        let chapter_directory = comic_directory.join(format!("{:06}", chapter.id));
        let marker = chapter_directory.join(".complete");
        if !marker.is_file() {
            let images = match source.comic_images(comic_id, &chapter.path_name).await {
                Ok(images) => images,
                Err(error) if is_risk_control_failure(&error) => return Err(error),
                Err(error) => {
                    if !chapter.is_vip {
                        return Err(error);
                    }
                    eprintln!(
                        "[sfacg] comic chapter skipped: chapter_id={}, title={}, error={error}",
                        chapter.id, chapter.title
                    );
                    skipped_chapters.push((chapter.title.clone(), error));
                    registry::update(state, job_id, |job| {
                        job.message = format!("漫画章节不可下载，已跳过：{}", chapter.title);
                    });
                    registry::emit_job(app, state, job_id);
                    continue 'chapters;
                }
            };
            fs::create_dir_all(&chapter_directory)
                .map_err(|error| format!("无法创建漫画章节目录：{error}"))?;
            for (page_index, image) in images.iter().enumerate() {
                if cancelled.load(Ordering::Relaxed) {
                    return Err("下载已暂停".to_string());
                }
                registry::update(state, job_id, |job| {
                    job.message = format!(
                        "正在下载：{}（{}/{})",
                        chapter.title,
                        page_index + 1,
                        images.len()
                    );
                });
                registry::emit_job(app, state, job_id);
                let extension = comic_image_extension(image);
                let target = chapter_directory.join(format!("{:03}.{extension}", page_index + 1));
                if target.is_file() {
                    continue;
                }
                let payload = match fetch_asset(
                    &source,
                    image,
                    "https://m.sfacg.com/",
                    "image/avif,image/webp,image/apng,image/*,*/*;q=0.8",
                    "漫画图片",
                )
                .await
                {
                    Ok(payload) => payload,
                    Err(error) if is_risk_control_failure(&error) => return Err(error),
                    Err(error) => {
                        if !chapter.is_vip {
                            return Err(error);
                        }
                        eprintln!(
                            "[sfacg] comic chapter skipped: chapter_id={}, title={}, error={error}",
                            chapter.id, chapter.title
                        );
                        skipped_chapters.push((chapter.title.clone(), error));
                        registry::update(state, job_id, |job| {
                            job.message =
                                format!("VIP 漫画章节不可下载，已跳过：{}", chapter.title);
                        });
                        registry::emit_job(app, state, job_id);
                        continue 'chapters;
                    }
                };
                write_bytes_atomically(&target, &payload, "漫画图片")?;
            }
            fs::write(&marker, chapter.title.as_bytes())
                .map_err(|error| format!("无法完成漫画章节写入：{error}"))?;
        }
        downloaded.insert(chapter.id);
        let mut downloaded_ids = downloaded.iter().copied().collect::<Vec<_>>();
        downloaded_ids.sort_unstable();
        manifest.downloaded_comic_chapter_ids = Some(downloaded_ids);
        write_book_manifest(&directory, &manifest)?;
        registry::update(state, job_id, |job| {
            job.progress = 2 + (((chapter_index + 1) * 94) / total) as u8;
            job.message = format!("已完成：{}", chapter.title);
        });
        registry::emit_job(app, state, job_id);
        registry::emit_library(app, state, job_id);
        tokio::time::sleep(std::time::Duration::from_millis(policy.request_interval_ms)).await;
    }
    let mut downloaded_ids = downloaded.into_iter().collect::<Vec<_>>();
    downloaded_ids.sort_unstable();
    manifest.downloaded_comic_chapter_ids = Some(downloaded_ids);
    write_book_manifest(&directory, &manifest)?;
    if skipped_chapters.is_empty() {
        Ok("comic".to_string())
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
            "{} 个 VIP 漫画章节未保存：{summary}{}。已成功章节已保存，可继续下载失败章节",
            skipped_chapters.len(),
            suffix.unwrap_or_default()
        ))
    }
}
