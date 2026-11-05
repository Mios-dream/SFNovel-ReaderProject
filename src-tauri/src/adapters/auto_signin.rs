//! 每日自动签到。
//!
//! 桌面端每 10 分钟检查一次：东八区 12 点之后、当天尚未尝试过、且设置已开启时，
//! 复用与手动签到相同的应用服务执行一次，失败不重试当日。

use crate::adapters::sfacg::source::SfSource;
use crate::adapters::storage::NativeStorage;
use crate::application::signin::SigninService;
use crate::domain::time::beijing_date_and_hour;
use std::time::{Duration, SystemTime};

/// 两次检查之间的间隔。
const CHECK_INTERVAL: Duration = Duration::from_secs(10 * 60);

/// 触发签到的整点小时，与网页端每日刷新时间一致。
const TRIGGER_HOUR: u32 = 12;

/// 启动后台自动签到循环。
pub(crate) fn spawn(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last_attempt = String::new();
        loop {
            if let Some(date) = due_date(&last_attempt, SystemTime::now()) {
                if run_once(&app, &date).await {
                    last_attempt = date;
                }
            }
            tokio::time::sleep(CHECK_INTERVAL).await;
        }
    });
}

/// 判断此刻是否需要尝试签到，并返回目标日期。
fn due_date(last_attempt: &str, now: SystemTime) -> Option<String> {
    let (date, hour) = beijing_date_and_hour(now);
    (hour >= TRIGGER_HOUR && last_attempt != date).then_some(date)
}

/// 执行一次签到，返回是否已经发出过请求。
///
/// 设置关闭或缺少 App 会话时返回 `false`，调用方会在下个周期重新判断。
async fn run_once(app: &tauri::AppHandle, date: &str) -> bool {
    let enabled = NativeStorage::new(app.clone())
        .settings()
        .map(|settings| settings.signin.auto_signin_enabled)
        .unwrap_or(false);
    if !enabled {
        return false;
    }
    let Ok(remote) = SfSource::connect(app).await else {
        log_warn!("自动签到跳过：无法创建内容源");
        return false;
    };
    if !remote.has_app_session() {
        return false;
    }
    match SigninService::new(&remote).auto_sign_in(date).await {
        Ok(true) => log_info!("自动签到成功：{date}"),
        Ok(false) => log_info!("今日已签到：{date}"),
        Err(error) => log_warn!("自动签到失败：{date} {error}"),
    }
    true
}
