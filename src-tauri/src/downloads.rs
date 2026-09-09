//! 原生下载任务的创建、调度、恢复与进度发布。
//!
//! 本模块编排文本、有声和漫画下载；SFACG 请求与本地文件格式分别委托给远程
//! 服务模块和书库模块，避免下载流程承担额外的协议或持久化职责。

use crate::app_client::AppClient;
use crate::endpoint_policy::{app_endpoint_client, web_endpoint_client, EndpointCapability};
use crate::library::{
    ensure_external_storage_access, get_request_policy, library_directory, persist_native_jobs,
    safe_library_name, StoredBookMetadata, StoredChapter, StoredChapterStore, StoredWorkMetadata,
};
use crate::ocr::{recognize_image, relative_book_path};
#[cfg(target_os = "android")]
use crate::sfacg::sync_android_auth_session;
use crate::sfacg::{validate_novel_id, NativeJob, NativeJobSpec, NativeJobState, TextContentKind};
use crate::utils::json::{read_or_default, write_atomically};
use crate::web_client::WebClient;
use serde_json::Value;
use std::fs;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager};

/// 更新一个原生任务，并将面向渲染进程的状态保留在 Rust 内存中。
///
/// # 参数
/// * `state` - 共享原生任务注册表。
/// * `job_id` - 待更新任务的标识符。
/// * `update` - 持有注册表锁时应用的状态修改。
fn update_native_job<F>(state: &NativeJobState, job_id: &str, update: F)
where
    F: FnOnce(&mut NativeJob),
{
    if let Ok(mut jobs) = state.jobs.lock() {
        if let Some(job) = jobs.get_mut(job_id) {
            update(job);
        }
    }
}

/// 向已订阅应用窗口发送一条渲染进程安全的下载任务更新。
///
/// # 参数
/// * `app` - 用于发布内部事件的应用句柄。
/// * `state` - 共享原生任务注册表。
/// * `job_id` - 要发送最新状态的任务标识符。
///
/// # 副作用
/// 发送 `download-progress`，但不暴露 Cookie、本地路径或任务规格。
fn emit_native_job_update(app: &tauri::AppHandle, state: &NativeJobState, job_id: &str) {
    let job = state
        .jobs
        .lock()
        .ok()
        .and_then(|jobs| jobs.get(job_id).cloned());
    if let Some(job) = job {
        let _ = app.emit("download-progress", job);
    }
}

/// 取得一部小说的文字下载存储锁。
///
/// 同一本小说可能被用户重复加入队列，或在一个未退出的旧任务仍运行时继续下载。章节
/// 索引是整份 JSON 快照，因而必须覆盖从读取到提交的整个下载过程，不能只锁单次文件
/// 写入；否则两个任务都会基于旧快照写回，后完成的任务会抹去前一个任务的新章节。
///
/// # 错误
/// 任务状态锁不可用时返回错误。
fn text_storage_lock(
    state: &NativeJobState,
    novel_id: i64,
) -> Result<Arc<tokio::sync::Mutex<()>>, String> {
    let mut locks = state
        .text_storage_locks
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?;
    Ok(locks
        .entry(novel_id)
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone())
}

/// 从已持久化的章节索引构造稳定的完成章节列表。
fn downloaded_text_chapter_ids(store: &StoredChapterStore) -> Vec<i64> {
    let mut chapter_ids: Vec<_> = store
        .chapters
        .keys()
        .filter_map(|id| id.parse().ok())
        .collect();
    chapter_ids.sort_unstable();
    chapter_ids
}

/// 下载章节插图，并将 HTML 图片标签改写为本地 Markdown 路径。
async fn materialize_chapter_images(
    client: &WebClient,
    directory: &std::path::Path,
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
        let response = client
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
        let temporary = target.with_extension(format!("{extension}.part"));
        let payload = response
            .bytes()
            .await
            .map_err(|error| format!("无法读取章节图片 {image_index}：{error}"))?;
        fs::write(&temporary, payload)
            .map_err(|error| format!("无法写入章节图片 {image_index}：{error}"))?;
        if target.exists() {
            fs::remove_file(&target)
                .map_err(|error| format!("无法替换章节图片 {image_index}：{error}"))?;
        }
        fs::rename(&temporary, &target)
            .map_err(|error| format!("无法完成章节图片 {image_index}：{error}"))?;
        output.push_str(&format!("![章节插图](imgs/{file_name})"));
        cursor = url_end + "[/img]".len();
    }

    if cursor < content.len() {
        output.push_str(&content[cursor..]);
    }
    Ok(output)
}

/// 从网站读取普通章节正文。
async fn resolve_text_content(
    web_client: &WebClient,
    novel_id: i64,
    volume_id: i64,
    chapter_id: i64,
) -> Result<String, String> {
    web_client
        .chapter_content(novel_id, volume_id, chapter_id)
        .await
        .map_err(|error| EndpointCapability::TextChapterWeb.unavailable_message(&error))
}

/// 下载 VIP 章节正文图片，保留原图并调用本地 OCR。
async fn resolve_vip_image_content(
    app: &tauri::AppHandle,
    web_client: &WebClient,
    directory: &std::path::Path,
    novel_id: i64,
    chapter_id: i64,
) -> Result<(String, String), String> {
    // 来源请求必须登记为独立的仅网页能力。第二个客户端仅用于强制校验会话边界；传入
    // 的客户端拥有实际请求及其 Cookie 快照。
    let _ = web_endpoint_client(app, EndpointCapability::TextVipImageWeb)?;
    let image = web_client.vip_chapter_image(novel_id, chapter_id).await?;
    let ocr_directory = directory.join("ocr");
    fs::create_dir_all(&ocr_directory).map_err(|error| format!("无法创建 OCR 目录：{error}"))?;
    let source = ocr_directory.join(format!("chapter-{chapter_id}.{}", image.extension));
    let source_partial = source.with_extension(format!("{}.part", image.extension));
    fs::write(&source_partial, image.bytes)
        .map_err(|error| format!("无法保存 VIP 章节原图：{error}"))?;
    if source.exists() {
        fs::remove_file(&source).map_err(|error| format!("无法替换 VIP 章节原图：{error}"))?;
    }
    fs::rename(&source_partial, &source)
        .map_err(|error| format!("无法完成 VIP 章节原图写入：{error}"))?;

    let segments = ocr_directory
        .join("segments")
        .join(format!("chapter-{chapter_id}"));
    let recognized = recognize_image(app, source.clone(), segments).await?;
    let source_relative = relative_book_path(&source, directory)?;
    Ok((recognized, source_relative))
}

/// 运行一个文字小说下载任务，并将远程章节持久化为本地可读的章节存储。
///
/// # 参数
/// * `app` - 用于私有存储和共享任务状态的 Tauri 应用句柄。
/// * `job_id` - 原生任务标识符。
/// * `novel_id` - SF 小说编号。
/// * `title` - 面向用户展示的作品标题。
/// * `chapter_ids` - 可选的选中章节编号；`None` 表示全部可用章节。
/// * `cancelled` - 由暂停、删除命令控制的协作式取消标记。
async fn run_text_download(
    app: tauri::AppHandle,
    job_id: String,
    novel_id: i64,
    title: String,
    chapter_ids: Option<Vec<i64>>,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
) {
    let state = app.state::<NativeJobState>();
    let storage_lock = text_storage_lock(&state, novel_id);
    let result: Result<String, String> = async {
        // 同一本书的任务共用章节索引。锁覆盖读取、OCR 和写入，避免并发任务的完整
        // 快照互相覆盖；不同作品仍可并行下载。
        let storage_lock = storage_lock?;
        let _storage_guard = storage_lock.lock().await;
        // AppClient 在此仅负责目录和可选元数据；WebClient 才是所有章节正文、VIP 图片与
        // 正文内插图资源的唯一请求入口。分别按能力构造可防止 App/Web Cookie 混用。
        let app_client = app_endpoint_client(&app, EndpointCapability::TextDirectory)?;
        let web_client = web_endpoint_client(&app, EndpointCapability::TextChapterWeb)?;
        // 读取下载限速策略，避免过快请求被 SFACG 服务器拒绝。若未配置则使用默认值。
        let policy = get_request_policy(app.clone()).unwrap_or_default();
        let directory = library_directory(&app)?.join(safe_library_name(&title));
        fs::create_dir_all(&directory).map_err(|error| format!("无法创建本地书籍目录：{error}"))?;

        // 元数据更新负责初始化默认值、刷新公开详情并尝试保存封面。远程详情或封面失败不
        // 阻断正文下载，避免非正文资源的短暂异常使已选章节无法保存。
        let mut metadata =
            read_or_default::<StoredBookMetadata>(&directory.join(".novel-flow.json"), "本地数据")?;
        let _ = update_local_novel_metadata(
            &app_client,
            &web_client,
            &directory,
            &mut metadata,
            novel_id,
            &title,
        )
        .await;

        // 章节索引是逐章检查点：下载中断后可从已写入的章节继续，而不必重新请求全部正文。
        let mut store = read_or_default::<StoredChapterStore>(
            &directory.join(".novel-flow-chapters.json"),
            "本地数据",
        )?;
        store.novel_id = novel_id;
        // 目录来自 App API，仅提供章节 ID、分卷、标题与内容类型；它不提供或解析正文。
        let directory_data = app_client.get_chapter_catalog(novel_id).await?;
        let requested =
            chapter_ids.map(|ids| ids.into_iter().collect::<std::collections::HashSet<_>>());
        let mut chapters = Vec::new();
        // 目录按分卷、章节顺序排列，`volume_index` 和 `chapter_index` 用于在本地索引中保留原始顺序。
        for (volume_index, volume) in directory_data.iter().enumerate() {
            for (chapter_index, chapter) in volume.chapters.iter().enumerate() {
                let chapter_id = chapter.chap_id;
                if requested
                    .as_ref()
                    .is_some_and(|ids| !ids.contains(&chapter_id))
                {
                    continue;
                }
                chapters.push((
                    chapter_id,
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
        // 先建立索引检查点。即使本次全部是 VIP 且资源/OCR 失败，也会留下带小说编号的
        // 空索引，避免失败被误判为下载流程完全没有运行。
        write_atomically(
            &directory.join(".novel-flow-chapters.json"),
            &store,
            "章节索引",
        )?;
        // 元数据也必须先建立检查点。普通章节在后续网络或图片资源请求中失败时，已保存的
        // 章节索引仍能被本地书库关联到正确的作品，而不是只留下无法识别的 JSON 文件。
        metadata.downloaded_text_chapter_ids = Some(downloaded_text_chapter_ids(&store));
        write_atomically(&directory.join(".novel-flow.json"), &metadata, "书籍元数据")?;
        let total = chapters.len();
        let mut failed_vip_chapters = Vec::new();
        // 按章节顺序下载，VIP 章节可能会被跳过但仍计入总数。
        for (
            index,
            (
                chapter_id,
                volume_id,
                volume,
                volume_index,
                chapter_index,
                chapter_title,
                content_kind,
            ),
        ) in chapters.into_iter().enumerate()
        {
            if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                return Err("下载已暂停".to_string());
            }
            update_native_job(&state, &job_id, |job| {
                job.status = "downloading".to_string();
                job.progress = ((index * 96) / total) as u8;
                job.message = format!("正在读取：{chapter_title}");
            });
            emit_native_job_update(&app, &state, &job_id);

            // 普通章节只要已存在即可复用。VIP 内容必须确认是成功 OCR 得到的非空文本，才
            // 能跳过再次请求图片和识别，防止旧格式或失败残留被标记为已完成。
            let existing_vip_is_complete =
                store
                    .chapters
                    .get(&chapter_id.to_string())
                    .is_some_and(|chapter| {
                        matches!(
                            content_kind,
                            TextContentKind::ImageVip | TextContentKind::EncryptedVip
                        ) && chapter.content_source.as_deref() == Some("webVipOcr")
                            && !chapter.content.trim().is_empty()
                    });
            let should_write = !store.chapters.contains_key(&chapter_id.to_string())
                || (matches!(
                    content_kind,
                    TextContentKind::ImageVip | TextContentKind::EncryptedVip
                ) && !existing_vip_is_complete);
            if should_write {
                let (content, content_source, ocr_source_path) = match content_kind {
                    TextContentKind::ImageVip | TextContentKind::EncryptedVip => {
                        // 网页 VIP 分支：不解析 #ChapterBody。这里从 Web VIP 图片端点读取
                        // 二进制图片，`resolve_vip_image_content` 将原图保存到 ocr/ 后调用
                        // 本地 OCR，并返回与普通网页解析相同的纯文本章节内容。
                        update_native_job(&state, &job_id, |job| {
                            job.message = format!("正在保存并识别图片正文：{chapter_title}");
                        });
                        emit_native_job_update(&app, &state, &job_id);
                        let vip_result = resolve_vip_image_content(
                            &app,
                            &web_client,
                            &directory,
                            novel_id,
                            chapter_id,
                        )
                        .await;
                        let (content, source_path) = match vip_result {
                            Ok(value) => value,
                            Err(error) => {
                                failed_vip_chapters.push((chapter_title.clone(), error.clone()));
                                update_native_job(&state, &job_id, |job| {
                                    job.message = format!(
                                        "VIP 章节处理失败，已跳过：{chapter_title}（{error}）"
                                    );
                                });
                                emit_native_job_update(&app, &state, &job_id);
                                continue;
                            }
                        };
                        (content, Some("webVipOcr".to_string()), Some(source_path))
                    }
                    TextContentKind::Text | TextContentKind::Unknown => {
                        // 普通网页解析分支：从章节页面 #ChapterBody 提取正文并转换为文本
                        // / Markdown。若正文含 [img=] 标记，只下载并本地化插图，不做 OCR。
                        let raw_content =
                            resolve_text_content(&web_client, novel_id, volume_id, chapter_id)
                                .await?;
                        let content = materialize_chapter_images(
                            &web_client,
                            &directory,
                            novel_id,
                            chapter_id,
                            &raw_content,
                        )
                        .await?;
                        (content, Some("web".to_string()), None)
                    }
                };
                store.chapters.insert(
                    chapter_id.to_string(),
                    StoredChapter {
                        id: chapter_id,
                        title: chapter_title,
                        volume,
                        content,
                        volume_index,
                        chapter_index: chapter_index as i64,
                        content_source,
                        ocr_source_path,
                    },
                );
                // 每一章成功后立刻提交检查点。存储锁已防止同一本书的任务覆盖彼此的
                // 快照，原子写入则避免中断时留下截断 JSON。
                write_atomically(
                    &directory.join(".novel-flow-chapters.json"),
                    &store,
                    "章节索引",
                )?;
                // 章节与完成标记作为同一检查点更新。即使下一章请求失败、任务暂停或进程
                // 退出，恢复后的书库和下载队列也会看见已经可靠保存的内容。
                metadata.downloaded_text_chapter_ids = Some(downloaded_text_chapter_ids(&store));
                write_atomically(&directory.join(".novel-flow.json"), &metadata, "书籍元数据")?;
            }
            // 限速发生在每个章节处理完成后；取消标记在下一个章节开始前读取。
            tokio::time::sleep(std::time::Duration::from_millis(policy.request_interval_ms)).await;
        }

        // 仅在遍历完成后更新书籍级完成记录，使 UI 可依据实际持久化的章节索引显示状态。
        metadata.downloaded_text_chapter_ids = Some(downloaded_text_chapter_ids(&store));
        write_atomically(&directory.join(".novel-flow.json"), &metadata, "书籍元数据")?;

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
    .await;
    match result {
        // 只向渲染层暴露任务结果，不返回 Cookie、上游原始 HTML、VIP 图片二进制或 OCR
        // worker 的内部诊断数据。
        Ok(_) => update_native_job(&state, &job_id, |job| {
            job.status = "done".to_string();
            job.progress = 100;
            job.message = "文本章节下载完成".to_string();
        }),
        Err(_error) if cancelled.load(std::sync::atomic::Ordering::Relaxed) => {
            update_native_job(&state, &job_id, |job| {
                job.status = "paused".to_string();
                job.message = "已暂停，可继续下载".to_string();
            })
        }
        Err(error) => update_native_job(&state, &job_id, |job| {
            job.status = "error".to_string();
            job.message = error;
        }),
    }
    emit_native_job_update(&app, &state, &job_id);
    let _ = persist_native_jobs(&app, &state);
}

/// 返回不含 Windows 保留字符、长度受限的文件名片段。
///
/// # 参数
/// * `value` - 上游章节标题。
///
/// # 返回值
/// 不含 Windows 保留字符、长度受限的文件名片段。
fn safe_audio_name(value: &str) -> String {
    safe_library_name(value).chars().take(100).collect()
}

/// 初始化并更新一本本地文字小说的元数据。
///
/// 调用方只需传入整份书籍元数据，无须了解 `novel` 的可选存储形式。本方法先保证本地
/// 已知的作品编号、标题及作者/简介默认值，再尝试从公开 App 详情刷新扩展字段和封面。
/// 即使远程刷新失败，已经初始化的元数据仍会由调用方随章节索引正常保存。
async fn update_local_novel_metadata(
    client: &AppClient,
    web_client: &WebClient,
    directory: &std::path::PathBuf,
    book_metadata: &mut StoredBookMetadata,
    novel_id: i64,
    title: &str,
) -> Result<(), String> {
    let metadata = book_metadata.novel.get_or_insert_with(Default::default);
    metadata.id = Some(novel_id);
    metadata.title = Some(title.to_string());
    metadata
        .author
        .get_or_insert_with(|| "未知作者".to_string());
    metadata
        .description
        .get_or_insert_with(|| "暂无简介".to_string());

    enrich_local_novel_metadata(client, web_client, directory, metadata, novel_id).await
}

/// 从公开详情刷新小说扩展字段，并将封面保存在下载媒体旁。
///
/// 此方法只处理上游响应解析和封面文件写入；默认元数据初始化由
/// [`update_local_novel_metadata`] 负责，避免下载编排层与上游解析层混杂。
async fn enrich_local_novel_metadata(
    client: &AppClient,
    web_client: &WebClient,
    directory: &std::path::PathBuf,
    metadata: &mut StoredWorkMetadata,
    novel_id: i64,
) -> Result<(), String> {
    let detail = client
        .get_public_data(
            &format!("/novels/{novel_id}"),
            &[(
                "expand",
                "intro,typeName,sysTags,chapterCount,pointCount,fav,ticket,latestchapter,bigNovelCover".to_string(),
            )],
        )
        .await?;
    if let Some(author) = detail
        .get("authorName")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        metadata.author = Some(author.to_string());
    }
    if let Some(description) = detail
        .get("expand")
        .and_then(|value| value.get("intro"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        metadata.description = Some(description.to_string());
    }
    metadata.type_name = detail
        .get("expand")
        .and_then(|value| value.get("typeName"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string);
    metadata.tags = detail
        .get("expand")
        .and_then(|value| value.get("sysTags"))
        .and_then(Value::as_array)
        .map(|tags| {
            tags.iter()
                .filter_map(|tag| {
                    tag.as_str()
                        .or_else(|| tag.get("tagName").and_then(Value::as_str))
                        .or_else(|| tag.get("name").and_then(Value::as_str))
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToString::to_string)
                })
                .take(8)
                .collect()
        })
        .unwrap_or_default();
    metadata.is_finished = detail.get("isFinish").and_then(Value::as_bool);
    metadata.score = detail.get("point").and_then(Value::as_f64);
    metadata.chapter_count = detail
        .get("expand")
        .and_then(|value| value.get("chapterCount"))
        .and_then(Value::as_i64);
    metadata.character_count = detail.get("charCount").and_then(Value::as_i64);
    metadata.view_count = detail.get("viewTimes").and_then(Value::as_i64);
    metadata.mark_count = detail.get("markCount").and_then(Value::as_i64);
    metadata.point_count = detail
        .get("expand")
        .and_then(|value| value.get("pointCount"))
        .and_then(Value::as_i64);
    metadata.favorite_count = detail
        .get("expand")
        .and_then(|value| value.get("fav"))
        .and_then(Value::as_i64);
    metadata.ticket_count = detail
        .get("expand")
        .and_then(|value| value.get("ticket"))
        .and_then(Value::as_i64);
    metadata.allow_download = detail.get("allowDown").and_then(Value::as_bool);
    if let Some(latest) = detail.get("expand").and_then(|value| {
        value
            .get("latestChapter")
            .or_else(|| value.get("latestchapter"))
    }) {
        metadata.latest_chapter_title = latest
            .get("title")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string);
        metadata.latest_chapter_time = latest
            .get("addTime")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string);
    }
    metadata.last_update_time = detail
        .get("lastUpdateTime")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string);
    let Some(cover_url) = detail
        .get("expand")
        .and_then(|expand| expand.get("bigNovelCover"))
        .and_then(Value::as_str)
        .or_else(|| detail.get("novelCover").and_then(Value::as_str))
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
    else {
        return Ok(());
    };
    let cover = directory.join("imgs").join("novel-cover.jpeg");
    if cover.is_file() {
        return Ok(());
    }
    let source = if cover_url.starts_with("http://") || cover_url.starts_with("https://") {
        cover_url
    } else {
        format!(
            "https://book.sfacg.com/{}",
            cover_url.trim_start_matches('/')
        )
    };
    let payload = web_client
        .asset_request(
            source,
            &format!("https://book.sfacg.com/Novel/{novel_id}/"),
            "image/avif,image/webp,image/*,*/*;q=0.8",
        )
        .send()
        .await
        .map_err(|error| format!("无法下载封面：{error}"))?
        .error_for_status()
        .map_err(|error| format!("封面下载被拒绝：{error}"))?
        .bytes()
        .await
        .map_err(|error| format!("无法读取封面：{error}"))?;
    let parent = cover.parent().ok_or_else(|| "封面目录无效".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("无法创建封面目录：{error}"))?;
    let partial = cover.with_extension("jpeg.part");
    fs::write(&partial, payload).map_err(|error| format!("无法写入封面：{error}"))?;
    fs::rename(&partial, &cover).map_err(|error| format!("无法完成封面写入：{error}"))?;
    Ok(())
}

/// 新增或更新本地有声书元数据，并将其封面保存在下载的媒体旁边。
async fn enrich_local_audio_book(
    client: &AppClient,
    web_client: &WebClient,
    directory: &std::path::PathBuf,
    metadata: &mut StoredWorkMetadata,
    novel_id: i64,
    album_id: Option<i64>,
) -> Result<(), String> {
    if let Some(album_id) = album_id {
        let detail = client
            .get_public_data(
                &format!("/albums/{album_id}"),
                &[("expand", "intro,typeName,sysTags,latestchapter".to_string())],
            )
            .await?;
        let text = |keys: &[&str]| {
            keys.iter().find_map(|key| {
                detail
                    .get(*key)
                    .and_then(Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .map(ToString::to_string)
                    .or_else(|| {
                        detail
                            .get("expand")
                            .and_then(|expand| expand.get(*key))
                            .and_then(Value::as_str)
                            .filter(|value| !value.trim().is_empty())
                            .map(ToString::to_string)
                    })
            })
        };
        metadata.id = Some(album_id);
        metadata.catalog_id = Some(novel_id);
        metadata.title =
            text(&["name", "albumName", "novelName"]).or_else(|| metadata.title.clone());
        metadata.author =
            text(&["authorName", "author", "anchorName"]).or_else(|| metadata.author.clone());
        metadata.description =
            text(&["intro", "description", "content"]).or_else(|| metadata.description.clone());
        metadata.type_name =
            text(&["typeName", "categoryName"]).or_else(|| metadata.type_name.clone());
        metadata.last_update_time =
            text(&["lastUpdateTime", "updateTime"]).or_else(|| metadata.last_update_time.clone());
        metadata.is_finished = detail
            .get("isFinished")
            .or_else(|| detail.get("isFinish"))
            .and_then(Value::as_bool)
            .or(metadata.is_finished);
        metadata.view_count = detail
            .get("visitTimes")
            .or_else(|| detail.get("viewTimes"))
            .and_then(Value::as_i64)
            .or(metadata.view_count);
        if let Some(tags) = detail
            .get("sysTags")
            .or_else(|| detail.get("tags"))
            .or_else(|| {
                detail
                    .get("expand")
                    .and_then(|expand| expand.get("sysTags"))
            })
            .and_then(Value::as_array)
        {
            metadata.tags = tags
                .iter()
                .filter_map(|tag| {
                    tag.as_str()
                        .or_else(|| tag.get("tagName").and_then(Value::as_str))
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToString::to_string)
                })
                .take(8)
                .collect();
        }
        let cover_url = if let Some(cover_url) = text(&["coverBig"]) {
            cover_url
        } else if let Some(cover_url) = fetch_novel_big_cover(client, novel_id).await.ok().flatten()
        {
            cover_url
        } else if let Some(cover_url) = text(&["coverMedium", "coverSmall", "albumCover", "cover"])
        {
            cover_url
        } else {
            return Ok(());
        };
        return save_audio_cover(web_client, directory, &cover_url).await;
    }
    let response = web_client
        .ajax_request(
            "https://i.sfacg.com/ajax/ashx/Common.ashx",
            "https://i.sfacg.com/consume/book/",
        )
        .query(&[("op", "getAudioInfo"), ("nid", &novel_id.to_string())])
        .send()
        .await
        .map_err(|error| format!("无法连接 SF 有声信息接口：{error}"))?
        .error_for_status()
        .map_err(|error| format!("SF 有声信息接口被拒绝：{error}"))?
        .json::<Value>()
        .await
        .map_err(|error| format!("SF 有声信息格式无效：{error}"))?;
    if response.get("status").and_then(Value::as_i64) != Some(200) {
        return Err("SF 有声信息接口未返回可用内容".to_string());
    }
    let data = response
        .get("data")
        .ok_or_else(|| "SF 有声信息接口未返回详情".to_string())?;
    let text = |keys: &[&str]| {
        keys.iter().find_map(|key| {
            data.get(*key)
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(ToString::to_string)
        })
    };
    metadata.id = Some(novel_id);
    metadata.catalog_id = Some(novel_id);
    metadata.title = text(&["AlbumName", "NovelName", "Title"]).or_else(|| metadata.title.clone());
    metadata.author =
        text(&["AuthorName", "Author", "AnchorName"]).or_else(|| metadata.author.clone());
    metadata.description =
        text(&["Intro", "Description", "Content"]).or_else(|| metadata.description.clone());
    metadata.type_name = text(&["TypeName", "CategoryName"]).or_else(|| metadata.type_name.clone());
    metadata.last_update_time =
        text(&["LastUpdateTime", "UpdateTime"]).or_else(|| metadata.last_update_time.clone());
    let cover_url =
        if let Some(cover_url) = fetch_novel_big_cover(client, novel_id).await.ok().flatten() {
            cover_url
        } else if let Some(cover_url) = text(&[
            "CoverBig",
            "AlbumCover",
            "CoverMedium",
            "CoverSmall",
            "Cover",
        ]) {
            cover_url
        } else {
            return Ok(());
        };
    save_audio_cover(web_client, directory, &cover_url).await
}

/// 获取小说的“大封面”URL，如果存在的话。
async fn fetch_novel_big_cover(
    client: &AppClient,
    novel_id: i64,
) -> Result<Option<String>, String> {
    let detail = client
        .get_public_data(
            &format!("/novels/{novel_id}"),
            &[("expand", "bigNovelCover".to_string())],
        )
        .await?;
    Ok(detail
        .get("expand")
        .and_then(|expand| expand.get("bigNovelCover"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string))
}

/// 下载并以原子替换方式保存有声作品封面。
///
/// # 参数
/// * `client` - 仅持有 Web 会话的资源请求客户端。
/// * `directory` - 已校验的本地书籍目录。
/// * `cover_url` - 上游返回的封面绝对或相对地址。
///
/// # 错误
/// 封面无法下载、读取、创建目录或写入本地文件时返回错误。
async fn save_audio_cover(
    client: &WebClient,
    directory: &std::path::PathBuf,
    cover_url: &str,
) -> Result<(), String> {
    let cover = directory.join("imgs").join("audio-cover.jpeg");
    let source = if cover_url.starts_with("http://") || cover_url.starts_with("https://") {
        cover_url.to_string()
    } else {
        format!("https://i.sfacg.com/{}", cover_url.trim_start_matches('/'))
    };
    let payload = client
        .asset_request(
            source,
            "https://i.sfacg.com/consume/book/",
            "image/avif,image/webp,image/*,*/*;q=0.8",
        )
        .send()
        .await
        .map_err(|error| format!("无法下载有声封面：{error}"))?
        .error_for_status()
        .map_err(|error| format!("有声封面下载被拒绝：{error}"))?
        .bytes()
        .await
        .map_err(|error| format!("无法读取有声封面：{error}"))?;
    let parent = cover.parent().ok_or_else(|| "封面目录无效".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("无法创建封面目录：{error}"))?;
    let partial = cover.with_extension("jpeg.part");
    fs::write(&partial, payload).map_err(|error| format!("无法写入有声封面：{error}"))?;
    fs::rename(&partial, &cover).map_err(|error| format!("无法完成有声封面写入：{error}"))?;
    Ok(())
}

/// 从SF漫画接口获取漫画元数据并持久化作品封面。
async fn enrich_local_comic_book(
    client: &AppClient,
    web_client: &WebClient,
    directory: &std::path::PathBuf,
    metadata: &mut StoredWorkMetadata,
    comic_id: i64,
) -> Result<(), String> {
    let detail = client
        .get_public_data(
            &format!("/comics/{comic_id}"),
            &[(
                "expand",
                "intro,typeName,sysTags,latestchapter,fav,ticket,pointCount".to_string(),
            )],
        )
        .await?;
    let text = |keys: &[&str]| {
        keys.iter().find_map(|key| {
            detail
                .get(*key)
                .and_then(Value::as_str)
                .filter(|v| !v.trim().is_empty())
                .map(ToString::to_string)
                .or_else(|| {
                    detail
                        .get("expand")
                        .and_then(|expand| expand.get(*key))
                        .and_then(Value::as_str)
                        .filter(|v| !v.trim().is_empty())
                        .map(ToString::to_string)
                })
        })
    };
    metadata.id = Some(comic_id);
    metadata.online_path = text(&["folderName"]);
    metadata.title = text(&["comicName", "novelName"]).or_else(|| metadata.title.clone());
    metadata.author = text(&["authorName", "author"]).or_else(|| metadata.author.clone());
    metadata.description =
        text(&["intro", "description", "content"]).or_else(|| metadata.description.clone());
    metadata.type_name = text(&["typeName", "categoryName"]).or_else(|| metadata.type_name.clone());
    metadata.last_update_time =
        text(&["lastUpdateTime", "updateTime"]).or_else(|| metadata.last_update_time.clone());
    metadata.is_finished = detail
        .get("isFinish")
        .and_then(Value::as_bool)
        .or(metadata.is_finished);
    metadata.score = detail
        .get("point")
        .and_then(Value::as_f64)
        .or(metadata.score);
    metadata.view_count = detail
        .get("viewTimes")
        .and_then(Value::as_i64)
        .or(metadata.view_count);
    metadata.mark_count = detail
        .get("markCount")
        .and_then(Value::as_i64)
        .or(metadata.mark_count);
    metadata.favorite_count = detail
        .get("favoriteCount")
        .or_else(|| detail.get("fav"))
        .and_then(Value::as_i64)
        .or(metadata.favorite_count);
    if let Some(tags) = detail
        .get("tags")
        .or_else(|| detail.get("sysTags"))
        .or_else(|| detail.get("expand").and_then(|expand| expand.get("tags")))
        .or_else(|| {
            detail
                .get("expand")
                .and_then(|expand| expand.get("sysTags"))
        })
        .and_then(Value::as_array)
    {
        metadata.tags = tags
            .iter()
            .filter_map(|tag| {
                tag.as_str()
                    .or_else(|| tag.get("tagName").and_then(Value::as_str))
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .map(ToString::to_string)
            })
            .take(8)
            .collect();
    }
    let Some(cover_url) = text(&[
        "coverBig",
        "comicCover",
        "coverMedium",
        "coverSmall",
        "novelCover",
    ]) else {
        return Ok(());
    };
    let cover = directory.join("imgs").join("comic-cover.jpeg");
    if cover.is_file() {
        return Ok(());
    }
    let source = if cover_url.starts_with("http://") || cover_url.starts_with("https://") {
        cover_url
    } else {
        format!(
            "https://manhua.sfacg.com{}",
            if cover_url.starts_with('/') {
                cover_url
            } else {
                format!("/{cover_url}")
            }
        )
    };
    let payload = web_client
        .asset_request(
            source,
            "https://manhua.sfacg.com/",
            "image/avif,image/webp,image/*,*/*;q=0.8",
        )
        .send()
        .await
        .map_err(|error| format!("无法下载漫画封面：{error}"))?
        .error_for_status()
        .map_err(|error| format!("漫画封面下载被拒绝：{error}"))?
        .bytes()
        .await
        .map_err(|error| format!("无法读取漫画封面：{error}"))?;
    let parent = cover.parent().ok_or_else(|| "封面目录无效".to_string())?;
    fs::create_dir_all(parent).map_err(|e| format!("无法创建封面目录：{e}"))?;
    let partial = cover.with_extension("jpeg.part");
    fs::write(&partial, payload).map_err(|e| format!("无法写入漫画封面：{e}"))?;
    fs::rename(&partial, &cover).map_err(|e| format!("无法完成漫画封面写入：{e}"))?;
    Ok(())
}

/// 运行已认证的有声下载任务，并重建本地 M3U8 列表。
///
/// # 参数
/// * `app` - 用于本地状态和受控本地存储的 Tauri 应用句柄。
/// * `job_id` - 本地任务标识符。
/// * `novel_id` - SF作品标识符。
/// * `title` - 用于展示的作品标题。
/// * `chapter_ids` - 所选音频章节标识符。
/// * `cancelled` - 由任务命令控制的协作取消标志。
async fn run_audio_download(
    app: tauri::AppHandle,
    job_id: String,
    novel_id: i64,
    album_id: Option<i64>,
    title: String,
    chapter_ids: Vec<i64>,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
) {
    let state = app.state::<NativeJobState>();
    let result: Result<String, String> = async {
        #[cfg(target_os = "android")]
        sync_android_auth_session(&app).await?;
        let app_client = app_endpoint_client(&app, EndpointCapability::NovelDetail)?;
        let web_client = web_endpoint_client(&app, EndpointCapability::Audio)?;
        let (_catalog_title, catalog) = web_client
            .get_audio_catalog(novel_id)
            .await
            .map_err(|error| EndpointCapability::Audio.unavailable_message(&error))?;
        let selected = chapter_ids
            .into_iter()
            .collect::<std::collections::HashSet<_>>();
        let chapters = catalog
            .iter()
            .filter(|chapter| selected.contains(&chapter.id))
            .collect::<Vec<_>>();
        if chapters.is_empty() {
            return Err("没有可下载的有声章节".to_string());
        }
        let directory = library_directory(&app)?.join(safe_library_name(&title));
        let audio_directory = directory.join("audio");
        fs::create_dir_all(&audio_directory)
            .map_err(|error| format!("无法创建有声目录：{error}"))?;
        let mut metadata =
            read_or_default::<StoredBookMetadata>(&directory.join(".novel-flow.json"), "本地数据")?;
        let audio_metadata = metadata.audio.get_or_insert_with(Default::default);
        audio_metadata.id = Some(album_id.unwrap_or(novel_id));
        audio_metadata.catalog_id = Some(novel_id);
        audio_metadata.title = Some(title.clone());
        audio_metadata
            .author
            .get_or_insert_with(|| "未知作者".to_string());
        audio_metadata
            .description
            .get_or_insert_with(|| "暂无简介".to_string());
        let _ = enrich_local_audio_book(
            &app_client,
            &web_client,
            &directory,
            audio_metadata,
            novel_id,
            album_id,
        )
        .await;
        let mut downloaded = metadata
            .downloaded_audio_chapter_ids
            .take()
            .unwrap_or_default()
            .into_iter()
            .collect::<std::collections::HashSet<_>>();
        let policy = get_request_policy(app.clone()).unwrap_or_default();
        let total = chapters.len();
        for (selected_index, chapter) in chapters.iter().enumerate() {
            if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
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
            update_native_job(&state, &job_id, |job| {
                job.status = "downloading".to_string();
                job.progress = 2 + ((selected_index * 94) / total) as u8;
                job.message = format!("正在下载：{}", chapter.title);
            });
            emit_native_job_update(&app, &state, &job_id);
            if !target.is_file() {
                let payload = web_client
                    .asset_request(
                        &chapter.source,
                        "https://i.sfacg.com/consume/book/",
                        "audio/mpeg,*/*;q=0.8",
                    )
                    .send()
                    .await
                    .map_err(|error| {
                        EndpointCapability::Audio
                            .unavailable_message(&format!("无法下载有声章节：{error}"))
                    })?
                    .error_for_status()
                    .map_err(|error| {
                        EndpointCapability::Audio
                            .unavailable_message(&format!("有声章节下载被拒绝：{error}"))
                    })?
                    .bytes()
                    .await
                    .map_err(|error| {
                        EndpointCapability::Audio
                            .unavailable_message(&format!("无法读取有声章节：{error}"))
                    })?;
                let partial = target.with_extension("mp3.part");
                fs::write(&partial, payload)
                    .map_err(|error| format!("无法写入有声章节：{error}"))?;
                if target.exists() {
                    fs::remove_file(&target)
                        .map_err(|error| format!("无法替换有声章节：{error}"))?;
                }
                fs::rename(&partial, &target)
                    .map_err(|error| format!("无法完成有声章节写入：{error}"))?;
            }
            downloaded.insert(chapter.id);
            tokio::time::sleep(std::time::Duration::from_millis(policy.request_interval_ms)).await;
        }
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
        .map_err(|error| format!("无法写入有声播放列表：{error}"))?;
        metadata.downloaded_audio_chapter_ids = Some(downloaded.into_iter().collect());
        let payload = serde_json::to_vec_pretty(&metadata)
            .map_err(|error| format!("无法序列化书籍元数据：{error}"))?;
        fs::write(directory.join(".novel-flow.json.tmp"), payload)
            .map_err(|error| format!("无法写入书籍元数据：{error}"))?;
        if directory.join(".novel-flow.json").exists() {
            fs::remove_file(directory.join(".novel-flow.json"))
                .map_err(|error| format!("无法替换书籍元数据：{error}"))?;
        }
        fs::rename(
            directory.join(".novel-flow.json.tmp"),
            directory.join(".novel-flow.json"),
        )
        .map_err(|error| format!("无法完成书籍元数据写入：{error}"))?;
        Ok("audio/有声目录.m3u8".to_string())
    }
    .await;
    match result {
        Ok(file) => update_native_job(&state, &job_id, |job| {
            job.status = "done".to_string();
            job.progress = 100;
            job.message = "有声章节下载完成".to_string();
            job.file = Some(file);
        }),
        Err(_error) if cancelled.load(std::sync::atomic::Ordering::Relaxed) => {
            update_native_job(&state, &job_id, |job| {
                job.status = "paused".to_string();
                job.message = "已暂停，可继续下载".to_string();
            })
        }
        Err(error) => update_native_job(&state, &job_id, |job| {
            job.status = "error".to_string();
            job.message = error;
        }),
    }
    emit_native_job_update(&app, &state, &job_id);
    let _ = persist_native_jobs(&app, &state);
}

/// 根据已校验的 SF 漫画图片地址确定文件扩展名。
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

/// 将选中的漫画章节下载到应用自有页面目录。
///
/// VIP 章节需要 SF 网页会话；每个目录和图片请求均复用同一经过校验的客户端快照。
///
/// # 参数
/// * `app` - 用于访问原生状态、会话和本地存储的 Tauri 应用句柄。
/// * `job_id` - 原生下载任务标识符。
/// * `comic_id` - SF 漫画编号。
/// * `source_path` - 可选的公开漫画目录标识。
/// * `title` - 用于本地目录的用户可见作品标题。
/// * `chapter_ids` - 待下载的漫画章节编号。
/// * `cancelled` - 由暂停、删除命令控制的协作式取消标记。
async fn run_comic_download(
    app: tauri::AppHandle,
    job_id: String,
    comic_id: i64,
    source_path: Option<String>,
    title: String,
    chapter_ids: Vec<i64>,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
) {
    let state = app.state::<NativeJobState>();
    let result: Result<String, String> = async {
        #[cfg(target_os = "android")]
        sync_android_auth_session(&app).await?;
        let web_client = web_endpoint_client(&app, EndpointCapability::ComicCatalog)?;
        let pages_client = web_endpoint_client(&app, EndpointCapability::ComicPages)?;
        let folder = if let Some(folder) = source_path.filter(|value| {
            !value.is_empty()
                && value.len() <= 100
                && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
        }) {
            folder
        } else {
            let app_client = app_endpoint_client(&app, EndpointCapability::ComicIdentity)?;
            let (_, folder) = app_client.comic_identity(comic_id).await?;
            folder
        };
        let catalog = web_client
            .get_comic_catalog(&folder)
            .await
            .map_err(|error| EndpointCapability::ComicCatalog.unavailable_message(&error))?;
        let requested = chapter_ids
            .into_iter()
            .collect::<std::collections::HashSet<_>>();
        let chapters = catalog
            .into_iter()
            .filter(|chapter| requested.contains(&chapter.id) && chapter.is_unlocked)
            .collect::<Vec<_>>();
        if chapters.is_empty() {
            return Err(
                "所选漫画章节没有可下载内容；VIP 章节必须由目录明确标记为已解锁".to_string(),
            );
        }
        let directory = library_directory(&app)?.join(safe_library_name(&title));
        let comic_directory = directory.join("comic");
        fs::create_dir_all(&comic_directory)
            .map_err(|error| format!("无法创建漫画目录：{error}"))?;
        let mut metadata =
            read_or_default::<StoredBookMetadata>(&directory.join(".novel-flow.json"), "本地数据")?;
        let comic_metadata = metadata.comic.get_or_insert_with(Default::default);
        comic_metadata.id = Some(comic_id);
        comic_metadata.title = Some(title.clone());
        comic_metadata
            .author
            .get_or_insert_with(|| "未知作者".to_string());
        comic_metadata
            .description
            .get_or_insert_with(|| "暂无简介".to_string());
        let mut downloaded = metadata
            .downloaded_comic_chapter_ids
            .take()
            .unwrap_or_default()
            .into_iter()
            .collect::<std::collections::HashSet<_>>();
        let policy = get_request_policy(app.clone()).unwrap_or_default();
        let total = chapters.len();
        for (chapter_index, chapter) in chapters.into_iter().enumerate() {
            if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                return Err("下载已暂停".to_string());
            }
            update_native_job(&state, &job_id, |job| {
                job.status = "downloading".to_string();
                job.progress = 2 + ((chapter_index * 94) / total) as u8;
                job.message = format!("正在读取：{}", chapter.title);
            });
            emit_native_job_update(&app, &state, &job_id);
            let chapter_directory = comic_directory.join(format!("{:06}", chapter.id));
            let marker = chapter_directory.join(".complete");
            if !marker.is_file() {
                let images = pages_client
                    .comic_chapter_images(&folder, chapter.id)
                    .await
                    .map_err(|error| EndpointCapability::ComicPages.unavailable_message(&error))?;
                fs::create_dir_all(&chapter_directory)
                    .map_err(|error| format!("无法创建漫画章节目录：{error}"))?;
                for (page_index, image) in images.iter().enumerate() {
                    if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                        return Err("下载已暂停".to_string());
                    }
                    update_native_job(&state, &job_id, |job| {
                        job.message = format!(
                            "正在下载：{}（{}/{})",
                            chapter.title,
                            page_index + 1,
                            images.len()
                        );
                    });
                    emit_native_job_update(&app, &state, &job_id);
                    let extension = comic_image_extension(image);
                    let target =
                        chapter_directory.join(format!("{:03}.{extension}", page_index + 1));
                    if target.is_file() {
                        continue;
                    }
                    let payload = pages_client
                        .asset_request(
                            image,
                            "https://manhua.sfacg.com/",
                            "image/avif,image/webp,image/apng,image/*,*/*;q=0.8",
                        )
                        .send()
                        .await
                        .map_err(|error| {
                            EndpointCapability::ComicPages
                                .unavailable_message(&format!("无法下载漫画图片：{error}"))
                        })?
                        .error_for_status()
                        .map_err(|error| {
                            EndpointCapability::ComicPages
                                .unavailable_message(&format!("漫画图片下载被拒绝：{error}"))
                        })?
                        .bytes()
                        .await
                        .map_err(|error| {
                            EndpointCapability::ComicPages
                                .unavailable_message(&format!("无法读取漫画图片：{error}"))
                        })?;
                    let partial = target.with_extension(format!("{extension}.part"));
                    fs::write(&partial, payload)
                        .map_err(|error| format!("无法写入漫画图片：{error}"))?;
                    fs::rename(&partial, &target)
                        .map_err(|error| format!("无法完成漫画图片写入：{error}"))?;
                }
                fs::write(&marker, chapter.title.as_bytes())
                    .map_err(|error| format!("无法完成漫画章节写入：{error}"))?;
            }
            downloaded.insert(chapter.id);
            tokio::time::sleep(std::time::Duration::from_millis(policy.request_interval_ms)).await;
        }
        metadata.downloaded_comic_chapter_ids = Some(downloaded.into_iter().collect());
        let payload = serde_json::to_vec_pretty(&metadata)
            .map_err(|error| format!("无法序列化漫画元数据：{error}"))?;
        fs::write(directory.join(".novel-flow.json.tmp"), payload)
            .map_err(|error| format!("无法写入漫画元数据：{error}"))?;
        if directory.join(".novel-flow.json").exists() {
            fs::remove_file(directory.join(".novel-flow.json"))
                .map_err(|error| format!("无法替换漫画元数据：{error}"))?;
        }
        fs::rename(
            directory.join(".novel-flow.json.tmp"),
            directory.join(".novel-flow.json"),
        )
        .map_err(|error| format!("无法完成漫画元数据写入：{error}"))?;
        Ok("comic".to_string())
    }
    .await;
    match result {
        Ok(file) => update_native_job(&state, &job_id, |job| {
            job.status = "done".to_string();
            job.progress = 100;
            job.message = "漫画章节下载完成".to_string();
            job.file = Some(file);
        }),
        Err(_error) if cancelled.load(std::sync::atomic::Ordering::Relaxed) => {
            update_native_job(&state, &job_id, |job| {
                job.status = "paused".to_string();
                job.message = "已暂停，可继续下载".to_string();
            })
        }
        Err(error) => update_native_job(&state, &job_id, |job| {
            job.status = "error".to_string();
            job.message = error;
        }),
    }
    emit_native_job_update(&app, &state, &job_id);
    let _ = persist_native_jobs(&app, &state);
}

/// 创建并调度一个 SF 文字下载任务。
///
/// # 参数
/// * `app` - Tauri 应用句柄，用于访问本地状态和存储。
/// * `novel_id` - SF 小说编号。
/// * `title` - 用户可见的标题，用于本地书籍目录。
/// * `chapter_ids` - 所选章节编号列表。
///
/// # 错误
/// 输入无效或本地任务注册表不可用时返回错误。
#[tauri::command]
pub(crate) async fn create_text_download(
    app: tauri::AppHandle,
    novel_id: i64,
    title: String,
    chapter_ids: Vec<i64>,
) -> Result<NativeJob, String> {
    validate_novel_id(novel_id)?;
    if title.trim().is_empty() || title.chars().count() > 200 {
        return Err("小说标题无效".to_string());
    }
    if chapter_ids.is_empty() || chapter_ids.iter().any(|id| *id <= 0) {
        return Err("章节选择无效".to_string());
    }
    if !ensure_external_storage_access(app.clone()).await? {
        return Err("请在系统设置中允许本应用管理所有文件，然后重试下载".to_string());
    }
    let id = format!(
        "{}-{novel_id}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "系统时间无效".to_string())?
            .as_millis()
    );
    let job = NativeJob {
        id: id.clone(),
        title: title.clone(),
        kind: "text".to_string(),
        status: "queued".to_string(),
        progress: 0,
        message: "等待开始".to_string(),
        file: None,
        novel_id: Some(novel_id),
    };
    let state = app.state::<NativeJobState>();
    state
        .jobs
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .insert(id.clone(), job.clone());
    let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    state
        .cancellations
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .insert(id.clone(), cancelled.clone());
    state
        .specs
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .insert(
            id.clone(),
            NativeJobSpec {
                kind: "text".to_string(),
                novel_id,
                source_id: None,
                source_path: None,
                title: title.clone(),
                chapter_ids: chapter_ids.clone(),
            },
        );
    persist_native_jobs(&app, &state)?;
    tauri::async_runtime::spawn(run_text_download(
        app,
        id,
        novel_id,
        title,
        Some(chapter_ids),
        cancelled,
    ));
    Ok(job)
}

/// 创建并调度一个 SF 有声下载任务。
///
/// # 参数
/// * `app` - Tauri 应用句柄，用于访问本地状态和存储。
/// * `novel_id` - SF 小说编号。
/// * `title` - 用户可见的标题，用于本地书籍目录。
/// * `chapter_ids` - 所选章节编号列表，不能为空。
///
/// # 错误
/// 输入无效或本地任务注册表不可用时返回错误。
#[tauri::command]
pub(crate) async fn create_audio_download(
    app: tauri::AppHandle,
    novel_id: i64,
    album_id: Option<i64>,
    title: String,
    chapter_ids: Vec<i64>,
) -> Result<NativeJob, String> {
    validate_novel_id(novel_id)?;
    if title.trim().is_empty() || title.chars().count() > 200 {
        return Err("小说标题无效".to_string());
    }
    if chapter_ids.is_empty() || chapter_ids.iter().any(|id| *id <= 0) {
        return Err("章节选择无效".to_string());
    }
    if !ensure_external_storage_access(app.clone()).await? {
        return Err("请在系统设置中允许本应用管理所有文件，然后重试下载".to_string());
    }
    #[cfg(target_os = "android")]
    sync_android_auth_session(&app).await?;
    let _ = web_endpoint_client(&app, EndpointCapability::Audio)?;
    let id = format!(
        "{}-{novel_id}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "系统时间无效".to_string())?
            .as_millis()
    );
    let job = NativeJob {
        id: id.clone(),
        title: title.clone(),
        kind: "audio".to_string(),
        status: "queued".to_string(),
        progress: 0,
        message: "等待开始".to_string(),
        file: None,
        novel_id: Some(novel_id),
    };
    let state = app.state::<NativeJobState>();
    state
        .jobs
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .insert(id.clone(), job.clone());
    let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    state
        .cancellations
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .insert(id.clone(), cancelled.clone());
    state
        .specs
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .insert(
            id.clone(),
            NativeJobSpec {
                kind: "audio".to_string(),
                novel_id,
                source_id: album_id,
                source_path: None,
                title: title.clone(),
                chapter_ids: chapter_ids.clone(),
            },
        );
    persist_native_jobs(&app, &state)?;
    tauri::async_runtime::spawn(run_audio_download(
        app,
        id,
        novel_id,
        album_id,
        title,
        chapter_ids,
        cancelled,
    ));
    Ok(job)
}

/// 创建并调度一个 SF 漫画下载任务。
///
/// 在登记任务前读取已认证漫画目录，以拒绝尚未解锁的 VIP 章节；实际下载在后台任务中执行。
///
/// # 参数
/// * `app` - 用于访问原生状态、会话和本地存储的 Tauri 应用句柄。
/// * `comic_id` - 正数 SF 漫画编号。
/// * `source_path` - 可选的公开漫画目录标识。
/// * `title` - 用于本地书籍目录的用户可见标题。
/// * `chapter_ids` - 非空的选中漫画章节编号列表。
///
/// # 错误
/// 输入无效、外部存储不可写、会话或目录不可用、所选 VIP 章节未解锁，或任务注册表不可用时返回错误。
#[tauri::command]
pub(crate) async fn create_comic_download(
    app: tauri::AppHandle,
    comic_id: i64,
    source_path: Option<String>,
    title: String,
    chapter_ids: Vec<i64>,
) -> Result<NativeJob, String> {
    validate_novel_id(comic_id)?;
    if title.trim().is_empty() || title.chars().count() > 200 {
        return Err("漫画标题无效".to_string());
    }
    if chapter_ids.is_empty() || chapter_ids.iter().any(|id| *id <= 0) {
        return Err("漫画章节选择无效".to_string());
    }
    if !ensure_external_storage_access(app.clone()).await? {
        return Err("请在系统设置中允许本应用管理所有文件，然后重试下载".to_string());
    }
    #[cfg(target_os = "android")]
    sync_android_auth_session(&app).await?;
    let web_client = web_endpoint_client(&app, EndpointCapability::ComicCatalog)?;
    let folder = if let Some(folder) = source_path.clone().filter(|value| {
        !value.is_empty()
            && value.len() <= 100
            && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
    }) {
        folder
    } else {
        let app_client = app_endpoint_client(&app, EndpointCapability::ComicIdentity)?;
        let (_, folder) = app_client.comic_identity(comic_id).await?;
        folder
    };
    let catalog = web_client
        .get_comic_catalog(&folder)
        .await
        .map_err(|error| EndpointCapability::ComicCatalog.unavailable_message(&error))?;
    let requested = chapter_ids
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    if catalog
        .iter()
        .any(|chapter| requested.contains(&chapter.id) && !chapter.is_unlocked)
    {
        return Err("所选 VIP 漫画章节尚未解锁，请确认账号已购买对应章节".to_string());
    }
    let id = format!(
        "{}-{comic_id}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "系统时间无效".to_string())?
            .as_millis()
    );
    let job = NativeJob {
        id: id.clone(),
        title: title.clone(),
        kind: "comic".to_string(),
        status: "queued".to_string(),
        progress: 0,
        message: "等待开始".to_string(),
        file: None,
        novel_id: Some(comic_id),
    };
    let state = app.state::<NativeJobState>();
    state
        .jobs
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .insert(id.clone(), job.clone());
    let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    state
        .cancellations
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .insert(id.clone(), cancelled.clone());
    state
        .specs
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .insert(
            id.clone(),
            NativeJobSpec {
                kind: "comic".to_string(),
                novel_id: comic_id,
                source_id: Some(comic_id),
                source_path: source_path.clone(),
                title: title.clone(),
                chapter_ids: chapter_ids.clone(),
            },
        );
    persist_native_jobs(&app, &state)?;
    tauri::async_runtime::spawn(run_comic_download(
        app,
        id,
        comic_id,
        source_path,
        title,
        chapter_ids,
        cancelled,
    ));
    Ok(job)
}

/// 获取下载任务列表。
///
/// # 参数
/// * `app` - 用于访问原生任务注册表的 Tauri 应用句柄。
///
/// # 错误
/// 任务注册表锁不可用时返回错误。
#[tauri::command]
pub(crate) fn list_download_jobs(app: tauri::AppHandle) -> Result<Vec<NativeJob>, String> {
    Ok(list_download_jobs_inner(&app.state::<NativeJobState>()))
}

/// 暂停正在运行的下载任务。
///
/// # 参数
/// * `app` - 用于访问原生任务状态的 Tauri 应用句柄。
/// * `job_id` - 已存在任务的标识符。
///
/// # 错误
/// 不存在匹配任务或任务状态不可用时返回错误。
#[tauri::command]
pub(crate) fn pause_download_job(
    app: tauri::AppHandle,
    job_id: String,
) -> Result<NativeJob, String> {
    let state = app.state::<NativeJobState>();
    let cancellation = state
        .cancellations
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .get(&job_id)
        .cloned()
        .ok_or_else(|| "下载任务不存在".to_string())?;
    cancellation.store(true, std::sync::atomic::Ordering::Relaxed);
    let mut jobs = state
        .jobs
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?;
    let job = jobs
        .get_mut(&job_id)
        .ok_or_else(|| "下载任务不存在".to_string())?;
    if job.status == "queued" || job.status == "downloading" {
        job.status = "paused".to_string();
        job.message = "正在暂停下载".to_string();
    }
    let result = job.clone();
    drop(jobs);
    persist_native_jobs(&app, &state)?;
    Ok(result)
}

/// 从持久化状态恢复已暂停的下载任务。
///
/// # 参数
/// * `app` - 用于访问原生任务状态的 Tauri 应用句柄。
/// * `job_id` - 已存在暂停任务的标识符。
///
/// # 错误
/// 任务不存在、未暂停、规格已过期或不再可恢复时返回错误。
#[tauri::command]
pub(crate) fn resume_download_job(
    app: tauri::AppHandle,
    job_id: String,
) -> Result<NativeJob, String> {
    let app_for_state = app.clone();
    let state = app_for_state.state::<NativeJobState>();
    let job = state
        .jobs
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .get(&job_id)
        .cloned()
        .ok_or_else(|| "下载任务不存在".to_string())?;
    if job.status != "paused" {
        return Err("只有已暂停的任务可以继续".to_string());
    }
    let spec = state
        .specs
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .get(&job_id)
        .map(|spec| NativeJobSpec {
            kind: spec.kind.clone(),
            novel_id: spec.novel_id,
            source_id: spec.source_id,
            source_path: spec.source_path.clone(),
            title: spec.title.clone(),
            chapter_ids: spec.chapter_ids.clone(),
        })
        .ok_or_else(|| "任务参数已过期，无法继续下载".to_string())?;
    let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    state
        .cancellations
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .insert(job_id.clone(), cancelled.clone());
    update_native_job(&state, &job_id, |job| {
        job.status = "queued".to_string();
        job.message = "等待继续".to_string();
    });
    persist_native_jobs(&app, &state)?;
    if spec.kind == "audio" {
        tauri::async_runtime::spawn(run_audio_download(
            app,
            job_id,
            spec.novel_id,
            spec.source_id,
            spec.title,
            spec.chapter_ids,
            cancelled,
        ));
    } else if spec.kind == "comic" {
        let comic_id = spec.source_id.unwrap_or(spec.novel_id);
        tauri::async_runtime::spawn(run_comic_download(
            app,
            job_id,
            comic_id,
            spec.source_path,
            spec.title,
            spec.chapter_ids,
            cancelled,
        ));
    } else {
        tauri::async_runtime::spawn(run_text_download(
            app,
            job_id,
            spec.novel_id,
            spec.title,
            Some(spec.chapter_ids),
            cancelled,
        ));
    }
    list_download_jobs_inner(&state)
        .into_iter()
        .find(|item| item.id == job.id)
        .ok_or_else(|| "下载任务不存在".to_string())
}

/// 获取所有下载任务的列表，并按最新到最旧排序。
///
/// # 参数
/// * `state` - 共享原生任务注册表。
///
/// # 返回值
/// 按最新到最旧排序的下载任务列表。
fn list_download_jobs_inner(state: &NativeJobState) -> Vec<NativeJob> {
    let mut jobs = state
        .jobs
        .lock()
        .map(|jobs| jobs.values().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    jobs.sort_by(|left, right| right.id.cmp(&left.id));
    jobs
}

/// 删除下载任务，并取消仍在下载的任务。
///
/// # 参数
/// * `app` - 用于访问原生任务状态的 Tauri 应用句柄。
/// * `job_id` - 任务标识符。
///
/// # 错误
/// 任务不存在或注册表访问失败时返回错误。
#[tauri::command]
pub(crate) fn delete_download_job(app: tauri::AppHandle, job_id: String) -> Result<(), String> {
    let state = app.state::<NativeJobState>();
    if let Some(cancellation) = state
        .cancellations
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .remove(&job_id)
    {
        cancellation.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    let deleted = state
        .jobs
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .remove(&job_id)
        .map(|_| ())
        .ok_or_else(|| "下载任务不存在".to_string());
    if deleted.is_ok() {
        let _ = state.specs.lock().map(|mut specs| specs.remove(&job_id));
        persist_native_jobs(&app, &state)?;
    }
    deleted
}
