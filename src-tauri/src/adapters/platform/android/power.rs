//! Android 的后台运行权限：应用设置入口、通知权限、电池优化白名单。
//!
//! 自动签到依赖后台周期任务，而任务能否存活取决于厂商的清理策略。能查询的只有通知
//! 权限与电池优化白名单；厂商的「自启动」是私有实现：页面名称随系统版本变化，没有
//! 公开 Intent，也没有读取接口。
//!
//! 因此这里不按品牌枚举页面，也不为自启动做任何授权校验——只打开系统提供的应用详情
//! 页，由用户在其中找到「自启动 / 后台」开关。该页面在所有 Android 版本与厂商上都存在，
//! 不会因 ROM 变化失效；代价是需要用户多点一步。

use super::call_auth;
use crate::adapters::platform::BackgroundPermissions;
use crate::domain::error::AppResult;
use serde::Deserialize;

/// 一次授权请求的结果。
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Granted {
    granted: bool,
}

/// 打开本应用的系统详情页，供用户开启厂商的自启动开关。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
pub(crate) async fn open_app_settings(app: &tauri::AppHandle) -> AppResult<()> {
    call_auth::<serde_json::Value, _>(app, "openAppSettings", (), "无法打开 Android 应用设置")
        .await
        .map(|_| ())
}

/// 读取后台运行相关权限的当前状态。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
pub(crate) async fn read_background_permissions(
    app: &tauri::AppHandle,
) -> AppResult<BackgroundPermissions> {
    call_auth::<BackgroundPermissions, _>(
        app,
        "readBackgroundPermissions",
        (),
        "无法读取 Android 后台权限",
    )
    .await
}

/// 请求通知权限，返回用户操作后的实际状态。
///
/// Android 13 起弹系统授权框；更早的版本没有这个运行时权限（关通知只改 AppOps），
/// 原生侧改为打开通知设置页，用户返回后再查一次。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
pub(crate) async fn request_notification_permission(app: &tauri::AppHandle) -> AppResult<bool> {
    let response = call_auth::<Granted, _>(
        app,
        "requestNotificationPermission",
        (),
        "无法请求 Android 通知权限",
    )
    .await?;
    log_info!("通知权限请求结果：{}", response.granted);
    Ok(response.granted)
}

/// 申请加入电池优化白名单，返回申请后的实际状态。
///
/// 系统弹窗的结果无法直接读取，原生侧在用户返回后重新查询一次。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
pub(crate) async fn request_ignore_battery_optimizations(
    app: &tauri::AppHandle,
) -> AppResult<bool> {
    let response = call_auth::<Granted, _>(
        app,
        "requestIgnoreBatteryOptimizations",
        (),
        "无法申请电池优化白名单",
    )
    .await?;
    log_info!("电池优化白名单申请结果：{}", response.granted);
    Ok(response.granted)
}
