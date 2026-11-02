//! 签到查询、提交与奖励领取用例。

use crate::application::ports::RemoteGateway;
use crate::domain::error::{AppError, AppResult};
use crate::domain::signin::{SigninDay, SigninOverview};

/// 组合签到接口的应用服务。
pub(crate) struct SigninService<'a, R: RemoteGateway> {
    /// 远程 SF 签到接口。
    remote: &'a R,
}

impl<'a, R: RemoteGateway> SigninService<'a, R> {
    /// 创建签到服务。
    pub(crate) fn new(remote: &'a R) -> Self {
        Self { remote }
    }

    /// 读取指定月份的签到日期。
    pub(crate) async fn month(&self, year: i64, month: i64) -> AppResult<Vec<SigninDay>> {
        if !(2000..=2100).contains(&year) {
            return Err(AppError::invalid_input("签到年份无效"));
        }
        if !(1..=12).contains(&month) {
            return Err(AppError::invalid_input("签到月份必须在 1 到 12 之间"));
        }
        self.remote.signin_month(year, month).await
    }

    /// 读取签到弹窗的连续天数、本周记录和奖励。
    pub(crate) async fn overview(&self) -> AppResult<SigninOverview> {
        let continue_days = self.remote.signin_continue_days().await?;
        let week = self.remote.signin_week().await?;
        let rewards = self.remote.signin_rewards().await?;
        Ok(SigninOverview {
            continue_days,
            week,
            rewards,
        })
    }

    /// 提交一次签到。
    pub(crate) async fn sign_in(&self, date: &str) -> AppResult<()> {
        if !is_valid_date(date) {
            return Err(AppError::invalid_input("签到日期格式无效"));
        }
        self.remote.submit_signin(date).await
    }

    /// 当天尚未签到时提交签到，并返回本次是否实际提交。
    #[cfg(not(target_os = "android"))]
    pub(crate) async fn auto_sign_in(&self, date: &str) -> AppResult<bool> {
        if !is_valid_date(date) {
            return Err(AppError::invalid_input("签到日期格式无效"));
        }
        let week = self.remote.signin_week().await?;
        if week
            .iter()
            .any(|item| item.date.get(..10) == Some(date) && item.is_sign_in)
        {
            return Ok(false);
        }
        self.remote.submit_signin(date).await?;
        Ok(true)
    }

    /// 领取一条签到奖励。
    pub(crate) async fn claim_reward(&self, config_id: i64) -> AppResult<()> {
        if config_id <= 0 {
            return Err(AppError::invalid_input("签到奖励编号无效"));
        }
        self.remote.claim_signin_reward(config_id).await
    }
}

/// 校验并确认一个 `YYYY-MM-DD` 自然日。
fn is_valid_date(value: &str) -> bool {
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
