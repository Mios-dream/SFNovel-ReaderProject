//! 官方网页登录窗口与第三方 OAuth 子窗口。
//!
//! 登录页必须由真实 WebView 打开：SF 的登录流程依赖同源 Cookie，
//! 无法用纯 HTTP 请求复现。

use crate::domain::auth::AuthSession;
use crate::domain::error::{AppError, AppResult};
use std::sync::atomic::{AtomicUsize, Ordering};
use tauri::{webview::NewWindowResponse, Manager, WebviewUrl, WebviewWindowBuilder};

/// SF 官方网页登录入口。
const OFFICIAL_LOGIN_URL: &str = "https://passport.sfacg.com/";
/// 官方登录 WebView 窗口的唯一标签。
const OFFICIAL_LOGIN_WINDOW_LABEL: &str = "sfacg-official-login";
/// 第三方 OAuth 子窗口标签前缀。
const OAUTH_LOGIN_WINDOW_PREFIX: &str = "sfacg-oauth-login-";
/// 第三方 OAuth 子窗口编号。
static OAUTH_LOGIN_WINDOW_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

/// 打开官方登录窗口，轮询 SF 域 Cookie 并返回 Web 会话。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 窗口创建、导航、Cookie 读取失败或等待超时时返回错误消息。
pub(crate) async fn start_official_login(app: &tauri::AppHandle) -> AppResult<AuthSession> {
    let login_url: tauri::Url = OFFICIAL_LOGIN_URL
        .parse()
        .map_err(|error| AppError::upstream(format!("官方登录地址无效：{error}")))?;
    let oauth_app = app.clone();
    let close_app = app.clone();
    let login_window = if let Some(window) = app.get_webview_window(OFFICIAL_LOGIN_WINDOW_LABEL) {
        window
            .set_focus()
            .map_err(|error| AppError::internal(format!("无法激活官方登录窗口：{error}")))?;
        window
    } else {
        WebviewWindowBuilder::new(
            app,
            OFFICIAL_LOGIN_WINDOW_LABEL,
            WebviewUrl::External(login_url.clone()),
        )
        .title("SF 官方登录")
        .inner_size(1020.0, 760.0)
        .min_inner_size(820.0, 600.0)
        .resizable(true)
        .on_new_window(move |url, features| {
            if url.scheme() != "https" {
                return NewWindowResponse::Deny;
            }
            let Ok(blank) = "about:blank".parse::<tauri::Url>() else {
                return NewWindowResponse::Deny;
            };
            let label = format!(
                "{OAUTH_LOGIN_WINDOW_PREFIX}{}",
                OAUTH_LOGIN_WINDOW_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            );
            match WebviewWindowBuilder::new(&oauth_app, label, WebviewUrl::External(blank))
                .window_features(features)
                .title("QQ / 微信登录")
                .inner_size(520.0, 760.0)
                .resizable(true)
                .center()
                .build()
            {
                Ok(window) => NewWindowResponse::Create { window },
                Err(_) => NewWindowResponse::Deny,
            }
        })
        .center()
        .build()
        .map_err(|error| AppError::internal(format!("无法创建官方登录窗口：{error}")))?
    };
    login_window.on_window_event(move |event| {
        if matches!(
            event,
            tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed
        ) {
            close_oauth_windows(&close_app);
        }
    });
    login_window
        .clear_all_browsing_data()
        .map_err(|error| AppError::internal(format!("无法清除官方登录状态：{error}")))?;
    login_window
        .navigate(login_url)
        .map_err(|error| AppError::internal(format!("无法打开官方登录页：{error}")))?;

    let cookie_urls = [
        "https://book.sfacg.com/",
        "https://api.sfacg.com/",
        OFFICIAL_LOGIN_URL,
    ];
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15 * 60);
    loop {
        if std::time::Instant::now() >= deadline {
            let _ = login_window.close();
            close_oauth_windows(app);
            return Err(AppError::upstream("官方登录等待超时，请重新打开登录窗口"));
        }
        let mut session_cookie = None;
        for cookie_url in cookie_urls {
            let Some(window) = app.get_webview_window(OFFICIAL_LOGIN_WINDOW_LABEL) else {
                close_oauth_windows(app);
                return Err(AppError::upstream("官方登录窗口已关闭，未检测到登录会话"));
            };
            let url = cookie_url
                .parse()
                .map_err(|error| AppError::internal(format!("官方 Cookie 地址无效：{error}")))?;
            let cookies = tokio::task::spawn_blocking(move || window.cookies_for_url(url))
                .await
                .map_err(|error| AppError::internal(format!("读取官方登录 Cookie 失败：{error}")))?
                .map_err(|error| AppError::internal(format!("读取官方登录 Cookie 失败：{error}")))?;
            session_cookie = merge_sfacg_session_cookie(session_cookie, cookies);
        }
        if let Some(cookie) = session_cookie.filter(|value| {
            value
                .split(';')
                .any(|part| part.trim().starts_with(".SFCommunity="))
        }) {
            let _ = login_window.close();
            close_oauth_windows(app);
            return Ok(AuthSession {
                app_cookie: None,
                web_cookie: Some(cookie),
                user_name: "已登录 SF 账号".to_string(),
            });
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}

/// 关闭本次登录产生的第三方 OAuth 子窗口。
fn close_oauth_windows(app: &tauri::AppHandle) {
    for (label, window) in app.webview_windows() {
        if label.starts_with(OAUTH_LOGIN_WINDOW_PREFIX) {
            let _ = window.close();
        }
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
