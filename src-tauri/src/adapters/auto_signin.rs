//! 每日自动签到。
//!
//! 桌面端每 2 小时检查一次：设置开启、且当前会话当天尚未成功时，提交一次当天签到。
//!
//! 只签当前业务日——历史日期需要补签卡，缺失会让请求失败，因此不自动补签。
//! 上游 0 点到 1 点维护，窗口内直接跳过，避免把维护记成签到失败。
//! 结果按会话 Cookie 落盘，供启动提示与签到页展示。

use crate::adapters::sfacg::source::SfSource;
use crate::adapters::storage::{NativeStorage, SigninLog};
use crate::application::signin::SigninService;
use crate::domain::signin::{business_day, is_maintenance, SigninNotice};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

/// 两次检查之间的间隔。
const CHECK_INTERVAL: Duration = Duration::from_secs(2 * 60 * 60);

/// 是否有检查正在执行。
///
/// 周期循环与打开开关都会触发检查，两次提交会让后一次的失败记录覆盖前一次的成功。
static CHECKING: AtomicBool = AtomicBool::new(false);

/// 检查期间的占位，离开作用域时释放标记。
struct CheckingGuard;

impl Drop for CheckingGuard {
    fn drop(&mut self) {
        CHECKING.store(false, Ordering::Release);
    }
}

/// 启动后台自动签到循环。
pub(crate) fn spawn(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            check_now(&app).await;
            tokio::time::sleep(CHECK_INTERVAL).await;
        }
    });
}

/// 立即执行一次检查，已在检查中时直接返回。
///
/// 打开自动签到开关时由命令调用，让用户不必等满一个轮询周期。
pub(crate) async fn check_now(app: &tauri::AppHandle) {
    if CHECKING.swap(true, Ordering::AcqRel) {
        return;
    }
    let _guard = CheckingGuard;
    run_once(app).await;
}

/// 执行一次检查；当天已成功或缺少前置条件时直接返回。
async fn run_once(app: &tauri::AppHandle) {
    let enabled = NativeStorage::new(app.clone())
        .settings()
        .map(|settings| settings.signin.auto_signin_enabled)
        .unwrap_or(false);
    if !enabled {
        return;
    }
    let now = SystemTime::now();
    if is_maintenance(now) {
        log_info!("自动签到跳过：上游维护时段（0-1 点）");
        return;
    }
    let Ok(remote) = SfSource::connect(app).await else {
        log_warn!("自动签到跳过：无法创建内容源");
        return;
    };
    let Some(cookie) = remote.app_cookie() else {
        return;
    };
    let Ok(log) = SigninLog::for_app(app) else {
        log_warn!("自动签到跳过：无法解析应用数据目录");
        return;
    };
    let day = business_day(now);
    let previous = match log.read(cookie) {
        Ok(notice) => notice,
        Err(error) => {
            log_warn!("自动签到记录不可读：{error}");
            None
        }
    };
    if previous
        .as_ref()
        .is_some_and(|notice| notice.day == day && notice.success)
    {
        return;
    }
    let result = SigninService::new(&remote).auto_sign_in(&day).await;
    let notice = SigninNotice::new(&day, result, previous.as_ref());
    if let Err(error) = log.save(cookie, &notice) {
        log_warn!("自动签到记录写入失败：{error}");
    }
    log_info!("自动签到 {}：{}", notice.day, notice.message);
}
