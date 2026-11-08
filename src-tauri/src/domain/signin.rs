//! 签到月历、连续天数、本周记录、奖励模型与应签日期计算。

use crate::domain::error::AppResult;
use crate::domain::time::beijing_date_and_hour;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// 上游维护在 1 点结束；0 点到 1 点之间签到接口不可用。
const MAINTENANCE_END_HOUR: u32 = 1;

/// 一个已签到的自然日。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SigninDay {
    /// 年份。
    pub(crate) year: i64,
    /// 月份。
    pub(crate) month: i64,
    /// 当月日期。
    pub(crate) day: i64,
}

/// 连续签到摘要。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SigninContinueDays {
    /// 累计连续签到天数。
    pub(crate) total_continue_days: i64,
    /// 本月连续签到天数。
    pub(crate) current_month_continue_days: i64,
    /// 历史最长连续签到天数。
    pub(crate) max_continue_days: i64,
}

/// 一份签到奖励。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SigninPrize {
    /// 奖励名称。
    pub(crate) name: String,
    /// 奖励数量。
    pub(crate) num: i64,
}

/// 本周某一天的签到记录。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SigninWeekItem {
    /// 星期编号，周一为 1。
    pub(crate) day: i64,
    /// 日期时间。
    pub(crate) date: String,
    /// 上游给出的星期文本。
    pub(crate) weekday: String,
    /// 当天是否已签到。
    pub(crate) is_sign_in: bool,
    /// 当天签到获得的奖励。
    pub(crate) rewards: Vec<SigninPrize>,
}

/// 可领取的连续签到奖励记录。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SigninReward {
    /// 用户奖励记录编号。
    pub(crate) record_id: i64,
    /// 奖励配置编号，领取时提交。
    pub(crate) config_id: i64,
    /// 奖励类型文本。
    pub(crate) type_name: String,
    /// 天数。
    pub(crate) day: i64,
    /// 天数单位。
    pub(crate) unit: String,
    /// 上游记录状态。
    pub(crate) status: i64,
    /// 是否已经领取。
    pub(crate) is_finish: bool,
    /// 奖励内容。
    pub(crate) rewards: Vec<SigninPrize>,
}

/// 签到弹窗一次刷新所需的全部只读数据。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SigninOverview {
    /// 连续签到摘要。
    pub(crate) continue_days: SigninContinueDays,
    /// 本周签到记录。
    pub(crate) week: Vec<SigninWeekItem>,
    /// 签到奖励记录。
    pub(crate) rewards: Vec<SigninReward>,
}

/// 当前可签到的自然日，格式为 `YYYY-MM-DD`。
///
/// 上游在 0 点重置签到状态，因此日期就是东八区自然日，不存在跨日归属问题。
pub(crate) fn business_day(now: SystemTime) -> String {
    beijing_date_and_hour(now).0
}

/// 是否处于上游维护窗口（0 点到 1 点）。
///
/// 维护期间签到接口不可用。此时提交只会拿到失败，却会被记成「签到失败」，
/// 因此自动签到在窗口内直接跳过，等下一次检查再执行。
pub(crate) fn is_maintenance(now: SystemTime) -> bool {
    beijing_date_and_hour(now).1 < MAINTENANCE_END_HOUR
}

/// 本周记录里指定业务日是否已签到。
pub(crate) fn is_signed_on(week: &[SigninWeekItem], day: &str) -> bool {
    week.iter()
        .any(|item| item.is_sign_in && item.date.get(..10) == Some(day))
}

/// 自动签到最近一次结果，用于启动提示与签到页展示。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SigninNotice {
    /// 业务日，格式为 `YYYY-MM-DD`。
    pub(crate) day: String,
    /// 本次是否签到成功。
    pub(crate) success: bool,
    /// 面向用户的结论文本，失败时为失败原因。
    pub(crate) message: String,
    /// 是否尚未向用户提示。
    pub(crate) pending: bool,
}

impl SigninNotice {
    /// 由本次结果与上一次记录生成提示。
    ///
    /// 同一天重复失败不重置提示状态，避免每次重试都弹一次同样的错误。
    pub(crate) fn new(day: &str, result: AppResult<bool>, previous: Option<&Self>) -> Self {
        let (success, message) = match result {
            Ok(true) => (true, "签到成功".to_string()),
            Ok(false) => (true, "今日已签到".to_string()),
            Err(error) => (false, format!("签到失败：{error}")),
        };
        let pending = if success {
            true
        } else {
            previous
                .filter(|notice| notice.day == day && !notice.success)
                .map(|notice| notice.pending)
                .unwrap_or(true)
        };
        Self {
            day: day.to_string(),
            success,
            message,
            pending,
        }
    }
}

/// 校验并确认一个 `YYYY-MM-DD` 自然日。
pub(crate) fn is_valid_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    if !bytes
        .iter()
        .enumerate()
        .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
    {
        return false;
    }
    let year = value[0..4].parse::<i64>().ok();
    let month = value[5..7].parse::<i64>().ok();
    let day = value[8..10].parse::<i64>().ok();
    let (Some(year), Some(month), Some(day)) = (year, month, day) else {
        return false;
    };
    if !(2000..=2100).contains(&year) || !(1..=12).contains(&month) {
        return false;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    (1..=days).contains(&day)
}
