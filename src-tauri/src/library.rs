use std::path::Path;

/// A renderer-safe summary of one locally stored book.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LibraryBook {
    name: String,
    updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cover: Option<String>,
    formats: LibraryFormats,
}

/// Describes which local media formats were found for a book.
#[derive(Debug, Serialize)]
struct LibraryFormats {
    text: bool,
    audio: bool,
    comic: bool,
}

/// A renderer-safe local book detail record.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalBookDetail {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    novel: Option<LocalWorkMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    audio: Option<LocalWorkMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    comic: Option<LocalWorkMetadata>,
    image_directory: String,
    audio_tracks: Vec<LocalAudioTrack>,
    epub_href: Option<String>,
    chapter_volumes: Vec<LocalChapterVolume>,
    comic_chapters: Vec<LocalComicChapter>,
}

/// Metadata for exactly one locally saved media type. Each source endpoint owns
/// its own identity, cover, and descriptive fields.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalWorkMetadata {
    id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    catalog_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    online_path: Option<String>,
    title: String,
    author: String,
    description: String,
    type_name: Option<String>,
    tags: Vec<String>,
    is_finished: Option<bool>,
    score: Option<f64>,
    chapter_count: Option<i64>,
    character_count: Option<i64>,
    view_count: Option<i64>,
    mark_count: Option<i64>,
    point_count: Option<i64>,
    favorite_count: Option<i64>,
    ticket_count: Option<i64>,
    allow_download: Option<bool>,
    latest_chapter_title: Option<String>,
    latest_chapter_time: Option<String>,
    last_update_time: Option<String>,
    cover: Option<String>,
}

/// A locally playable audio track. The renderer converts the validated absolute
/// path with Tauri's asset protocol before assigning it to an audio element.
#[derive(Debug, Serialize)]
struct LocalAudioTrack {
    title: String,
    href: String,
}

/// A local text chapter volume grouped in source order.
#[derive(Debug, Serialize)]
struct LocalChapterVolume {
    volume: String,
    chapters: Vec<LocalChapterSummary>,
}

/// A renderer-safe local chapter summary.
#[derive(Debug, Serialize)]
struct LocalChapterSummary {
    id: i64,
    title: String,
}

/// The persisted metadata written by the native-compatible download format.
#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct StoredBookMetadata {
    #[serde(default)]
    novel: Option<StoredWorkMetadata>,
    #[serde(default)]
    audio: Option<StoredWorkMetadata>,
    #[serde(default)]
    comic: Option<StoredWorkMetadata>,
    #[serde(default)]
    downloaded_text_chapter_ids: Option<Vec<i64>>,
    #[serde(default)]
    downloaded_audio_chapter_ids: Option<Vec<i64>>,
    #[serde(default)]
    downloaded_comic_chapter_ids: Option<Vec<i64>>,
}

/// Persisted source metadata for one media type. It intentionally has no
/// cross-media fallback fields: a comic ID must never be used as a novel ID.
#[derive(Debug, Deserialize, Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
struct StoredWorkMetadata {
    id: Option<i64>,
    catalog_id: Option<i64>,
    online_path: Option<String>,
    title: Option<String>,
    author: Option<String>,
    description: Option<String>,
    type_name: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    is_finished: Option<bool>,
    score: Option<f64>,
    chapter_count: Option<i64>,
    character_count: Option<i64>,
    view_count: Option<i64>,
    mark_count: Option<i64>,
    point_count: Option<i64>,
    favorite_count: Option<i64>,
    ticket_count: Option<i64>,
    allow_download: Option<bool>,
    latest_chapter_title: Option<String>,
    latest_chapter_time: Option<String>,
    last_update_time: Option<String>,
}

/// One persisted text chapter from `.novel-flow-chapters.json`.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredChapter {
    id: i64,
    title: String,
    volume: String,
    content: String,
    volume_index: i64,
    chapter_index: i64,
}

/// Persisted chapter store shape written by the existing downloader.
#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct StoredChapterStore {
    novel_id: i64,
    chapters: std::collections::HashMap<String, StoredChapter>,
}

/// The persisted request limits applied by native network download workers.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RequestPolicy {
    request_interval_ms: u64,
    max_concurrent_downloads: u8,
    #[serde(default = "default_app_fallback_enabled")]
    app_fallback_enabled: bool,
    #[serde(default = "default_android_device_report_enabled")]
    android_device_report_enabled: bool,
}

/// A locally stored comic chapter and its ordered page files.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalComicChapter {
    id: i64,
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cover: Option<String>,
    pages: Vec<String>,
}

/// 返回请求策略中“允许 App API 回退”的 serde 默认值。
///
/// 旧版持久化策略缺少该字段时也会保留启用回退的默认行为。
fn default_app_fallback_enabled() -> bool {
    true
}

/// 返回请求策略中“允许 App 设备信息上报”的默认值。
///
/// 该接口属于实验项，默认开启以保持当前行为；用户可在设置中关闭。
fn default_android_device_report_enabled() -> bool {
    true
}

impl Default for RequestPolicy {
    /// Creates conservative defaults that reduce upstream request pressure.
    ///
    /// # Returns
    /// A one-request worker with a 500 ms interval between requests.
    fn default() -> Self {
        Self {
            request_interval_ms: 500,
            max_concurrent_downloads: 1,
            app_fallback_enabled: true,
            android_device_report_enabled: true,
        }
    }
}

/// Renderer-safe result returned after reading or updating the正文恢复字典.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ContentDictionaryResult {
    size: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    added: Option<usize>,
}

/// Returns true for the CJK Unified Ideographs ranges used by SF正文内容.
///
/// # Arguments
/// * `character` - Unicode scalar value to classify.
///
/// # Returns
/// `true` when the scalar is a Han character that can participate in a mapping.
fn is_han_character(character: char) -> bool {
    matches!(
        character,
        '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{F900}'..='\u{FAFF}'
    )
}

/// Resolves the user-editable正文恢复字典 below the private app data folder.
///
/// # Arguments
/// * `app` - Application handle used to resolve the platform data path.
///
/// # Errors
/// Returns an error when the app data directory cannot be resolved.
fn content_dictionary_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|directory| directory.join("sfacg-content-dictionary.json"))
        .map_err(|error| format!("无法解析正文恢复字典路径：{error}"))
}

/// Validates a JSON dictionary and drops entries that are not one Han scalar
/// mapped to another Han scalar.
///
/// # Arguments
/// * `value` - Parsed JSON value from the embedded or persisted dictionary.
///
/// # Returns
/// A conflict-safe map containing only valid one-character mappings.
fn valid_content_dictionary(value: Value) -> std::collections::HashMap<String, String> {
    let Some(entries) = value.as_object() else {
        return std::collections::HashMap::new();
    };
    entries
        .iter()
        .filter_map(|(key, value)| {
            let replacement = value.as_str()?;
            if key.chars().count() != 1
                || replacement.chars().count() != 1
                || !key.chars().all(is_han_character)
                || !replacement.chars().all(is_han_character)
            {
                return None;
            }
            Some((key.clone(), replacement.to_string()))
        })
        .collect()
}

/// Loads the persisted正文恢复字典 and seeds it from the bundled defaults once.
///
/// # Arguments
/// * `app` - Application handle used to resolve private app storage.
///
/// # Errors
/// Returns an error when an existing dictionary is malformed or cannot be saved.
fn load_content_dictionary(
    app: &tauri::AppHandle,
) -> Result<std::collections::HashMap<String, String>, String> {
    let path = content_dictionary_path(app)?;
    if path.exists() {
        let contents =
            fs::read_to_string(&path).map_err(|error| format!("无法读取正文恢复字典：{error}"))?;
        let value = serde_json::from_str::<Value>(&contents)
            .map_err(|error| format!("正文恢复字典格式无效：{error}"))?;
        return Ok(valid_content_dictionary(value));
    }
    let defaults = valid_content_dictionary(
        serde_json::from_str(DEFAULT_CONTENT_DICTIONARY)
            .map_err(|error| format!("内置正文恢复字典格式无效：{error}"))?,
    );
    write_content_dictionary(app, &defaults)?;
    Ok(defaults)
}

/// Restores API-confused Han characters using the persisted web alignment map.
fn decode_api_content(app: &tauri::AppHandle, content: &str) -> Result<String, String> {
    let dictionary = load_content_dictionary(app)?;
    Ok(content
        .chars()
        .map(|character| {
            dictionary
                .get(&character.to_string())
                .and_then(|value| value.chars().next())
                .unwrap_or(character)
        })
        .collect())
}

/// Persists a validated dictionary through a sibling temporary file.
///
/// # Arguments
/// * `app` - Application handle used to resolve private app storage.
/// * `dictionary` - Validated character mapping to store.
///
/// # Errors
/// Returns an error when serialization or filesystem replacement fails.
fn write_content_dictionary(
    app: &tauri::AppHandle,
    dictionary: &std::collections::HashMap<String, String>,
) -> Result<(), String> {
    let path = content_dictionary_path(app)?;
    let parent = path
        .parent()
        .ok_or_else(|| "无法解析正文恢复字典目录".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("无法创建正文恢复字典目录：{error}"))?;
    let temporary = path.with_extension("json.tmp");
    let payload = serde_json::to_vec_pretty(dictionary)
        .map_err(|error| format!("无法序列化正文恢复字典：{error}"))?;
    fs::write(&temporary, payload).map_err(|error| format!("无法写入正文恢复字典：{error}"))?;
    if path.exists() {
        fs::remove_file(&path).map_err(|error| format!("无法替换正文恢复字典：{error}"))?;
    }
    fs::rename(&temporary, &path).map_err(|error| format!("无法完成正文恢复字典写入：{error}"))
}

/// Returns the number of valid mappings in the private正文恢复字典.
///
/// # Arguments
/// * `app` - Application handle used to resolve private app storage.
///
/// # Errors
/// Returns an error when the dictionary cannot be read or initialized.
#[tauri::command]
fn get_content_dictionary(app: tauri::AppHandle) -> Result<ContentDictionaryResult, String> {
    let dictionary = load_content_dictionary(&app)?;
    Ok(ContentDictionaryResult {
        size: dictionary.len(),
        added: None,
    })
}

/// Aligns API-confused Han characters with the same chapter's web正文 and saves
/// only non-conflicting mappings in the private dictionary.
///
/// # Arguments
/// * `app` - Application handle used for native session state and storage.
/// * `chapter_id` - Positive public chapter identifier used for alignment.
///
/// # Errors
/// Returns an error when either source is unavailable, the character counts do
/// not match, or an existing mapping conflicts with the observed web character.
#[tauri::command]
async fn update_content_dictionary(
    app: tauri::AppHandle,
    chapter_id: i64,
) -> Result<ContentDictionaryResult, String> {
    let app_client = AppClient::new(&app)?;
    let api = app_client
        .chapter_content_with_metadata_from_api(chapter_id)
        .await?;
    let web_client = WebClient::new(&app)?;
    let web = web_client
        .chapter_content(api.novel_id, api.volume_id, chapter_id)
        .await?;
    let source: Vec<char> = api
        .content
        .chars()
        .filter(|character| is_han_character(*character))
        .collect();
    let target: Vec<char> = web
        .chars()
        .filter(|character| is_han_character(*character))
        .collect();
    if source.is_empty() || source.len() != target.len() {
        return Err(format!(
            "API 与网页正文无法对齐（API {} 字，网页 {} 字）",
            source.len(),
            target.len()
        ));
    }
    let mut dictionary = load_content_dictionary(&app)?;
    let previous_size = dictionary.len();
    for (from, to) in source.into_iter().zip(target) {
        let key = from.to_string();
        let replacement = to.to_string();
        if let Some(existing) = dictionary.get(&key) {
            if existing != &replacement {
                return Err(format!(
                    "映射冲突：{from} 已映射为 {existing}，网页对应 {to}"
                ));
            }
        } else {
            dictionary.insert(key, replacement);
        }
    }
    let added = dictionary.len().saturating_sub(previous_size);
    write_content_dictionary(&app, &dictionary)?;
    Ok(ContentDictionaryResult {
        size: dictionary.len(),
        added: Some(added),
    })
}

/// Resolves the application-owned library directory without accepting a path
/// from the renderer.
///
/// The directory is created below the platform application-data directory and
/// is therefore writable on both Windows and Android without broad filesystem
/// permissions.
///
/// # Arguments
/// * `app` - Tauri application handle used to resolve the platform data path.
///
/// # Errors
/// Returns the application library directory, using the public Android
/// Downloads folder and the private application-data folder on desktop.
///
/// Returns an error when the directory cannot be resolved or created.
fn library_directory(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    #[cfg(target_os = "android")]
    let directory = PathBuf::from("/storage/emulated/0/Download/SF Novel Flow");
    #[cfg(not(target_os = "android"))]
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("无法解析应用数据目录：{error}"))?
        .join("library");

    #[cfg(target_os = "android")]
    migrate_legacy_android_library(app, &directory)?;
    fs::create_dir_all(&directory).map_err(|error| format!("无法创建本地书库目录：{error}"))?;
    Ok(directory)
}

/// Moves an existing private Android library into the public download folder once.
#[cfg(target_os = "android")]
fn migrate_legacy_android_library(app: &tauri::AppHandle, target: &PathBuf) -> Result<(), String> {
    if target.exists() {
        return Ok(());
    }
    let legacy = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("无法解析旧书库目录：{error}"))?
        .join("library");
    if !legacy.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(target).map_err(|error| format!("无法创建外部书库目录：{error}"))?;
    copy_directory_contents(&legacy, target)
        .map_err(|error| format!("无法迁移旧书库到外部目录：{error}"))
}

/// 递归复制旧 Android 私有书库的所有内容。
///
/// # 参数
/// * `source` - 已确认存在的旧书库目录。
/// * `target` - 已创建的公开下载目录。
///
/// # 错误
/// 枚举源目录、创建子目录或复制任一文件失败时返回 I/O 错误。
#[cfg(target_os = "android")]
fn copy_directory_contents(source: &PathBuf, target: &PathBuf) -> std::io::Result<()> {
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            fs::create_dir_all(&target_path)?;
            copy_directory_contents(&source_path, &target_path)?;
        } else {
            fs::copy(source_path, target_path)?;
        }
    }
    Ok(())
}

/// Resolves the native settings file without accepting a renderer-controlled
/// location.
///
/// # Arguments
/// * `app` - Tauri application handle used to resolve the application-data path.
///
/// # Errors
/// Returns an error when the application-data directory cannot be resolved.
fn request_policy_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|directory| directory.join("request-policy.json"))
        .map_err(|error| format!("无法解析请求策略文件：{error}"))
}

/// Resolves the private task-state snapshot used to recover paused downloads.
///
/// # Arguments
/// * `app` - Application handle used to resolve the app data path.
///
/// # Errors
/// Returns an error when the platform app data path cannot be resolved.
fn native_jobs_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|directory| directory.join("download-jobs.json"))
        .map_err(|error| format!("无法解析下载任务文件：{error}"))
}

/// Persists renderer-safe jobs and private resume specifications atomically.
///
/// Cookies, passwords, output paths, and cancellation handles are intentionally
/// excluded from the snapshot.
///
/// # Arguments
/// * `app` - Application handle used to resolve native storage.
/// * `state` - Shared in-memory download task state.
///
/// # Errors
/// Returns an error when the task snapshot cannot be serialized or replaced.
fn persist_native_jobs(app: &tauri::AppHandle, state: &NativeJobState) -> Result<(), String> {
    let jobs = state
        .jobs
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .clone();
    let specs = state
        .specs
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .clone();
    let path = native_jobs_path(app)?;
    let parent = path
        .parent()
        .ok_or_else(|| "无法解析下载任务目录".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("无法创建下载任务目录：{error}"))?;
    let payload = serde_json::to_vec_pretty(&PersistedNativeJobs { jobs, specs })
        .map_err(|error| format!("无法序列化下载任务：{error}"))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, payload).map_err(|error| format!("无法写入下载任务：{error}"))?;
    if path.exists() {
        fs::remove_file(&path).map_err(|error| format!("无法替换下载任务：{error}"))?;
    }
    fs::rename(&temporary, &path).map_err(|error| format!("无法完成下载任务写入：{error}"))
}

/// Loads task state after startup and makes unfinished jobs explicitly paused.
///
/// The app never resumes an upstream request automatically after a process
/// restart; the user must select continue from the download queue.
///
/// # Arguments
/// * `app` - Application handle used to access native state and app data.
///
/// # Errors
/// Returns an error when an existing snapshot is malformed or unreadable.
fn restore_native_jobs(app: &tauri::AppHandle) -> Result<(), String> {
    let path = native_jobs_path(app)?;
    if !path.exists() {
        return Ok(());
    }
    let contents =
        fs::read_to_string(&path).map_err(|error| format!("无法读取下载任务：{error}"))?;
    let mut persisted = serde_json::from_str::<PersistedNativeJobs>(&contents)
        .map_err(|error| format!("下载任务格式无效：{error}"))?;
    persisted
        .jobs
        .retain(|id, _| persisted.specs.contains_key(id));
    for job in persisted.jobs.values_mut() {
        if matches!(job.status.as_str(), "queued" | "downloading") {
            job.status = "paused".to_string();
            job.message = "应用重启后已暂停，可继续下载".to_string();
        }
    }
    let state = app.state::<NativeJobState>();
    *state
        .jobs
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())? = persisted.jobs;
    *state
        .specs
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())? = persisted.specs;
    persist_native_jobs(app, &state)
}

/// Reads the persisted request policy, returning conservative defaults when no
/// settings file has been created yet.
///
/// # Arguments
/// * `app` - Tauri application handle used to resolve private application data.
///
/// # Errors
/// Returns an error when an existing policy file is unreadable or malformed.
#[tauri::command]
fn get_request_policy(app: tauri::AppHandle) -> Result<RequestPolicy, String> {
    let path = request_policy_path(&app)?;
    if !path.exists() {
        return Ok(RequestPolicy::default());
    }
    let contents =
        fs::read_to_string(&path).map_err(|error| format!("无法读取请求策略：{error}"))?;
    serde_json::from_str(&contents).map_err(|error| format!("请求策略格式无效：{error}"))
}

/// Validates and persists request limits for future native download tasks.
///
/// The renderer cannot select the output path. The file is written through a
/// sibling temporary file and then replaced to avoid leaving partial JSON after
/// an interrupted write.
///
/// # Arguments
/// * `app` - Tauri application handle used to resolve private application data.
/// * `policy` - Candidate interval and concurrency values from the renderer.
///
/// # Errors
/// Returns an error when values are outside their safe range or the policy
/// cannot be serialized or stored.
#[tauri::command]
fn save_request_policy(
    app: tauri::AppHandle,
    policy: RequestPolicy,
) -> Result<RequestPolicy, String> {
    if !(100..=60_000).contains(&policy.request_interval_ms) {
        return Err("请求间隔必须在 100 到 60000 毫秒之间".to_string());
    }
    if !(1..=4).contains(&policy.max_concurrent_downloads) {
        return Err("最大并发下载数必须在 1 到 4 之间".to_string());
    }

    let path = request_policy_path(&app)?;
    let parent = path
        .parent()
        .ok_or_else(|| "无法解析请求策略目录".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("无法创建请求策略目录：{error}"))?;
    let temporary = path.with_extension("json.tmp");
    let payload = serde_json::to_vec_pretty(&policy)
        .map_err(|error| format!("无法序列化请求策略：{error}"))?;
    fs::write(&temporary, payload).map_err(|error| format!("无法写入请求策略：{error}"))?;
    if path.exists() {
        fs::remove_file(&path).map_err(|error| format!("无法替换请求策略：{error}"))?;
    }
    fs::rename(&temporary, &path).map_err(|error| format!("无法完成请求策略写入：{error}"))?;
    Ok(policy)
}

/// Lists books stored in the Tauri-owned local library.
///
/// The command only returns directory names and format flags. It never returns
/// absolute paths, file contents, or credentials; media access will use a
/// separate validated asset command during the library migration.
///
/// # Arguments
/// * `app` - Tauri application handle used to locate the library.
///
/// # Errors
/// Returns an error if the library cannot be scanned.
#[tauri::command]
fn list_local_library(app: tauri::AppHandle) -> Result<Vec<LibraryBook>, String> {
    let directory = library_directory(&app)?;
    let entries = fs::read_dir(&directory).map_err(|error| format!("无法读取本地书库：{error}"))?;
    let mut books = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("读取书库条目失败：{error}"))?;
        let metadata = entry
            .metadata()
            .map_err(|error| format!("读取书库条目元数据失败：{error}"))?;
        if !metadata.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let text = entry.path().join(".novel-flow-chapters.json").exists()
            || entry.path().join("book.md").exists();
        let audio = entry.path().join("audio").is_dir();
        let comic = entry.path().join("comic").is_dir();
        books.push(LibraryBook {
            name,
            updated_at: metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs().to_string())
                .unwrap_or_else(|| "0".to_string()),
            cover: ["novel-cover.jpeg", "audio-cover.jpeg", "comic-cover.jpeg"]
                .into_iter()
                .map(|file_name| entry.path().join("imgs").join(file_name))
                .find(|cover| cover.is_file())
                .map(|cover| cover.to_string_lossy().into_owned()),
            formats: LibraryFormats {
                text,
                audio,
                comic,
            },
        });
    }
    books.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(books)
}

/// Resolves a single child directory in the application-owned library.
///
/// # Arguments
/// * `app` - Tauri application handle used to locate the private library root.
/// * `name` - Exact directory name previously returned by `list_local_library`.
///
/// # Errors
/// Returns an error for path separators, traversal segments, missing entries, or
/// a directory that escapes the library root after canonicalization.
fn local_book_directory(app: &tauri::AppHandle, name: &str) -> Result<PathBuf, String> {
    let trimmed = name.trim();
    if trimmed.is_empty()
        || trimmed == "."
        || trimmed == ".."
        || trimmed.contains(['/', '\\', ':'])
        || trimmed.contains("..")
    {
        return Err("本地书籍目录无效".to_string());
    }
    let root = library_directory(app)?
        .canonicalize()
        .map_err(|error| format!("无法解析书库目录：{error}"))?;
    let directory = root.join(trimmed);
    let canonical = directory
        .canonicalize()
        .map_err(|_| "本地书籍不存在".to_string())?;
    if !canonical.starts_with(&root) || !canonical.is_dir() {
        return Err("本地书籍目录无效".to_string());
    }
    Ok(canonical)
}

/// Reads a local M3U8 playlist and exposes only existing first-level MP3 files.
///
/// # Arguments
/// * `name` - Validated local book directory name.
/// * `directory` - Canonical local book directory.
///
/// # Errors
/// Returns an error when the playlist exists but cannot be read as UTF-8.
fn read_local_audio_tracks(directory: &PathBuf) -> Result<Vec<LocalAudioTrack>, String> {
    let audio_directory = directory.join("audio");
    let playlist = audio_directory.join("有声目录.m3u8");
    if !playlist.is_file() {
        return Ok(Vec::new());
    }
    let contents =
        fs::read_to_string(&playlist).map_err(|error| format!("无法读取有声播放列表：{error}"))?;
    let mut tracks = Vec::new();
    let mut title: Option<String> = None;
    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(value) = line.strip_prefix("#EXTINF:") {
            title = value
                .split_once(',')
                .map(|(_, track_title)| track_title.trim().to_string())
                .filter(|value| !value.is_empty());
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        let file_name = PathBuf::from(line);
        if file_name.components().count() != 1
            || file_name
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.eq_ignore_ascii_case("mp3"))
                != Some(true)
        {
            title = None;
            continue;
        }
        let file_name = file_name.file_name().and_then(|value| value.to_str());
        let Some(file_name) = file_name else {
            title = None;
            continue;
        };
        let file = audio_directory.join(file_name);
        if file.is_file() {
            let fallback = file
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("未命名音频");
            tracks.push(LocalAudioTrack {
                title: title.take().unwrap_or_else(|| fallback.to_string()),
                href: file.to_string_lossy().into_owned(),
            });
        }
        title = None;
    }
    Ok(tracks)
}

/// Reads a local book's metadata and downloaded text chapter index.
///
/// # Arguments
/// * `app` - Tauri application handle used to locate the private library.
/// * `name` - Exact local directory name.
///
/// # Errors
/// Returns an error when the book directory or a malformed chapter store cannot be read.
#[tauri::command]
fn get_local_book(app: tauri::AppHandle, name: String) -> Result<LocalBookDetail, String> {
    let directory = local_book_directory(&app, &name)?;
    let metadata = read_json_or_default::<StoredBookMetadata>(&directory.join(".novel-flow.json"))?;
    let store =
        read_json_or_default::<StoredChapterStore>(&directory.join(".novel-flow-chapters.json"))?;
    let _ = (
        &metadata.downloaded_text_chapter_ids,
        &metadata.downloaded_audio_chapter_ids,
    );
    let mut chapters: Vec<_> = store.chapters.into_values().collect();
    chapters.sort_by_key(|chapter| (chapter.volume_index, chapter.chapter_index, chapter.id));
    let mut chapter_volumes: Vec<LocalChapterVolume> = Vec::new();
    for chapter in chapters {
        if chapter_volumes.last().map(|volume| volume.volume.as_str())
            != Some(chapter.volume.as_str())
        {
            chapter_volumes.push(LocalChapterVolume {
                volume: chapter.volume.clone(),
                chapters: Vec::new(),
            });
        }
        chapter_volumes
            .last_mut()
            .expect("chapter volume was just inserted")
            .chapters
            .push(LocalChapterSummary {
                id: chapter.id,
                title: chapter.title,
            });
    }
    let epub_name = format!("{}.epub", name);
    let epub_href = directory.join(&epub_name).is_file().then(|| {
        directory.join(&epub_name).to_string_lossy().into_owned()
    });
    let audio_tracks = read_local_audio_tracks(&directory)?;
    let comic_chapters = read_local_comic_chapters(&directory)?;
    Ok(LocalBookDetail {
        name,
        novel: local_work_metadata(metadata.novel, &directory, "novel-cover.jpeg"),
        audio: local_work_metadata(metadata.audio, &directory, "audio-cover.jpeg"),
        comic: local_work_metadata(metadata.comic, &directory, "comic-cover.jpeg"),
        image_directory: directory.join("imgs").to_string_lossy().into_owned(),
        audio_tracks,
        epub_href,
        chapter_volumes,
        comic_chapters,
    })
}

/// 根据本地资源类型读取已持久化的作品元数据。
///
/// # 参数
/// * `metadata` - 本地书籍的完整元数据容器。
/// * `kind` - 资源类型，支持 `novel`、`audio` 和 `comic`。
///
/// # 返回值
/// 对应资源类型的元数据；未知类型返回 `None`。
fn local_work_metadata(
    metadata: Option<StoredWorkMetadata>,
    directory: &PathBuf,
    cover_file: &str,
) -> Option<LocalWorkMetadata> {
    let metadata = metadata?;
    let id = metadata.id?;
    let cover = directory
        .join("imgs")
        .join(cover_file)
        .is_file()
        .then(|| directory.join("imgs").join(cover_file).to_string_lossy().into_owned());
    Some(LocalWorkMetadata {
        id,
        catalog_id: metadata.catalog_id,
        online_path: metadata.online_path,
        title: metadata.title.unwrap_or_else(|| "未命名作品".to_string()),
        author: metadata.author.unwrap_or_else(|| "未知作者".to_string()),
        description: metadata.description.unwrap_or_default(),
        type_name: metadata.type_name,
        tags: metadata.tags,
        is_finished: metadata.is_finished,
        score: metadata.score,
        chapter_count: metadata.chapter_count,
        character_count: metadata.character_count,
        view_count: metadata.view_count,
        mark_count: metadata.mark_count,
        point_count: metadata.point_count,
        favorite_count: metadata.favorite_count,
        ticket_count: metadata.ticket_count,
        allow_download: metadata.allow_download,
        latest_chapter_title: metadata.latest_chapter_title,
        latest_chapter_time: metadata.latest_chapter_time,
        last_update_time: metadata.last_update_time,
        cover,
    })
}

/// 从已校验的本地书籍目录读取已下载漫画章节及其有序页面。
///
/// # 错误
/// 章节目录无法枚举、编号无效或页面文件无法读取时返回错误。
fn read_local_comic_chapters(directory: &PathBuf) -> Result<Vec<LocalComicChapter>, String> {
    let root = directory.join("comic");
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let mut chapters = Vec::new();
    for entry in fs::read_dir(root).map_err(|error| format!("无法读取漫画目录：{error}"))? {
        let entry = entry.map_err(|error| format!("读取漫画章节失败：{error}"))?;
        let chapter_dir = entry.path();
        if !chapter_dir.is_dir() || !chapter_dir.join(".complete").is_file() {
            continue;
        }
        let Some(id) = entry.file_name().to_str().and_then(|value| value.parse::<i64>().ok()) else {
            continue;
        };
        let title = fs::read_to_string(chapter_dir.join(".complete"))
            .unwrap_or_else(|_| format!("第 {id} 话"))
            .trim()
            .to_string();
        let mut pages = fs::read_dir(&chapter_dir)
            .map_err(|error| format!("无法读取漫画页：{error}"))?
            .filter_map(Result::ok)
            .filter(|item| item.path().is_file() && item.file_name() != ".complete")
            .map(|item| item.path())
            .filter(|path| matches!(path.extension().and_then(|value| value.to_str()).map(str::to_ascii_lowercase).as_deref(), Some("jpg" | "jpeg" | "png" | "webp" | "avif")))
            .collect::<Vec<_>>();
        pages.sort();
        if !pages.is_empty() {
            let pages = pages
                .into_iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect::<Vec<_>>();
            chapters.push(LocalComicChapter {
                id,
                title,
                cover: pages.first().cloned(),
                pages,
            });
        }
    }
    chapters.sort_by_key(|chapter| chapter.id);
    Ok(chapters)
}

/// 返回一章已下载漫画的标题、封面和页面本地路径。
///
/// # 参数
/// * `app` - 用于定位应用自有书库的 Tauri 应用句柄。
/// * `name` - 先前由书库命令返回的精确书籍目录名。
/// * `chapter_id` - 要读取的正整数漫画章节标识。
///
/// # 错误
/// 书籍或章节不存在、目录未通过校验或页面信息无法读取时返回错误。
#[tauri::command]
fn get_local_comic_chapter(app: tauri::AppHandle, name: String, chapter_id: i64) -> Result<LocalComicChapter, String> {
    if chapter_id <= 0 { return Err("漫画章节参数无效".to_string()); }
    let directory = local_book_directory(&app, &name)?;
    read_local_comic_chapters(&directory)?.into_iter().find(|chapter| chapter.id == chapter_id).ok_or_else(|| "本地漫画章节不存在".to_string())
}

/// Reads one downloaded text chapter without exposing its filesystem path.
///
/// # Arguments
/// * `app` - Tauri application handle used to locate the private library.
/// * `name` - Exact local book directory name.
/// * `chapter_id` - Positive persisted chapter identifier.
///
/// # Errors
/// Returns an error for invalid IDs, missing books, malformed stores, or absent chapters.
#[tauri::command]
fn get_local_chapter(
    app: tauri::AppHandle,
    name: String,
    chapter_id: i64,
) -> Result<StoredChapter, String> {
    if chapter_id <= 0 {
        return Err("章节参数无效".to_string());
    }
    let directory = local_book_directory(&app, &name)?;
    let store =
        read_json_or_default::<StoredChapterStore>(&directory.join(".novel-flow-chapters.json"))?;
    if store.novel_id != 0 && metadata_novel_id(&directory)? != Some(store.novel_id) {
        return Err("本地章节与书籍元数据不匹配".to_string());
    }
    store
        .chapters
        .into_values()
        .find(|chapter| chapter.id == chapter_id)
        .ok_or_else(|| "本地未找到该章节正文".to_string())
}

/// A renderer-safe description of one generated local export file.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalExport {
    href: String,
    file_name: String,
}

/// Ensures Android can write the public download directory used by the library.
#[tauri::command]
async fn ensure_external_storage_access(app: tauri::AppHandle) -> Result<bool, String> {
    #[cfg(target_os = "android")]
    {
        let result = app
            .state::<AndroidSfacgAuth<tauri::Wry>>()
            .mobile_plugin_handle
            .run_mobile_plugin_async::<Value>("ensureExternalStorageAccess", ())
            .await
            .map_err(|error| format!("无法检查 Android 外部存储权限：{error}"))?;
        return Ok(result
            .get("granted")
            .and_then(Value::as_bool)
            .unwrap_or(false));
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Ok(true)
    }
}

/// Returns plain text without local Markdown formatting for TXT export.
///
/// # Arguments
/// * `value` - Stored chapter Markdown.
///
/// # Returns
/// Readable text with basic heading and image syntax removed.
fn text_without_markdown(value: &str) -> String {
    let mut output = String::new();
    for line in value.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("## ") {
            continue;
        }
        if trimmed.starts_with("![") {
            if let Some((alt, _)) = trimmed[2..].split_once("](") {
                output.push_str(alt);
                output.push('\n');
            }
            continue;
        }
        output.push_str(line);
        output.push('\n');
    }
    output.trim().to_string()
}

/// Groups stored chapters into a complete Markdown book document.
///
/// # Arguments
/// * `title` - User-visible book title.
/// * `author` - Book author.
/// * `description` - Book description.
/// * `chapters` - Already sorted downloaded chapters.
///
/// # Returns
/// UTF-8 Markdown including YAML metadata and volume headings.
fn local_markdown(
    title: &str,
    author: &str,
    description: &str,
    chapters: &[StoredChapter],
) -> String {
    let description = description
        .lines()
        .map(|line| format!("  {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut output = format!(
        "---\ntitle: '{}'\nauthor: '{}'\nlang: 'zh-Hans'\ndescription: |-\n{}\n...\n",
        title.replace('\'', "\\'"),
        author.replace('\'', "\\'"),
        description,
    );
    let mut current_volume = None;
    for chapter in chapters {
        if current_volume != Some(chapter.volume.as_str()) {
            output.push_str(&format!("\n# {}\n", chapter.volume));
            current_volume = Some(&chapter.volume);
        }
        output.push_str("\n");
        output.push_str(&chapter.content);
        output.push('\n');
    }
    output
}

/// Writes a text-only EPUB 3 archive for downloaded chapters.
///
/// The archive deliberately contains a simple fixed layout so both Windows and
/// Android readers can consume it without access to the app's local paths.
///
/// # Arguments
/// * `target` - Native-generated export file path.
/// * `title` - Book title.
/// * `author` - Book author.
/// * `description` - Book description.
/// * `chapters` - Already sorted downloaded chapters.
///
/// # Errors
/// Returns an error when the archive cannot be created or written.
fn write_epub_export(
    target: &PathBuf,
    title: &str,
    author: &str,
    description: &str,
    chapters: &[StoredChapter],
) -> Result<(), String> {
    /// Escapes XML-reserved characters before inserting user or chapter text.
    ///
    /// # Arguments
    /// * `value` - Untrusted text that will be embedded in an EPUB XML document.
    ///
    /// # Returns
    /// The escaped text, safe for the XML text and attribute contexts used here.
    fn xml(value: &str) -> String {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }
    let file = File::create(target).map_err(|error| format!("无法创建 EPUB：{error}"))?;
    let mut archive = ZipWriter::new(file);
    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let compressed =
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    archive
        .start_file("mimetype", stored)
        .map_err(|error| format!("无法创建 EPUB 条目：{error}"))?;
    archive
        .write_all(b"application/epub+zip")
        .map_err(|error| format!("无法写入 EPUB：{error}"))?;
    let container = "<?xml version=\"1.0\"?><container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\"><rootfiles><rootfile full-path=\"OEBPS/content.opf\" media-type=\"application/oebps-package+xml\"/></rootfiles></container>";
    archive
        .start_file("META-INF/container.xml", compressed)
        .map_err(|error| format!("无法创建 EPUB 条目：{error}"))?;
    archive
        .write_all(container.as_bytes())
        .map_err(|error| format!("无法写入 EPUB：{error}"))?;
    let mut manifest = String::from("<item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>");
    let mut spine = String::new();
    let mut nav = String::new();
    for (index, chapter) in chapters.iter().enumerate() {
        let id = format!("chapter-{}", index + 1);
        let file_name = format!("chapter-{:04}.xhtml", index + 1);
        manifest.push_str(&format!(
            "<item id=\"{id}\" href=\"{file_name}\" media-type=\"application/xhtml+xml\"/>"
        ));
        spine.push_str(&format!("<itemref idref=\"{id}\"/>"));
        nav.push_str(&format!(
            "<li><a href=\"{file_name}\">{}</a></li>",
            xml(&chapter.title)
        ));
        let body = chapter
            .content
            .lines()
            .filter(|line| !line.trim_start().starts_with("## "))
            .map(xml)
            .collect::<Vec<_>>()
            .join("<br/>");
        let page = format!("<?xml version=\"1.0\" encoding=\"utf-8\"?><!DOCTYPE html><html xmlns=\"http://www.w3.org/1999/xhtml\" xml:lang=\"zh-Hans\"><head><meta charset=\"utf-8\"/><title>{}</title></head><body><h1>{}</h1><p>{}</p></body></html>", xml(&chapter.title), xml(&chapter.title), body);
        archive
            .start_file(format!("OEBPS/{file_name}"), compressed)
            .map_err(|error| format!("无法创建 EPUB 条目：{error}"))?;
        archive
            .write_all(page.as_bytes())
            .map_err(|error| format!("无法写入 EPUB：{error}"))?;
    }
    let nav_document = format!("<?xml version=\"1.0\" encoding=\"utf-8\"?><!DOCTYPE html><html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\"><head><meta charset=\"utf-8\"/><title>目录</title></head><body><nav epub:type=\"toc\"><h1>目录</h1><ol>{nav}</ol></nav></body></html>");
    archive
        .start_file("OEBPS/nav.xhtml", compressed)
        .map_err(|error| format!("无法创建 EPUB 条目：{error}"))?;
    archive
        .write_all(nav_document.as_bytes())
        .map_err(|error| format!("无法写入 EPUB：{error}"))?;
    let package = format!("<?xml version=\"1.0\" encoding=\"utf-8\"?><package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"book-id\" xml:lang=\"zh-Hans\"><metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:identifier id=\"book-id\">urn:uuid:local</dc:identifier><dc:title>{}</dc:title><dc:creator>{}</dc:creator><dc:language>zh-Hans</dc:language><dc:description>{}</dc:description></metadata><manifest>{manifest}</manifest><spine>{spine}</spine></package>", xml(title), xml(author), xml(description));
    archive
        .start_file("OEBPS/content.opf", compressed)
        .map_err(|error| format!("无法创建 EPUB 条目：{error}"))?;
    archive
        .write_all(package.as_bytes())
        .map_err(|error| format!("无法写入 EPUB：{error}"))?;
    archive
        .finish()
        .map(|_| ())
        .map_err(|error| format!("无法完成 EPUB：{error}"))
}

/// Generates an application-owned EPUB, Markdown ZIP, TXT, audio ZIP, or comic ZIP export from one
/// validated local book at a user-selected destination.
///
/// # Arguments
/// * `app` - Application handle used to resolve the selected book.
/// * `name` - Exact book directory name previously returned by the library.
/// * `format` - One of `epub`, `markdown`, `txt`, `audio`, or `comic`.
/// * `output_path` - Destination chosen through the platform save dialog.
///
/// # Errors
/// Returns an error for unsupported formats, missing local resources, or export failures.
#[tauri::command]
async fn export_local_book(
    app: tauri::AppHandle,
    name: String,
    format: String,
    output_path: String,
) -> Result<LocalExport, String> {
    if !ensure_external_storage_access(app.clone()).await? {
        return Err("请在系统设置中允许本应用管理所有文件，然后重试导出".to_string());
    }
    if !matches!(format.as_str(), "epub" | "markdown" | "txt" | "audio" | "comic") {
        return Err("不支持的导出格式".to_string());
    }
    let directory = local_book_directory(&app, &name)?;
    let metadata = read_json_or_default::<StoredBookMetadata>(&directory.join(".novel-flow.json"))?;
    let store =
        read_json_or_default::<StoredChapterStore>(&directory.join(".novel-flow-chapters.json"))?;
    let mut chapters: Vec<_> = store.chapters.into_values().collect();
    chapters.sort_by_key(|chapter| (chapter.volume_index, chapter.chapter_index, chapter.id));
    let audio_directory = directory.join("audio");
    let comic_chapters = if format == "comic" {
        read_local_comic_chapters(&directory)?
    } else {
        Vec::new()
    };
    if format == "audio" {
        if !audio_directory.join("有声目录.m3u8").is_file() {
            return Err("这本书没有可打包的有声章节".to_string());
        }
    } else if format == "comic" {
        if comic_chapters.is_empty() {
            return Err("这本书没有可导出的已下载漫画章节".to_string());
        }
    } else if chapters.is_empty() {
        return Err("这本书没有可导出的已下载文字章节".to_string());
    }
    let export_metadata = if format == "audio" {
        metadata.audio.as_ref()
    } else if format == "comic" {
        metadata.comic.as_ref()
    } else {
        metadata.novel.as_ref()
    };
    let title = export_metadata
        .and_then(|work| work.title.as_deref())
        .unwrap_or(&name);
    let author = export_metadata
        .and_then(|work| work.author.as_deref())
        .unwrap_or("未知作者");
    let description = export_metadata
        .and_then(|work| work.description.as_deref())
        .unwrap_or("");
    let file_name = match format.as_str() {
        "epub" => format!("{name}.epub"),
        "audio" => format!("{name}-有声.zip"),
        "comic" => format!("{name}-漫画.zip"),
        _ => format!("{name}.txt"),
    };
    let expected_extension = match format.as_str() {
        "epub" => "epub",
        "markdown" | "audio" | "comic" => "zip",
        _ => "txt",
    };
    let requested_path = output_path.trim();
    let is_document_uri = requested_path.starts_with("content://");
    let target = if is_document_uri {
        app.path()
            .temp_dir()
            .map_err(|error| format!("无法解析导出临时目录：{error}"))?
            .join(format!(".sf-export-{}.{}", Uuid::new_v4(), expected_extension))
    } else {
        PathBuf::from(requested_path)
    };
    if !is_document_uri
        && (!target.is_absolute()
            || target.file_name().is_none()
            || !target
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case(expected_extension)))
    {
        return Err(format!("导出文件必须使用 .{expected_extension} 扩展名"));
    }
    match format.as_str() {
        "epub" => write_epub_export(&target, title, author, description, &chapters)?,
        "markdown" => {
            let file = File::create(&target)
                .map_err(|error| format!("无法创建 Markdown 导出：{error}"))?;
            let mut archive = ZipWriter::new(file);
            archive
                .start_file(
                    format!("{name}.md"),
                    SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated),
                )
                .map_err(|error| format!("无法创建 Markdown 条目：{error}"))?;
            archive
                .write_all(local_markdown(title, author, description, &chapters).as_bytes())
                .map_err(|error| format!("无法写入 Markdown 导出：{error}"))?;
            archive
                .finish()
                .map_err(|error| format!("无法完成 Markdown 导出：{error}"))?;
        }
        "audio" => {
            let file =
                File::create(&target).map_err(|error| format!("无法创建有声导出：{error}"))?;
            let mut archive = ZipWriter::new(file);
            let options =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            let entries = fs::read_dir(&audio_directory)
                .map_err(|error| format!("无法读取有声目录：{error}"))?;
            for entry in entries {
                let entry = entry.map_err(|error| format!("读取有声文件失败：{error}"))?;
                if !entry
                    .file_type()
                    .map_err(|error| format!("读取有声文件类型失败：{error}"))?
                    .is_file()
                {
                    continue;
                }
                let file_name = entry.file_name().to_string_lossy().into_owned();
                if file_name != "有声目录.m3u8" && !file_name.to_ascii_lowercase().ends_with(".mp3")
                {
                    continue;
                }
                archive
                    .start_file(format!("audio/{file_name}"), options)
                    .map_err(|error| format!("无法创建有声归档条目：{error}"))?;
                let bytes =
                    fs::read(entry.path()).map_err(|error| format!("无法读取有声文件：{error}"))?;
                archive
                    .write_all(&bytes)
                    .map_err(|error| format!("无法写入有声导出：{error}"))?;
            }
            archive
                .finish()
                .map_err(|error| format!("无法完成有声导出：{error}"))?;
        }
        "comic" => {
            let file = File::create(&target)
                .map_err(|error| format!("无法创建漫画导出：{error}"))?;
            let mut archive = ZipWriter::new(file);
            let options =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            let mut catalog = format!("{title}\n{author}\n\n{description}\n\n");
            for (chapter_index, chapter) in comic_chapters.iter().enumerate() {
                catalog.push_str(&format!("{:03}. {}\n", chapter_index + 1, chapter.title));
                for (page_index, page) in chapter.pages.iter().enumerate() {
                    let extension = Path::new(page)
                        .extension()
                        .and_then(|value| value.to_str())
                        .unwrap_or("jpg")
                        .to_ascii_lowercase();
                    archive
                        .start_file(
                            format!(
                                "comic/{:03}/{:03}.{extension}",
                                chapter_index + 1,
                                page_index + 1
                            ),
                            options,
                        )
                        .map_err(|error| format!("无法创建漫画归档条目：{error}"))?;
                    let bytes = fs::read(page)
                        .map_err(|error| format!("无法读取漫画页面：{error}"))?;
                    archive
                        .write_all(&bytes)
                        .map_err(|error| format!("无法写入漫画导出：{error}"))?;
                }
            }
            archive
                .start_file("目录.txt", options)
                .map_err(|error| format!("无法创建漫画目录：{error}"))?;
            archive
                .write_all(catalog.as_bytes())
                .map_err(|error| format!("无法写入漫画目录：{error}"))?;
            archive
                .finish()
                .map_err(|error| format!("无法完成漫画导出：{error}"))?;
        }
        "txt" => {
            let mut text = format!("{title}\n{author}\n\n{description}\n\n");
            for chapter in &chapters {
                text.push_str(&format!(
                    "{} - {}\n\n{}\n\n",
                    chapter.volume,
                    chapter.title,
                    text_without_markdown(&chapter.content)
                ));
            }
            fs::write(&target, format!("\u{feff}{text}"))
                .map_err(|error| format!("无法写入 TXT 导出：{error}"))?;
        }
        _ => return Err("不支持的导出格式".to_string()),
    }
    if is_document_uri {
        #[cfg(target_os = "android")]
        {
            let write_result = app
                .state::<AndroidSfacgAuth<tauri::Wry>>()
                .mobile_plugin_handle
                .run_mobile_plugin_async::<Value>(
                    "writeExportToUri",
                    serde_json::json!({
                        "sourcePath": target.to_string_lossy(),
                        "uri": requested_path,
                    }),
                )
                .await
                .map_err(|error| format!("无法写入系统保存位置：{error}"));
            let _ = fs::remove_file(&target);
            write_result?;
        }
        #[cfg(not(target_os = "android"))]
        {
            return Err("当前平台不支持 content:// 导出位置".to_string());
        }
    }
    Ok(LocalExport {
        href: requested_path.to_string(),
        file_name,
    })
}

/// Reads only the optional novel identifier used to validate a local chapter store.
///
/// # Arguments
/// * `directory` - Already validated local book directory.
///
/// # Errors
/// Returns an error if an existing metadata file cannot be decoded.
fn metadata_novel_id(directory: &PathBuf) -> Result<Option<i64>, String> {
    Ok(read_json_or_default::<StoredBookMetadata>(&directory.join(".novel-flow.json"))?
        .novel
        .and_then(|work| work.id))
}

/// Deletes one local book directory after validating it remains below the app library root.
///
/// # Arguments
/// * `app` - Tauri application handle used to locate the private library.
/// * `name` - Exact local book directory name.
///
/// # Errors
/// Returns an error when the book cannot be resolved or filesystem removal fails.
#[tauri::command]
fn delete_local_book(app: tauri::AppHandle, name: String) -> Result<(), String> {
    let directory = local_book_directory(&app, &name)?;
    fs::remove_dir_all(directory).map_err(|error| format!("删除本地书籍失败：{error}"))
}

/// Reads JSON from a private app file and treats a missing file as its default value.
///
/// # Arguments
/// * `path` - Native-resolved file path; callers must not pass renderer-controlled paths.
///
/// # Errors
/// Returns an error when an existing file is not valid UTF-8 JSON of the requested type.
fn read_json_or_default<T>(path: &PathBuf) -> Result<T, String>
where
    T: for<'de> Deserialize<'de> + Default,
{
    if !path.exists() {
        return Ok(T::default());
    }
    let contents =
        fs::read_to_string(path).map_err(|error| format!("无法读取本地数据：{error}"))?;
    serde_json::from_str(&contents).map_err(|error| format!("本地数据格式无效：{error}"))
}

/// Sanitizes a renderer-provided title before using it as a child library directory.
///
/// # Arguments
/// * `value` - User-visible work title.
///
/// # Returns
/// A bounded filesystem-safe title, or a stable fallback when all characters are removed.
fn safe_library_name(value: &str) -> String {
    let mut result = value
        .chars()
        .map(|character| {
            if ['<', '>', ':', '"', '/', '\\', '|', '?', '*'].contains(&character) {
                '_'
            } else {
                character
            }
        })
        .collect::<String>()
        .trim()
        .trim_end_matches(['.', ' '])
        .to_string();
    if result.is_empty() {
        result = "未命名作品".to_string();
    }
    result.chars().take(120).collect()
}

