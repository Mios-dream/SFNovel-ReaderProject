//! Android 的应用设置入口。
//!
//! 自动签到依赖后台周期任务，而任务能否存活取决于厂商的清理策略。厂商的「自启动」
//! 开关是私有实现：页面名称随系统版本变化，没有公开 Intent，也没有读取接口。
//!
//! 因此这里不按品牌枚举页面，也不做任何授权校验——只打开系统提供的应用详情页，
//! 由用户在其中找到「自启动 / 后台」开关。该页面在所有 Android 版本与厂商上都存在，
//! 不会因 ROM 变化失效；代价是需要用户多点一步。

use super::call_auth;
use crate::domain::error::AppResult;

/// 打开本应用的系统详情页。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
pub(crate) async fn open_app_settings(app: &tauri::AppHandle) -> AppResult<()> {
    call_auth::<serde_json::Value, _>(app, "openAppSettings", (), "无法打开 Android 应用设置")
        .await
        .map(|_| ())
}
