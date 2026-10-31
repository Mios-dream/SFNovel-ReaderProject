//! Windows 会话加密、设备身份与官方登录窗口。

use crate::adapters::sfacg::device::{
    install_device_profile, install_device_state_path, is_valid_uuid, DeviceProfile,
};
use crate::domain::auth::AuthSession;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

/// SF 官方网页登录入口。
const OFFICIAL_LOGIN_URL: &str = "https://m.sfacg.com/login";
/// 官方登录 WebView 窗口的唯一标签。
const OFFICIAL_LOGIN_WINDOW_LABEL: &str = "sfacg-official-login";

#[derive(Deserialize, Serialize)]
/// 加密保存的 App 或 Web 登录会话。
struct PersistedAuthSession {
    /// 持久化格式版本。
    version: u8,
    /// Cookie 请求头内容。
    cookie: String,
    /// 账号展示名称。
    user_name: String,
}

#[derive(Deserialize, Serialize)]
/// 加密保存的设备身份。
struct PersistedDeviceIdentity {
    /// 持久化格式版本。
    version: u8,
    /// 设备 UUID。
    device_id: String,
}

/// 返回 App 会话加密文件路径。
///
/// # 错误
/// 应用数据目录无法解析时返回错误消息。
fn auth_session_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|directory| directory.join("auth-session.bin"))
        .map_err(|error| format!("无法解析登录会话路径：{error}"))
}

/// 返回 Web 会话加密文件路径。
///
/// # 错误
/// 应用数据目录无法解析时返回错误消息。
fn web_session_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|directory| directory.join("web-session.v2.bin"))
        .map_err(|error| format!("无法解析网页登录会话路径：{error}"))
}

/// 使用 Windows DPAPI 加密敏感数据。
///
/// # 参数
/// - `input`：需要加密的明文。
///
/// # 错误
/// DPAPI 调用失败时返回系统错误码消息。
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

/// 使用 Windows DPAPI 解密敏感数据。
///
/// # 参数
/// - `input`：由 `protect_auth_data` 生成的密文。
///
/// # 错误
/// DPAPI 调用失败时返回系统错误码消息。
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

/// 加密并通过临时文件替换写入敏感数据。
///
/// # 参数
/// - `path`：目标文件路径。
/// - `payload`：需要加密的明文。
///
/// # 错误
/// 目录创建、加密、写入或替换失败时返回错误消息。
fn write_encrypted(path: &PathBuf, payload: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "登录会话目录无效".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("无法创建登录会话目录：{error}"))?;
    let encrypted = protect_auth_data(payload)?;
    let temporary = path.with_extension("bin.tmp");
    fs::write(&temporary, encrypted).map_err(|error| format!("无法保存登录会话：{error}"))?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| format!("无法替换登录会话：{error}"))?;
    }
    fs::rename(temporary, path).map_err(|error| format!("无法完成登录会话保存：{error}"))
}

/// 加密保存 App Cookie 和账号名称。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `cookie`：App 会话 Cookie。
/// - `user_name`：账号展示名称。
///
/// # 错误
/// 序列化、路径解析或加密写入失败时返回错误消息。
pub(super) fn persist_app_session(
    app: &tauri::AppHandle,
    cookie: &str,
    user_name: &str,
) -> Result<(), String> {
    let payload = serde_json::to_vec(&PersistedAuthSession {
        version: 2,
        cookie: cookie.to_string(),
        user_name: user_name.to_string(),
    })
    .map_err(|error| format!("无法序列化登录会话：{error}"))?;
    write_encrypted(&auth_session_path(app)?, &payload)
}

/// 读取并解密 App 会话；文件缺失或版本无效时返回 `None`。
///
/// # 错误
/// 文件读取、解密或 JSON 解析失败时返回错误消息。
pub(crate) fn restore_app_session(app: &tauri::AppHandle) -> Result<Option<AuthSession>, String> {
    let path = auth_session_path(app)?;
    if !path.is_file() {
        return Ok(None);
    }
    let payload = unprotect_auth_data(
        &fs::read(path).map_err(|error| format!("无法读取登录会话：{error}"))?,
    )?;
    let stored = serde_json::from_slice::<PersistedAuthSession>(&payload)
        .map_err(|error| format!("登录会话格式无效：{error}"))?;
    if stored.version != 2 || stored.cookie.trim().is_empty() || stored.user_name.trim().is_empty()
    {
        return Ok(None);
    }
    Ok(Some(AuthSession {
        app_cookie: Some(stored.cookie),
        web_cookie: None,
        user_name: stored.user_name,
    }))
}

/// 删除持久化的 App 会话文件。
///
/// # 错误
/// 文件存在但无法删除时返回错误消息。
pub(super) fn clear_app_session(app: &tauri::AppHandle) -> Result<(), String> {
    let path = auth_session_path(app)?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| format!("无法清除登录会话：{error}"))?;
    }
    Ok(())
}

/// 加密保存 Web Cookie。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `cookie`：网页会话 Cookie。
///
/// # 错误
/// 序列化、路径解析或加密写入失败时返回错误消息。
pub(super) fn persist_web_session(app: &tauri::AppHandle, cookie: &str) -> Result<(), String> {
    let payload = serde_json::to_vec(&PersistedAuthSession {
        version: 2,
        cookie: cookie.to_string(),
        user_name: "已登录 SF 账号".to_string(),
    })
    .map_err(|error| format!("无法序列化网页登录会话：{error}"))?;
    write_encrypted(&web_session_path(app)?, &payload)
}

/// 读取并解密 Web 会话；文件缺失或版本无效时返回 `None`。
///
/// # 错误
/// 文件读取、解密或 JSON 解析失败时返回错误消息。
pub(crate) fn restore_web_session(app: &tauri::AppHandle) -> Result<Option<AuthSession>, String> {
    let path = web_session_path(app)?;
    if !path.is_file() {
        return Ok(None);
    }
    let payload = unprotect_auth_data(
        &fs::read(path).map_err(|error| format!("无法读取网页登录会话：{error}"))?,
    )?;
    let stored = serde_json::from_slice::<PersistedAuthSession>(&payload)
        .map_err(|error| format!("网页登录会话格式无效：{error}"))?;
    if stored.version != 2 || stored.cookie.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(AuthSession {
        app_cookie: None,
        web_cookie: Some(stored.cookie),
        user_name: stored.user_name,
    }))
}

/// 删除持久化的 Web 会话文件。
///
/// # 错误
/// 文件存在但无法删除时返回错误消息。
pub(super) fn clear_web_session(app: &tauri::AppHandle) -> Result<(), String> {
    let path = web_session_path(app)?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| format!("无法清除网页登录会话：{error}"))?;
    }
    Ok(())
}

/// 恢复或生成设备身份，加密落盘并安装到进程全局状态。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 路径解析、文件读取、加密写入或设备身份生成失败时返回错误消息。
pub(super) fn initialize_device_identity(app: &tauri::AppHandle) -> Result<(), String> {
    let path = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("无法解析设备身份路径：{error}"))?
        .join("device-identity.v1.bin");
    let stored = fs::read(&path)
        .ok()
        .and_then(|encrypted| unprotect_auth_data(&encrypted).ok())
        .and_then(|payload| serde_json::from_slice::<PersistedDeviceIdentity>(&payload).ok())
        .filter(|identity| identity.version == 2 && is_valid_uuid(&identity.device_id));
    let profile = match stored {
        Some(identity) => DeviceProfile {
            device_id: identity.device_id.to_lowercase(),
            ..DeviceProfile::generate()
        },
        None => {
            if path.is_file() {
                let _ = fs::remove_file(&path);
            }
            DeviceProfile::generate()
        }
    };
    let payload = serde_json::to_vec(&PersistedDeviceIdentity {
        version: 2,
        device_id: profile.device_id.clone(),
    })
    .map_err(|error| format!("无法序列化设备身份：{error}"))?;
    write_encrypted(&path, &payload)?;
    install_device_state_path(path.with_file_name("device-state.json"));
    install_device_profile(profile);
    Ok(())
}

/// 打开官方登录窗口，轮询 SF 域 Cookie 并返回 Web 会话。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 窗口创建、导航、Cookie 读取失败或等待超时时返回错误消息。
pub(super) async fn start_official_login(app: &tauri::AppHandle) -> Result<AuthSession, String> {
    let login_url: tauri::Url = OFFICIAL_LOGIN_URL
        .parse()
        .map_err(|error| format!("官方登录地址无效：{error}"))?;
    let login_window = if let Some(window) = app.get_webview_window(OFFICIAL_LOGIN_WINDOW_LABEL) {
        window
            .set_focus()
            .map_err(|error| format!("无法激活官方登录窗口：{error}"))?;
        window
    } else {
        WebviewWindowBuilder::new(
            app,
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
            let _ = login_window.close();
            return Ok(AuthSession {
                app_cookie: None,
                web_cookie: Some(cookie),
                user_name: "已登录 SF 账号".to_string(),
            });
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}

/// 合并已采集的 Cookie，只保留 SF 会话所需的名称。
///
/// # 参数
/// - `current`：此前从其他 SF 域取得的 Cookie 请求头。
/// - `cookies`：当前 WebView 域返回的 Cookie。
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
    if cookie_pairs.is_empty() {
        return None;
    }
    Some(
        cookie_pairs
            .into_iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join("; "),
    )
}
