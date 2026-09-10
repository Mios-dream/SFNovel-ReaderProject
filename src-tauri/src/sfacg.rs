//! SFACG 远程查询、认证和原生会话生命周期。
//!
//! 本模块仅处理上游 SFACG 协议、平台认证桥接与会话持久化。网络客户端的协议
//! 细节分别封装在 [`crate::app_client`] 与 [`crate::web_client`]，离线书库和下载
//! 任务由相邻业务模块负责。

use crate::endpoint_policy::{app_endpoint_client, web_endpoint_client, EndpointCapability};
use crate::library::{
    get_request_policy, library_directory, safe_library_name, StoredBookMetadata,
};
use crate::utils::json::read_or_default;
use serde::{Deserialize, Serialize, Serializer};
#[cfg(target_os = "android")]
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::{Mutex, OnceLock};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use uuid::Uuid;

#[cfg(target_os = "android")]
use tauri::plugin::{PluginHandle, TauriPlugin};

/// 首次创建用户正文恢复字典时使用的内置默认映射。
pub(crate) const DEFAULT_CONTENT_DICTIONARY: &str =
    include_str!("../../sfacg-content-dictionary.json");
/// 官方网页登录页面地址。
const OFFICIAL_LOGIN_URL: &str = "https://m.sfacg.com/login";
// 备用登录地址
// const OFFICIAL_LOGIN_URL: &str = "https://passport.sfacg.com/";

/// 官方登录窗口的稳定 Tauri 标签。
const OFFICIAL_LOGIN_WINDOW_LABEL: &str = "sfacg-official-login";
/// 所有平台访问 SF 网页时统一使用的桌面浏览器标识。
pub(crate) const SF_WEB_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";
/// 进程级设备身份令牌，仅在初始化后可供签名客户端读取。
pub(crate) static SF_DEVICE_TOKEN: OnceLock<String> = OnceLock::new();

/// 返回已初始化的设备身份令牌。
///
/// # 错误
/// 应用尚未完成设备身份初始化时返回错误，调用方不应自行生成替代值。
pub(crate) fn device_token() -> Result<&'static str, String> {
    SF_DEVICE_TOKEN
        .get()
        .map(String::as_str)
        .ok_or_else(|| "设备身份尚未初始化，请重启应用".to_string())
}

/// 搜索命令返回给渲染进程的一条公开作品记录。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchNovel {
    pub(crate) novel_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) media_id: Option<i64>,
    pub(crate) novel_name: String,
    pub(crate) author_name: String,
    pub(crate) novel_cover: String,
    pub(crate) last_update_time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) bookshelf_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) bookshelf_type: Option<String>,
    #[serde(rename = "sourcePath", skip_serializing_if = "Option::is_none")]
    pub(crate) source_path: Option<String>,
}

/// 章节选择视图使用的公开作品详细元数据。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NovelDetail {
    pub(crate) novel_id: i64,
    pub(crate) novel_name: String,
    pub(crate) author_name: String,
    pub(crate) novel_cover: String,
    pub(crate) last_update_time: String,
    pub(crate) description: String,
    pub(crate) is_finish: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) type_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) chapter_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) character_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) view_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) mark_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) point_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) favorite_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ticket_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) latest_chapter_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) latest_chapter_time: Option<String>,
    #[serde(rename = "sourcePath", skip_serializing_if = "Option::is_none")]
    pub(crate) source_path: Option<String>,
}

/// SF 小说单个分卷的渲染进程安全章节集合。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChapterVolume {
    pub(crate) volume_id: i64,
    pub(crate) title: String,
    pub(crate) chapters: Vec<ChapterSummary>,
}

/// 渲染进程安全的章节可用性记录。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChapterSummary {
    pub(crate) chap_id: i64,
    pub(crate) title: String,
    pub(crate) is_vip: bool,
    pub(crate) content_kind: TextContentKind,
    pub(crate) access_state: TextAccessState,
    pub(crate) downloaded: bool,
}

/// 网页文字目录标记的章节正文形态。
///
/// 此字段只描述上游目录可见的内容类型，不表达账户购买状态。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // 后续实际资源请求会写入非 Unknown 值。
pub(crate) enum TextContentKind {
    /// 常规 HTML 正文。
    Text,
    /// 由图片组成、后续需要本地 OCR 的 VIP 正文。
    ImageVip,
    /// 由加密 GIF 提供、后续需要本地 OCR 的 VIP 正文。
    EncryptedVip,
    /// 网页目录未提供足够标记，需在实际请求正文时判断。
    Unknown,
}

/// 一次网页目录读取可确认的章节访问状态。
///
/// 目录不会以登录 Cookie 或销售属性推断购买态；只有实际请求正文资源后才能更新
/// 为其他状态。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // 阶段一目录读取仅能安全地产生 Unknown。
pub(crate) enum TextAccessState {
    /// 尚未请求章节资源，购买与会话状态未知。
    Unknown,
    /// 章节资源已成功读取。
    Available,
    /// 上游明确没有提供该章节内容。
    Unavailable,
    /// 网页登录会话失效。
    SessionExpired,
}

/// 返回给渲染进程且不含会话数据的已登录 SF 书架。
#[derive(Clone, Debug, Serialize)]
pub(crate) struct BookshelfCollection {
    pub(crate) categories: Vec<String>,
    pub(crate) items: Vec<SearchNovel>,
}

/// 与渲染进程契约兼容的原生下载任务状态。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeJob {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) kind: String,
    pub(crate) status: String,
    pub(crate) progress: u8,
    pub(crate) message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) file: Option<String>,
    pub(crate) novel_id: Option<i64>,
}

/// 共享的原生任务注册表、取消标记和续传规格。
#[derive(Default)]
pub(crate) struct NativeJobState {
    pub(crate) jobs: Mutex<std::collections::HashMap<String, NativeJob>>,
    pub(crate) cancellations:
        Mutex<std::collections::HashMap<String, Arc<std::sync::atomic::AtomicBool>>>,
    pub(crate) specs: Mutex<std::collections::HashMap<String, NativeJobSpec>>,
    /// 以小说编号区分的文字下载存储锁。
    ///
    /// 同一本书的多个任务会读写同一个 `.novel-flow-chapters.json`；该锁使目录读取、
    /// OCR 和索引提交串行执行，避免两个任务基于不同旧快照互相覆盖章节索引。
    pub(crate) text_storage_locks:
        Mutex<std::collections::HashMap<i64, Arc<tokio::sync::Mutex<()>>>>,
}

/// 恢复原生下载任务所需的私有输入。
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct NativeJobSpec {
    pub(crate) kind: String,
    pub(crate) novel_id: i64,
    #[serde(default)]
    pub(crate) source_id: Option<i64>,
    #[serde(default)]
    pub(crate) source_path: Option<String>,
    pub(crate) title: String,
    pub(crate) chapter_ids: Vec<i64>,
}

/// 允许用户在应用重启后手动恢复任务的持久化数据。
#[derive(Default, Deserialize, Serialize)]
pub(crate) struct PersistedNativeJobs {
    pub(crate) jobs: std::collections::HashMap<String, NativeJob>,
    pub(crate) specs: std::collections::HashMap<String, NativeJobSpec>,
}

/// 仅原生层使用的有声章节记录，包含上游音频地址。
pub(crate) struct NativeAudioChapter {
    pub(crate) id: i64,
    pub(crate) title: String,
    pub(crate) volume: String,
    pub(crate) is_vip: bool,
    pub(crate) is_unlocked: bool,
    pub(crate) source: String,
}

/// 单个 SF 作品的渲染进程安全有声目录。
#[derive(Debug, Serialize)]
pub(crate) struct AudioCatalog {
    pub(crate) title: String,
    pub(crate) chapters: Vec<AudioChapterSummary>,
}

/// 不含受保护上游音频地址的渲染进程安全有声章节行。
#[derive(Debug, Serialize)]
pub(crate) struct AudioChapterSummary {
    pub(crate) id: i64,
    pub(crate) title: String,
    pub(crate) volume: String,
    pub(crate) is_vip: bool,
    pub(crate) is_unlocked: bool,
    pub(crate) downloaded: bool,
}

/// 当前原生会话的渲染进程安全认证状态。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthStatus {
    pub(crate) authenticated: bool,
    pub(crate) app_authenticated: bool,
    pub(crate) web_authenticated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) user_name: Option<String>,
}

/// 已登录 SF 账户资料的渲染进程安全快照。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UserProfile {
    pub(crate) account_id: i64,
    pub(crate) nick_name: String,
    pub(crate) avatar: String,
    pub(crate) app_details_available: bool,
    pub(crate) web_details_available: bool,
    pub(crate) vip_details_available: bool,
    pub(crate) vip_system: String,
    pub(crate) welfare_coin: i64,
    pub(crate) fire_money_remain: i64,
    pub(crate) coupons_remain: i64,
    pub(crate) monthly_ticket: i64,
    pub(crate) vip_level: i64,
    pub(crate) vip_name: String,
}

/// 在原生内存中保存相互隔离的 SF 会话。
#[derive(Default)]
pub(crate) struct AuthSessionState {
    pub(crate) session: Mutex<Option<NativeAuthSession>>,
}

/// App 与 Web Cookie 刻意分离，不能相互转换。
pub(crate) struct NativeAuthSession {
    pub(crate) app_cookie: Option<String>,
    pub(crate) web_cookie: Option<String>,
    pub(crate) user_name: String,
}

/// 仅原生层使用的漫画章节记录。
///
/// 漫画章节的账户拥有权状态。
///
/// 网页目录和匿名 App API 的 VIP 标记只能产生 `Unknown`。只有带有效 App 会话的 API
/// 返回明确拥有权时，才允许产生 `Unlocked` 或 `Locked`；`Unknown` 必须由真实资源请求
/// 决定，不得在目录阶段阻止用户选择。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ComicUnlockState {
    Unlocked,
    Locked,
    Unknown,
}

impl ComicUnlockState {
    /// 判断已认证 App API 是否明确禁止下载该章节。
    pub(crate) fn is_selectable(self) -> bool {
        self != Self::Locked
    }
}

impl Serialize for ComicUnlockState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Unlocked => serializer.serialize_bool(true),
            Self::Locked => serializer.serialize_bool(false),
            Self::Unknown => serializer.serialize_str("unknown"),
        }
    }
}

/// 公开漫画站仅通过网页端点提供章节目录。网页 VIP 标记仅表示章节类型，拥有权状态始终
/// 为 [`ComicUnlockState::Unknown`]。
pub(crate) struct NativeComicChapter {
    pub(crate) id: i64,
    pub(crate) title: String,
    pub(crate) is_vip: bool,
    pub(crate) is_unlocked: ComicUnlockState,
}

/// 章节选择器使用的渲染进程安全漫画目录。
#[derive(Debug, Serialize)]
pub(crate) struct ComicCatalog {
    pub(crate) title: String,
    pub(crate) chapters: Vec<ComicChapterSummary>,
}

/// 不含页面地址和来源目录信息的一条漫画章节。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ComicChapterSummary {
    pub(crate) id: i64,
    pub(crate) title: String,
    pub(crate) is_vip: bool,
    pub(crate) is_unlocked: ComicUnlockState,
    pub(crate) downloaded: bool,
}

/// Windows 端加密保存的 App 登录会话序列化格式。
///
/// Cookie 在写入磁盘前必须由系统数据保护 API 加密。
#[derive(Deserialize, Serialize)]
struct PersistedAuthSession {
    pub(crate) version: u8,
    pub(crate) cookie: String,
    pub(crate) user_name: String,
}

/// Windows 端加密保存的稳定设备身份序列化格式。
#[derive(Deserialize, Serialize)]
struct PersistedDeviceIdentity {
    pub(crate) version: u8,
    pub(crate) token: String,
}

/// 返回 Windows 端加密 App 会话文件的私有存储路径。
///
/// # 错误
/// 无法解析应用数据目录时返回错误。
#[cfg(target_os = "windows")]
fn auth_session_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|directory| directory.join("auth-session.bin"))
        .map_err(|error| format!("无法解析登录会话路径：{error}"))
}

/// 返回 Windows 端加密 Web 会话文件的私有存储路径。
///
/// # 错误
/// 无法解析应用数据目录时返回错误。
#[cfg(target_os = "windows")]
fn web_session_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|directory| directory.join("web-session.v2.bin"))
        .map_err(|error| format!("无法解析网页登录会话路径：{error}"))
}

/// 使用当前 Windows 用户的数据保护密钥加密会话或设备身份数据。
///
/// # 参数
/// * `input` - 待加密的序列化字节。
///
/// # 错误
/// Windows 数据保护 API 调用失败时返回带系统错误码的错误。
#[cfg(target_os = "windows")]
fn protect_auth_data(input: &[u8]) -> Result<Vec<u8>, String> {
    use std::ptr;
    use windows_sys::Win32::Foundation::{GetLastError, LocalFree};
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let input_blob = CRYPT_INTEGER_BLOB {
        cbData: input.len() as u32,
        pbData: input.as_ptr() as *mut u8,
    };
    let mut output_blob = CRYPT_INTEGER_BLOB::default();
    let success = unsafe {
        CryptProtectData(
            &input_blob,
            ptr::null(),
            ptr::null(),
            ptr::null(),
            ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output_blob,
        )
    };
    if success == 0 {
        return Err(format!("无法加密登录会话（错误码 {}）", unsafe {
            GetLastError()
        }));
    }
    let protected = unsafe {
        std::slice::from_raw_parts(output_blob.pbData, output_blob.cbData as usize).to_vec()
    };
    unsafe { LocalFree(output_blob.pbData.cast()) };
    Ok(protected)
}

/// 使用当前 Windows 用户的数据保护密钥解密会话或设备身份数据。
///
/// # 参数
/// * `input` - 由 [`protect_auth_data`] 生成的密文字节。
///
/// # 错误
/// 密文不属于当前用户或 Windows 数据保护 API 调用失败时返回错误。
#[cfg(target_os = "windows")]
fn unprotect_auth_data(input: &[u8]) -> Result<Vec<u8>, String> {
    use std::ptr;
    use windows_sys::Win32::Foundation::{GetLastError, LocalFree};
    use windows_sys::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let input_blob = CRYPT_INTEGER_BLOB {
        cbData: input.len() as u32,
        pbData: input.as_ptr() as *mut u8,
    };
    let mut output_blob = CRYPT_INTEGER_BLOB::default();
    let success = unsafe {
        CryptUnprotectData(
            &input_blob,
            ptr::null_mut(),
            ptr::null(),
            ptr::null(),
            ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output_blob,
        )
    };
    if success == 0 {
        return Err(format!("无法解密登录会话（错误码 {}）", unsafe {
            GetLastError()
        }));
    }
    let plain = unsafe {
        std::slice::from_raw_parts(output_blob.pbData, output_blob.cbData as usize).to_vec()
    };
    unsafe { LocalFree(output_blob.pbData.cast()) };
    Ok(plain)
}

/// 将 App 会话经系统加密后原子写入 Windows 私有存储。
///
/// # 错误
/// 路径解析、序列化、加密或替换目标文件失败时返回错误。
#[cfg(target_os = "windows")]
fn persist_desktop_auth_session(
    app: &tauri::AppHandle,
    cookie: &str,
    user_name: &str,
) -> Result<(), String> {
    let path = auth_session_path(app)?;
    let directory = path
        .parent()
        .ok_or_else(|| "登录会话目录无效".to_string())?;
    fs::create_dir_all(directory).map_err(|error| format!("无法创建登录会话目录：{error}"))?;
    let payload = serde_json::to_vec(&PersistedAuthSession {
        version: 2,
        cookie: cookie.to_string(),
        user_name: user_name.to_string(),
    })
    .map_err(|error| format!("无法序列化登录会话：{error}"))?;
    let encrypted = protect_auth_data(&payload)?;
    let temporary = path.with_extension("bin.tmp");
    fs::write(&temporary, encrypted).map_err(|error| format!("无法写入登录会话：{error}"))?;
    if path.exists() {
        fs::remove_file(&path).map_err(|error| format!("无法替换登录会话：{error}"))?;
    }
    fs::rename(temporary, path).map_err(|error| format!("无法完成登录会话保存：{error}"))
}

/// 从 Windows 私有存储恢复已加密的 App 会话。
///
/// # 返回值
/// 不存在、格式过期或字段为空的会话返回 `Ok(None)`。
///
/// # 错误
/// 已存在的会话文件无法读取或解密时返回错误。
#[cfg(target_os = "windows")]
pub(crate) fn restore_desktop_auth_session(
    app: &tauri::AppHandle,
) -> Result<Option<NativeAuthSession>, String> {
    let path = auth_session_path(app)?;
    if !path.is_file() {
        return Ok(None);
    }
    let encrypted = fs::read(path).map_err(|error| format!("无法读取登录会话：{error}"))?;
    let payload = unprotect_auth_data(&encrypted)?;
    let stored = serde_json::from_slice::<PersistedAuthSession>(&payload)
        .map_err(|error| format!("登录会话格式无效：{error}"))?;
    if stored.version != 2 || stored.cookie.trim().is_empty() || stored.user_name.trim().is_empty()
    {
        return Ok(None);
    }
    Ok(Some(NativeAuthSession {
        app_cookie: Some(stored.cookie),
        web_cookie: None,
        user_name: stored.user_name,
    }))
}

/// 删除 Windows 私有存储中的加密 App 会话。
///
/// # 错误
/// 删除已存在的会话文件失败时返回错误。
#[cfg(target_os = "windows")]
fn clear_desktop_auth_session(app: &tauri::AppHandle) -> Result<(), String> {
    let path = auth_session_path(app)?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| format!("无法清除登录会话：{error}"))?;
    }
    Ok(())
}

/// 将 Web 会话经系统加密后写入 Windows 私有存储。
///
/// # 错误
/// 序列化、加密、目录创建或文件写入失败时返回错误。
#[cfg(target_os = "windows")]
fn persist_desktop_web_session(app: &tauri::AppHandle, cookie: &str) -> Result<(), String> {
    let path = web_session_path(app)?;
    let payload = serde_json::to_vec(&PersistedAuthSession {
        version: 2,
        cookie: cookie.to_string(),
        user_name: "已登录 SF 账号".to_string(),
    })
    .map_err(|e| format!("无法序列化网页登录会话：{e}"))?;
    let encrypted = protect_auth_data(&payload)?;
    let parent = path
        .parent()
        .ok_or_else(|| "网页登录会话目录无效".to_string())?;
    fs::create_dir_all(parent).map_err(|e| format!("无法创建网页登录会话目录：{e}"))?;
    fs::write(&path, encrypted).map_err(|e| format!("无法保存网页登录会话：{e}"))
}

/// 从 Windows 私有存储恢复已加密的 Web 会话。
///
/// # 返回值
/// 会话不存在、格式过期或 Cookie 为空时返回 `Ok(None)`。
///
/// # 错误
/// 已存在的会话文件无法读取或解密时返回错误。
#[cfg(target_os = "windows")]
pub(crate) fn restore_desktop_web_session(
    app: &tauri::AppHandle,
) -> Result<Option<NativeAuthSession>, String> {
    let path = web_session_path(app)?;
    if !path.is_file() {
        return Ok(None);
    }
    let payload =
        unprotect_auth_data(&fs::read(path).map_err(|e| format!("无法读取网页登录会话：{e}"))?)?;
    let stored = serde_json::from_slice::<PersistedAuthSession>(&payload)
        .map_err(|e| format!("网页登录会话格式无效：{e}"))?;
    if stored.version != 2 || stored.cookie.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(NativeAuthSession {
        app_cookie: None,
        web_cookie: Some(stored.cookie),
        user_name: stored.user_name,
    }))
}

/// 删除 Windows 私有存储中的加密 Web 会话。
///
/// # 错误
/// 删除已存在的会话文件失败时返回错误。
#[cfg(target_os = "windows")]
fn clear_desktop_web_session(app: &tauri::AppHandle) -> Result<(), String> {
    let path = web_session_path(app)?;
    if path.exists() {
        fs::remove_file(path).map_err(|e| format!("无法清除网页登录会话：{e}"))?;
    }
    Ok(())
}

/// 在 Windows 上恢复或生成稳定设备身份，并以系统数据保护机制保存。
///
/// # 错误
/// 设备身份文件无法读写、加解密或应用数据目录无法解析时返回错误。
#[cfg(target_os = "windows")]
pub(crate) fn initialize_device_identity(app: &tauri::AppHandle) -> Result<(), String> {
    let path = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("无法解析设备身份路径：{error}"))?
        .join("device-identity.v1.bin");
    let token = if path.is_file() {
        let encrypted = fs::read(&path).map_err(|error| format!("无法读取设备身份：{error}"))?;
        let payload = unprotect_auth_data(&encrypted)?;
        serde_json::from_slice::<PersistedDeviceIdentity>(&payload)
            .ok()
            .filter(|identity| identity.version == 1 && is_valid_device_token(&identity.token))
            .map(|identity| identity.token)
    } else {
        None
    }
    .unwrap_or_else(|| Uuid::new_v4().hyphenated().to_string().to_uppercase());
    let directory = path
        .parent()
        .ok_or_else(|| "设备身份目录无效".to_string())?;
    fs::create_dir_all(directory).map_err(|error| format!("无法创建设备身份目录：{error}"))?;
    let payload = serde_json::to_vec(&PersistedDeviceIdentity {
        version: 1,
        token: token.clone(),
    })
    .map_err(|error| format!("无法序列化设备身份：{error}"))?;
    let encrypted = protect_auth_data(&payload)?;
    let temporary = path.with_extension("bin.tmp");
    fs::write(&temporary, encrypted).map_err(|error| format!("无法保存设备身份：{error}"))?;
    if path.exists() {
        fs::remove_file(&path).map_err(|error| format!("无法替换设备身份：{error}"))?;
    }
    fs::rename(temporary, path).map_err(|error| format!("无法完成设备身份保存：{error}"))?;
    let _ = SF_DEVICE_TOKEN.set(token);
    Ok(())
}

/// 在非 Windows、非 Android 平台为当前进程生成临时设备身份。
///
/// 此身份不会落盘，应用重启后会重新生成。
#[cfg(all(not(target_os = "windows"), not(target_os = "android")))]
pub(crate) fn initialize_device_identity(_app: &tauri::AppHandle) -> Result<(), String> {
    let _ = SF_DEVICE_TOKEN.set(Uuid::new_v4().hyphenated().to_string().to_uppercase());
    Ok(())
}

/// 判断设备身份令牌是否为合法 UUID。
fn is_valid_device_token(token: &str) -> bool {
    Uuid::parse_str(token).is_ok()
}

/// Android 原生插件返回的 WebView Cookie 容器。
///
/// Cookie 仅由 Kotlin 插件提供，绝不接受来自 JavaScript 的会话值。
#[cfg(target_os = "android")]
#[derive(Deserialize)]
struct AndroidCookieResponse {
    pub(crate) cookie: Option<String>,
}

/// Android 原生插件返回的稳定设备身份容器。
#[cfg(target_os = "android")]
#[derive(Deserialize)]
struct AndroidDeviceTokenResponse {
    pub(crate) token: Option<String>,
}

/// Android ONNX OCR 插件返回的正文容器。
#[cfg(target_os = "android")]
#[derive(Deserialize)]
struct AndroidOcrResponse {
    text: String,
}

/// 保存只供 Rust 命令调用的 Android 原生认证插件句柄。
#[cfg(target_os = "android")]
pub(crate) struct AndroidSfacgAuth<R: tauri::Runtime> {
    pub(crate) mobile_plugin_handle: PluginHandle<R>,
}

/// 保存只供 Rust OCR 命令调用的 Android 原生识别插件句柄。
#[cfg(target_os = "android")]
pub(crate) struct AndroidSfacgOcr<R: tauri::Runtime> {
    pub(crate) mobile_plugin_handle: PluginHandle<R>,
}

/// 构建拥有官方登录 WebView 与 Cookie 桥接能力的 Android 专用插件。
///
/// # 返回值
/// 在命令运行前注册 Kotlin 实现的 Tauri 插件。
#[cfg(target_os = "android")]
pub(crate) fn android_sfacg_auth_plugin<R: tauri::Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("sfacg-auth")
        .setup(|app, api| {
            let handle =
                api.register_android_plugin("com.sansan.sf_novel_flow", "SfacgAuthPlugin")?;
            app.manage(AndroidSfacgAuth {
                mobile_plugin_handle: handle,
            });
            Ok(())
        })
        .build()
}

/// 构建 Android 专用 OCR 插件；识别实现与认证插件保持独立。
#[cfg(target_os = "android")]
pub(crate) fn android_sfacg_ocr_plugin<R: tauri::Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("sfacg-ocr")
        .setup(|app, api| {
            let handle =
                api.register_android_plugin("com.sansan.sf_novel_flow", "SfacgOcrPlugin")?;
            app.manage(AndroidSfacgOcr {
                mobile_plugin_handle: handle,
            });
            Ok(())
        })
        .build()
}

/// 将持久化的 Android WebView 会话同步到原生请求状态。
///
/// # 参数
/// * `app` - 用于调用 Android 插件和访问状态的应用句柄。
///
/// # 错误
/// Android 插件无法提供其自有 Cookie 存储时返回错误。
#[cfg(target_os = "android")]
pub(crate) async fn sync_android_auth_session(app: &tauri::AppHandle) -> Result<(), String> {
    let web_cookie = app
        .state::<AndroidSfacgAuth<tauri::Wry>>()
        .mobile_plugin_handle
        .run_mobile_plugin_async::<AndroidCookieResponse>("readSessionCookie", ())
        .await
        .map_err(|error| format!("无法读取 Android 登录会话：{error}"))?
        .cookie
        .filter(|value| !value.trim().is_empty());
    let app_cookie = app
        .state::<AndroidSfacgAuth<tauri::Wry>>()
        .mobile_plugin_handle
        .run_mobile_plugin_async::<AndroidCookieResponse>("readAppSessionCookie", ())
        .await
        .map_err(|error| format!("无法读取 Android App 会话：{error}"))?
        .cookie
        .filter(|value| !value.trim().is_empty());
    let auth_state = app.state::<AuthSessionState>();
    let mut session = auth_state
        .session
        .lock()
        .map_err(|_| "登录会话状态不可用".to_string())?;
    *session = if app_cookie.is_some() || web_cookie.is_some() {
        Some(NativeAuthSession {
            app_cookie,
            web_cookie,
            user_name: "已登录 SF 账号".to_string(),
        })
    } else {
        None
    };
    Ok(())
}

/// 从 Android 原生插件读取或创建稳定设备身份，并初始化进程缓存。
///
/// # 错误
/// 插件调用失败或返回的令牌不是合法 UUID 时返回错误。
#[cfg(target_os = "android")]
pub(crate) async fn initialize_device_identity(app: &tauri::AppHandle) -> Result<(), String> {
    let token = app
        .state::<AndroidSfacgAuth<tauri::Wry>>()
        .mobile_plugin_handle
        .run_mobile_plugin_async::<AndroidDeviceTokenResponse>("readOrCreateDeviceToken", ())
        .await
        .map_err(|error| format!("无法读取设备身份：{error}"))?
        .token
        .filter(|token| is_valid_device_token(token))
        .ok_or_else(|| "Android 设备身份无效".to_string())?;
    let _ = SF_DEVICE_TOKEN.set(token.to_uppercase());
    Ok(())
}

/// 在 Android 原生插件中识别已保存的章节图片，并返回与桌面 worker 相同的纯文本。
#[cfg(target_os = "android")]
pub(crate) async fn recognize_android_image(
    app: &tauri::AppHandle,
    source: std::path::PathBuf,
    segments_dir: std::path::PathBuf,
) -> Result<String, String> {
    let result = app
        .state::<AndroidSfacgOcr<tauri::Wry>>()
        .mobile_plugin_handle
        .run_mobile_plugin_async::<AndroidOcrResponse>(
            "recognizeChapter",
            serde_json::json!({
                "sourcePath": source.to_string_lossy(),
                "segmentsDir": segments_dir.to_string_lossy(),
            }),
        )
        .await
        .map_err(|error| format!("Android OCR 失败：{error}"))?;
    if result.text.trim().is_empty() {
        return Err("Android OCR 未识别到可用文字；原始图片已保留，可稍后重试".to_string());
    }
    Ok(result.text)
}

/// 通过 Android 原生插件持久化 App 会话 Cookie。
///
/// # 错误
/// 插件无法写入其受管 Cookie 存储时返回错误。
#[cfg(target_os = "android")]
async fn persist_android_auth_session(app: &tauri::AppHandle, cookie: &str) -> Result<(), String> {
    app.state::<AndroidSfacgAuth<tauri::Wry>>()
        .mobile_plugin_handle
        .run_mobile_plugin_async::<Value>(
            "writeAppSessionCookie",
            serde_json::json!({ "cookie": cookie }),
        )
        .await
        .map_err(|error| format!("无法保存 Android 登录会话：{error}"))?;
    Ok(())
}

/// 将仅原生层可见的会话状态转换为渲染进程安全的登录元数据。
///
/// # 参数
/// * `app` - 用于访问原生会话状态的应用句柄。
///
/// # 错误
/// 会话锁因先前原生 panic 而不可用时返回错误。
fn current_auth_status(app: &tauri::AppHandle) -> Result<AuthStatus, String> {
    let auth_state = app.state::<AuthSessionState>();
    let session = auth_state
        .session
        .lock()
        .map_err(|_| "登录会话状态不可用".to_string())?;
    Ok(AuthStatus {
        authenticated: session
            .as_ref()
            .is_some_and(|value| value.app_cookie.is_some() || value.web_cookie.is_some()),
        app_authenticated: session
            .as_ref()
            .is_some_and(|value| value.app_cookie.is_some()),
        web_authenticated: session
            .as_ref()
            .is_some_and(|value| value.web_cookie.is_some()),
        user_name: session.as_ref().map(|value| value.user_name.clone()),
    })
}

/// API 章节正文及与网页正文比对所需的标识。
pub(crate) struct ApiChapterContent {
    pub(crate) content: String,
    pub(crate) novel_id: i64,
    pub(crate) volume_id: i64,
}

/// 搜索 SF 公开作品，并将上游记录转换为渲染进程 DTO。
///
/// # 参数
/// * `query` - 已去除首尾空白的用户搜索词，最多 100 个 Unicode 标量值。
///
/// # 错误
/// 输入为空、过长或上游服务不可用时返回错误。
#[tauri::command]
pub(crate) async fn search_novels(
    app: tauri::AppHandle,
    query: String,
) -> Result<Vec<SearchNovel>, String> {
    let query = query.trim().to_string();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    if query.chars().count() > 100 {
        return Err("搜索关键词不能超过 100 个字符".to_string());
    }
    let client = app_endpoint_client(&app, EndpointCapability::Search)?;
    client.search_novels(&query).await
}

/// 校验渲染进程提供的公开 SF 小说编号。
///
/// # 参数
/// * `novel_id` - 待校验的正数 SF 小说编号。
///
/// # 错误
/// 编号为零、负数或不符合其他约束时返回错误。
pub(crate) fn validate_novel_id(novel_id: i64) -> Result<(), String> {
    if novel_id <= 0 {
        return Err("小说编号无效".to_string());
    }
    Ok(())
}

/// 通过固定的原生 SF API 适配器读取公开小说详情。
///
/// # 参数
/// * `novel_id` - 正数 SF 小说编号。
///
/// # 错误
/// 编号无效、上游元数据不可用或格式错误时返回错误。
#[tauri::command]
pub(crate) async fn get_novel_details(
    app: tauri::AppHandle,
    novel_id: i64,
) -> Result<NovelDetail, String> {
    validate_novel_id(novel_id)?;
    let client = app_endpoint_client(&app, EndpointCapability::NovelDetail)?;
    client.get_novel_details(novel_id).await
}

/// 读取有声专辑的公开元数据，同时保留有声目录所需的来源小说编号。
///
/// # 参数
/// * `app` - 用于构造原生 App 客户端的应用句柄。
/// * `album_id` - SF 有声专辑编号，必须为正数。
/// * `novel_id` - 有声目录使用的来源小说编号，必须为正数。
///
/// # 错误
/// 当编号无效、上游不可用或元数据格式异常时返回错误。
#[tauri::command]
pub(crate) async fn get_audio_details(
    app: tauri::AppHandle,
    album_id: i64,
    novel_id: i64,
) -> Result<NovelDetail, String> {
    validate_novel_id(album_id)?;
    validate_novel_id(novel_id)?;
    let client = app_endpoint_client(&app, EndpointCapability::AudioDetail)?;
    client.get_audio_details(album_id, novel_id).await
}

/// 通过 App API 列出文字章节，并合并本地完成状态。
///
/// App 目录仅提供元数据。章节正文仍从网页下载；VIP 正文使用已认证图片端点和本地 OCR。
///
/// # 参数
/// * `novel_id` - 正数 SF 小说编号。
///
/// # 错误
/// 编号无效、缺少 App 会话或 SF App 目录格式错误时返回错误。
#[tauri::command]
pub(crate) async fn get_chapter_volumes(
    app: tauri::AppHandle,
    novel_id: i64,
) -> Result<Vec<ChapterVolume>, String> {
    validate_novel_id(novel_id)?;
    #[cfg(target_os = "android")]
    sync_android_auth_session(&app).await?;
    let client = app_endpoint_client(&app, EndpointCapability::TextDirectory)?;
    let mut result = client.get_chapter_catalog(novel_id).await?;
    let book_directory = library_directory(&app)?;
    let mut downloaded_by_title = std::collections::HashSet::new();
    if result.is_empty() {
        return Err("该作品没有可用的小说章节".to_string());
    }
    // 通过章节编号关联书库安全元数据文件中的本地完成状态。
    for entry in fs::read_dir(&book_directory)
        .into_iter()
        .flatten()
        .flatten()
    {
        let path = entry.path().join(".novel-flow.json");
        if let Ok(metadata) = read_or_default::<StoredBookMetadata>(&path, "本地数据") {
            if let Some(ids) = metadata.downloaded_text_chapter_ids {
                downloaded_by_title.extend(ids);
            }
        }
    }
    for volume in &mut result {
        for chapter in &mut volume.chapters {
            chapter.downloaded = downloaded_by_title.contains(&chapter.chap_id);
        }
    }
    Ok(result)
}

/// 列出已认证有声章节，并标记已存在于应用自有书库的条目。
///
/// # 参数
/// * `app` - 用于访问原生会话和书库的应用句柄。
/// * `novel_id` - 正数 SF 作品编号。
///
/// # 错误
/// 没有原生会话或 SF 拒绝有声请求时返回错误。
#[tauri::command]
pub(crate) async fn get_audio_chapters(
    app: tauri::AppHandle,
    novel_id: i64,
) -> Result<AudioCatalog, String> {
    #[cfg(target_os = "android")]
    sync_android_auth_session(&app).await?;
    let client = web_endpoint_client(&app, EndpointCapability::Audio)?;
    let (title, chapters) = client
        .get_audio_catalog(novel_id)
        .await
        .map_err(|error| EndpointCapability::Audio.unavailable_message(&error))?;
    let book_directory = library_directory(&app)?.join(safe_library_name(&title));
    let metadata = read_or_default::<StoredBookMetadata>(
        &book_directory.join(".novel-flow.json"),
        "本地数据",
    )?;
    let downloaded = metadata
        .downloaded_audio_chapter_ids
        .unwrap_or_default()
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    Ok(AudioCatalog {
        title,
        chapters: chapters
            .into_iter()
            .map(|chapter| AudioChapterSummary {
                downloaded: downloaded.contains(&chapter.id),
                id: chapter.id,
                title: chapter.title,
                volume: chapter.volume,
                is_vip: chapter.is_vip,
                is_unlocked: chapter.is_unlocked,
            })
            .collect(),
    })
}

/// 列出漫画章节并合并本地完成状态。
///
/// VIP 拥有权仅从已认证目录中的明确标记读取，单独的登录 Cookie 绝不视为章节拥有权。
#[tauri::command]
pub(crate) async fn get_comic_chapters(
    app: tauri::AppHandle,
    comic_id: i64,
    source_path: Option<String>,
    title_hint: Option<String>,
) -> Result<ComicCatalog, String> {
    validate_novel_id(comic_id)?;
    #[cfg(target_os = "android")]
    sync_android_auth_session(&app).await?;
    let web_client = web_endpoint_client(&app, EndpointCapability::ComicCatalog)?;
    let (title, folder) = if let Some(folder) = source_path.filter(|value| {
        !value.is_empty()
            && value.len() <= 100
            && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
    }) {
        (
            title_hint.unwrap_or_else(|| format!("漫画 {comic_id}")),
            folder,
        )
    } else {
        let app_client = app_endpoint_client(&app, EndpointCapability::ComicIdentity)?;
        app_client.comic_identity(comic_id).await?
    };
    let chapters = web_client
        .get_comic_catalog(&folder)
        .await
        .map_err(|error| EndpointCapability::ComicCatalog.unavailable_message(&error))?;
    let book_directory = library_directory(&app)?.join(safe_library_name(&title));
    let metadata = read_or_default::<StoredBookMetadata>(
        &book_directory.join(".novel-flow.json"),
        "本地数据",
    )?;
    let downloaded = metadata
        .downloaded_comic_chapter_ids
        .unwrap_or_default()
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    Ok(ComicCatalog {
        title,
        chapters: chapters
            .into_iter()
            .map(|chapter| ComicChapterSummary {
                id: chapter.id,
                title: chapter.title,
                is_vip: chapter.is_vip,
                is_unlocked: chapter.is_unlocked,
                downloaded: downloaded.contains(&chapter.id),
            })
            .collect(),
    })
}

/// 通过匿名 App API 读取公开漫画详情。
///
/// # 参数
/// * `app` - 用于构造原生 App 客户端的 Tauri 应用句柄。
/// * `comic_id` - 书架条目携带的公开漫画编号。
///
/// # 错误
/// 当漫画编号无效、匿名 App API 不可用或响应格式无效时返回错误。
#[tauri::command]
pub(crate) async fn get_comic_details(
    app: tauri::AppHandle,
    comic_id: i64,
) -> Result<NovelDetail, String> {
    validate_novel_id(comic_id)?;
    let client = app_endpoint_client(&app, EndpointCapability::ComicIdentity)?;
    client.get_comic_details(comic_id).await
}

/// 通过原生请求层同步已登录用户的 SF 书架。
///
/// 为保持旧渲染层路由的命令契约而保留 `force_refresh` 参数。当前原生实现每次调用均发起
/// 新请求，且刻意不在 Web 存储中持久化用户专属数据。
///
/// # 参数
/// * `app` - 用于读取仅原生层可见会话的 Tauri 应用句柄。
/// * `force_refresh` - 调用方绕过缓存的意图，仅为维持稳定命令 API 而保留。
///
/// # 错误
/// 没有已认证原生会话或 SF 返回无效书架时返回错误。
#[tauri::command]
pub(crate) async fn get_bookshelf(
    app: tauri::AppHandle,
    force_refresh: Option<bool>,
) -> Result<BookshelfCollection, String> {
    let _ = force_refresh;
    #[cfg(target_os = "android")]
    sync_android_auth_session(&app).await?;
    let client = web_endpoint_client(&app, EndpointCapability::WebBookshelf)?;
    let public_shelf = client.get_public_bookshelf().await?;
    let categories = public_shelf.categories;
    let mut items = Vec::new();
    for (category, record) in public_shelf.items {
        if !items.iter().any(|item: &SearchNovel| {
            item.novel_id == record.id && item.bookshelf_type.as_deref() == Some(record.kind)
        }) {
            items.push(SearchNovel {
                novel_id: record.id,
                media_id: None,
                novel_name: record.title,
                author_name: record.author,
                novel_cover: record.cover,
                last_update_time: String::new(),
                bookshelf_name: Some(category),
                bookshelf_type: Some(record.kind.to_string()),
                source_path: record.source_path,
            });
        }
    }
    Ok(BookshelfCollection { categories, items })
}

/// 返回当前活动原生 SF 会话的渲染进程安全状态。
///
/// Android 会从应用自有 `CookieManager` 刷新状态；实际 Cookie 始终留在 Kotlin 和 Rust
/// 原生层。
///
/// # 参数
/// * `app` - 用于访问原生认证状态的 Tauri 应用句柄。
///
/// # 错误
/// Android Cookie 同步或原生状态访问失败时返回错误。
#[tauri::command]
pub(crate) async fn auth_status(app: tauri::AppHandle) -> Result<AuthStatus, String> {
    #[cfg(target_os = "android")]
    sync_android_auth_session(&app).await?;
    current_auth_status(&app)
}

/// 使用直接提交到原生 SF 请求层的凭据登录。
///
/// 密码仅用于本次上游请求，不会被此命令记录、返回或持久化。
///
/// # 参数
/// * `app` - 用于更新原生会话状态的 Tauri 应用句柄。
/// * `username` - 用户输入的 SF 账户名。
/// * `password` - 用户输入的 SF 密码。
///
/// # 错误
/// 输入无效、凭据被拒绝或原生状态不可用时，返回已脱敏的上游错误。
#[tauri::command]
pub(crate) async fn login_with_password(
    app: tauri::AppHandle,
    username: String,
    password: String,
) -> Result<AuthStatus, String> {
    let username = username.trim();
    if username.is_empty() || password.is_empty() {
        return Err("请输入 SF 账号和密码".to_string());
    }
    if username.chars().count() > 128 || password.chars().count() > 512 {
        return Err("账号或密码长度无效".to_string());
    }
    let client = app_endpoint_client(&app, EndpointCapability::AppPasswordLogin)?;
    let cookie = client.login_with_password(username, &password).await?;
    #[cfg(target_os = "android")]
    persist_android_auth_session(&app, &cookie).await?;
    #[cfg(target_os = "windows")]
    persist_desktop_auth_session(&app, &cookie, username)?;
    {
        let auth_state = app.state::<AuthSessionState>();
        let mut session = auth_state
            .session
            .lock()
            .map_err(|_| "登录会话状态不可用".to_string())?;
        let existing_web_cookie = session.as_ref().and_then(|value| value.web_cookie.clone());
        *session = Some(NativeAuthSession {
            app_cookie: Some(cookie),
            web_cookie: existing_web_cookie,
            user_name: username.to_string(),
        });
    }

    // 设备上报是登录后的可选诊断；端点不存在、受限或与 App API 授权无关时，不能把
    // 已成功的凭据登录变为失败。
    let report_enabled = get_request_policy(app.clone())
        .map(|policy| policy.android_device_report_enabled)
        .unwrap_or(true);
    if report_enabled {
        if let Ok(authenticated_client) =
            app_endpoint_client(&app, EndpointCapability::AndroidDeviceReport)
        {
            if let Err(error) = authenticated_client
                .report_android_device_info_for_current_account()
                .await
            {
                eprintln!("[sfacg] optional device report skipped: {error}");
            }
        }
    } else {
        eprintln!("[sfacg] optional device report disabled by request policy");
    }
    current_auth_status(&app)
}

/// 打开平台自有 WebView 进行 SF 官方登录。
///
/// Android 使用专用登录 Activity。Windows 创建独立 Tauri WebView2 窗口并轮询其仅限 SF
/// 的 Cookie 存储。检测到有效会话后命令结束，原始 Cookie 永不进入渲染进程。
///
/// # 参数
/// * `app` - 用于打开平台登录界面的 Tauri 应用句柄。
///
/// # 错误
/// 平台不支持、无法打开登录界面或未得到有效会话时返回明确的原生错误。
#[tauri::command]
pub(crate) async fn start_official_login(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        app.state::<AndroidSfacgAuth<tauri::Wry>>()
            .mobile_plugin_handle
            .run_mobile_plugin_async::<Value>("startOfficialLogin", ())
            .await
            .map_err(|error| format!("无法打开 Android 官方登录页：{error}"))?;
        sync_android_auth_session(&app).await?;
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        let login_url: tauri::Url = OFFICIAL_LOGIN_URL
            .parse()
            .map_err(|error| format!("官方登录地址无效：{error}"))?;
        let login_window = if let Some(window) = app.get_webview_window(OFFICIAL_LOGIN_WINDOW_LABEL)
        {
            window
                .set_focus()
                .map_err(|error| format!("无法激活官方登录窗口：{error}"))?;
            window
        } else {
            WebviewWindowBuilder::new(
                &app,
                OFFICIAL_LOGIN_WINDOW_LABEL,
                WebviewUrl::External(login_url.clone()),
            )
            .title("SF 官方登录")
            .inner_size(480.0, 760.0)
            .min_inner_size(360.0, 560.0)
            .resizable(true)
            .center()
            .build()
            .map_err(|error| format!("无法创建官方登录窗口：{error}"))?
        };

        // 此专用 WebView 可能保留旧网站会话。每次尝试登录前都清理，避免旧 Cookie 在用户
        // 再次认证前就结束轮询。
        login_window
            .clear_all_browsing_data()
            .map_err(|error| format!("无法清除官方登录状态：{error}"))?;
        login_window
            .navigate(login_url)
            .map_err(|error| format!("无法打开官方登录页：{error}"))?;

        let cookie_urls = [
            "https://book.sfacg.com/",
            "https://api.sfacg.com/",
            OFFICIAL_LOGIN_URL,
        ];
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15 * 60);
        loop {
            if std::time::Instant::now() >= deadline {
                let _ = login_window.close();
                return Err("官方登录等待超时，请重新打开登录窗口".to_string());
            }

            let mut session_cookie = None;
            for cookie_url in cookie_urls {
                let Some(window) = app.get_webview_window(OFFICIAL_LOGIN_WINDOW_LABEL) else {
                    return Err("官方登录窗口已关闭，未检测到登录会话".to_string());
                };
                let url = cookie_url
                    .parse()
                    .map_err(|error| format!("官方 Cookie 地址无效：{error}"))?;
                let cookies = tokio::task::spawn_blocking(move || window.cookies_for_url(url))
                    .await
                    .map_err(|error| format!("读取官方登录 Cookie 失败：{error}"))?
                    .map_err(|error| format!("读取官方登录 Cookie 失败：{error}"))?;
                session_cookie = merge_sfacg_session_cookie(session_cookie, cookies);
            }

            if let Some(cookie) = session_cookie.filter(|value| {
                value
                    .split(';')
                    .any(|part| part.trim().starts_with(".SFCommunity="))
            }) {
                let cookie_names = cookie
                    .split(';')
                    .filter_map(|part| part.trim().split_once('=').map(|(name, _)| name))
                    .collect::<Vec<_>>()
                    .join(",");
                eprintln!("[sfacg] official login cookies captured: {cookie_names}");
                let auth_state = app.state::<AuthSessionState>();
                #[cfg(target_os = "windows")]
                persist_desktop_web_session(&app, &cookie)?;
                let mut session = auth_state
                    .session
                    .lock()
                    .map_err(|_| "登录会话状态不可用".to_string())?;
                let existing_app_cookie =
                    session.as_ref().and_then(|value| value.app_cookie.clone());
                let user_name = session
                    .as_ref()
                    .map(|value| value.user_name.clone())
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| "已登录 SF 账号".to_string());
                *session = Some(NativeAuthSession {
                    app_cookie: existing_app_cookie,
                    web_cookie: Some(cookie),
                    user_name,
                });
                drop(session);
                let _ = login_window.close();
                return Ok(());
            }

            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }
    #[cfg(all(not(target_os = "android"), not(target_os = "windows")))]
    {
        let _ = app;
        Err("官方网页登录目前仅支持 Windows 和 Android".to_string())
    }
}

/// 合并已识别的 SF 认证 Cookie，不暴露无关 Cookie。
///
/// 返回请求头仅保留在原生内存，绝不序列化到渲染进程状态、日志或持久化任务快照。
///
/// # 参数
/// * `current` - 从先前 SF 域名累积的 Cookie 请求头。
/// * `cookies` - 一次 Tauri WebView URL 查询返回的 Cookie。
///
/// # 返回值
/// 存在主要 `.SFCommunity` 会话 Cookie 时返回去重请求头；否则返回已累积的可选请求头。
#[cfg(target_os = "windows")]
fn merge_sfacg_session_cookie(
    current: Option<String>,
    cookies: Vec<tauri::webview::Cookie<'static>>,
) -> Option<String> {
    let mut cookie_pairs = std::collections::BTreeMap::new();
    if let Some(header) = current {
        for part in header.split(';') {
            let pair = part.trim();
            let Some(separator) = pair.find('=') else {
                continue;
            };
            cookie_pairs.insert(
                pair[..separator].to_string(),
                pair[separator + 1..].to_string(),
            );
        }
    }
    for cookie in cookies {
        let name = cookie.name();
        if name == ".SFCommunity" || name == "session_PC" {
            cookie_pairs.insert(name.to_string(), cookie.value().to_string());
        }
    }
    if !cookie_pairs.contains_key(".SFCommunity") {
        return if cookie_pairs.is_empty() {
            None
        } else {
            Some(
                cookie_pairs
                    .into_iter()
                    .map(|(name, value)| format!("{name}={value}"))
                    .collect::<Vec<_>>()
                    .join("; "),
            )
        };
    }
    Some(
        cookie_pairs
            .into_iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join("; "),
    )
}

/// 清除原生 SF 会话和 Android 应用 WebView 会话 Cookie。
///
/// # 参数
/// * `app` - 用于访问原生状态和平台 API 的 Tauri 应用句柄。
///
/// # 错误
/// Android 拒绝清除应用自有 Cookie 存储时返回错误。
#[tauri::command]
pub(crate) async fn logout(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "android")]
    app.state::<AndroidSfacgAuth<tauri::Wry>>()
        .mobile_plugin_handle
        .run_mobile_plugin_async::<Value>("clearSessionCookie", ())
        .await
        .map_err(|error| format!("无法清除 Android 登录会话：{error}"))?;
    let auth_state = app.state::<AuthSessionState>();
    let mut session = auth_state
        .session
        .lock()
        .map_err(|_| "登录会话状态不可用".to_string())?;
    *session = None;
    #[cfg(target_os = "windows")]
    {
        clear_desktop_auth_session(&app)?;
        clear_desktop_web_session(&app)?;
    }
    Ok(())
}

/// 仅清除原生 App API 会话，保留已有网站会话。
///
/// # 错误
/// 平台专属 App 会话存储无法清除或原生状态不可用时返回错误。
#[tauri::command]
pub(crate) async fn logout_app_session(app: tauri::AppHandle) -> Result<AuthStatus, String> {
    #[cfg(target_os = "android")]
    app.state::<AndroidSfacgAuth<tauri::Wry>>()
        .mobile_plugin_handle
        .run_mobile_plugin_async::<Value>("clearAppSessionCookie", ())
        .await
        .map_err(|error| format!("无法清除 Android App 登录会话：{error}"))?;
    #[cfg(target_os = "windows")]
    clear_desktop_auth_session(&app)?;
    let auth_state = app.state::<AuthSessionState>();
    let mut session = auth_state
        .session
        .lock()
        .map_err(|_| "登录会话状态不可用".to_string())?;
    if let Some(current) = session.as_mut() {
        current.app_cookie = None;
        if current.web_cookie.is_some() {
            current.user_name = "已登录 SF 账号".to_string();
        }
    }
    if session
        .as_ref()
        .is_some_and(|current| current.web_cookie.is_none())
    {
        *session = None;
    }
    drop(session);
    current_auth_status(&app)
}

/// 仅清除官方网页会话，保留已有 App API 会话。
///
/// # 错误
/// 平台专属网站会话存储无法清除或原生状态不可用时返回错误。
#[tauri::command]
pub(crate) async fn logout_web_session(app: tauri::AppHandle) -> Result<AuthStatus, String> {
    #[cfg(target_os = "android")]
    app.state::<AndroidSfacgAuth<tauri::Wry>>()
        .mobile_plugin_handle
        .run_mobile_plugin_async::<Value>("clearWebSessionCookie", ())
        .await
        .map_err(|error| format!("无法清除 Android 网站登录会话：{error}"))?;
    #[cfg(target_os = "windows")]
    clear_desktop_web_session(&app)?;
    let auth_state = app.state::<AuthSessionState>();
    let mut session = auth_state
        .session
        .lock()
        .map_err(|_| "登录会话状态不可用".to_string())?;
    if let Some(current) = session.as_mut() {
        current.web_cookie = None;
    }
    if session
        .as_ref()
        .is_some_and(|current| current.app_cookie.is_none())
    {
        *session = None;
    }
    drop(session);
    current_auth_status(&app)
}

/// 验证当前原生会话能否用于需要认证的 SF 请求。
///
/// 此命令刻意保持渲染进程安全：绝不返回 Cookie 或上游账户载荷，仅返回当前认证状态。
///
/// # 参数
/// * `app` - 用于读取仅原生层可见会话数据的 Tauri 应用句柄。
///
/// # 错误
/// 没有可用的已认证原生会话时返回错误。
#[tauri::command]
pub(crate) async fn verify_authenticated_request(
    app: tauri::AppHandle,
) -> Result<AuthStatus, String> {
    let client = app_endpoint_client(&app, EndpointCapability::AccountProfile)?;
    client.verify_authenticated_session().await?;
    current_auth_status(&app)
}

/// 通过原生请求层读取已认证 SF 账户资料。
///
/// # 参数
/// * `app` - 用于读取仅原生层可见会话的 Tauri 应用句柄。
///
/// # 错误
/// 会话不可用或上游资料数据无效时返回错误。
#[tauri::command]
pub(crate) async fn get_user_profile(app: tauri::AppHandle) -> Result<UserProfile, String> {
    #[cfg(target_os = "android")]
    sync_android_auth_session(&app).await?;
    let client = app_endpoint_client(&app, EndpointCapability::AccountProfile)?;
    let profile = client.get_user_profile().await?;
    let auth_state = app.state::<AuthSessionState>();
    let mut session = auth_state
        .session
        .lock()
        .map_err(|_| "登录会话状态不可用".to_string())?;
    if let Some(session) = session.as_mut() {
        session.user_name = profile.nick_name.clone();
    }
    Ok(profile)
}

/// 通过官方网页登录信息接口验证 Web 会话并读取基础账户资料。
///
/// 此命令只使用 `session_PC`，从网页账户中心读取火券、代券和月票，并通过网页 VIP
/// 接口读取新 VIP 资料。调用方不得将 Web Cookie 用于 App 接口。
///
/// # 参数
/// * `app` - 用于读取仅原生层可见 Web 会话的应用句柄。
///
/// # 错误
/// 未连接 Web 凭证、网页登录会话失效或上游资料格式无效时返回错误。
#[tauri::command]
pub(crate) async fn get_web_user_profile(app: tauri::AppHandle) -> Result<UserProfile, String> {
    #[cfg(target_os = "android")]
    sync_android_auth_session(&app).await?;
    let client = web_endpoint_client(&app, EndpointCapability::WebAccountProfile)?;
    let profile = client.get_user_profile().await?;
    let auth_state = app.state::<AuthSessionState>();
    let mut session = auth_state
        .session
        .lock()
        .map_err(|_| "登录会话状态不可用".to_string())?;
    if let Some(session) = session.as_mut() {
        session.user_name = profile.nick_name.clone();
    }
    Ok(profile)
}
