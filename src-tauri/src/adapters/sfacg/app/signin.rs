use super::error::AppApiError;
use super::parsing::{json_bool_field, json_i64_field, json_text_field};
use super::AppClient;
use crate::domain::signin::{
    SigninContinueDays, SigninDay, SigninPrize, SigninReward, SigninWeekItem,
};
use serde_json::Value;

impl AppClient {
    /// 读取指定月份的签到日期。
    pub(crate) async fn get_signin_month(
        &self,
        year: i64,
        month: i64,
    ) -> Result<Vec<SigninDay>, AppApiError> {
        let query = [("year", year.to_string()), ("month", month.to_string())];
        let response = self.get_data_uncached("/user/newSignInfo", &query).await?;
        parse_days(&response)
    }

    /// 读取连续签到摘要。
    pub(crate) async fn get_signin_continue_days(&self) -> Result<SigninContinueDays, AppApiError> {
        let response = self
            .get_data_uncached("/user/sign/continueDay", &[])
            .await?;
        if !response.is_object() {
            return Err(AppApiError::local("SF 签到连续天数返回格式无效".to_string()));
        }
        Ok(SigninContinueDays {
            total_continue_days: json_i64_field(&response, &["totalContinueDays"]).unwrap_or(0),
            current_month_continue_days: json_i64_field(&response, &["currentMonthContinueDays"])
                .unwrap_or(0),
            max_continue_days: json_i64_field(&response, &["maxContinueDays"]).unwrap_or(0),
        })
    }

    /// 读取本周签到记录。
    pub(crate) async fn get_signin_week(&self) -> Result<Vec<SigninWeekItem>, AppApiError> {
        let response = self.get_data_uncached("/user/sign/week", &[]).await?;
        parse_week(&response)
    }

    /// 读取签到奖励记录。
    pub(crate) async fn get_signin_rewards(&self) -> Result<Vec<SigninReward>, AppApiError> {
        let query = [("page", "0".to_string()), ("size", "100".to_string())];
        let response = self.get_data_uncached("/user/sign/reward", &query).await?;
        parse_rewards(&response)
    }

    /// 提交指定日期的签到。
    pub(crate) async fn submit_signin(&self, date: &str) -> Result<(), AppApiError> {
        self.put_data(
            "/user/newSignInfo",
            &serde_json::json!({ "signDate": date }),
        )
        .await
        .map(|_| ())
    }

    /// 领取指定配置的签到奖励。
    pub(crate) async fn claim_signin_reward(&self, config_id: i64) -> Result<(), AppApiError> {
        self.put_data(
            "/user/sign/reward",
            &serde_json::json!({ "configId": config_id }),
        )
        .await
        .map(|_| ())
    }
}

/// 解析已签到日期数组。
fn parse_days(value: &Value) -> Result<Vec<SigninDay>, AppApiError> {
    let items = value
        .as_array()
        .ok_or_else(|| AppApiError::local("SF 签到月历未返回日期列表".to_string()))?;
    Ok(items
        .iter()
        .filter_map(|item| {
            let year = json_i64_field(item, &["year"])?;
            let month = json_i64_field(item, &["month"])?;
            let day = json_i64_field(item, &["day"])?;
            ((1..=12).contains(&month) && (1..=31).contains(&day)).then_some(SigninDay {
                year,
                month,
                day,
            })
        })
        .collect())
}

/// 解析本周签到记录。
fn parse_week(value: &Value) -> Result<Vec<SigninWeekItem>, AppApiError> {
    let items = value
        .get("items")
        .and_then(Value::as_array)
        .or_else(|| value.as_array())
        .ok_or_else(|| AppApiError::local("SF 本周签到未返回记录列表".to_string()))?;
    Ok(items
        .iter()
        .enumerate()
        .map(|(index, item)| SigninWeekItem {
            day: json_i64_field(item, &["day"]).unwrap_or(index as i64 + 1),
            date: json_text_field(item, &["date"]).unwrap_or_default(),
            weekday: json_text_field(item, &["desc"]).unwrap_or_default(),
            is_sign_in: json_bool_field(item, &["isSignIn"]).unwrap_or(false),
            rewards: parse_prizes(item.get("reward")),
        })
        .collect())
}

/// 解析签到奖励记录。
fn parse_rewards(value: &Value) -> Result<Vec<SigninReward>, AppApiError> {
    let items = value
        .get("items")
        .and_then(Value::as_array)
        .or_else(|| value.as_array())
        .ok_or_else(|| AppApiError::local("SF 签到奖励未返回记录列表".to_string()))?;
    Ok(items
        .iter()
        .map(|item| SigninReward {
            record_id: json_i64_field(item, &["recordID"]).unwrap_or(0),
            config_id: json_i64_field(item, &["configId"]).unwrap_or(0),
            type_name: json_text_field(item, &["type"]).unwrap_or_default(),
            day: json_i64_field(item, &["day"]).unwrap_or(0),
            unit: json_text_field(item, &["unit"]).unwrap_or_default(),
            status: json_i64_field(item, &["status"]).unwrap_or(0),
            is_finish: json_bool_field(item, &["isFinish"]).unwrap_or(false),
            rewards: parse_prizes(item.get("rewards")),
        })
        .collect())
}

/// 解析奖励名称与数量数组。
fn parse_prizes(value: Option<&Value>) -> Vec<SigninPrize> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some(SigninPrize {
                name: json_text_field(item, &["name"])?,
                num: json_i64_field(item, &["num"]).unwrap_or(0),
            })
        })
        .collect()
}
