//! Android 的应用设置入口与通知权限。
//!
//! 自动签到依赖后台周期任务，而任务能否存活取决于厂商的清理策略。厂商的「自启动」
//! 开关是私有实现：页面名称随系统版本变化，没有公开 Intent，也没有读取接口。
//!
//! 因此这里不按品牌枚举页面，也不做任何授权校验——只打开系统提供的应用详情页，
//! 由用户在其中找到「自启动 / 后台」开关。该页面在所有 Android 版本与厂商上都存在，
//! 不会因 ROM 变化失效；代价是需要用户多点一步。

use super::call_auth;
use crate::domain::error::AppResult;
use serde::Deserialize;

/// 通知权限的请求结果。
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NotificationPermission {
    granted: bool,
}

/// 打开本应用的系统详情页。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
pub(crate) async fn open_app_settings(app: &tauri::AppHandle) -> AppResult<()> {
    call_auth::<serde_json::Value, _>(app, "openAppSettings", (), "无法打开 Android 应用设置")
        .await
        .map(|_| ())
}

/// 请求通知权限，返回是否已授予。
///
/// Android 13 起没有该权限时通知会被静默丢弃；用户拒绝不影响签到本身。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
pub(crate) async fn request_notification_permission(app: &tauri::AppHandle) -> AppResult<bool> {
    let response = call_auth::<NotificationPermission, _>(
        app,
        "requestNotificationPermission",
        (),
        "无法请求 Android 通知权限",
    )
    .await?;
    log_info!("通知权限请求结果：{}", response.granted);
    Ok(response.granted)
}
