//! SFACG 远程查询、认证和原生会话生命周期。
//!
//! 本模块仅处理上游 SFACG 协议、平台认证桥接与会话持久化。网络客户端的协议
//! 细节分别封装在 [`crate::app_client`] 与 [`crate::web_client`]，离线书库和下载
//! 任务由相邻业务模块负责。

use crate::endpoint_policy::{app_endpoint_client, web_endpoint_client, EndpointCapability};
use crate::library::{
    get_request_policy, library_directory, read_json_or_default, safe_library_name,
    StoredBookMetadata,
};
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use uuid::Uuid;

#[cfg(target_os = "android")]
use tauri::plugin::{PluginHandle, TauriPlugin};

/// SF App API 的固定服务根地址。
pub(crate) const SF_API_HOST: &str = "https://api.sfacg.com";
/// App API HTTP 基础认证使用的公开客户端标识。
pub(crate) const SF_API_USER: &str = "androiduser";
/// App API HTTP 基础认证的协议常量。
pub(crate) const SF_API_PASSWORD: &str = "1a#$51-yt69;*Acv@qxq";
/// 生成 `SFSecurity` 请求头所需的协议盐值。
const SF_SECURITY_SALT: &str = "FN_Q29XHVmfV3mYX";
/// 首次创建用户正文恢复字典时使用的内置默认映射。
pub(crate) const DEFAULT_CONTENT_DICTIONARY: &str =
    include_str!("../../sfacg-content-dictionary.json");
/// 官方网页登录页面地址。
const OFFICIAL_LOGIN_URL: &str = "https://passport.sfacg.com/";
/// 官方登录窗口的稳定 Tauri 标签。
const OFFICIAL_LOGIN_WINDOW_LABEL: &str = "sfacg-official-login";
/// Windows 上模拟官方网页访问时使用的浏览器标识。
#[cfg(target_os = "windows")]
pub(crate) const SF_WEB_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";
/// Android 上模拟官方网页访问时使用的浏览器标识。
#[cfg(target_os = "android")]
pub(crate) const SF_WEB_USER_AGENT: &str = "Mozilla/5.0 (Linux; Android 15; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Mobile Safari/537.36";
/// 其他桌面平台访问 SF 网页时使用的浏览器标识。
#[cfg(all(not(target_os = "windows"), not(target_os = "android")))]
pub(crate) const SF_WEB_USER_AGENT: &str =
    "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";
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
    // Ok("910D166A-736E-3231-8B21-8D12DFD75F16")
}

/// A public novel item returned to the renderer by the search command.
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

/// Detailed public novel metadata used by the chapter selection view.
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

/// A renderer-safe chapter collection for one SF novel volume.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChapterVolume {
    pub(crate) volume_id: i64,
    pub(crate) title: String,
    pub(crate) chapters: Vec<ChapterSummary>,
}

/// A renderer-safe chapter availability record.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChapterSummary {
    pub(crate) chap_id: i64,
    pub(crate) title: String,
    pub(crate) need_fire_money: i64,
    pub(crate) is_vip: bool,
    pub(crate) is_unlocked: bool,
    pub(crate) downloaded: bool,
}

/// The authenticated SF bookshelf returned to the renderer without session data.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct BookshelfCollection {
    pub(crate) categories: Vec<String>,
    pub(crate) items: Vec<SearchNovel>,
}

/// Renderer-compatible native download task state.
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

/// Shared native task registry and cancellation flags.
#[derive(Default)]
pub(crate) struct NativeJobState {
    pub(crate) jobs: Mutex<std::collections::HashMap<String, NativeJob>>,
    pub(crate) cancellations:
        Mutex<std::collections::HashMap<String, Arc<std::sync::atomic::AtomicBool>>>,
    pub(crate) specs: Mutex<std::collections::HashMap<String, NativeJobSpec>>,
}

/// Private inputs required to resume a native download task.
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

/// Persisted task data that permits user-controlled resumption after restart.
#[derive(Default, Deserialize, Serialize)]
pub(crate) struct PersistedNativeJobs {
    pub(crate) jobs: std::collections::HashMap<String, NativeJob>,
    pub(crate) specs: std::collections::HashMap<String, NativeJobSpec>,
}

/// Native-only audio chapter record, including the upstream stream URL.
pub(crate) struct NativeAudioChapter {
    pub(crate) id: i64,
    pub(crate) title: String,
    pub(crate) volume: String,
    pub(crate) is_vip: bool,
    pub(crate) is_unlocked: bool,
    pub(crate) source: String,
}

/// Renderer-safe audio catalog for a single SF work.
#[derive(Debug, Serialize)]
pub(crate) struct AudioCatalog {
    pub(crate) title: String,
    pub(crate) chapters: Vec<AudioChapterSummary>,
}

/// Renderer-safe audio chapter row without the protected upstream stream URL.
#[derive(Debug, Serialize)]
pub(crate) struct AudioChapterSummary {
    pub(crate) id: i64,
    pub(crate) title: String,
    pub(crate) volume: String,
    pub(crate) is_vip: bool,
    pub(crate) is_unlocked: bool,
    pub(crate) downloaded: bool,
}

/// The renderer-safe authentication state for the current native session.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthStatus {
    pub(crate) authenticated: bool,
    pub(crate) app_authenticated: bool,
    pub(crate) web_authenticated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) user_name: Option<String>,
}

/// A renderer-safe snapshot of the authenticated SF account profile.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UserProfile {
    pub(crate) account_id: i64,
    pub(crate) nick_name: String,
    pub(crate) avatar: String,
    pub(crate) welfare_coin: i64,
    pub(crate) fire_money_remain: i64,
    pub(crate) coupons_remain: i64,
    pub(crate) vip_level: i64,
}

/// Holds independently scoped SF sessions in native memory.
#[derive(Default)]
pub(crate) struct AuthSessionState {
    pub(crate) session: Mutex<Option<NativeAuthSession>>,
}

/// App and Web cookies are intentionally separate and cannot be converted.
pub(crate) struct NativeAuthSession {
    pub(crate) app_cookie: Option<String>,
    pub(crate) web_cookie: Option<String>,
    pub(crate) user_name: String,
}

/// Native-only comic chapter record. The public comic site supplies chapter
/// folders only on its web endpoint, so these values never leave the native
/// download boundary except for renderer-safe availability metadata.
pub(crate) struct NativeComicChapter {
    pub(crate) id: i64,
    pub(crate) title: String,
    pub(crate) is_vip: bool,
    pub(crate) is_unlocked: bool,
}

/// Renderer-safe comic catalog used by the chapter picker.
#[derive(Debug, Serialize)]
pub(crate) struct ComicCatalog {
    pub(crate) title: String,
    pub(crate) chapters: Vec<ComicChapterSummary>,
}

/// One comic chapter without page URLs or source folder information.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ComicChapterSummary {
    pub(crate) id: i64,
    pub(crate) title: String,
    pub(crate) is_vip: bool,
    pub(crate) is_unlocked: bool,
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

/// 保存只供 Rust 命令调用的 Android 原生认证插件句柄。
#[cfg(target_os = "android")]
struct AndroidSfacgAuth<R: tauri::Runtime> {
    mobile_plugin_handle: PluginHandle<R>,
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

/// Converts the native-only session state to renderer-safe login metadata.
///
/// # Arguments
/// * `app` - Application handle used to access the native session state.
///
/// # Errors
/// Returns an error if the session lock has been poisoned by a prior native panic.
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

/// API chapter content and the identifiers needed for web comparison.
pub(crate) struct ApiChapterContent {
    pub(crate) content: String,
    pub(crate) novel_id: i64,
    pub(crate) volume_id: i64,
}

/// Builds the SF `SFSecurity` header without logging any credential material.
///
/// # Arguments
/// * `nonce` - Uppercase UUID generated for this request.
///
/// # Errors
/// Returns an error if the system clock or device identity is unavailable.
pub(crate) fn security_header(nonce: &str) -> Result<String, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "系统时间无效".to_string())?
        .as_secs();
    let device_token = device_token()?.to_uppercase();
    let data = format!("{nonce}{timestamp}{device_token}{SF_SECURITY_SALT}");
    let digest = Md5::digest(data.as_bytes());
    Ok(format!(
        "nonce={nonce}&timestamp={timestamp}&devicetoken={device_token}&sign={:X}",
        digest
    ))
}

/// Extracts one numeric variable from the inline script emitted by the comic
/// chapter page. The values originate from a fixed SF page, never from the
/// renderer, and are used solely to call the matching image endpoint.
pub(crate) fn extract_js_number(html: &str, variable: &str) -> Option<i64> {
    let marker = format!("var {variable} =");
    let value = html.split(&marker).nth(1)?.split(';').next()?.trim();
    value.parse().ok()
}

/// Extracts one quoted inline JavaScript variable from a comic chapter page.
pub(crate) fn extract_js_string(html: &str, variable: &str) -> Option<String> {
    let marker = format!("var {variable} =");
    let value = html.split(&marker).nth(1)?.split(';').next()?.trim();
    value
        .strip_prefix('"')?
        .strip_suffix('"')
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

/// Parses the chapter anchors exposed by a public SF comic work page.
pub(crate) fn parse_comic_catalog(
    html: &str,
    folder: &str,
) -> Result<Vec<NativeComicChapter>, String> {
    let marker = format!("href=\"/mh/{folder}/");
    let mut remaining = html;
    let mut chapters = Vec::new();
    let mut seen = std::collections::HashSet::new();
    while let Some(position) = remaining.find(&marker) {
        remaining = &remaining[position + marker.len()..];
        let Some(id_end) = remaining.find('/') else {
            break;
        };
        let Ok(id) = remaining[..id_end].parse::<i64>() else {
            continue;
        };
        let Some(anchor_end) = remaining[id_end..].find("</a>") else {
            continue;
        };
        let anchor = &remaining[id_end..id_end + anchor_end];
        let Some(title_start) = anchor.find('>') else {
            continue;
        };
        let title = strip_html_tags(anchor[title_start + 1..].trim());
        if title.is_empty() || title == "点击浏览" || !seen.insert(id) {
            continue;
        }
        let is_vip = title.starts_with("VIP");
        let anchor_markup = &remaining[id_end..id_end + anchor_end];
        let is_unlocked = !is_vip || comic_anchor_is_unlocked(anchor_markup);
        chapters.push(NativeComicChapter {
            id,
            title,
            is_vip,
            is_unlocked,
        });
    }
    if chapters.is_empty() {
        return Err("SF 漫画目录格式无效".to_string());
    }
    chapters.reverse();
    Ok(chapters)
}

/// Reads explicit ownership markers emitted by the authenticated comic catalog.
/// Login presence alone is not an entitlement signal: an account may be logged
/// in without owning a particular VIP chapter.
fn comic_anchor_is_unlocked(anchor: &str) -> bool {
    let normalized = anchor.to_ascii_lowercase();
    [
        "data-has=\"true\"",
        "data-has=true",
        "data-has='true'",
        "data-isunlocked=\"true\"",
        "data-isunlocked=true",
        "data-isunlocked='true'",
    ]
    .iter()
    .any(|marker| normalized.contains(&marker.to_ascii_lowercase()))
}

/// Removes simple HTML tags from an anchor title without treating response
/// text as executable markup.
fn strip_html_tags(value: &str) -> String {
    let mut output = String::new();
    let mut in_tag = false;
    for character in value.chars() {
        match character {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => output.push(character),
            _ => {}
        }
    }
    output.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Searches public SF novels and converts upstream records to renderer DTOs.
///
/// # Arguments
/// * `query` - A trimmed user search term, limited to 100 Unicode scalar values.
///
/// # Errors
/// Returns an error for empty/oversized input or an unavailable upstream service.
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
    let response = client
        .get_public_data(
            "/search/novels/result/new",
            &[
                ("page", "0".to_string()),
                ("q", query),
                ("size", "12".to_string()),
                ("sort", "hot".to_string()),
                ("searchType", "0".to_string()),
            ],
        )
        .await?;
    let mut results = Vec::new();
    for (key, kind) in [
        ("novels", "novel"),
        ("albums", "audio"),
        ("comics", "comic"),
    ] {
        let Some(items) = response.get(key).and_then(Value::as_array) else {
            continue;
        };
        for item in items {
            let id = item
                .get("novelId")
                .or_else(|| item.get("comicId"))
                .and_then(Value::as_i64);
            let Some(novel_id) = id else { continue };
            let name = item
                .get("novelName")
                .or_else(|| item.get("comicName"))
                .or_else(|| item.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("未命名作品")
                .to_string();
            results.push(SearchNovel {
                novel_id,
                media_id: if kind == "audio" {
                    item.get("albumId").and_then(Value::as_i64)
                } else if kind == "comic" {
                    item.get("comicId").and_then(Value::as_i64)
                } else {
                    None
                },
                novel_name: name,
                author_name: item
                    .get("authorName")
                    .and_then(Value::as_str)
                    .unwrap_or("未知作者")
                    .to_string(),
                novel_cover: item
                    .get("novelCover")
                    .or_else(|| item.get("coverBig"))
                    .or_else(|| item.get("comicCover"))
                    .or_else(|| item.get("coverMedium"))
                    .or_else(|| item.get("coverSmall"))
                    .or_else(|| item.get("cover"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                last_update_time: item
                    .get("lastUpdateTime")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                bookshelf_name: None,
                bookshelf_type: Some(kind.to_string()),
                source_path: (kind == "comic")
                    .then(|| {
                        item.get("folderName")
                            .or_else(|| item.get("sourcePath"))
                            .and_then(Value::as_str)
                            .filter(|value| {
                                !value.is_empty()
                                    && value.len() <= 100
                                    && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
                            })
                            .map(str::to_string)
                    })
                    .flatten(),
            });
        }
    }
    Ok(results)
}

/// Validates a public SF novel identifier supplied by the renderer.
///
/// # Arguments
/// * `novel_id` - Candidate positive SF novel identifier.
///
/// # Errors
/// Returns an error for zero, negative, or otherwise invalid identifiers.
pub(crate) fn validate_novel_id(novel_id: i64) -> Result<(), String> {
    if novel_id <= 0 {
        return Err("小说编号无效".to_string());
    }
    Ok(())
}

/// Reads a public novel's details through the fixed native SF API adapter.
///
/// # Arguments
/// * `novel_id` - Positive SF novel identifier.
///
/// # Errors
/// Returns an error for invalid identifiers or unavailable/malformed upstream metadata.
#[tauri::command]
pub(crate) async fn get_novel_details(
    app: tauri::AppHandle,
    novel_id: i64,
) -> Result<NovelDetail, String> {
    validate_novel_id(novel_id)?;
    let client = app_endpoint_client(&app, EndpointCapability::NovelDetail)?;
    let detail = client
        .get_public_data(
            &format!("/novels/{novel_id}"),
            &[(
                "expand",
                "chapterCount,bigBgBanner,bigNovelCover,typeName,intro,fav,ticket,pointCount,sysTags,totalNeedFireMoney,latestchapter".to_string(),
            )],
        )
        .await?;
    let resolved_id = detail
        .get("novelId")
        .and_then(Value::as_i64)
        .ok_or_else(|| "SF 未返回有效小说信息".to_string())?;
    Ok(NovelDetail {
        novel_id: resolved_id,
        novel_name: detail
            .get("novelName")
            .and_then(Value::as_str)
            .unwrap_or("未命名作品")
            .to_string(),
        author_name: detail
            .get("authorName")
            .and_then(Value::as_str)
            .unwrap_or("未知作者")
            .to_string(),
        novel_cover: detail
            .get("novelCover")
            .and_then(Value::as_str)
            .or_else(|| {
                detail
                    .get("expand")
                    .and_then(|value| value.get("bigNovelCover"))
                    .and_then(Value::as_str)
            })
            .unwrap_or_default()
            .to_string(),
        last_update_time: detail
            .get("lastUpdateTime")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        description: detail
            .get("expand")
            .and_then(|value| value.get("intro"))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("暂无简介")
            .to_string(),
        is_finish: detail
            .get("isFinish")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        type_name: detail
            .get("expand")
            .and_then(|value| value.get("typeName"))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string),
        tags: detail
            .get("expand")
            .and_then(|value| value.get("sysTags"))
            .and_then(Value::as_array)
            .map(|tags| {
                tags.iter()
                    .filter_map(|tag| {
                        tag.as_str()
                            .or_else(|| tag.get("tagName").and_then(Value::as_str))
                    })
                    .map(str::trim)
                    .filter(|tag| !tag.is_empty())
                    .map(ToString::to_string)
                    .take(8)
                    .collect()
            }),
        score: detail.get("point").and_then(Value::as_f64),
        chapter_count: detail
            .get("expand")
            .and_then(|value| value.get("chapterCount"))
            .and_then(Value::as_i64),
        character_count: detail.get("charCount").and_then(Value::as_i64),
        view_count: detail.get("viewTimes").and_then(Value::as_i64),
        mark_count: detail.get("markCount").and_then(Value::as_i64),
        point_count: detail
            .get("expand")
            .and_then(|value| value.get("pointCount"))
            .and_then(Value::as_i64),
        favorite_count: detail
            .get("expand")
            .and_then(|value| value.get("fav"))
            .and_then(Value::as_i64),
        ticket_count: detail
            .get("expand")
            .and_then(|value| value.get("ticket"))
            .and_then(Value::as_i64),
        latest_chapter_title: detail
            .get("expand")
            .and_then(|value| {
                value
                    .get("latestChapter")
                    .or_else(|| value.get("latestchapter"))
            })
            .and_then(|value| value.get("title"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        latest_chapter_time: detail
            .get("expand")
            .and_then(|value| {
                value
                    .get("latestChapter")
                    .or_else(|| value.get("latestchapter"))
            })
            .and_then(|value| value.get("addTime"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        source_path: None,
    })
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
    let tags = detail
        .get("sysTags")
        .or_else(|| detail.get("tags"))
        .or_else(|| {
            detail
                .get("expand")
                .and_then(|expand| expand.get("sysTags"))
        })
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(|tag| {
                    tag.as_str()
                        .or_else(|| tag.get("tagName").and_then(Value::as_str))
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToString::to_string)
                })
                .take(8)
                .collect()
        });
    let latest = detail.get("expand").and_then(|expand| {
        expand
            .get("latestChapter")
            .or_else(|| expand.get("latestchapter"))
    });
    Ok(NovelDetail {
        novel_id,
        novel_name: text(&["name", "albumName", "novelName"])
            .unwrap_or_else(|| "未命名有声作品".to_string()),
        author_name: text(&["authorName", "author", "anchorName"])
            .unwrap_or_else(|| "未知作者".to_string()),
        novel_cover: text(&[
            "coverBig",
            "coverMedium",
            "coverSmall",
            "albumCover",
            "cover",
        ])
        .unwrap_or_default(),
        last_update_time: text(&["lastUpdateTime", "updateTime"]).unwrap_or_default(),
        description: text(&["intro", "description", "content"])
            .unwrap_or_else(|| "暂无简介".to_string()),
        is_finish: detail
            .get("isFinished")
            .or_else(|| detail.get("isFinish"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        type_name: text(&["typeName", "categoryName"]),
        tags,
        score: detail.get("point").and_then(Value::as_f64),
        chapter_count: detail
            .get("chapterCount")
            .or_else(|| {
                detail
                    .get("expand")
                    .and_then(|expand| expand.get("chapterCount"))
            })
            .and_then(Value::as_i64),
        character_count: detail.get("charCount").and_then(Value::as_i64),
        view_count: detail
            .get("visitTimes")
            .or_else(|| detail.get("viewTimes"))
            .and_then(Value::as_i64),
        mark_count: detail.get("markCount").and_then(Value::as_i64),
        point_count: detail.get("pointCount").and_then(Value::as_i64),
        favorite_count: detail.get("fav").and_then(Value::as_i64),
        ticket_count: detail.get("ticket").and_then(Value::as_i64),
        latest_chapter_title: latest
            .and_then(|value| value.get("title"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        latest_chapter_time: latest
            .and_then(|value| value.get("addTime"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        source_path: None,
    })
}

/// Lists text chapters and their upstream entitlement metadata for a public novel.
///
/// The command deliberately reports `downloaded: false`; local download state is
/// owned by the native download service and will be joined when that service migrates.
///
/// # Arguments
/// * `novel_id` - Positive SF novel identifier.
///
/// # Errors
/// Returns an error for invalid identifiers or a malformed/unavailable SF directory.
#[tauri::command]
pub(crate) async fn get_chapter_volumes(
    app: tauri::AppHandle,
    novel_id: i64,
) -> Result<Vec<ChapterVolume>, String> {
    validate_novel_id(novel_id)?;
    let client = app_endpoint_client(&app, EndpointCapability::TextDirectory)?;
    let response = client
        .get_public_data(&format!("/novels/{novel_id}/dirs"), &[])
        .await?;
    let volumes = response
        .get("volumeList")
        .and_then(Value::as_array)
        .ok_or_else(|| "SF 未返回章节目录".to_string())?;
    let mut output = Vec::with_capacity(volumes.len());
    for volume in volumes {
        let volume_id = volume
            .get("volumeId")
            .and_then(Value::as_i64)
            .ok_or_else(|| "SF 章节卷编号无效".to_string())?;
        let chapters = volume
            .get("chapterList")
            .and_then(Value::as_array)
            .ok_or_else(|| "SF 章节卷格式无效".to_string())?
            .iter()
            .filter_map(|chapter| {
                let chap_id = chapter.get("chapId").and_then(Value::as_i64)?;
                let is_vip = chapter
                    .get("isVip")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let need_fire_money = chapter
                    .get("needFireMoney")
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                Some(ChapterSummary {
                    chap_id,
                    title: chapter
                        .get("ntitle")
                        .and_then(Value::as_str)
                        .unwrap_or("未命名章节")
                        .to_string(),
                    need_fire_money,
                    is_vip,
                    is_unlocked: chapter.get("has").and_then(Value::as_bool).unwrap_or(false)
                        || (is_vip && need_fire_money == 0),
                    downloaded: false,
                })
            })
            .collect();
        output.push(ChapterVolume {
            volume_id,
            title: volume
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("未命名分卷")
                .to_string(),
            chapters,
        });
    }
    Ok(output)
}

/// Lists authenticated audio chapters and marks entries already present in the
/// app-owned local library.
///
/// # Arguments
/// * `app` - Application handle used to access the native session and library.
/// * `novel_id` - Positive SF work identifier.
///
/// # Errors
/// Returns an error when no native session exists or SF rejects the audio request.
#[tauri::command]
pub(crate) async fn get_audio_chapters(
    app: tauri::AppHandle,
    novel_id: i64,
) -> Result<AudioCatalog, String> {
    #[cfg(target_os = "android")]
    sync_android_auth_session(&app).await?;
    let client = web_endpoint_client(&app, EndpointCapability::Audio)?;
    let (title, chapters) = client
        .audio_catalog(novel_id)
        .await
        .map_err(|error| EndpointCapability::Audio.unavailable_message(&error))?;
    let book_directory = library_directory(&app)?.join(safe_library_name(&title));
    let metadata =
        read_json_or_default::<StoredBookMetadata>(&book_directory.join(".novel-flow.json"))?;
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

/// Lists comic chapters and joins local completion state. VIP ownership is read
/// only from explicit markers in the authenticated catalog; a login cookie by
/// itself is never treated as chapter ownership.
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
        .comic_catalog(&folder)
        .await
        .map_err(|error| EndpointCapability::ComicCatalog.unavailable_message(&error))?;
    let book_directory = library_directory(&app)?.join(safe_library_name(&title));
    let metadata =
        read_json_or_default::<StoredBookMetadata>(&book_directory.join(".novel-flow.json"))?;
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

/// 通过漫画网页目录标识读取漫画详情；缺少目录标识时先解析公开漫画身份。
///
/// # 参数
/// * `app` - 用于读取原生 Web 会话的 Tauri 应用句柄。
/// * `comic_id` - 书架条目携带的公开漫画编号。
/// * `source_path` - 可选的 Web 漫画目录标识，例如 `XJYQS`。
///
/// # 错误
/// 当漫画身份、网页不可访问或页面格式无效时返回错误。
#[tauri::command]
pub(crate) async fn get_comic_details(
    app: tauri::AppHandle,
    comic_id: i64,
    source_path: Option<String>,
) -> Result<NovelDetail, String> {
    validate_novel_id(comic_id)?;
    let folder = source_path.filter(|value| {
        !value.is_empty()
            && value.len() <= 100
            && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
    });
    let folder = if let Some(folder) = folder {
        folder
    } else {
        let app_client = app_endpoint_client(&app, EndpointCapability::ComicIdentity)?;
        let (_, folder) = app_client.comic_identity(comic_id).await?;
        folder
    };
    let web_client = web_endpoint_client(&app, EndpointCapability::ComicDetail)?;
    let mut detail = web_client
        .comic_details(&folder)
        .await
        .map_err(|error| EndpointCapability::ComicDetail.unavailable_message(&error))?;
    detail.novel_id = comic_id;
    detail.source_path = Some(folder);
    Ok(detail)
}

/// Synchronizes the authenticated user's SF bookshelf through the native request layer.
///
/// The `force_refresh` parameter is accepted for API parity with the old renderer
/// route. Native migration currently performs a fresh request for every call and
/// deliberately does not persist user-specific data in web storage.
///
/// # Arguments
/// * `app` - Tauri application handle used to retrieve the native-only session.
/// * `force_refresh` - Caller intent to bypass cache; retained for a stable command API.
///
/// # Errors
/// Returns an error if no authenticated native session exists or SF returns an invalid shelf.
#[tauri::command]
pub(crate) async fn get_bookshelf(
    app: tauri::AppHandle,
    force_refresh: Option<bool>,
) -> Result<BookshelfCollection, String> {
    let _ = force_refresh;
    #[cfg(target_os = "android")]
    sync_android_auth_session(&app).await?;
    let client = web_endpoint_client(&app, EndpointCapability::WebBookshelf)?;
    let public_shelf = client.public_bookshelf().await?;
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

/// Returns renderer-safe state for the currently active native SF session.
///
/// On Android this refreshes from the application-owned `CookieManager`; the
/// actual cookie remains within Kotlin and Rust native layers.
///
/// # Arguments
/// * `app` - Tauri application handle used to access native authentication state.
///
/// # Errors
/// Returns an error if Android cookie synchronization or native state access fails.
#[tauri::command]
pub(crate) async fn auth_status(app: tauri::AppHandle) -> Result<AuthStatus, String> {
    #[cfg(target_os = "android")]
    sync_android_auth_session(&app).await?;
    current_auth_status(&app)
}

/// Signs in with credentials submitted directly to the native SF request layer.
///
/// Password text is used only for the upstream request and is neither logged,
/// returned, nor persisted by this command.
///
/// # Arguments
/// * `app` - Tauri application handle used to update native session state.
/// * `username` - User-entered SF account name.
/// * `password` - User-entered SF password.
///
/// # Errors
/// Returns a redacted upstream error for invalid input, rejected credentials, or
/// unavailable native state.
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

    // Device reporting is an optional post-login diagnostic. It must not turn
    // a successful credential login into a failure when the endpoint is absent,
    // restricted, or unrelated to App API authorization.
    let report_enabled = get_request_policy(app.clone())
        .map(|policy| policy.android_device_report_enabled)
        .unwrap_or(true);
    if report_enabled {
        if let Ok(authenticated_client) =
            app_endpoint_client(&app, EndpointCapability::AndroidDeviceReport)
        {
            match authenticated_client.get_data("/user", &[]).await {
                Ok(user) => {
                    if let Some(account_id) = user.get("accountId").and_then(Value::as_i64) {
                        if let Err(error) = authenticated_client
                            .report_android_device_info(account_id)
                            .await
                        {
                            eprintln!("[sfacg] optional device report rejected: {error}");
                        }
                    }
                }
                Err(error) => eprintln!("[sfacg] optional device report skipped: {error}"),
            }
        }
    } else {
        eprintln!("[sfacg] optional device report disabled by request policy");
    }
    current_auth_status(&app)
}

/// Opens the platform-owned WebView for official SF login.
///
/// Android uses its dedicated login activity. Windows creates a separate Tauri
/// WebView2 window and polls its SF-only cookie store. The command completes
/// after a valid session is detected; raw cookies never reach the renderer.
///
/// # Arguments
/// * `app` - Tauri application handle used to open the platform login surface.
///
/// # Errors
/// Returns an explicit unsupported-platform error or a native error when the
/// platform login surface cannot be opened or does not yield a valid session.
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

        // This dedicated WebView can retain a previous website session. Clear it
        // before every login attempt so an old cookie cannot complete the poll
        // loop before the user has a chance to authenticate again.
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

/// Merges recognized SF authentication cookies without exposing unrelated cookies.
///
/// The returned header is retained only in native memory and is never serialized
/// into renderer state, logs, or persisted task snapshots.
///
/// # Arguments
/// * `current` - The cookie header accumulated from earlier SF domains.
/// * `cookies` - Cookies returned by one Tauri WebView URL query.
///
/// # Returns
/// A de-duplicated header when the primary `.SFCommunity` session cookie exists;
/// otherwise the accumulated optional header.
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

/// Clears the native SF session and Android application WebView session cookies.
///
/// # Arguments
/// * `app` - Tauri application handle used to access native state and platform APIs.
///
/// # Errors
/// Returns an error when Android refuses to clear its application-owned cookie jar.
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

/// Clears only the native App API session while preserving any website session.
///
/// # Errors
/// Returns an error when the platform-specific App session store cannot be
/// cleared or the native state is unavailable.
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

/// Clears only the official website session while preserving any App API session.
///
/// # Errors
/// Returns an error when the platform-specific website session store cannot be
/// cleared or the native state is unavailable.
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

/// Verifies that the current native session can be used by authenticated SF requests.
///
/// This command is intentionally renderer-safe: it never returns the cookie or
/// upstream account payload, only the current authentication status.
///
/// # Arguments
/// * `app` - Tauri application handle used to retrieve native-only session data.
///
/// # Errors
/// Returns an error when no authenticated native session is available.
#[tauri::command]
pub(crate) async fn verify_authenticated_request(
    app: tauri::AppHandle,
) -> Result<AuthStatus, String> {
    let client = app_endpoint_client(&app, EndpointCapability::AccountProfile)?;
    let _ = client
        .get_data("/user", &[("expand", "welfareCoin".to_string())])
        .await?;
    current_auth_status(&app)
}

/// Reads the authenticated SF account profile through the native request layer.
///
/// # Arguments
/// * `app` - Tauri application handle used to retrieve the native-only session.
///
/// # Errors
/// Returns an error if the session is unavailable or the upstream profile data is invalid.
#[tauri::command]
pub(crate) async fn get_user_profile(app: tauri::AppHandle) -> Result<UserProfile, String> {
    #[cfg(target_os = "android")]
    sync_android_auth_session(&app).await?;
    let client = app_endpoint_client(&app, EndpointCapability::AccountProfile)?;
    let user = client
        .get_data("/user", &[("expand", "welfareCoin".to_string())])
        .await?;
    let money = client.get_data("/user/money", &[]).await?;
    let account_id = user
        .get("accountId")
        .and_then(Value::as_i64)
        .ok_or_else(|| "SF 登录会话可能已失效".to_string())?;
    let nick_name = user
        .get("nickName")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("SF 用户")
        .to_string();
    let profile = UserProfile {
        account_id,
        nick_name: nick_name.clone(),
        avatar: user
            .get("avatar")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        welfare_coin: user
            .get("expand")
            .and_then(|value| value.get("welfareCoin"))
            .and_then(Value::as_i64)
            .unwrap_or(0),
        fire_money_remain: money
            .get("fireMoneyRemain")
            .and_then(Value::as_i64)
            .unwrap_or(0),
        coupons_remain: money
            .get("couponsRemain")
            .and_then(Value::as_i64)
            .unwrap_or(0),
        vip_level: money.get("vipLevel").and_then(Value::as_i64).unwrap_or(0),
    };
    let auth_state = app.state::<AuthSessionState>();
    let mut session = auth_state
        .session
        .lock()
        .map_err(|_| "登录会话状态不可用".to_string())?;
    if let Some(session) = session.as_mut() {
        session.user_name = nick_name;
    }
    Ok(profile)
}
