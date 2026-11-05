//! 应用设置命令。

use crate::adapters::storage::NativeStorage;
use crate::application::settings::SettingsService;
use crate::domain::error::AppResult;
use crate::domain::settings::AppSettings;

#[tauri::command]
/// 读取当前应用设置。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 设置文件不存在、损坏或无法读取时返回错误消息。
pub(crate) fn get_settings(app: tauri::AppHandle) -> AppResult<AppSettings> {
    let storage = NativeStorage::new(app);
    SettingsService::new(&storage).settings()
}

#[tauri::command]
/// 校验并保存应用设置。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `settings`：按功能分类的应用设置。
///
/// # 错误
/// 配置越界或写入失败时返回错误消息。
pub(crate) async fn save_settings(
    app: tauri::AppHandle,
    settings: AppSettings,
) -> AppResult<AppSettings> {
    let storage = NativeStorage::new(app.clone());
    #[cfg(target_os = "android")]
    let previous_auto_signin = storage
        .settings()
        .ok()
        .map(|settings| settings.signin.auto_signin_enabled);
    let saved = SettingsService::new(&storage).save_settings(settings)?;
    crate::adapters::downloads::configure_concurrency(saved.download.max_concurrent_downloads);
    #[cfg(target_os = "windows")]
    crate::adapters::platform::set_autostart(saved.desktop.autostart)?;
    #[cfg(target_os = "android")]
    if previous_auto_signin != Some(saved.signin.auto_signin_enabled) {
        crate::adapters::platform::android::configure_auto_signin(
            &app,
            saved.signin.auto_signin_enabled,
        )
        .await?;
    }
    Ok(saved)
}
