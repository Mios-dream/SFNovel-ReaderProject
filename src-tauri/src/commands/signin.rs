//! 签到与奖励领取命令。

use crate::application::signin::SigninService;
use crate::domain::error::AppResult;
use crate::domain::signin::{SigninDay, SigninOverview};

#[tauri::command]
/// 读取指定月份的签到日期。
pub(crate) async fn get_signin_month(
    app: tauri::AppHandle,
    year: i64,
    month: i64,
) -> AppResult<Vec<SigninDay>> {
    let remote = super::remote(&app).await?;
    SigninService::new(&remote).month(year, month).await
}

#[tauri::command]
/// 读取签到页的全部只读数据。
pub(crate) async fn get_signin_overview(app: tauri::AppHandle) -> AppResult<SigninOverview> {
    let remote = super::remote(&app).await?;
    SigninService::new(&remote).overview().await
}

#[tauri::command]
/// 提交一次签到。
pub(crate) async fn submit_signin(app: tauri::AppHandle, date: String) -> AppResult<()> {
    let remote = super::remote(&app).await?;
    SigninService::new(&remote).sign_in(&date).await
}

#[tauri::command]
/// 领取一条签到奖励。
pub(crate) async fn claim_signin_reward(app: tauri::AppHandle, config_id: i64) -> AppResult<()> {
    let remote = super::remote(&app).await?;
    SigninService::new(&remote).claim_reward(config_id).await
}
