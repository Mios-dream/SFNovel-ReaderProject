//! Android 窗口刷新率偏好。
//!
//! 偏好本身存在设置文件里，这里只负责把它推到原生侧：Kotlin 会再存一份到自己的
//! 偏好中，好让下次冷启动在 Rust 初始化之前就能按正确的刷新率创建窗口。

use crate::domain::error::AppResult;
use serde_json::Value;

use super::call_auth;

/// 把「高刷新率」偏好写入原生侧，并立刻作用到当前窗口。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `enabled`：是否跟随屏幕的最高刷新率；关闭时窗口被压回 60Hz。
///
/// # 错误
/// 原生插件调用失败时返回错误消息。
pub(crate) async fn configure_peak_refresh_rate(
    app: &tauri::AppHandle,
    enabled: bool,
) -> AppResult<()> {
    call_auth::<Value, _>(
        app,
        "setPeakRefreshRate",
        serde_json::json!({ "enabled": enabled }),
        "无法配置 Android 刷新率",
    )
    .await
    .map(|_| ())
}
