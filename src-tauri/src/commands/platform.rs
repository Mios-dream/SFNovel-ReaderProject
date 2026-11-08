//! 平台侧的应用设置入口命令。

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
