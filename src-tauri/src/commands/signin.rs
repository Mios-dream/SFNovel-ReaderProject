//! 签到与奖励领取命令。

use crate::adapters::storage::SigninLog;
use crate::application::signin::SigninService;
use crate::domain::error::AppResult;
use crate::domain::signin::{SigninDay, SigninNotice, SigninOverview};

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
/// 提交一次签到，并把结果写入自动签到记录。
///
/// 手动签到时用户已看到结果，因此记录标记为已提示，启动时不再重复弹窗。
pub(crate) async fn submit_signin(app: tauri::AppHandle, date: String) -> AppResult<()> {
    let remote = super::remote(&app).await?;
    SigninService::new(&remote).sign_in(&date).await?;
    let Some(cookie) = remote.app_cookie() else {
        return Ok(());
    };
    let Ok(log) = SigninLog::for_app(&app) else {
        return Ok(());
    };
    let notice = SigninNotice {
        day: date,
        success: true,
        message: "签到成功".to_string(),
        pending: false,
    };
    if let Err(error) = log.save(cookie, &notice) {
        log_warn!("手动签到记录写入失败：{error}");
    }
    Ok(())
}

#[tauri::command]
/// 领取一条签到奖励。
pub(crate) async fn claim_signin_reward(app: tauri::AppHandle, config_id: i64) -> AppResult<()> {
    let remote = super::remote(&app).await?;
    SigninService::new(&remote).claim_reward(config_id).await
}

#[tauri::command]
/// 读取自动签到最近一次结果；`consume` 为真时同时标记为已提示。
pub(crate) async fn signin_notice(
    app: tauri::AppHandle,
    consume: bool,
) -> AppResult<Option<SigninNotice>> {
    let remote = super::remote(&app).await?;
    let Some(cookie) = remote.app_cookie() else {
        return Ok(None);
    };
    let log = SigninLog::for_app(&app)?;
    if consume {
        log.take_pending(cookie)
    } else {
        log.read(cookie)
    }
}
