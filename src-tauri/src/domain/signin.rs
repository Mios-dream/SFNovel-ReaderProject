//! 签到月历、连续天数、本周记录与奖励模型。

use serde::Serialize;

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
