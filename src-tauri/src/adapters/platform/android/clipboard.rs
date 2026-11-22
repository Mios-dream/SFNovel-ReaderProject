//! 系统剪贴板写入。

use super::call_auth;
use crate::domain::error::AppResult;
use serde_json::Value;

/// 把文本写入系统剪贴板。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `text`：待复制的文本。
///
/// # 错误
/// 插件调用失败时返回存储类错误。
pub(crate) async fn copy_to_clipboard(app: &tauri::AppHandle, text: &str) -> AppResult<()> {
    call_auth::<Value, _>(
        app,
        "copyToClipboard",
        serde_json::json!({ "text": text }),
        "无法复制到剪贴板",
    )
    .await
    .map(|_| ())
}
