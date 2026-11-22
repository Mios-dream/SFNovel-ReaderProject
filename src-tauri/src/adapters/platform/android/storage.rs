//! Android 外部存储权限与导出落盘。

use super::call_auth;
use crate::domain::error::{AppError, AppResult};
use serde_json::Value;
use std::path::Path;

/// 外部书库在共享存储中的位置。
const EXTERNAL_LIBRARY: &str = "/storage/emulated/0/Download/SF Novel Flow";

/// 请求或确认外部存储访问权限，并在获准后准备书库目录。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 权限查询失败或目录无法创建时返回存储类错误。
pub(crate) async fn ensure_external_storage_access(app: &tauri::AppHandle) -> AppResult<bool> {
    let result = call_auth::<Value, _>(
        app,
        "ensureExternalStorageAccess",
        (),
        "无法检查 Android 外部存储权限",
    )
    .await?;
    let granted = result
        .get("granted")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if granted {
        std::fs::create_dir_all(EXTERNAL_LIBRARY)
            .map_err(|error| AppError::storage(format!("无法创建外部书库目录：{error}")))?;
    }
    Ok(granted)
}

/// 让系统创建应用专属外部目录（`Android/data/<包名>/files`）。
///
/// 只能由 `getExternalFilesDir` 创建：应用进程自己 `mkdir` 这个包目录会被 FUSE
/// 拒绝（EACCES），而 shell 建的目录属主是 shell，应用反而删不掉自己那份凭据。
/// 电脑端脚本投递登录凭据到这里，所以启动时确保目录存在。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 系统拒绝创建目录时返回存储类错误。
pub(crate) async fn ensure_app_files_dir(app: &tauri::AppHandle) -> AppResult<()> {
    call_auth::<Value, _>(app, "ensureAppFilesDir", (), "无法创建应用专属目录")
        .await
        .map(|_| ())
}

/// 在系统文件管理器中打开下载目录。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
pub(crate) async fn open_download_directory(app: &tauri::AppHandle) -> AppResult<()> {
    call_auth::<Value, _>(app, "openDownloadDirectory", (), "无法打开下载目录")
        .await
        .map(|_| ())
}

/// 把临时导出文件写入系统保存位置。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `source`：本地临时文件路径。
/// - `uri`：系统文件选择器返回的 `content://` 地址。
pub(crate) async fn write_export_to_uri(
    app: &tauri::AppHandle,
    source: &Path,
    uri: &str,
) -> AppResult<()> {
    call_auth::<Value, _>(
        app,
        "writeExportToUri",
        serde_json::json!({
            "sourcePath": source.to_string_lossy(),
            "uri": uri,
        }),
        "无法写入系统保存位置",
    )
    .await
    .map(|_| ())
}
