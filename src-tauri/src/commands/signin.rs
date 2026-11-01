//! 签到与奖励领取命令。

use crate::adapters::sfacg::source::SfSource;
use crate::application::signin::SigninService;
use crate::domain::error::AppError;
use crate::domain::signin::{SigninDay, SigninOverview};

/// 创建使用当前 App 会话的签到服务。
async fn source(app: &tauri::AppHandle) -> Result<SfSource, String> {
    SfSource::connect(app)
        .await
        .map_err(|error| AppError::upstream(error).into_message())
}

#[tauri::command]
/// 读取指定月份的签到日期。
pub(crate) async fn get_signin_month(
    app: tauri::AppHandle,
    year: i64,
    month: i64,
) -> Result<Vec<SigninDay>, String> {
    let remote = source(&app).await?;
    SigninService::new(&remote)
        .month(year, month)
        .await
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 读取签到页的全部只读数据。
pub(crate) async fn get_signin_overview(app: tauri::AppHandle) -> Result<SigninOverview, String> {
    let remote = source(&app).await?;
    SigninService::new(&remote)
        .overview()
        .await
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 提交一次签到。
pub(crate) async fn submit_signin(app: tauri::AppHandle, date: String) -> Result<(), String> {
    let remote = source(&app).await?;
    SigninService::new(&remote)
        .sign_in(&date)
        .await
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 领取一条签到奖励。
pub(crate) async fn claim_signin_reward(
    app: tauri::AppHandle,
    config_id: i64,
) -> Result<(), String> {
    let remote = source(&app).await?;
    SigninService::new(&remote)
        .claim_reward(config_id)
        .await
        .map_err(AppError::into_message)
}
