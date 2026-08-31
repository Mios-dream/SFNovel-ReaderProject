use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
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
const DEFAULT_CONTENT_DICTIONARY: &str = include_str!("../../sfacg-content-dictionary.json");
const OFFICIAL_LOGIN_URL: &str = "https://passport.sfacg.com/";
const OFFICIAL_LOGIN_WINDOW_LABEL: &str = "sfacg-official-login";

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

/// Holds a cookie only in native memory while it is needed by SF API commands.
///
/// On Android the app-owned WebView remains the persistent source of truth;
/// this state is repopulated from `CookieManager` after application restart.
#[derive(Default)]
struct AuthSessionState {
    session: Mutex<Option<NativeAuthSession>>,
}

/// Describes a native-only SF session without implementing serialization.
struct NativeAuthSession {
    cookie: String,
    user_name: String,
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

/// Reconciles the Android app WebView session with native request state.
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
            .user_agent("boluobao/5.2.16 (Novel Flow Tauri)")
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
            .header("SFSecurity", security_header(&nonce)?)
            .query(query);
        if let Some(cookie) = cookie.filter(|value| !value.trim().is_empty()) {
            request = request.header(reqwest::header::COOKIE, cookie);
        }
        let response = request
            .send()
            .await
            .map_err(|error| format!("SF 请求失败：{error}"))?;
        if !response.status().is_success() {
            return Err(format!("SF 请求返回 HTTP {}", response.status().as_u16()));
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
        for _ in 0..3 {
            let nonce = Uuid::new_v4().to_string().to_uppercase();
            let response = self
                .client
                .get(format!("{SF_API_HOST}/Chaps/8436696"))
                .basic_auth(SF_API_USER, Some(SF_API_PASSWORD))
                .header("Accept", "application/vnd.sfacg.api+json;version=1")
                .header("SFSecurity", security_header(&nonce)?)
                .query(&[("expand", "content,expand.content")])
                .send()
                .await
                .map_err(|error| format!("SF nonce 请求失败：{error}"))?;
            let body = response
                .json::<Value>()
                .await
                .map_err(|error| format!("SF nonce 响应格式无效：{error}"))?;
            if body
                .get("status")
                .and_then(|status| status.get("httpCode"))
                .and_then(Value::as_i64)
                != Some(417)
            {
                return Ok(nonce);
            }
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
                let auth_state = app.state::<AuthSessionState>();
                let mut session = auth_state
                    .session
                    .lock()
                    .map_err(|_| "登录会话状态不可用".to_string())?;
                *session = Some(NativeAuthSession {
                    cookie,
                    user_name: "已登录 SF 账号".to_string(),
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

/// A renderer-safe summary of one locally stored book.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LibraryBook {
    name: String,
    updated_at: String,
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
    novel_id: Option<i64>,
    author: String,
    description: String,
    cover: Option<String>,
    audio_tracks: Vec<LocalAudioTrack>,
    epub_href: Option<String>,
    chapter_volumes: Vec<LocalChapterVolume>,
}

/// A locally playable audio track; the native asset protocol URL is added later.
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
    novel_id: Option<i64>,
    title: Option<String>,
    author: Option<String>,
    description: Option<String>,
    downloaded_text_chapter_ids: Option<Vec<i64>>,
    downloaded_audio_chapter_ids: Option<Vec<i64>>,
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
    let cookie = current_session_cookie(&app).ok();
    let client = SfacgHttpClient::new()?;
    let api = client
        .chapter_content_with_metadata_from_api(chapter_id, cookie.as_deref())
        .await?;
    let web = client
        .chapter_content_from_web(api.novel_id, api.volume_id, chapter_id, cookie.as_deref())
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
/// Returns an error when the platform data directory cannot be resolved or
/// created.
fn library_directory(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("无法解析应用数据目录：{error}"))?
        .join("library");
    fs::create_dir_all(&directory).map_err(|error| format!("无法创建本地书库目录：{error}"))?;
    Ok(directory)
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
        let text =
            entry.path().join(".novel-flow.json").exists() || entry.path().join("book.md").exists();
        let audio = entry.path().join("audio").is_dir();
        books.push(LibraryBook {
            name,
            updated_at: metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs().to_string())
                .unwrap_or_else(|| "0".to_string()),
            formats: LibraryFormats {
                text,
                audio,
                comic: false,
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
fn read_local_audio_tracks(
    name: &str,
    directory: &PathBuf,
) -> Result<Vec<LocalAudioTrack>, String> {
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
                href: format!(
                    "asset://localhost/library/{}/audio/{}",
                    urlencoding::encode(name),
                    urlencoding::encode(file_name),
                ),
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
    let cover = if directory.join("imgs").join("cover.jpeg").is_file() {
        Some(format!(
            "asset://localhost/library/{}/imgs/cover.jpeg",
            urlencoding::encode(&name)
        ))
    } else {
        None
    };
    let epub_name = format!("{}.epub", name);
    let epub_href = directory.join(&epub_name).is_file().then(|| {
        format!(
            "asset://localhost/library/{}/{}",
            urlencoding::encode(&name),
            urlencoding::encode(&epub_name)
        )
    });
    let audio_tracks = read_local_audio_tracks(&name, &directory)?;
    Ok(LocalBookDetail {
        name,
        novel_id: metadata.novel_id,
        author: metadata.author.unwrap_or_else(|| "未知作者".to_string()),
        description: metadata
            .description
            .unwrap_or_else(|| "暂无简介".to_string()),
        cover,
        audio_tracks,
        epub_href,
        chapter_volumes,
    })
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

/// Builds the application-owned export directory without accepting a renderer
/// filesystem path.
///
/// # Arguments
/// * `app` - Application handle used to resolve the private data location.
///
/// # Errors
/// Returns an error when the export directory cannot be resolved or created.
fn export_directory(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("无法解析导出目录：{error}"))?
        .join("exports");
    fs::create_dir_all(&directory).map_err(|error| format!("无法创建导出目录：{error}"))?;
    Ok(directory)
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

/// Generates an application-owned EPUB, Markdown ZIP, TXT, or audio ZIP export from one
/// validated local book and returns only its controlled asset URL.
///
/// # Arguments
/// * `app` - Application handle used to resolve controlled paths.
/// * `name` - Exact book directory name previously returned by the library.
/// * `format` - One of `epub`, `markdown`, `txt`, or `audio`.
///
/// # Errors
/// Returns an error for unsupported formats, missing text chapters, or export failures.
#[tauri::command]
fn export_local_book(
    app: tauri::AppHandle,
    name: String,
    format: String,
) -> Result<LocalExport, String> {
    if !matches!(format.as_str(), "epub" | "markdown" | "txt" | "audio") {
        return Err("不支持的导出格式".to_string());
    }
    let directory = local_book_directory(&app, &name)?;
    let metadata = read_json_or_default::<StoredBookMetadata>(&directory.join(".novel-flow.json"))?;
    let store =
        read_json_or_default::<StoredChapterStore>(&directory.join(".novel-flow-chapters.json"))?;
    let mut chapters: Vec<_> = store.chapters.into_values().collect();
    chapters.sort_by_key(|chapter| (chapter.volume_index, chapter.chapter_index, chapter.id));
    let audio_directory = directory.join("audio");
    if format == "audio" {
        if !audio_directory.join("有声目录.m3u8").is_file() {
            return Err("这本书没有可打包的有声章节".to_string());
        }
    } else if chapters.is_empty() {
        return Err("这本书没有可导出的已下载文字章节".to_string());
    }
    let title = metadata.title.as_deref().unwrap_or(&name);
    let author = metadata.author.as_deref().unwrap_or("未知作者");
    let description = metadata.description.as_deref().unwrap_or("");
    let file_name = match format.as_str() {
        "epub" => format!("{name}.epub"),
        "markdown" => format!("{name}-Markdown.zip"),
        "audio" => format!("{name}-有声.zip"),
        _ => format!("{name}.txt"),
    };
    let target = export_directory(&app)?.join(&file_name);
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
        _ => {
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
    }
    Ok(LocalExport {
        href: format!(
            "asset://localhost/exports/{}",
            urlencoding::encode(&file_name)
        ),
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
    Ok(read_json_or_default::<StoredBookMetadata>(&directory.join(".novel-flow.json"))?.novel_id)
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
                let content = client
                    .chapter_content_from_web(novel_id, volume_id, chapter_id, cookie.as_deref())
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
        let (catalog_title, catalog) = client.audio_catalog(novel_id, &cookie).await?;
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
        let directory = library_directory(&app)?.join(safe_library_name(&catalog_title));
        let audio_directory = directory.join("audio");
        fs::create_dir_all(&audio_directory)
            .map_err(|error| format!("无法创建有声目录：{error}"))?;
        let mut metadata =
            read_json_or_default::<StoredBookMetadata>(&directory.join(".novel-flow.json"))?;
        metadata.novel_id = Some(novel_id);
        metadata.title = Some(if title.trim().is_empty() {
            catalog_title.clone()
        } else {
            title
        });
        metadata
            .author
            .get_or_insert_with(|| "未知作者".to_string());
        metadata
            .description
            .get_or_insert_with(|| "暂无简介".to_string());
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

/// Returns a non-sensitive description of the native runtime.
///
/// This command is intentionally small: it verifies the Vue-to-Rust invoke
/// bridge without exposing filesystem paths, credentials, or network state.
///
/// # Arguments
/// * `name` - Optional display name supplied by the renderer.
///
/// # Returns
/// A greeting suitable for the development diagnostics view.
#[tauri::command]
fn greet(name: &str) -> String {
    let subject = if name.trim().is_empty() {
        "Novel Flow"
    } else {
        name.trim()
    };
    format!("Novel Flow native runtime ready for {subject}.")
}

/// Returns the application runtime version used by the renderer diagnostics.
///
/// The value is compiled into the native binary and has no side effects.
///
/// # Returns
/// The semantic version declared by the Cargo package.
#[tauri::command]
fn app_runtime_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Configures and runs the Tauri application on the current platform.
///
/// # Side effects
/// Registers native state, commands, the opener plugin, Android authentication
/// support when applicable, and restores persisted download jobs as paused.
/// The function blocks until the application exits.
pub fn run() {
    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default()
        .manage(AuthSessionState::default())
        .manage(NativeJobState::default())
        .setup(|app| {
            // A malformed recovery snapshot must not block application startup.
            let _ = restore_native_jobs(&app.handle());
            Ok(())
        })
        .plugin(tauri_plugin_opener::init());
    #[cfg(target_os = "android")]
    {
        builder = builder.plugin(android_sfacg_auth_plugin());
    }
    builder
        .invoke_handler(tauri::generate_handler![
            greet,
            app_runtime_version,
            list_local_library,
            get_request_policy,
            save_request_policy,
            get_content_dictionary,
            update_content_dictionary,
            search_novels,
            get_novel_details,
            get_chapter_volumes,
            get_audio_chapters,
            get_bookshelf,
            get_local_book,
            get_local_chapter,
            delete_local_book,
            export_local_book,
            create_text_download,
            create_audio_download,
            list_download_jobs,
            pause_download_job,
            resume_download_job,
            delete_download_job,
            auth_status,
            login_with_password,
            start_official_login,
            logout,
            verify_authenticated_request,
            get_user_profile
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
