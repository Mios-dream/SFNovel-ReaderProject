//! 签到查询、提交与奖励领取用例。

use crate::application::ports::RemoteGateway;
use crate::domain::error::{AppError, AppResult};
use crate::domain::signin::{is_valid_date, SigninDay, SigninOverview};
#[cfg(not(target_os = "android"))]
use crate::domain::signin::is_signed_on;

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

    /// 当天尚未签到时提交签到，返回本次是否实际提交。
    ///
    /// 只处理当前业务日：历史日期需要补签卡，缺少时会直接失败，因此不自动补签。
    /// Android 侧由 JNI 入口直接驱动客户端，不经过本服务。
    #[cfg(not(target_os = "android"))]
    pub(crate) async fn auto_sign_in(&self, day: &str) -> AppResult<bool> {
        if !is_valid_date(day) {
            return Err(AppError::invalid_input("签到日期格式无效"));
        }
        let week = self.remote.signin_week().await?;
        if is_signed_on(&week, day) {
            return Ok(false);
        }
        self.remote.submit_signin(day).await?;
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
