//! 会话与账户命令。

use crate::adapters::platform::NativeAuthGateway;
use crate::adapters::sfacg::cookie::{filter_cookie_header, has_cookie_name};
use crate::adapters::sfacg::device::{adopt_device_id, device_profile};
use crate::application::auth::AuthService;
use crate::application::state::AuthSessionState;
use crate::domain::auth::{AuthStatus, CredentialKind, LoginCredentials, UserProfile};
use crate::domain::error::{AppError, AppResult};
use tauri::Manager;

/// 应用专属外部目录下的凭据文件，内容为 `{"cookie":"…","deviceId":"…"}`。
const DELIVERED_CREDENTIALS: &str =
    "/storage/emulated/0/Android/data/com.sansan.sf_novel_flow/files/login-sync.json";

#[tauri::command]
/// 消费电脑端脚本投递的登录凭据并建立会话。
///
/// 读完立即删除（无论内容是否可用），避免每次启动重复尝试。该路径只在
/// Android 上有意义，其他平台读不到文件，直接返回 `false`。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 返回
/// 是否建立了登录。
///
/// # 错误
/// 设备编号非法或会话持久化失败时返回可展示的错误消息。
pub(crate) async fn consume_delivered_login(app: tauri::AppHandle) -> AppResult<bool> {
    let path = std::path::Path::new(DELIVERED_CREDENTIALS);
    // 目录只能交给系统创建（`getExternalFilesDir`）：应用自己 mkdir 这个包目录会被
    // FUSE 拒绝；也不能让脚本 mkdir，那样属主是 shell、应用删不掉凭据。
    #[cfg(target_os = "android")]
    {
        if path.parent().map(|dir| !dir.is_dir()).unwrap_or(false) {
            if let Err(error) = crate::adapters::platform::android::ensure_app_files_dir(&app).await
            {
                log_warn!("[login-sync] 创建应用专属目录失败：{error}");
            }
        }
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(false);
    };
    let _ = std::fs::remove_file(path);
    let Ok(credentials) = serde_json::from_str::<LoginCredentials>(&text) else {
        return Ok(false);
    };
    adopt_device_id(&credentials.device_id)?;
    connect_cookie(app, credentials.cookie).await?;
    Ok(true)
}

#[tauri::command]
/// 同步平台会话并返回当前认证状态。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 会话同步、上游初始化或状态读取失败时返回可展示的错误消息。
pub(crate) async fn auth_status(app: tauri::AppHandle) -> AppResult<AuthStatus> {
    let remote = super::remote(&app).await?;
    let gateway = NativeAuthGateway::new(app.clone());
    let state = app.state::<AuthSessionState>();
    AuthService::new(&remote, &gateway, &state).status().await
}

#[tauri::command]
/// 使用 SF 账号密码登录。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `username`：SF 账号。
/// - `password`：SF 密码。
///
/// # 错误
/// 账号密码为空或过长、登录被拒、会话持久化失败时返回错误消息。
pub(crate) async fn login_with_password(
    app: tauri::AppHandle,
    username: String,
    password: String,
) -> AppResult<AuthStatus> {
    let remote = super::remote(&app).await?;
    let gateway = NativeAuthGateway::new(app.clone());
    let state = app.state::<AuthSessionState>();
    AuthService::new(&remote, &gateway, &state)
        .login_with_password(&username, &password)
        .await
}

#[tauri::command]
/// 使用其他设备导出的 Cookie 与设备编号登录。
///
/// 类型由 Cookie 自带：App 会话认 `session_APP`，网页会话认 `session_PC`，命令
/// 不再接收类型参数。两者同时出现时按 App 处理——App 导出的是完整 Cookie，可能
/// 连带网页键，而 App 会话是更完整的那一侧。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `cookie`：从已登录设备复制的 Cookie。
/// - `device_id`：与该 Cookie 配套的设备编号；留空表示沿用本机设备编号。
///
/// # 错误
/// Cookie 缺少两个会话键、设备编号不是 UUID、会话持久化失败时返回错误消息。
pub(crate) async fn login_with_cookie(
    app: tauri::AppHandle,
    cookie: String,
    device_id: String,
) -> AppResult<AuthStatus> {
    if has_cookie_name(&cookie, "session_APP") {
        let cookie = normalize_app_cookie(&cookie)?;
        if !device_id.trim().is_empty() {
            adopt_device_id(&device_id)?;
        }
        return connect_cookie(app, cookie).await;
    }
    if has_cookie_name(&cookie, "session_PC") {
        return connect_web_cookie(app, normalize_web_cookie(&cookie)?).await;
    }
    Err(AppError::invalid_input("Cookie错误，无法判断登录类型"))
}

#[tauri::command]
/// 读取当前可复制的登录凭据。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `kind`：要导出的会话；网站会话不需要设备编号。
///
/// # 错误
/// 对应会话未连接、会话同步或设备身份读取失败时返回错误消息。
pub(crate) async fn get_login_credentials(
    app: tauri::AppHandle,
    kind: CredentialKind,
) -> AppResult<LoginCredentials> {
    let remote = super::remote(&app).await?;
    let gateway = NativeAuthGateway::new(app.clone());
    let state = app.state::<AuthSessionState>();
    AuthService::new(&remote, &gateway, &state).status().await?;
    let session = state.snapshot()?;
    match kind {
        CredentialKind::App => Ok(LoginCredentials {
            cookie: session
                .and_then(|value| value.app_cookie)
                .ok_or_else(|| AppError::unauthenticated("尚未连接 App 登录"))?,
            device_id: device_profile()?.device_id,
        }),
        CredentialKind::Web => Ok(LoginCredentials {
            cookie: session
                .and_then(|value| value.web_cookie)
                .and_then(|cookie| filter_cookie_header(&cookie, WEB_COOKIE_NAMES))
                .ok_or_else(|| AppError::unauthenticated("尚未连接网站登录"))?,
            device_id: String::new(),
        }),
    }
}

#[tauri::command]
/// 把文本写入系统剪贴板。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `text`：待复制的文本。
///
/// # 错误
/// 平台不支持原生剪贴板或写入失败时返回错误消息。
pub(crate) async fn copy_to_clipboard(app: tauri::AppHandle, text: String) -> AppResult<()> {
    #[cfg(target_os = "android")]
    {
        return crate::adapters::platform::android::copy_to_clipboard(&app, &text).await;
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, text);
        Err(AppError::internal("当前平台无法写入系统剪贴板"))
    }
}

/// 网页会话用到的 Cookie 键；`.SFCommunity` 标识账号，`session_PC` 才是会话本身。
const WEB_COOKIE_NAMES: &[&str] = &[".SFCommunity", "session_PC"];

/// 用导入的 App Cookie 建立会话。
///
/// 设备编号由调用方先替换，命令只负责装配服务。
async fn connect_cookie(app: tauri::AppHandle, cookie: String) -> AppResult<AuthStatus> {
    let remote = super::remote(&app).await?;
    let gateway = NativeAuthGateway::new(app.clone());
    let state = app.state::<AuthSessionState>();
    AuthService::new(&remote, &gateway, &state)
        .login_with_cookie(&cookie)
        .await
}

/// 用导入的 Web Cookie 建立会话。
async fn connect_web_cookie(app: tauri::AppHandle, cookie: String) -> AppResult<AuthStatus> {
    let remote = super::remote(&app).await?;
    let gateway = NativeAuthGateway::new(app.clone());
    let state = app.state::<AuthSessionState>();
    AuthService::new(&remote, &gateway, &state)
        .login_with_web_cookie(&cookie)
        .await
}

/// 过滤粘贴内容中的网页会话键，并要求存在 `session_PC`。
fn normalize_web_cookie(value: &str) -> AppResult<String> {
    let cookie = filter_cookie_header(value, WEB_COOKIE_NAMES)
        .ok_or_else(|| AppError::invalid_input("请粘贴官方网页的登录 Cookie"))?;
    if !has_cookie_name(&cookie, "session_PC") {
        return Err(AppError::invalid_input(
            "Cookie 缺少 session_PC，请重新从网页登录获取",
        ));
    }
    Ok(cookie)
}

/// 过滤粘贴内容中的 App 会话键，并要求存在 `session_APP`。
fn normalize_app_cookie(value: &str) -> AppResult<String> {
    let cookie = filter_cookie_header(value, &[".SFCommunity", "session_APP"])
        .ok_or_else(|| AppError::invalid_input("请粘贴官方 App 的登录 Cookie"))?;
    if !has_cookie_name(&cookie, "session_APP") {
        return Err(AppError::invalid_input(
            "Cookie 缺少 session_APP，请重新从官方 App 获取",
        ));
    }
    Ok(cookie)
}

#[tauri::command]
/// 启动官方网页登录并保存返回的 Web 会话。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 官方登录取消后通常返回成功；流程失败或未返回 Web Cookie 时返回错误。
pub(crate) async fn start_official_login(app: tauri::AppHandle) -> AppResult<()> {
    let remote = super::remote(&app).await?;
    let gateway = NativeAuthGateway::new(app.clone());
    let state = app.state::<AuthSessionState>();
    AuthService::new(&remote, &gateway, &state)
        .start_official_login()
        .await
}

#[tauri::command]
/// 清除 App 与 Web 的全部登录会话。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 平台清理或状态更新失败时返回错误消息。
pub(crate) async fn logout(app: tauri::AppHandle) -> AppResult<()> {
    let remote = super::remote(&app).await?;
    let gateway = NativeAuthGateway::new(app.clone());
    let state = app.state::<AuthSessionState>();
    AuthService::new(&remote, &gateway, &state).logout().await
}

#[tauri::command]
/// 仅退出 App 会话并返回剩余认证状态。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// App 会话清理或状态更新失败时返回错误消息。
pub(crate) async fn logout_app_session(app: tauri::AppHandle) -> AppResult<AuthStatus> {
    let remote = super::remote(&app).await?;
    let gateway = NativeAuthGateway::new(app.clone());
    let state = app.state::<AuthSessionState>();
    AuthService::new(&remote, &gateway, &state)
        .logout_app()
        .await
}

#[tauri::command]
/// 仅退出 Web 会话并返回剩余认证状态。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// Web 会话清理或状态更新失败时返回错误消息。
pub(crate) async fn logout_web_session(app: tauri::AppHandle) -> AppResult<AuthStatus> {
    let remote = super::remote(&app).await?;
    let gateway = NativeAuthGateway::new(app.clone());
    let state = app.state::<AuthSessionState>();
    AuthService::new(&remote, &gateway, &state)
        .logout_web()
        .await
}

#[tauri::command]
/// 通过一次真实账号请求验证当前登录是否有效。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 未登录、会话过期或上游请求失败时返回错误消息。
pub(crate) async fn verify_authenticated_request(app: tauri::AppHandle) -> AppResult<AuthStatus> {
    let remote = super::remote(&app).await?;
    let gateway = NativeAuthGateway::new(app.clone());
    let state = app.state::<AuthSessionState>();
    AuthService::new(&remote, &gateway, &state).verify().await
}

#[tauri::command]
/// 读取当前 SF 账户资料。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 未登录或上游资料接口失败时返回错误消息。
pub(crate) async fn get_account_profile(app: tauri::AppHandle) -> AppResult<UserProfile> {
    let remote = super::remote(&app).await?;
    let gateway = NativeAuthGateway::new(app.clone());
    let state = app.state::<AuthSessionState>();
    AuthService::new(&remote, &gateway, &state).profile().await
}
