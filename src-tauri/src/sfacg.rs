use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

#[cfg(target_os = "android")]
use tauri::plugin::{PluginHandle, TauriPlugin};

const SF_API_HOST: &str = "https://api.sfacg.com";
const SF_API_USER: &str = "androiduser";
const SF_API_PASSWORD: &str = "1a#$51-yt69;*Acv@qxq";
const SF_DEVICE_TOKEN: &str = "910D166A-736E-3231-8B21-8D12DFD75F16";
const SF_SECURITY_SALT: &str = "lPQDb9AKO7$LjkPG";
const SF_APP_USER_AGENT: &str =
    "boluobao/5.2.16(android;35)/OPPO/910d166a-736e-3231-8b21-8d12dfd75f16/OPPO";
const DEFAULT_CONTENT_DICTIONARY: &str = include_str!("../../sfacg-content-dictionary.json");
const OFFICIAL_LOGIN_URL: &str = "https://passport.sfacg.com/";
const OFFICIAL_LOGIN_WINDOW_LABEL: &str = "sfacg-official-login";
static SF_API_NONCE: OnceLock<tokio::sync::Mutex<Option<String>>> = OnceLock::new();

fn sf_api_nonce_state() -> &'static tokio::sync::Mutex<Option<String>> {
    SF_API_NONCE.get_or_init(|| tokio::sync::Mutex::new(None))
}

async fn clear_sf_api_nonce() {
    *sf_api_nonce_state().lock().await = None;
}

/// A public novel item returned to the renderer by the search command.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchNovel {
    novel_id: i64,
    novel_name: String,
    author_name: String,
    novel_cover: String,
    last_update_time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    bookshelf_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bookshelf_type: Option<String>,
}

/// Detailed public novel metadata used by the chapter selection view.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NovelDetail {
    novel_id: i64,
    novel_name: String,
    author_name: String,
    novel_cover: String,
    last_update_time: String,
    description: String,
    is_finish: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    type_name: Option<String>,
}

/// A renderer-safe chapter collection for one SF novel volume.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChapterVolume {
    volume_id: i64,
    title: String,
    chapters: Vec<ChapterSummary>,
}

/// A renderer-safe chapter availability record.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChapterSummary {
    chap_id: i64,
    title: String,
    need_fire_money: i64,
    is_vip: bool,
    is_unlocked: bool,
    downloaded: bool,
}

/// The authenticated SF bookshelf returned to the renderer without session data.
#[derive(Clone, Debug, Serialize)]
struct BookshelfCollection {
    categories: Vec<String>,
    items: Vec<SearchNovel>,
}

/// Renderer-compatible native download task state.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeJob {
    id: String,
    title: String,
    kind: String,
    status: String,
    progress: u8,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    novel_id: Option<i64>,
}

/// Shared native task registry and cancellation flags.
#[derive(Default)]
struct NativeJobState {
    jobs: Mutex<std::collections::HashMap<String, NativeJob>>,
    cancellations: Mutex<std::collections::HashMap<String, Arc<std::sync::atomic::AtomicBool>>>,
    specs: Mutex<std::collections::HashMap<String, NativeJobSpec>>,
}

/// Private inputs required to resume a native download task.
#[derive(Clone, Debug, Deserialize, Serialize)]
struct NativeJobSpec {
    kind: String,
    novel_id: i64,
    title: String,
    chapter_ids: Vec<i64>,
}

/// Persisted task data that permits user-controlled resumption after restart.
#[derive(Default, Deserialize, Serialize)]
struct PersistedNativeJobs {
    jobs: std::collections::HashMap<String, NativeJob>,
    specs: std::collections::HashMap<String, NativeJobSpec>,
}

/// Native-only audio chapter record, including the upstream stream URL.
struct NativeAudioChapter {
    id: i64,
    title: String,
    volume: String,
    source: String,
}

/// Renderer-safe audio catalog for a single SF work.
#[derive(Debug, Serialize)]
struct AudioCatalog {
    title: String,
    chapters: Vec<AudioChapterSummary>,
}

/// Renderer-safe audio chapter row without the protected upstream stream URL.
#[derive(Debug, Serialize)]
struct AudioChapterSummary {
    id: i64,
    title: String,
    volume: String,
    downloaded: bool,
}

/// The renderer-safe authentication state for the current native session.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthStatus {
    authenticated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_name: Option<String>,
}

/// A renderer-safe snapshot of the authenticated SF account profile.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UserProfile {
    account_id: i64,
    nick_name: String,
    avatar: String,
    welfare_coin: i64,
    fire_money_remain: i64,
    coupons_remain: i64,
    vip_level: i64,
}

/// Holds the active SF session in native memory while API commands are running.
///
/// Windows additionally restores this state from a DPAPI-protected app-data file;
/// Android repopulates it from the persistent app-owned WebView cookie jar.
#[derive(Default)]
struct AuthSessionState {
    session: Mutex<Option<NativeAuthSession>>,
}

/// Describes a native-only SF session without implementing serialization.
struct NativeAuthSession {
    cookie: String,
    user_name: String,
}

#[derive(Deserialize, Serialize)]
struct PersistedAuthSession {
    cookie: String,
    user_name: String,
}

#[cfg(target_os = "windows")]
fn auth_session_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|directory| directory.join("auth-session.bin"))
        .map_err(|error| format!("无法解析登录会话路径：{error}"))
}

#[cfg(target_os = "windows")]
fn protect_auth_data(input: &[u8]) -> Result<Vec<u8>, String> {
    use std::ptr;
    use windows_sys::Win32::Foundation::{GetLastError, LocalFree};
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN,
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

#[cfg(target_os = "windows")]
fn unprotect_auth_data(input: &[u8]) -> Result<Vec<u8>, String> {
    use std::ptr;
    use windows_sys::Win32::Foundation::{GetLastError, LocalFree};
    use windows_sys::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN,
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

#[cfg(target_os = "windows")]
fn restore_desktop_auth_session(app: &tauri::AppHandle) -> Result<Option<NativeAuthSession>, String> {
    let path = auth_session_path(app)?;
    if !path.is_file() {
        return Ok(None);
    }
    let encrypted = fs::read(path).map_err(|error| format!("无法读取登录会话：{error}"))?;
    let payload = unprotect_auth_data(&encrypted)?;
    let stored = serde_json::from_slice::<PersistedAuthSession>(&payload)
        .map_err(|error| format!("登录会话格式无效：{error}"))?;
    if stored.cookie.trim().is_empty() || stored.user_name.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(NativeAuthSession {
        cookie: stored.cookie,
        user_name: stored.user_name,
    }))
}

#[cfg(target_os = "windows")]
fn clear_desktop_auth_session(app: &tauri::AppHandle) -> Result<(), String> {
    let path = auth_session_path(app)?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| format!("无法清除登录会话：{error}"))?;
    }
    Ok(())
}

/// Receives an Android WebView cookie from the native plugin, never from JavaScript.
#[cfg(target_os = "android")]
#[derive(Deserialize)]
struct AndroidCookieResponse {
    cookie: Option<String>,
}

/// Stores the Android native plugin handle used exclusively by Rust commands.
#[cfg(target_os = "android")]
struct AndroidSfacgAuth<R: tauri::Runtime> {
    mobile_plugin_handle: PluginHandle<R>,
}

/// Builds the Android-only plugin that owns the official-login WebView and cookie bridge.
///
/// # Returns
/// A Tauri plugin that registers the Kotlin implementation before commands run.
#[cfg(target_os = "android")]
fn android_sfacg_auth_plugin<R: tauri::Runtime>() -> TauriPlugin<R> {
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

/// Reconciles the persistent Android WebView session with native request state.
///
/// # Arguments
/// * `app` - Application handle used to call the Android plugin and access state.
///
/// # Errors
/// Returns an error when the Android plugin cannot provide its app-owned cookie jar.
#[cfg(target_os = "android")]
async fn sync_android_auth_session(app: &tauri::AppHandle) -> Result<(), String> {
    let cookie = app
        .state::<AndroidSfacgAuth<tauri::Wry>>()
        .mobile_plugin_handle
        .run_mobile_plugin_async::<AndroidCookieResponse>("readSessionCookie", ())
        .await
        .map_err(|error| format!("无法读取 Android 登录会话：{error}"))?
        .cookie
        .filter(|value| !value.trim().is_empty());
    let auth_state = app.state::<AuthSessionState>();
    let mut session = auth_state
        .session
        .lock()
        .map_err(|_| "登录会话状态不可用".to_string())?;
    *session = cookie.map(|cookie| NativeAuthSession {
        cookie,
        user_name: "已登录 SF 账号".to_string(),
    });
    Ok(())
}

#[cfg(target_os = "android")]
async fn persist_android_auth_session(
    app: &tauri::AppHandle,
    cookie: &str,
) -> Result<(), String> {
    app.state::<AndroidSfacgAuth<tauri::Wry>>()
        .mobile_plugin_handle
        .run_mobile_plugin_async::<Value>(
            "writeSessionCookie",
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
        authenticated: session.is_some(),
        user_name: session.as_ref().map(|value| value.user_name.clone()),
    })
}

/// Returns the authenticated cookie for native request code without exposing it to the renderer.
///
/// # Arguments
/// * `app` - Application handle used to access the native session state.
///
/// # Errors
/// Returns an error if no active session exists or the state lock is unavailable.
fn current_session_cookie(app: &tauri::AppHandle) -> Result<String, String> {
    app.state::<AuthSessionState>()
        .session
        .lock()
        .map_err(|_| "登录会话状态不可用".to_string())?
        .as_ref()
        .map(|session| session.cookie.clone())
        .ok_or_else(|| "请先登录 SF 账号".to_string())
}

/// Creates the signed SF API HTTP client used by public catalog commands.
struct SfacgHttpClient {
    client: reqwest::Client,
}

impl SfacgHttpClient {
    /// Creates a client with a bounded request timeout and a desktop-like user agent.
    ///
    /// # Returns
    /// A configured client that does not retain user credentials or cookies.
    fn new() -> Result<Self, String> {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent(SF_APP_USER_AGENT)
            .build()
            .map(|client| Self { client })
            .map_err(|error| format!("无法创建 SF 网络客户端：{error}"))
    }

    /// Requests a signed SF API resource and unwraps its `data` envelope.
    ///
    /// # Arguments
    /// * `path` - Relative API path from the fixed SF API host.
    /// * `query` - Query parameters restricted to the calling command.
    ///
    /// # Errors
    /// Returns a脱敏 error when nonce negotiation, transport, status, or JSON parsing fails.
    async fn get_data(&self, path: &str, query: &[(&str, String)]) -> Result<Value, String> {
        self.get_data_with_cookie(path, query, None).await
    }

    /// Requests a signed SF API resource with an optional in-memory session cookie.
    ///
    /// # Arguments
    /// * `path` - Relative API path from the fixed SF API host.
    /// * `query` - Query parameters restricted to the calling command.
    /// * `cookie` - An SF session value that never reaches the renderer.
    ///
    /// # Errors
    /// Returns a redacted error when nonce negotiation, transport, status, or
    /// JSON parsing fails.
    async fn get_data_with_cookie(
        &self,
        path: &str,
        query: &[(&str, String)],
        cookie: Option<&str>,
    ) -> Result<Value, String> {
        let nonce = self.obtain_nonce().await?;
        let mut request = self
            .client
            .get(format!("{SF_API_HOST}{path}"))
            .basic_auth(SF_API_USER, Some(SF_API_PASSWORD))
            .header("Accept", "application/vnd.sfacg.api+json;version=1")
            .header("Accept-Charset", "UTF-8")
            .header("Content-Type", "application/json; charset=UTF-8")
            .header("SFSecurity", security_header(&nonce)?)
            .query(query);
        if let Some(cookie) = cookie.filter(|value| !value.trim().is_empty()) {
            request = request.header(reqwest::header::COOKIE, cookie);
        }
        let response = request
            .send()
            .await
            .map_err(|error| format!("SF 请求失败：{error}"))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.json::<Value>().await.unwrap_or(Value::Null);
            let api_code = body
                .get("status")
                .and_then(|value| value.get("errorCode"))
                .and_then(Value::as_i64);
            let api_message = body
                .get("status")
                .and_then(|value| value.get("msg"))
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty());
            eprintln!(
                "[sfacg] request rejected: path={path}, http={}, api_code={api_code:?}, message={api_message:?}",
                status.as_u16(),
            );
            if status == reqwest::StatusCode::EXPECTATION_FAILED {
                clear_sf_api_nonce().await;
            }
            return Err(match api_message {
                Some(message) => format!("SF 请求返回 HTTP {}：{message}", status.as_u16()),
                None => format!("SF 请求返回 HTTP {}", status.as_u16()),
            });
        }
        let body = response
            .json::<Value>()
            .await
            .map_err(|error| format!("SF 响应格式无效：{error}"))?;
        Ok(body.get("data").cloned().unwrap_or(body))
    }

    /// Signs in using the SF App endpoint and extracts an issued session cookie.
    ///
    /// # Arguments
    /// * `username` - The user-entered SF account name, used only for this request.
    /// * `password` - The user-entered SF password, used only for this request.
    ///
    /// # Errors
    /// Returns a redacted error when SF rejects the credentials or does not issue
    /// a supported session cookie.
    async fn login_with_password(&self, username: &str, password: &str) -> Result<String, String> {
        let nonce = self.obtain_nonce().await?;
        let response = self
            .client
            .post(format!("{SF_API_HOST}/sessions"))
            .basic_auth(SF_API_USER, Some(SF_API_PASSWORD))
            .header("Accept", "application/vnd.sfacg.api+json;version=1")
            .header("Accept-Charset", "UTF-8")
            .header("Content-Type", "application/json; charset=UTF-8")
            .header("SFSecurity", security_header(&nonce)?)
            .json(&serde_json::json!({
                "username": username,
                "password": password,
                "shuMeiId": "",
            }))
            .send()
            .await
            .map_err(|error| format!("SF 登录请求失败：{error}"))?;
        let status = response.status();
        let cookie = response
            .headers()
            .get_all(reqwest::header::SET_COOKIE)
            .iter()
            .filter_map(|header| header.to_str().ok())
            .filter_map(|header| header.split(';').next())
            .filter(|pair| pair.starts_with(".SFCommunity=") || pair.starts_with("session_APP="))
            .collect::<Vec<_>>()
            .join("; ");
        let body = response.json::<Value>().await.unwrap_or(Value::Null);
        let successful = status.is_success()
            && body
                .get("status")
                .and_then(|value| value.get("httpCode"))
                .and_then(Value::as_i64)
                == Some(200);
        if !successful {
            return Err(body
                .get("status")
                .and_then(|value| value.get("msg"))
                .and_then(Value::as_str)
                .filter(|message| !message.trim().is_empty())
                .unwrap_or("SF 账号密码登录失败")
                .to_string());
        }
        if cookie.is_empty() {
            return Err("SF 登录成功但未返回会话 Cookie".to_string());
        }
        Ok(cookie)
    }

    /// Fetches API chapter text together with the novel and volume identifiers
    /// required to compare it with the public web chapter.
    ///
    /// # Arguments
    /// * `chapter_id` - Positive SF chapter identifier.
    /// * `cookie` - Optional authenticated session kept in native memory.
    ///
    /// # Errors
    /// Returns an error when the API response lacks usable text or ownership
    /// metadata, or when the upstream request fails.
    async fn chapter_content_with_metadata_from_api(
        &self,
        chapter_id: i64,
        cookie: Option<&str>,
    ) -> Result<ApiChapterContent, String> {
        if chapter_id <= 0 {
            return Err("章节编号无效".to_string());
        }
        let response = self
            .get_data_with_cookie(
                &format!("/Chaps/{chapter_id}"),
                &[("expand", "content,expand.content".to_string())],
                cookie,
            )
            .await?;
        let content = response
            .get("expand")
            .and_then(|value| value.get("content"))
            .and_then(Value::as_str)
            .or_else(|| response.get("content").and_then(Value::as_str))
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "SF App API 未返回章节正文".to_string())?;
        let novel_id = response
            .get("novelId")
            .and_then(Value::as_i64)
            .ok_or_else(|| "SF App API 未返回章节所属作品信息".to_string())?;
        let volume_id = response
            .get("volumeId")
            .and_then(Value::as_i64)
            .ok_or_else(|| "SF App API 未返回章节所属卷信息".to_string())?;
        Ok(ApiChapterContent {
            content: content.to_string(),
            novel_id,
            volume_id,
        })
    }

    /// Reads the authenticated SF audio catalog without exposing stream URLs to
    /// the renderer.
    ///
    /// # Arguments
    /// * `novel_id` - Positive SF work identifier.
    /// * `cookie` - Authenticated session retained only in native memory.
    ///
    /// # Errors
    /// Returns an error when the session is rejected, the work has no audio, or
    /// the upstream catalog response is malformed.
    async fn audio_catalog(
        &self,
        novel_id: i64,
        cookie: &str,
    ) -> Result<(String, Vec<NativeAudioChapter>), String> {
        validate_novel_id(novel_id)?;
        let response = self
            .client
            .get("https://i.sfacg.com/ajax/ashx/Common.ashx")
            .query(&[("op", "getAudioInfo"), ("nid", &novel_id.to_string())])
            .header(reqwest::header::COOKIE, cookie)
            .header("Accept", "application/json, text/javascript, */*; q=0.01")
            .header("X-Requested-With", "XMLHttpRequest")
            .header(
                reqwest::header::REFERER,
                "https://i.sfacg.com/consume/book/",
            )
            .send()
            .await
            .map_err(|error| format!("无法连接 SF 有声接口：{error}"))?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED
            || response.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err("SF 登录会话已失效，请重新登录".to_string());
        }
        let payload = response
            .json::<Value>()
            .await
            .map_err(|error| format!("SF 有声目录格式无效：{error}"))?;
        if payload.get("status").and_then(Value::as_i64) != Some(200) {
            let message = payload.get("msg").and_then(Value::as_str).unwrap_or("");
            return Err(if message == "参数不正确" {
                "该作品没有可用的有声章节".to_string()
            } else if message.is_empty() {
                "SF 有声接口拒绝了请求".to_string()
            } else {
                format!("SF 有声接口拒绝了请求：{message}")
            });
        }
        let data = payload
            .get("data")
            .ok_or_else(|| "该作品没有可用的有声章节".to_string())?;
        let title = data
            .get("NovelName")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("未命名有声作品")
            .to_string();
        let mut chapters = Vec::new();
        for volume in data
            .get("VolumeSet")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let volume_title = volume
                .get("VolumeName")
                .and_then(Value::as_str)
                .unwrap_or("未分卷")
                .to_string();
            for audio in volume
                .get("AudioSet")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let Some(source) = audio.get("AudioSrc").and_then(Value::as_str) else {
                    continue;
                };
                if source.trim().is_empty() {
                    continue;
                }
                let id = audio
                    .get("AudioID")
                    .and_then(Value::as_i64)
                    .unwrap_or(chapters.len() as i64 + 1);
                chapters.push(NativeAudioChapter {
                    id,
                    title: audio
                        .get("ChapterTitle")
                        .and_then(Value::as_str)
                        .unwrap_or("未命名章节")
                        .to_string(),
                    volume: volume_title.clone(),
                    source: source.to_string(),
                });
            }
        }
        if chapters.is_empty() {
            return Err("该作品没有可用的有声章节".to_string());
        }
        Ok((title, chapters))
    }

    /// Fetches and extracts a public SF web chapter from `#ChapterBody`.
    ///
    /// # Arguments
    /// * `novel_id` - SF novel identifier.
    /// * `volume_id` - SF volume identifier.
    /// * `chapter_id` - SF chapter identifier.
    /// * `cookie` - Optional authenticated session used for accessible chapters.
    ///
    /// # Errors
    /// Returns an error when the page is unavailable or its chapter body cannot be found.
    async fn chapter_content_from_web(
        &self,
        novel_id: i64,
        volume_id: i64,
        chapter_id: i64,
        cookie: Option<&str>,
    ) -> Result<String, String> {
        let mut request = self.client.get(format!(
            "https://book.sfacg.com/Novel/{novel_id}/{volume_id}/{chapter_id}/"
        ));
        if let Some(cookie) = cookie.filter(|value| !value.trim().is_empty()) {
            request = request.header(reqwest::header::COOKIE, cookie);
        }
        let html = request
            .header(reqwest::header::ACCEPT, "text/html,application/xhtml+xml")
            .header(
                reqwest::header::REFERER,
                format!("https://book.sfacg.com/Novel/{novel_id}/MainIndex/"),
            )
            .send()
            .await
            .map_err(|error| format!("无法读取 SF 网页章节：{error}"))?
            .text()
            .await
            .map_err(|error| format!("SF 网页章节格式无效：{error}"))?;
        let start = html
            .find("id=\"ChapterBody\"")
            .or_else(|| html.find("id='ChapterBody'"))
            .ok_or_else(|| "SF 网页响应中没有找到 #ChapterBody".to_string())?;
        let body_start = html[start..]
            .find('>')
            .map(|offset| start + offset + 1)
            .ok_or_else(|| "SF 章节正文结构无效".to_string())?;
        let body_end = html[body_start..]
            .find("</div>")
            .map(|offset| body_start + offset)
            .ok_or_else(|| "SF 章节正文结构无效".to_string())?;
        let body = html[body_start..body_end]
            .replace("<br>", "\n")
            .replace("<br/>", "\n")
            .replace("<br />", "\n")
            .replace("</p>", "\n")
            .replace("&nbsp;", " ")
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">");
        let mut plain = String::with_capacity(body.len());
        let mut in_tag = false;
        for character in body.chars() {
            match character {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => plain.push(character),
                _ => {}
            }
        }
        let plain = plain.replace("\r\n", "\n").replace('\r', "\n");
        if plain.trim().is_empty() {
            return Err("SF 网页章节正文为空".to_string());
        }
        Ok(plain.trim().to_string())
    }

    /// Negotiates a short-lived nonce required by the SF API signature.
    ///
    /// # Errors
    /// Returns an error after three rejected nonce candidates or any transport failure.
    async fn obtain_nonce(&self) -> Result<String, String> {
        let mut shared_nonce = sf_api_nonce_state().lock().await;
        if let Some(nonce) = shared_nonce.as_ref() {
            return Ok(nonce.clone());
        }
        for _ in 0..3 {
            let nonce = Uuid::new_v4().to_string().to_uppercase();
            let response = self
                .client
                .get(format!("{SF_API_HOST}/Chaps/8436696"))
                .basic_auth(SF_API_USER, Some(SF_API_PASSWORD))
                .header("Accept", "application/vnd.sfacg.api+json;version=1")
                .header("Accept-Charset", "UTF-8")
                .header("Content-Type", "application/json; charset=UTF-8")
                .header("SFSecurity", security_header(&nonce)?)
                .query(&[("expand", "content,expand.content")])
                .send()
                .await
                .map_err(|error| format!("SF nonce 请求失败：{error}"))?;
            let http_status = response.status();
            let body = response
                .json::<Value>()
                .await
                .map_err(|error| format!("SF nonce 响应格式无效：{error}"))?;
            let api_status = body
                .get("status")
                .and_then(|status| status.get("httpCode"))
                .and_then(Value::as_i64);
            if api_status != Some(417) {
                if !http_status.is_success() {
                    let message = body
                        .get("status")
                        .and_then(|status| status.get("msg"))
                        .and_then(Value::as_str)
                        .unwrap_or("无上游说明");
                    eprintln!(
                        "[sfacg] nonce probe unexpected response: http={}, api_status={api_status:?}, message={message}",
                        http_status.as_u16(),
                    );
                }
                *shared_nonce = Some(nonce.clone());
                return Ok(nonce);
            }
            eprintln!("[sfacg] nonce probe rejected: HTTP 417 / errorCode 782");
        }
        Err("SF API 未返回可用 nonce".to_string())
    }
}

/// API chapter content and the identifiers needed for web comparison.
struct ApiChapterContent {
    content: String,
    novel_id: i64,
    volume_id: i64,
}

/// Builds the SF `SFSecurity` header without logging any credential material.
///
/// # Arguments
/// * `nonce` - Uppercase UUID candidate used for this request.
///
/// # Errors
/// Returns an error if the generated signing input is unexpectedly short.
fn security_header(nonce: &str) -> Result<String, String> {
    let repeated = nonce.repeat(4).into_bytes();
    let offset = |index: usize| -> usize {
        let value = repeated[index] as usize;
        value - (value / 0x24) * 0x24
    };
    let mut reordered = Vec::with_capacity(101);
    for (index, length) in [(1, 13), (2, 16), (3, 36), (4, 36)] {
        let start = offset(index);
        let end = start.saturating_add(length);
        if end > repeated.len() {
            return Err("SF 签名输入长度无效".to_string());
        }
        reordered.extend_from_slice(&repeated[start..end]);
    }
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "系统时间无效".to_string())?
        .as_millis();
    let auth = format!("{timestamp}{SF_SECURITY_SALT}{SF_DEVICE_TOKEN}{nonce}").into_bytes();
    if auth.len() < 101 || reordered.len() < 101 {
        return Err("SF 签名输入不足".to_string());
    }
    let mixed: Vec<char> = (0..101)
        .map(|index| {
            char::from_u32(((auth[index] as u32 + reordered[index] as u32) >> 1) & 0x10FFFF)
                .unwrap_or('?')
        })
        .collect();
    let result: Vec<char> = mixed[65..]
        .iter()
        .chain(&mixed[..13])
        .chain(&mixed[29..65])
        .chain(&mixed[13..29])
        .copied()
        .collect();
    let mut normalized = String::with_capacity(result.len());
    for character in result {
        let code = character as u32;
        let next = if code < 0x30 {
            let shifted = code + 19;
            if (0x39 < shifted) && (shifted < 0x41) {
                0x39
            } else {
                shifted
            }
        } else if (0x39 < code && code < 0x41) || (0x5A < code && code < 0x61) {
            code + 19
        } else {
            code
        };
        normalized.push(char::from_u32(next).unwrap_or('?'));
    }
    let digest = Md5::digest(normalized.as_bytes());
    Ok(format!(
        "nonce={nonce}&timestamp={timestamp}&devicetoken={SF_DEVICE_TOKEN}&sign={:X}",
        digest
    ))
}

/// Searches public SF novels and converts upstream records to renderer DTOs.
///
/// # Arguments
/// * `query` - A trimmed user search term, limited to 100 Unicode scalar values.
///
/// # Errors
/// Returns an error for empty/oversized input or an unavailable upstream service.
#[tauri::command]
async fn search_novels(query: String) -> Result<Vec<SearchNovel>, String> {
    let query = query.trim().to_string();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    if query.chars().count() > 100 {
        return Err("搜索关键词不能超过 100 个字符".to_string());
    }
    let client = SfacgHttpClient::new()?;
    let response = client
        .get_data(
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
                novel_name: name,
                author_name: item
                    .get("authorName")
                    .and_then(Value::as_str)
                    .unwrap_or("未知作者")
                    .to_string(),
                novel_cover: item
                    .get("novelCover")
                    .or_else(|| item.get("coverBig"))
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
fn validate_novel_id(novel_id: i64) -> Result<(), String> {
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
async fn get_novel_details(novel_id: i64) -> Result<NovelDetail, String> {
    validate_novel_id(novel_id)?;
    let client = SfacgHttpClient::new()?;
    let detail = client
        .get_data(
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
async fn get_chapter_volumes(novel_id: i64) -> Result<Vec<ChapterVolume>, String> {
    validate_novel_id(novel_id)?;
    let client = SfacgHttpClient::new()?;
    let response = client
        .get_data(&format!("/novels/{novel_id}/dirs"), &[])
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
async fn get_audio_chapters(app: tauri::AppHandle, novel_id: i64) -> Result<AudioCatalog, String> {
    #[cfg(target_os = "android")]
    sync_android_auth_session(&app).await?;
    let cookie = current_session_cookie(&app)?;
    let client = SfacgHttpClient::new()?;
    let (title, chapters) = client.audio_catalog(novel_id, &cookie).await?;
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
            })
            .collect(),
    })
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
async fn get_bookshelf(
    app: tauri::AppHandle,
    force_refresh: Option<bool>,
) -> Result<BookshelfCollection, String> {
    let _ = force_refresh;
    #[cfg(target_os = "android")]
    sync_android_auth_session(&app).await?;
    let cookie = current_session_cookie(&app)?;
    let client = SfacgHttpClient::new()?;
    let shelves = client
        .get_data_with_cookie(
            "/user/Pockets",
            &[("expand", "novels,albums,comics".to_string())],
            Some(&cookie),
        )
        .await?;
    let shelves = shelves
        .as_array()
        .ok_or_else(|| "SF 未返回有效书架数据".to_string())?;
    let mut categories = Vec::new();
    let mut items = Vec::new();
    for shelf in shelves {
        let category = shelf
            .get("name")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("未分类")
            .to_string();
        if !categories.contains(&category) {
            categories.push(category.clone());
        }
        let Some(expand) = shelf.get("expand") else {
            continue;
        };
        for (key, kind) in [
            ("novels", "novel"),
            ("albums", "audio"),
            ("comics", "comic"),
        ] {
            let Some(records) = expand.get(key).and_then(Value::as_array) else {
                continue;
            };
            for record in records {
                let Some(novel_id) = record
                    .get("novelId")
                    .or_else(|| record.get("comicId"))
                    .and_then(Value::as_i64)
                else {
                    continue;
                };
                items.push(SearchNovel {
                    novel_id,
                    novel_name: record
                        .get("novelName")
                        .or_else(|| record.get("comicName"))
                        .or_else(|| record.get("name"))
                        .and_then(Value::as_str)
                        .unwrap_or("未命名作品")
                        .to_string(),
                    author_name: record
                        .get("authorName")
                        .and_then(Value::as_str)
                        .unwrap_or("未知作者")
                        .to_string(),
                    novel_cover: record
                        .get("novelCover")
                        .or_else(|| record.get("coverBig"))
                        .or_else(|| record.get("comicCover"))
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    last_update_time: record
                        .get("lastUpdateTime")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    bookshelf_name: Some(category.clone()),
                    bookshelf_type: Some(kind.to_string()),
                });
            }
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
async fn auth_status(app: tauri::AppHandle) -> Result<AuthStatus, String> {
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
async fn login_with_password(
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
    let client = SfacgHttpClient::new()?;
    let cookie = client.login_with_password(username, &password).await?;
    #[cfg(target_os = "android")]
    persist_android_auth_session(&app, &cookie).await?;
    #[cfg(target_os = "windows")]
    persist_desktop_auth_session(&app, &cookie, username)?;
    let auth_state = app.state::<AuthSessionState>();
    let mut session = auth_state
        .session
        .lock()
        .map_err(|_| "登录会话状态不可用".to_string())?;
    *session = Some(NativeAuthSession {
        cookie,
        user_name: username.to_string(),
    });
    drop(session);
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
async fn start_official_login(app: tauri::AppHandle) -> Result<(), String> {
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
                let client = SfacgHttpClient::new()?;
                if let Err(error) = client
                    .get_data_with_cookie(
                        "/user",
                        &[("expand", "welfareCoin".to_string())],
                        Some(&cookie),
                    )
                    .await
                {
                    let _ = login_window.close();
                    return Err(format!(
                        "官方网页登录未获得可用于 App 接口的会话，请改用账号密码登录：{error}"
                    ));
                }
                let auth_state = app.state::<AuthSessionState>();
                let mut session = auth_state
                    .session
                    .lock()
                    .map_err(|_| "登录会话状态不可用".to_string())?;
                *session = Some(NativeAuthSession {
                    cookie,
                    user_name: "已登录 SF 账号".to_string(),
                });
                #[cfg(target_os = "windows")]
                persist_desktop_auth_session(
                    &app,
                    &session
                        .as_ref()
                        .ok_or_else(|| "登录会话状态不可用".to_string())?
                        .cookie,
                    "已登录 SF 账号",
                )?;
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
        if name == ".SFCommunity" || name == "session_APP" || name.starts_with("session_") {
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
async fn logout(app: tauri::AppHandle) -> Result<(), String> {
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
    clear_desktop_auth_session(&app)?;
    Ok(())
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
async fn verify_authenticated_request(app: tauri::AppHandle) -> Result<AuthStatus, String> {
    let cookie = current_session_cookie(&app)?;
    let client = SfacgHttpClient::new()?;
    let _ = client
        .get_data_with_cookie(
            "/user",
            &[("expand", "welfareCoin".to_string())],
            Some(&cookie),
        )
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
async fn get_user_profile(app: tauri::AppHandle) -> Result<UserProfile, String> {
    #[cfg(target_os = "android")]
    sync_android_auth_session(&app).await?;
    let cookie = current_session_cookie(&app)?;
    let client = SfacgHttpClient::new()?;
    let user = client
        .get_data_with_cookie(
            "/user",
            &[("expand", "welfareCoin".to_string())],
            Some(&cookie),
        )
        .await?;
    let money = client
        .get_data_with_cookie("/user/money", &[], Some(&cookie))
        .await?;
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

