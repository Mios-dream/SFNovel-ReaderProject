//! 应用更新命令。

use crate::adapters::updater;
use crate::domain::error::AppError;
use crate::domain::update::UpdateInfo;
use tauri::AppHandle;

#[tauri::command]
/// 检查 GitHub 是否发布了高于当前版本的最新 Release。
pub(crate) async fn check_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    updater::check(&app).await.map_err(AppError::into_message)
}
