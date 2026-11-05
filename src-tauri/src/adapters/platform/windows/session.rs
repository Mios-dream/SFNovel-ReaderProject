//! Windows 登录会话的 DPAPI 加密与落盘。
//!
//! App 会话与网页会话各存一份文件，内容都由 DPAPI 以当前用户密钥加密；
//! 写入统一走 `replace_file`，避免掉电留下半截文件。

use crate::adapters::storage::replace_file;
use crate::domain::auth::AuthSession;
use crate::domain::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::Manager;

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

/// 返回 App 会话加密文件路径。
///
/// # 错误
/// 应用数据目录无法解析时返回错误消息。
fn auth_session_path(app: &tauri::AppHandle) -> AppResult<PathBuf> {
    app.path()
        .app_data_dir()
        .map(|directory| directory.join("auth-session.bin"))
        .map_err(|error| AppError::storage(format!("无法解析登录会话路径：{error}")))
}

/// 返回 Web 会话加密文件路径。
///
/// # 错误
/// 应用数据目录无法解析时返回错误消息。
fn web_session_path(app: &tauri::AppHandle) -> AppResult<PathBuf> {
    app.path()
        .app_data_dir()
        .map(|directory| directory.join("web-session.v2.bin"))
        .map_err(|error| AppError::storage(format!("无法解析网页登录会话路径：{error}")))
}

/// 使用 Windows DPAPI 加密敏感数据。
///
/// # 参数
/// - `input`：需要加密的明文。
///
/// # 错误
/// DPAPI 调用失败时返回系统错误码消息。
pub(crate) fn protect_auth_data(input: &[u8]) -> AppResult<Vec<u8>> {
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
        return Err(AppError::storage(format!(
            "无法加密登录会话（错误码 {}）",
            unsafe { GetLastError() }
        )));
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
pub(crate) fn unprotect_auth_data(input: &[u8]) -> AppResult<Vec<u8>> {
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
        return Err(AppError::storage(format!(
            "无法解密登录会话（错误码 {}）",
            unsafe { GetLastError() }
        )));
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
pub(crate) fn write_encrypted(path: &Path, payload: &[u8]) -> AppResult<()> {
    let encrypted = protect_auth_data(payload)?;
    replace_file(path, &encrypted, "登录会话")
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
pub(crate) fn persist_app_session(
    app: &tauri::AppHandle,
    cookie: &str,
    user_name: &str,
) -> AppResult<()> {
    let payload = serde_json::to_vec(&PersistedAuthSession {
        version: 2,
        cookie: cookie.to_string(),
        user_name: user_name.to_string(),
    })
    .map_err(|error| AppError::storage(format!("无法序列化登录会话：{error}")))?;
    write_encrypted(&auth_session_path(app)?, &payload)
}

/// 读取并解密 App 会话；文件缺失或版本无效时返回 `None`。
///
/// # 错误
/// 文件读取、解密或 JSON 解析失败时返回错误消息。
pub(crate) fn restore_app_session(app: &tauri::AppHandle) -> AppResult<Option<AuthSession>> {
    let path = auth_session_path(app)?;
    if !path.is_file() {
        return Ok(None);
    }
    let payload = unprotect_auth_data(
        &fs::read(path).map_err(|error| AppError::storage(format!("无法读取登录会话：{error}")))?,
    )?;
    let stored = serde_json::from_slice::<PersistedAuthSession>(&payload)
        .map_err(|error| AppError::storage(format!("登录会话格式无效：{error}")))?;
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
pub(crate) fn clear_app_session(app: &tauri::AppHandle) -> AppResult<()> {
    let path = auth_session_path(app)?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| AppError::storage(format!("无法清除登录会话：{error}")))?;
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
pub(crate) fn persist_web_session(app: &tauri::AppHandle, cookie: &str) -> AppResult<()> {
    let payload = serde_json::to_vec(&PersistedAuthSession {
        version: 2,
        cookie: cookie.to_string(),
        user_name: "已登录 SF 账号".to_string(),
    })
    .map_err(|error| AppError::storage(format!("无法序列化网页登录会话：{error}")))?;
    write_encrypted(&web_session_path(app)?, &payload)
}

/// 读取并解密 Web 会话；文件缺失或版本无效时返回 `None`。
///
/// # 错误
/// 文件读取、解密或 JSON 解析失败时返回错误消息。
pub(crate) fn restore_web_session(app: &tauri::AppHandle) -> AppResult<Option<AuthSession>> {
    let path = web_session_path(app)?;
    if !path.is_file() {
        return Ok(None);
    }
    let payload = unprotect_auth_data(
        &fs::read(path).map_err(|error| AppError::storage(format!("无法读取网页登录会话：{error}")))?,
    )?;
    let stored = serde_json::from_slice::<PersistedAuthSession>(&payload)
        .map_err(|error| AppError::storage(format!("网页登录会话格式无效：{error}")))?;
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
pub(crate) fn clear_web_session(app: &tauri::AppHandle) -> AppResult<()> {
    let path = web_session_path(app)?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| AppError::storage(format!("无法清除网页登录会话：{error}")))?;
    }
    Ok(())
}
