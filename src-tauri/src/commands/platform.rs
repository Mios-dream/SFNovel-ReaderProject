//! 平台侧的应用设置入口与通知权限命令。

use crate::domain::error::AppResult;

/// 打开本应用的系统详情页，供用户手动开启厂商的自启动与后台白名单。
///
/// 桌面端没有这类管控，什么也不做。
#[tauri::command]
pub(crate) async fn open_app_settings(app: tauri::AppHandle) -> AppResult<()> {
    #[cfg(target_os = "android")]
    {
        return crate::adapters::platform::android::open_app_settings(&app).await;
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Ok(())
    }
}

/// 请求通知权限，返回是否已授予。
///
/// 自动签到在后台完成，结果通过系统通知告知；用户拒绝时自动签到照常执行。
#[tauri::command]
pub(crate) async fn request_notification_permission(app: tauri::AppHandle) -> AppResult<bool> {
    #[cfg(target_os = "android")]
    {
        return crate::adapters::platform::android::request_notification_permission(&app).await;
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Ok(false)
    }
}
