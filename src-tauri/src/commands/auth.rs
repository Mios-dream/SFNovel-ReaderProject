//! 会话与账户命令。

use crate::adapters::platform::NativeAuthGateway;
use crate::adapters::sfacg::source::SfSource;
use crate::application::auth::AuthService;
use crate::application::state::AuthSessionState;
use crate::domain::auth::{AuthStatus, UserProfile};
use crate::domain::error::AppError;
use tauri::Manager;

/// 根据当前应用句柄创建已装配的 SF 内容源。
///
/// # 参数
/// - `app`：Tauri 应用句柄，用于读取平台状态和持久化会话。
async fn source(app: &tauri::AppHandle) -> Result<SfSource, AppError> {
    SfSource::connect(app).await.map_err(AppError::upstream)
}

#[tauri::command]
/// 同步平台会话并返回当前认证状态。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 会话同步、上游初始化或状态读取失败时返回可展示的错误消息。
pub(crate) async fn auth_status(app: tauri::AppHandle) -> Result<AuthStatus, String> {
    let remote = source(&app).await.map_err(AppError::into_message)?;
    let gateway = NativeAuthGateway::new(app.clone());
    let state = app.state::<AuthSessionState>();
    AuthService::new(&remote, &gateway, &state)
        .status()
        .await
        .map_err(AppError::into_message)
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
) -> Result<AuthStatus, String> {
    let remote = source(&app).await.map_err(AppError::into_message)?;
    let gateway = NativeAuthGateway::new(app.clone());
    let state = app.state::<AuthSessionState>();
    AuthService::new(&remote, &gateway, &state)
        .login_with_password(&username, &password)
        .await
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 启动官方网页登录并保存返回的 Web 会话。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 官方登录取消后通常返回成功；流程失败或未返回 Web Cookie 时返回错误。
pub(crate) async fn start_official_login(app: tauri::AppHandle) -> Result<(), String> {
    let remote = source(&app).await.map_err(AppError::into_message)?;
    let gateway = NativeAuthGateway::new(app.clone());
    let state = app.state::<AuthSessionState>();
    AuthService::new(&remote, &gateway, &state)
        .start_official_login()
        .await
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 清除 App 与 Web 的全部登录会话。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 平台清理或状态更新失败时返回错误消息。
pub(crate) async fn logout(app: tauri::AppHandle) -> Result<(), String> {
    let remote = source(&app).await.map_err(AppError::into_message)?;
    let gateway = NativeAuthGateway::new(app.clone());
    let state = app.state::<AuthSessionState>();
    AuthService::new(&remote, &gateway, &state)
        .logout()
        .await
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 仅退出 App 会话并返回剩余认证状态。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// App 会话清理或状态更新失败时返回错误消息。
pub(crate) async fn logout_app_session(app: tauri::AppHandle) -> Result<AuthStatus, String> {
    let remote = source(&app).await.map_err(AppError::into_message)?;
    let gateway = NativeAuthGateway::new(app.clone());
    let state = app.state::<AuthSessionState>();
    AuthService::new(&remote, &gateway, &state)
        .logout_app()
        .await
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 仅退出 Web 会话并返回剩余认证状态。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// Web 会话清理或状态更新失败时返回错误消息。
pub(crate) async fn logout_web_session(app: tauri::AppHandle) -> Result<AuthStatus, String> {
    let remote = source(&app).await.map_err(AppError::into_message)?;
    let gateway = NativeAuthGateway::new(app.clone());
    let state = app.state::<AuthSessionState>();
    AuthService::new(&remote, &gateway, &state)
        .logout_web()
        .await
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 通过一次真实账号请求验证当前登录是否有效。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 未登录、会话过期或上游请求失败时返回错误消息。
pub(crate) async fn verify_authenticated_request(
    app: tauri::AppHandle,
) -> Result<AuthStatus, String> {
    let remote = source(&app).await.map_err(AppError::into_message)?;
    let gateway = NativeAuthGateway::new(app.clone());
    let state = app.state::<AuthSessionState>();
    AuthService::new(&remote, &gateway, &state)
        .verify()
        .await
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 读取当前 SF 账户资料。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 未登录或上游资料接口失败时返回错误消息。
pub(crate) async fn get_account_profile(app: tauri::AppHandle) -> Result<UserProfile, String> {
    let remote = source(&app).await.map_err(AppError::into_message)?;
    let gateway = NativeAuthGateway::new(app.clone());
    let state = app.state::<AuthSessionState>();
    AuthService::new(&remote, &gateway, &state)
        .profile()
        .await
        .map_err(AppError::into_message)
}
