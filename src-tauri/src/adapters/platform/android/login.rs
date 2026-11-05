//! Android 官方登录窗口。

use super::call_auth;
use crate::domain::error::AppResult;
use serde_json::Value;

/// 打开平台官方登录页。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 插件无法打开登录页时返回存储类错误。
pub(crate) async fn start_official_login(app: &tauri::AppHandle) -> AppResult<()> {
    call_auth::<Value, _>(app, "startOfficialLogin", (), "无法打开 Android 官方登录页")
        .await
        .map(|_| ())
}
