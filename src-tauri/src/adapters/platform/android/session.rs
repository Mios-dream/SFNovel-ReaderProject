//! Android 会话读写与自动签到开关。

use super::call_auth;
use crate::domain::auth::AuthSession;
use crate::domain::error::AppResult;
use serde::Deserialize;
use serde_json::Value;

/// 认证插件返回的单个 Cookie。
#[derive(Deserialize)]
struct CookieResponse {
    cookie: Option<String>,
}

/// 读取平台保存的 App 与网站会话。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
pub(crate) async fn synchronize_session(app: &tauri::AppHandle) -> AppResult<Option<AuthSession>> {
    let web_cookie = read_cookie(app, "readSessionCookie", "无法读取 Android 登录会话").await?;
    let app_cookie = read_cookie(app, "readAppSessionCookie", "无法读取 Android App 会话").await?;
    Ok(if app_cookie.is_some() || web_cookie.is_some() {
        Some(AuthSession {
            app_cookie,
            web_cookie,
            user_name: "已登录 SF 账号".to_string(),
        })
    } else {
        None
    })
}

/// 读取一个非空 Cookie。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `method`：Java 侧的插件方法名。
/// - `action`：错误消息前缀。
async fn read_cookie(
    app: &tauri::AppHandle,
    method: &str,
    action: &str,
) -> AppResult<Option<String>> {
    let response = call_auth::<CookieResponse, _>(app, method, (), action).await?;
    Ok(response.cookie.filter(|value| !value.trim().is_empty()))
}

/// 将 App Cookie 写入平台存储。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `cookie`：待保存的 App Cookie。
pub(crate) async fn persist_app_session(app: &tauri::AppHandle, cookie: &str) -> AppResult<()> {
    call_auth::<Value, _>(
        app,
        "writeAppSessionCookie",
        serde_json::json!({ "cookie": cookie }),
        "无法保存 Android 登录会话",
    )
    .await
    .map(|_| ())
}

/// 清除全部平台会话。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
pub(crate) async fn clear_session(app: &tauri::AppHandle) -> AppResult<()> {
    call_auth::<Value, _>(app, "clearSessionCookie", (), "无法清除 Android 登录会话")
        .await
        .map(|_| ())
}

/// 仅清除 App 会话。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
pub(crate) async fn clear_app_session(app: &tauri::AppHandle) -> AppResult<()> {
    call_auth::<Value, _>(
        app,
        "clearAppSessionCookie",
        (),
        "无法清除 Android App 登录会话",
    )
    .await
    .map(|_| ())
}

/// 仅清除网站会话。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
pub(crate) async fn clear_web_session(app: &tauri::AppHandle) -> AppResult<()> {
    call_auth::<Value, _>(
        app,
        "clearWebSessionCookie",
        (),
        "无法清除 Android 网站登录会话",
    )
    .await
    .map(|_| ())
}

/// 开关平台侧的自动签到。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `enabled`：是否启用自动签到。
pub(crate) async fn configure_auto_signin(app: &tauri::AppHandle, enabled: bool) -> AppResult<()> {
    call_auth::<Value, _>(
        app,
        "setAutoSignin",
        serde_json::json!({ "enabled": enabled }),
        "无法配置 Android 自动签到",
    )
    .await
    .map(|_| ())
}
