//! 请求策略命令。

use crate::adapters::storage::NativeStorage;
use crate::application::settings::SettingsService;
use crate::domain::error::AppError;
use crate::domain::settings::RequestPolicy;

#[tauri::command]
/// 读取当前原生请求策略。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 设置文件不存在、损坏或无法读取时返回错误消息。
pub(crate) fn get_request_policy(app: tauri::AppHandle) -> Result<RequestPolicy, String> {
    let storage = NativeStorage::new(app);
    SettingsService::new(&storage)
        .policy()
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 校验并保存原生请求策略。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `policy`：新的请求间隔和并发下载配置。
///
/// # 错误
/// 配置越界或写入失败时返回错误消息。
pub(crate) fn save_request_policy(
    app: tauri::AppHandle,
    policy: RequestPolicy,
) -> Result<RequestPolicy, String> {
    let storage = NativeStorage::new(app);
    SettingsService::new(&storage)
        .save_policy(policy)
        .map_err(AppError::into_message)
}
