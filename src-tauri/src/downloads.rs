/// Updates one native task while keeping all renderer-facing state in Rust memory.
///
/// # Arguments
/// * `state` - Shared native task registry.
/// * `job_id` - Identifier of the task to update.
/// * `update` - Mutation applied while the registry lock is held.
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

/// Emits one renderer-safe download task update to subscribed application windows.
///
/// # Arguments
/// * `app` - Application handle used to publish the internal event.
/// * `state` - Shared native task registry.
/// * `job_id` - Identifier of the task whose latest state should be emitted.
///
/// # Side Effects
/// Emits `download-progress` without exposing cookies, local paths, or task specs.
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

/// Downloads chapter images and rewrites SF image tags to local Markdown paths.
///
/// Only newly fetched chapters use this normalization. Existing chapter stores
/// are deliberately left untouched; re-downloading a chapter recreates its
///正文 and image files together.
async fn materialize_chapter_images(
    client: &SfacgHttpClient,
    directory: &std::path::Path,
    novel_id: i64,
    chapter_id: i64,
    content: &str,
    cookie: Option<&str>,
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
        let mut request = client
            .client
            .get(url)
            .header(reqwest::header::REFERER, &referer)
            .header(reqwest::header::ACCEPT, "image/avif,image/webp,image/*,*/*;q=0.8");
        if let Some(cookie) = cookie.filter(|value| !value.trim().is_empty()) {
            request = request.header(reqwest::header::COOKIE, cookie);
        }
        let response = request
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

/// Runs a text download task using the native SF web parser and atomic local stores.
///
/// # Arguments
/// * `app` - Tauri handle used for private storage and shared task state.
/// * `job_id` - Native task identifier.
/// * `novel_id` - SF novel identifier.
/// * `title` - User-visible work title.
/// * `chapter_ids` - Optional selected chapter IDs; `None` means all available chapters.
/// * `cancelled` - Cooperative cancellation flag controlled by pause/delete commands.
async fn run_text_download(
    app: tauri::AppHandle,
    job_id: String,
    novel_id: i64,
    title: String,
    chapter_ids: Option<Vec<i64>>,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
) {
    let state = app.state::<NativeJobState>();
    let result: Result<String, String> = async {
        let cookie = current_session_cookie(&app).ok();
        let client = SfacgHttpClient::new()?;
        let policy = get_request_policy(app.clone()).unwrap_or_default();
        let directory = library_directory(&app)?.join(safe_library_name(&title));
        fs::create_dir_all(&directory).map_err(|error| format!("无法创建本地书籍目录：{error}"))?;
        let mut metadata =
            read_json_or_default::<StoredBookMetadata>(&directory.join(".novel-flow.json"))?;
        metadata.novel_id = Some(novel_id);
        metadata.title = Some(title.clone());
        metadata
            .author
            .get_or_insert_with(|| "未知作者".to_string());
        metadata
            .description
            .get_or_insert_with(|| "暂无简介".to_string());
        let _ = enrich_local_book(
            &client,
            &directory,
            &mut metadata,
            novel_id,
            cookie.as_deref(),
        )
        .await;
        let mut store = read_json_or_default::<StoredChapterStore>(
            &directory.join(".novel-flow-chapters.json"),
        )?;
        store.novel_id = novel_id;
        let directory_data = client
            .get_data_with_cookie(&format!("/novels/{novel_id}/dirs"), &[], cookie.as_deref())
            .await?;
        let requested =
            chapter_ids.map(|ids| ids.into_iter().collect::<std::collections::HashSet<_>>());
        let mut chapters = Vec::new();
        for (volume_index, volume) in directory_data
            .get("volumeList")
            .and_then(Value::as_array)
            .ok_or_else(|| "SF 未返回章节目录".to_string())?
            .iter()
            .enumerate()
        {
            let volume_id = volume.get("volumeId").and_then(Value::as_i64).unwrap_or(0);
            let volume_title = volume
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("未命名分卷");
            let Some(volume_chapters) = volume.get("chapterList").and_then(Value::as_array) else {
                continue;
            };
            for (chapter_index, chapter) in volume_chapters.iter().enumerate() {
                let Some(chapter_id) = chapter.get("chapId").and_then(Value::as_i64) else {
                    continue;
                };
                if requested
                    .as_ref()
                    .is_some_and(|ids| !ids.contains(&chapter_id))
                {
                    continue;
                }
                if chapter
                    .get("isVip")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                    && !chapter.get("has").and_then(Value::as_bool).unwrap_or(false)
                    && cookie.is_none()
                {
                    continue;
                }
                chapters.push((
                    chapter_id,
                    volume_id,
                    volume_title.to_string(),
                    volume_index as i64,
                    chapter_index,
                    chapter,
                ));
            }
        }
        if chapters.is_empty() {
            return Err("没有可下载的章节".to_string());
        }
        let total = chapters.len();
        for (index, (chapter_id, volume_id, volume, volume_index, chapter_index, chapter)) in
            chapters.into_iter().enumerate()
        {
            if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                return Err("下载已暂停".to_string());
            }
            let chapter_title = chapter
                .get("ntitle")
                .and_then(Value::as_str)
                .unwrap_or("未命名章节")
                .to_string();
            update_native_job(&state, &job_id, |job| {
                job.status = "downloading".to_string();
                job.progress = ((index * 96) / total) as u8;
                job.message = format!("正在读取：{chapter_title}");
            });
            emit_native_job_update(&app, &state, &job_id);
            if !store.chapters.contains_key(&chapter_id.to_string()) {
                let raw_content = match client
                    .chapter_content_with_metadata_from_api(chapter_id, cookie.as_deref())
                    .await
                {
                    Ok(api) => decode_api_content(&app, &api.content)?,
                    Err(api_error) if policy.web_fallback_enabled => {
                        update_native_job(&state, &job_id, |job| {
                            job.message = format!("API 不可用，切换网页：{api_error}");
                        });
                        emit_native_job_update(&app, &state, &job_id);
                        client
                            .chapter_content_from_web(
                                novel_id,
                                volume_id,
                                chapter_id,
                                cookie.as_deref(),
                            )
                            .await?
                    }
                    Err(error) => return Err(format!("API 正文下载失败：{error}")),
                };
                let content = materialize_chapter_images(
                    &client,
                    &directory,
                    novel_id,
                    chapter_id,
                    &raw_content,
                    cookie.as_deref(),
                )
                .await?;
                store.chapters.insert(
                    chapter_id.to_string(),
                    StoredChapter {
                        id: chapter_id,
                        title: chapter_title,
                        volume,
                        content,
                        volume_index,
                        chapter_index: chapter_index as i64,
                    },
                );
                let payload = serde_json::to_vec_pretty(&store)
                    .map_err(|error| format!("无法序列化章节：{error}"))?;
                fs::write(directory.join(".novel-flow-chapters.json.tmp"), payload)
                    .map_err(|error| format!("无法写入章节：{error}"))?;
                if directory.join(".novel-flow-chapters.json").exists() {
                    fs::remove_file(directory.join(".novel-flow-chapters.json"))
                        .map_err(|error| format!("无法替换章节：{error}"))?;
                }
                fs::rename(
                    directory.join(".novel-flow-chapters.json.tmp"),
                    directory.join(".novel-flow-chapters.json"),
                )
                .map_err(|error| format!("无法完成章节写入：{error}"))?;
            }
            tokio::time::sleep(std::time::Duration::from_millis(policy.request_interval_ms)).await;
        }
        metadata.downloaded_text_chapter_ids = Some(
            store
                .chapters
                .keys()
                .filter_map(|id| id.parse().ok())
                .collect(),
        );
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
        Ok(directory.to_string_lossy().into_owned())
    }
    .await;
    match result {
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

/// Produces a filesystem-safe name for an audio file while preserving readable
/// Unicode chapter titles.
///
/// # Arguments
/// * `value` - Upstream chapter title.
///
/// # Returns
/// A bounded filename segment without Windows-reserved characters.
fn safe_audio_name(value: &str) -> String {
    safe_library_name(value).chars().take(100).collect()
}

/// Fetches public book metadata and persists its cover beside downloaded media.
/// Metadata or cover failures deliberately do not fail a chapter download: the
/// content remains usable and the next download can fill the missing artwork.
async fn enrich_local_book(
    client: &SfacgHttpClient,
    directory: &std::path::PathBuf,
    metadata: &mut StoredBookMetadata,
    novel_id: i64,
    cookie: Option<&str>,
) -> Result<(), String> {
    let detail = client
        .get_data_with_cookie(
            &format!("/novels/{novel_id}"),
            &[("expand", "intro".to_string())],
            cookie,
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
    let Some(cover_url) = detail
        .get("novelCover")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
    else {
        return Ok(());
    };
    let cover = directory.join("imgs").join("cover.jpeg");
    if cover.is_file() {
        return Ok(());
    }
    let source = if cover_url.starts_with("http://") || cover_url.starts_with("https://") {
        cover_url
    } else {
        format!("https://book.sfacg.com/{}", cover_url.trim_start_matches('/'))
    };
    let payload = client
        .client
        .get(source)
        .header(reqwest::header::REFERER, format!("https://book.sfacg.com/Novel/{novel_id}/"))
        .header(reqwest::header::ACCEPT, "image/avif,image/webp,image/*,*/*;q=0.8")
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

/// Runs one authenticated audio download task and rebuilds the local M3U8 list.
///
/// # Arguments
/// * `app` - Tauri handle used for native state and controlled local storage.
/// * `job_id` - Native task identifier.
/// * `novel_id` - Positive SF work identifier.
/// * `title` - Display title captured when the task was created.
/// * `chapter_ids` - Selected audio chapter identifiers.
/// * `cancelled` - Cooperative cancellation flag controlled by task commands.
async fn run_audio_download(
    app: tauri::AppHandle,
    job_id: String,
    novel_id: i64,
    title: String,
    chapter_ids: Vec<i64>,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
) {
    let state = app.state::<NativeJobState>();
    let result: Result<String, String> = async {
        #[cfg(target_os = "android")]
        sync_android_auth_session(&app).await?;
        let cookie = current_session_cookie(&app)?;
        let client = SfacgHttpClient::new()?;
        let (_catalog_title, catalog) = client.audio_catalog(novel_id, &cookie).await?;
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
            read_json_or_default::<StoredBookMetadata>(&directory.join(".novel-flow.json"))?;
        metadata.novel_id = Some(novel_id);
        metadata.title = Some(title.clone());
        metadata
            .author
            .get_or_insert_with(|| "未知作者".to_string());
        metadata
            .description
            .get_or_insert_with(|| "暂无简介".to_string());
        let _ = enrich_local_book(
            &client,
            &directory,
            &mut metadata,
            novel_id,
            Some(&cookie),
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
                let payload = client
                    .client
                    .get(&chapter.source)
                    .header(reqwest::header::COOKIE, &cookie)
                    .header(
                        reqwest::header::REFERER,
                        "https://i.sfacg.com/consume/book/",
                    )
                    .header(reqwest::header::ACCEPT, "audio/mpeg,*/*;q=0.8")
                    .send()
                    .await
                    .map_err(|error| format!("无法下载有声章节：{error}"))?
                    .error_for_status()
                    .map_err(|error| format!("有声章节下载被拒绝：{error}"))?
                    .bytes()
                    .await
                    .map_err(|error| format!("无法读取有声章节：{error}"))?;
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

/// Creates and schedules a native text download task.
///
/// # Arguments
/// * `app` - Tauri application handle used for private state and storage.
/// * `novel_id` - Positive SF novel identifier.
/// * `title` - User-visible title used for the local book directory.
/// * `chapter_ids` - Selected positive chapter identifiers, or `None` for all chapters.
///
/// # Errors
/// Returns an error for invalid input or an unavailable native task registry.
#[tauri::command]
async fn create_text_download(
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

/// Creates and schedules an authenticated native audio download task.
///
/// # Arguments
/// * `app` - Tauri application handle used for private state and storage.
/// * `novel_id` - Positive SF work identifier.
/// * `title` - User-visible title retained for the local book metadata.
/// * `chapter_ids` - Selected positive audio chapter identifiers.
///
/// # Errors
/// Returns an error for invalid input, missing login state, or unavailable task state.
#[tauri::command]
async fn create_audio_download(
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
    #[cfg(target_os = "android")]
    sync_android_auth_session(&app).await?;
    current_session_cookie(&app)?;
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
                title: title.clone(),
                chapter_ids: chapter_ids.clone(),
            },
        );
    persist_native_jobs(&app, &state)?;
    tauri::async_runtime::spawn(run_audio_download(
        app,
        id,
        novel_id,
        title,
        chapter_ids,
        cancelled,
    ));
    Ok(job)
}

/// Lists native download tasks in reverse creation order.
///
/// # Arguments
/// * `app` - Tauri application handle used to access the native task registry.
///
/// # Errors
/// Returns an error if the task registry lock is unavailable.
#[tauri::command]
fn list_download_jobs(app: tauri::AppHandle) -> Result<Vec<NativeJob>, String> {
    Ok(list_download_jobs_inner(&app.state::<NativeJobState>()))
}

/// Pauses a running native download task through cooperative cancellation.
///
/// # Arguments
/// * `app` - Tauri application handle used to access native task state.
/// * `job_id` - Existing task identifier.
///
/// # Errors
/// Returns an error if no matching task exists or task state is unavailable.
#[tauri::command]
fn pause_download_job(app: tauri::AppHandle, job_id: String) -> Result<NativeJob, String> {
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

/// Resumes a paused native text download task from its private task specification.
///
/// # Arguments
/// * `app` - Tauri application handle used to access native task state.
/// * `job_id` - Existing paused task identifier.
///
/// # Errors
/// Returns an error when the task is absent, not paused, or no longer resumable.
#[tauri::command]
fn resume_download_job(app: tauri::AppHandle, job_id: String) -> Result<NativeJob, String> {
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

/// Reads and sorts jobs from an already acquired native state handle.
///
/// # Arguments
/// * `state` - Shared native task registry.
///
/// # Returns
/// Jobs sorted from newest to oldest.
fn list_download_jobs_inner(state: &NativeJobState) -> Vec<NativeJob> {
    let mut jobs = state
        .jobs
        .lock()
        .map(|jobs| jobs.values().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    jobs.sort_by(|left, right| right.id.cmp(&left.id));
    jobs
}

/// Deletes a native download task and requests cooperative cancellation if needed.
///
/// # Arguments
/// * `app` - Tauri application handle used to access native task state.
/// * `job_id` - Existing task identifier.
///
/// # Errors
/// Returns an error if the task does not exist or registry access fails.
#[tauri::command]
fn delete_download_job(app: tauri::AppHandle, job_id: String) -> Result<(), String> {
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

