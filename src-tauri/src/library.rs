//! 本地书库、持久化设置、正文恢复字典和导出。
//!
//! 本模块拥有应用数据目录下的书库文件格式。下载模块通过受限的 crate 内接口
//! 更新这些格式，但不会自行定义或迁移持久化模型。

use crate::endpoint_policy::{app_endpoint_client, web_endpoint_client, EndpointCapability};
use crate::sfacg::{NativeJobState, PersistedNativeJobs, DEFAULT_CONTENT_DICTIONARY};
use crate::utils::json::{read_or_default, write_atomically};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use tauri::Manager;
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// 一个本地存储书籍的渲染进程安全摘要。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LibraryBook {
    pub(crate) name: String,
    pub(crate) updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) cover: Option<String>,
    pub(crate) formats: LibraryFormats,
}

/// 描述一本书已存在的本地媒体格式。
#[derive(Debug, Serialize)]
pub(crate) struct LibraryFormats {
    pub(crate) text: bool,
    pub(crate) audio: bool,
    pub(crate) comic: bool,
}

/// 本地书籍详情的渲染进程安全记录。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalBookDetail {
    pub(crate) name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) novel: Option<LocalWorkMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) audio: Option<LocalWorkMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) comic: Option<LocalWorkMetadata>,
    pub(crate) image_directory: String,
    pub(crate) audio_tracks: Vec<LocalAudioTrack>,
    pub(crate) epub_href: Option<String>,
    pub(crate) chapter_volumes: Vec<LocalChapterVolume>,
    pub(crate) comic_chapters: Vec<LocalComicChapter>,
}

/// 恰好一种本地媒体类型的元数据。
///
/// 每个来源端点独立拥有自己的标识、封面和描述字段，禁止跨媒体类型复用。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalWorkMetadata {
    pub(crate) id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) catalog_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) online_path: Option<String>,
    pub(crate) title: String,
    pub(crate) author: String,
    pub(crate) description: String,
    pub(crate) type_name: Option<String>,
    pub(crate) tags: Vec<String>,
    pub(crate) is_finished: Option<bool>,
    pub(crate) score: Option<f64>,
    pub(crate) chapter_count: Option<i64>,
    pub(crate) character_count: Option<i64>,
    pub(crate) view_count: Option<i64>,
    pub(crate) mark_count: Option<i64>,
    pub(crate) point_count: Option<i64>,
    pub(crate) favorite_count: Option<i64>,
    pub(crate) ticket_count: Option<i64>,
    pub(crate) allow_download: Option<bool>,
    pub(crate) latest_chapter_title: Option<String>,
    pub(crate) latest_chapter_time: Option<String>,
    pub(crate) last_update_time: Option<String>,
    pub(crate) cover: Option<String>,
}

/// 可在本地播放的一条有声轨道。
///
/// 渲染进程需先通过 Tauri 资源协议转换已校验的绝对路径，再将其赋给音频元素。
#[derive(Debug, Serialize)]
pub(crate) struct LocalAudioTrack {
    pub(crate) title: String,
    pub(crate) href: String,
}

/// 按来源顺序分组的本地文字章节卷。
#[derive(Debug, Serialize)]
pub(crate) struct LocalChapterVolume {
    pub(crate) volume: String,
    pub(crate) chapters: Vec<LocalChapterSummary>,
}

/// 本地章节的渲染进程安全摘要。
#[derive(Debug, Serialize)]
pub(crate) struct LocalChapterSummary {
    pub(crate) id: i64,
    pub(crate) title: String,
}

/// 由原生兼容下载格式写入的持久化元数据。
#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredBookMetadata {
    #[serde(default)]
    pub(crate) novel: Option<StoredWorkMetadata>,
    #[serde(default)]
    pub(crate) audio: Option<StoredWorkMetadata>,
    #[serde(default)]
    pub(crate) comic: Option<StoredWorkMetadata>,
    #[serde(default)]
    pub(crate) downloaded_text_chapter_ids: Option<Vec<i64>>,
    #[serde(default)]
    pub(crate) downloaded_audio_chapter_ids: Option<Vec<i64>>,
    #[serde(default)]
    pub(crate) downloaded_comic_chapter_ids: Option<Vec<i64>>,
}

/// 一种媒体类型的持久化来源元数据。
///
/// 不提供跨媒体回退字段：漫画编号绝不能作为小说编号使用。
#[derive(Debug, Deserialize, Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredWorkMetadata {
    pub(crate) id: Option<i64>,
    pub(crate) catalog_id: Option<i64>,
    pub(crate) online_path: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) author: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) type_name: Option<String>,
    #[serde(default)]
    pub(crate) tags: Vec<String>,
    pub(crate) is_finished: Option<bool>,
    pub(crate) score: Option<f64>,
    pub(crate) chapter_count: Option<i64>,
    pub(crate) character_count: Option<i64>,
    pub(crate) view_count: Option<i64>,
    pub(crate) mark_count: Option<i64>,
    pub(crate) point_count: Option<i64>,
    pub(crate) favorite_count: Option<i64>,
    pub(crate) ticket_count: Option<i64>,
    pub(crate) allow_download: Option<bool>,
    pub(crate) latest_chapter_title: Option<String>,
    pub(crate) latest_chapter_time: Option<String>,
    pub(crate) last_update_time: Option<String>,
}

/// `.novel-flow-chapters.json` 中的一条持久化文字章节。
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredChapter {
    pub(crate) id: i64,
    pub(crate) title: String,
    pub(crate) volume: String,
    pub(crate) content: String,
    pub(crate) volume_index: i64,
    pub(crate) chapter_index: i64,
    /// 正文来源：`web` 表示 HTML/App 文本，`webVipOcr` 表示本地识别的图片正文。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) content_source: Option<String>,
    /// 使用 OCR 时保留的已授权图片或 GIF 来源相对路径。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ocr_source_path: Option<String>,
}

/// 现有下载器写入的持久化章节存储结构。
#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredChapterStore {
    pub(crate) novel_id: i64,
    pub(crate) chapters: std::collections::HashMap<String, StoredChapter>,
}

/// 原生网络下载任务使用的持久化请求限制。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RequestPolicy {
    pub(crate) request_interval_ms: u64,
    pub(crate) max_concurrent_downloads: u8,
    #[serde(default = "default_app_fallback_enabled")]
    pub(crate) app_fallback_enabled: bool,
    #[serde(default = "default_android_device_report_enabled")]
    pub(crate) android_device_report_enabled: bool,
}

/// 一条本地存储的漫画章节及其有序页面文件。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalComicChapter {
    pub(crate) id: i64,
    pub(crate) title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) cover: Option<String>,
    pub(crate) pages: Vec<String>,
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
    /// 创建降低上游请求压力的保守默认策略。
    ///
    /// # 返回值
    /// 并发数为一、两次请求间隔为 500 毫秒的下载策略。
    fn default() -> Self {
        Self {
            request_interval_ms: 500,
            max_concurrent_downloads: 1,
            app_fallback_enabled: true,
            android_device_report_enabled: true,
        }
    }
}

/// 读取或更新正文恢复字典后返回的渲染进程安全结果。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContentDictionaryResult {
    pub(crate) size: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) added: Option<usize>,
}

/// 判断字符是否属于 SF 正文中使用的中日韩统一表意文字范围。
///
/// # 参数
/// * `character` - 待分类的 Unicode 标量值。
///
/// # 返回值
/// 可参与字典映射的汉字时返回 `true`。
fn is_han_character(character: char) -> bool {
    matches!(
        character,
        '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{F900}'..='\u{FAFF}'
    )
}

/// 解析位于应用私有数据目录内、可由用户编辑的正文恢复字典路径。
///
/// # 参数
/// * `app` - 用于解析平台数据目录的应用句柄。
///
/// # 错误
/// 无法解析应用数据目录时返回错误。
fn content_dictionary_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|directory| directory.join("sfacg-content-dictionary.json"))
        .map_err(|error| format!("无法解析正文恢复字典路径：{error}"))
}

/// 校验 JSON 字典，并丢弃不是单个汉字到单个汉字映射的条目。
///
/// # 参数
/// * `value` - 从内置或持久化字典解析得到的 JSON 值。
///
/// # 返回值
/// 仅包含有效单字符映射且可安全合并的字典。
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

/// 加载持久化正文恢复字典；首次不存在时以随包默认值初始化。
///
/// # 参数
/// * `app` - 用于解析应用私有存储的应用句柄。
///
/// # 错误
/// 已存在字典格式错误或无法保存初始化字典时返回错误。
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

/// 使用持久化的网页对齐映射恢复 API 中混淆的汉字。
pub(crate) fn decode_api_content(app: &tauri::AppHandle, content: &str) -> Result<String, String> {
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

/// 通过同目录临时文件替换方式持久化已校验字典。
///
/// # 参数
/// * `app` - 用于解析应用私有存储的应用句柄。
/// * `dictionary` - 待保存的已校验字符映射。
///
/// # 错误
/// 序列化或替换文件失败时返回错误。
fn write_content_dictionary(
    app: &tauri::AppHandle,
    dictionary: &std::collections::HashMap<String, String>,
) -> Result<(), String> {
    let path = content_dictionary_path(app)?;
    write_atomically(&path, dictionary, "正文恢复字典")
}

/// 返回私有正文恢复字典中的有效映射数量。
///
/// # 参数
/// * `app` - 用于解析应用私有存储的应用句柄。
///
/// # 错误
/// 无法读取或初始化字典时返回错误。
#[tauri::command]
pub(crate) fn get_content_dictionary(
    app: tauri::AppHandle,
) -> Result<ContentDictionaryResult, String> {
    let dictionary = load_content_dictionary(&app)?;
    Ok(ContentDictionaryResult {
        size: dictionary.len(),
        added: None,
    })
}

/// 用同一章节的网页正文对齐 API 混淆汉字，并仅保存无冲突映射。
///
/// # 参数
/// * `app` - 用于访问原生会话状态和存储的应用句柄。
/// * `chapter_id` - 用于对齐的正数公开章节编号。
///
/// # 错误
/// 任一来源不可用、字符数量不一致，或已有映射与网页字符冲突时返回错误。
#[tauri::command]
pub(crate) async fn update_content_dictionary(
    app: tauri::AppHandle,
    chapter_id: i64,
) -> Result<ContentDictionaryResult, String> {
    let app_client = app_endpoint_client(&app, EndpointCapability::TextChapterApp)?;
    let api = app_client
        .get_chapter_content_and_metadata(chapter_id)
        .await?;
    let web_client = web_endpoint_client(&app, EndpointCapability::TextChapterWeb)?;
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

/// 解析应用自有书库目录，不接受渲染进程传入的路径。
///
/// 此目录位于平台应用数据目录下，因此 Windows 和 Android 均可写入，无需授予宽泛
/// 文件系统权限。Android 使用公开下载目录，桌面端使用私有应用数据目录。
///
/// # 参数
/// * `app` - 用于解析平台数据目录的 Tauri 应用句柄。
///
/// # 错误
/// 无法解析或创建书库目录时返回错误。
pub(crate) fn library_directory(app: &tauri::AppHandle) -> Result<PathBuf, String> {
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

/// 将已有 Android 私有书库一次性迁移至公开下载目录。
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

/// 解析原生请求策略文件，不接受渲染进程控制的位置。
///
/// # 参数
/// * `app` - 用于解析应用数据目录的 Tauri 应用句柄。
///
/// # 错误
/// 无法解析应用数据目录时返回错误。
fn request_policy_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|directory| directory.join("request-policy.json"))
        .map_err(|error| format!("无法解析请求策略文件：{error}"))
}

/// 解析用于恢复已暂停下载任务的私有状态快照路径。
///
/// # 参数
/// * `app` - 用于解析应用数据路径的应用句柄。
///
/// # 错误
/// 无法解析平台应用数据路径时返回错误。
fn native_jobs_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|directory| directory.join("download-jobs.json"))
        .map_err(|error| format!("无法解析下载任务文件：{error}"))
}

/// 以临时文件替换方式持久化渲染进程安全的任务和私有续传规格。
///
/// 快照刻意排除 Cookie、密码、输出路径和取消句柄。
///
/// # 参数
/// * `app` - 用于解析原生存储位置的应用句柄。
/// * `state` - 共享的内存下载任务状态。
///
/// # 错误
/// 任务快照无法序列化或替换时返回错误。
pub(crate) fn persist_native_jobs(
    app: &tauri::AppHandle,
    state: &NativeJobState,
) -> Result<(), String> {
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
    write_atomically(&path, &PersistedNativeJobs { jobs, specs }, "下载任务")
}

/// 启动后加载任务状态，并将未完成任务显式标记为暂停。
///
/// 进程重启后绝不自动恢复上游请求；用户必须在下载队列中手动继续。
///
/// # 参数
/// * `app` - 用于访问原生状态和应用数据的应用句柄。
///
/// # 错误
/// 已存在快照格式错误或无法读取时返回错误。
pub(crate) fn restore_native_jobs(app: &tauri::AppHandle) -> Result<(), String> {
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

/// 读取或初始化原生下载任务使用的请求限制
///
/// # 参数
/// * `app` - 用于解析私有应用数据的 Tauri 应用句柄。
///
/// # 错误
/// 已存在策略文件无法读取或格式错误时返回错误。
#[tauri::command]
pub(crate) fn get_request_policy(app: tauri::AppHandle) -> Result<RequestPolicy, String> {
    read_or_default(&request_policy_path(&app)?, "请求策略")
}

/// 校验并持久化后续原生下载任务使用的请求限制。
///
/// 渲染进程不能指定输出路径。文件经同目录临时文件完成写入再替换，避免中断后在目标
/// 路径留下不完整 JSON。
///
/// # 参数
/// * `app` - 用于解析私有应用数据的 Tauri 应用句柄。
/// * `policy` - 渲染进程提交的请求间隔和并发候选值。
///
/// # 错误
/// 值不在安全范围内，或策略无法序列化、保存时返回错误。
#[tauri::command]
pub(crate) fn save_request_policy(
    app: tauri::AppHandle,
    policy: RequestPolicy,
) -> Result<RequestPolicy, String> {
    if !(100..=60_000).contains(&policy.request_interval_ms) {
        return Err("请求间隔必须在 100 到 60000 毫秒之间".to_string());
    }
    if !(1..=4).contains(&policy.max_concurrent_downloads) {
        return Err("最大并发下载数必须在 1 到 4 之间".to_string());
    }

    write_atomically(&request_policy_path(&app)?, &policy, "请求策略")?;
    Ok(policy)
}

/// 列出 Tauri 自有本地书库中的书籍。
///
/// 此命令只返回目录名和格式标记，绝不返回绝对路径、文件内容或凭据；媒体访问由单独
/// 的已校验资源命令处理。
///
/// # 参数
/// * `app` - 用于定位书库的 Tauri 应用句柄。
///
/// # 错误
/// 无法扫描书库时返回错误。
#[tauri::command]
pub(crate) fn list_local_library(app: tauri::AppHandle) -> Result<Vec<LibraryBook>, String> {
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
            formats: LibraryFormats { text, audio, comic },
        });
    }
    books.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(books)
}

/// 解析应用自有书库内的单个子目录。
///
/// # 参数
/// * `app` - 用于定位私有书库根目录的 Tauri 应用句柄。
/// * `name` - 之前由 `list_local_library` 返回的精确目录名。
///
/// # 错误
/// 名称包含路径分隔符、遍历片段、条目不存在，或规范化后目录逸出书库根目录时返回错误。
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

/// 读取本地 M3U8 播放列表，仅暴露实际存在的一级 MP3 文件。
///
/// # 参数
/// * `name` - 已校验的本地书籍目录名。
/// * `directory` - 规范化后的本地书籍目录。
///
/// # 错误
/// 播放列表存在但无法按 UTF-8 读取时返回错误。
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

/// 读取本地书籍元数据和已下载文字章节索引。
///
/// # 参数
/// * `app` - 用于定位私有书库的 Tauri 应用句柄。
/// * `name` - 精确的本地目录名。
///
/// # 错误
/// 无法读取书籍目录或章节存储格式错误时返回错误。
#[tauri::command]
pub(crate) fn get_local_book(
    app: tauri::AppHandle,
    name: String,
) -> Result<LocalBookDetail, String> {
    let directory = local_book_directory(&app, &name)?;
    let metadata =
        read_or_default::<StoredBookMetadata>(&directory.join(".novel-flow.json"), "本地数据")?;
    let store = read_or_default::<StoredChapterStore>(
        &directory.join(".novel-flow-chapters.json"),
        "本地数据",
    )?;
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
    let epub_href = directory
        .join(&epub_name)
        .is_file()
        .then(|| directory.join(&epub_name).to_string_lossy().into_owned());
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
/// * `directory` - 已校验的本地书籍目录，用于查找封面文件。
/// * `cover_file` - 对应媒体类型的封面文件名。
///
/// # 返回值
/// 包含可用封面路径的本地作品元数据；元数据或编号缺失时返回 `None`。
fn local_work_metadata(
    metadata: Option<StoredWorkMetadata>,
    directory: &PathBuf,
    cover_file: &str,
) -> Option<LocalWorkMetadata> {
    let metadata = metadata?;
    let id = metadata.id?;
    let cover = directory.join("imgs").join(cover_file).is_file().then(|| {
        directory
            .join("imgs")
            .join(cover_file)
            .to_string_lossy()
            .into_owned()
    });
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
    for entry in fs::read_dir(root).map_err(|error| format!("无法读取漫画目录：{error}"))?
    {
        let entry = entry.map_err(|error| format!("读取漫画章节失败：{error}"))?;
        let chapter_dir = entry.path();
        if !chapter_dir.is_dir() || !chapter_dir.join(".complete").is_file() {
            continue;
        }
        let Some(id) = entry
            .file_name()
            .to_str()
            .and_then(|value| value.parse::<i64>().ok())
        else {
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
            .filter(|path| {
                matches!(
                    path.extension()
                        .and_then(|value| value.to_str())
                        .map(str::to_ascii_lowercase)
                        .as_deref(),
                    Some("jpg" | "jpeg" | "png" | "webp" | "avif")
                )
            })
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
pub(crate) fn get_local_comic_chapter(
    app: tauri::AppHandle,
    name: String,
    chapter_id: i64,
) -> Result<LocalComicChapter, String> {
    if chapter_id <= 0 {
        return Err("漫画章节参数无效".to_string());
    }
    let directory = local_book_directory(&app, &name)?;
    read_local_comic_chapters(&directory)?
        .into_iter()
        .find(|chapter| chapter.id == chapter_id)
        .ok_or_else(|| "本地漫画章节不存在".to_string())
}

/// 读取一条已下载文字章节，但不暴露其文件系统路径。
///
/// # 参数
/// * `app` - 用于定位私有书库的 Tauri 应用句柄。
/// * `name` - 精确的本地书籍目录名。
/// * `chapter_id` - 正数持久化章节编号。
///
/// # 错误
/// 编号无效、书籍不存在、存储格式错误或章节缺失时返回错误。
#[tauri::command]
pub(crate) fn get_local_chapter(
    app: tauri::AppHandle,
    name: String,
    chapter_id: i64,
) -> Result<StoredChapter, String> {
    if chapter_id <= 0 {
        return Err("章节参数无效".to_string());
    }
    let directory = local_book_directory(&app, &name)?;
    let store = read_or_default::<StoredChapterStore>(
        &directory.join(".novel-flow-chapters.json"),
        "本地数据",
    )?;
    if store.novel_id != 0 && metadata_novel_id(&directory)? != Some(store.novel_id) {
        return Err("本地章节与书籍元数据不匹配".to_string());
    }
    store
        .chapters
        .into_values()
        .find(|chapter| chapter.id == chapter_id)
        .ok_or_else(|| "本地未找到该章节正文".to_string())
}

/// 一个已生成本地导出文件的渲染进程安全描述。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalExport {
    pub(crate) href: String,
    pub(crate) file_name: String,
}

/// 确保 Android 可写入书库使用的公开下载目录。
#[tauri::command]
pub(crate) async fn ensure_external_storage_access(app: tauri::AppHandle) -> Result<bool, String> {
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

/// 为 TXT 导出移除本地 Markdown 格式，返回纯文本。
///
/// # 参数
/// * `value` - 已存储的章节 Markdown 文本。
///
/// # 返回值
/// 移除基本标题和图片语法后的可读文本。
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

/// 将已存储章节组织为完整的 Markdown 书籍文档。
///
/// # 参数
/// * `title` - 面向用户展示的书名。
/// * `author` - 作者。
/// * `description` - 书籍简介。
/// * `chapters` - 已按顺序排列的下载章节。
///
/// # 返回值
/// 包含 YAML 元数据和分卷标题的 UTF-8 Markdown。
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

/// 为已下载章节写入纯文字 EPUB 3 归档。
///
/// 归档刻意使用简单固定布局，使 Windows 与 Android 阅读器无需访问应用本地路径即可
/// 阅读。
///
/// # 参数
/// * `target` - 原生层生成的导出文件路径。
/// * `title` - 书名。
/// * `author` - 作者。
/// * `description` - 书籍简介。
/// * `chapters` - 已按顺序排列的下载章节。
///
/// # 错误
/// 无法创建或写入归档时返回错误。
fn write_epub_export(
    target: &PathBuf,
    title: &str,
    author: &str,
    description: &str,
    chapters: &[StoredChapter],
) -> Result<(), String> {
    /// 在插入用户或章节文本前转义 XML 保留字符。
    ///
    /// # 参数
    /// * `value` - 将嵌入 EPUB XML 文档的不可信文本。
    ///
    /// # 返回值
    /// 可安全用于此处 XML 文本和属性上下文的转义文本。
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

/// 将一本已校验的本地书籍导出到用户选择的位置。
///
/// 支持 EPUB、Markdown ZIP、TXT、有声 ZIP 和漫画 ZIP，文件始终由应用原生层生成。
///
/// # 参数
/// * `app` - 用于解析所选书籍的应用句柄。
/// * `name` - 书库此前返回的精确书籍目录名。
/// * `format` - `epub`、`markdown`、`txt`、`audio` 或 `comic` 之一。
/// * `output_path` - 通过平台保存对话框选择的目标位置。
///
/// # 错误
/// 格式不支持、本地资源缺失或导出失败时返回错误。
#[tauri::command]
pub(crate) async fn export_local_book(
    app: tauri::AppHandle,
    name: String,
    format: String,
    output_path: String,
) -> Result<LocalExport, String> {
    if !ensure_external_storage_access(app.clone()).await? {
        return Err("请在系统设置中允许本应用管理所有文件，然后重试导出".to_string());
    }
    if !matches!(
        format.as_str(),
        "epub" | "markdown" | "txt" | "audio" | "comic"
    ) {
        return Err("不支持的导出格式".to_string());
    }
    let directory = local_book_directory(&app, &name)?;
    let metadata =
        read_or_default::<StoredBookMetadata>(&directory.join(".novel-flow.json"), "本地数据")?;
    let store = read_or_default::<StoredChapterStore>(
        &directory.join(".novel-flow-chapters.json"),
        "本地数据",
    )?;
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
            .join(format!(
                ".sf-export-{}.{}",
                Uuid::new_v4(),
                expected_extension
            ))
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
            let file =
                File::create(&target).map_err(|error| format!("无法创建漫画导出：{error}"))?;
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
                    let bytes =
                        fs::read(page).map_err(|error| format!("无法读取漫画页面：{error}"))?;
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

/// 仅读取用于校验本地章节存储的可选小说编号。
///
/// # 参数
/// * `directory` - 已校验的本地书籍目录。
///
/// # 错误
/// 已存在元数据文件无法解析时返回错误。
fn metadata_novel_id(directory: &PathBuf) -> Result<Option<i64>, String> {
    Ok(
        read_or_default::<StoredBookMetadata>(&directory.join(".novel-flow.json"), "本地数据")?
            .novel
            .and_then(|work| work.id),
    )
}

/// 校验目录仍位于应用书库根目录后，删除一本本地书籍。
///
/// # 参数
/// * `app` - 用于定位私有书库的 Tauri 应用句柄。
/// * `name` - 精确的本地书籍目录名。
///
/// # 错误
/// 无法解析书籍目录或文件系统删除失败时返回错误。
#[tauri::command]
pub(crate) fn delete_local_book(app: tauri::AppHandle, name: String) -> Result<(), String> {
    let directory = local_book_directory(&app, &name)?;
    fs::remove_dir_all(directory).map_err(|error| format!("删除本地书籍失败：{error}"))
}

/// 将作品标题规范为书库子目录名。
///
/// # 参数
/// * `value` - 面向用户展示的作品标题。
///
/// # 返回值
/// 长度受限的文件系统安全标题；所有字符都被移除时返回稳定的默认名称。
pub(crate) fn safe_library_name(value: &str) -> String {
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
