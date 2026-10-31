//! 认证状态与面向渲染进程的账户模型。

use serde::{Deserialize, Serialize};

/// 当前活动会话的安全快照。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthStatus {
    /// App 或 Web 任一会话存在时为 `true`。
    pub(crate) authenticated: bool,
    /// App Cookie 会话是否已建立。
    pub(crate) app_authenticated: bool,
    /// Web Cookie 会话是否已建立。
    pub(crate) web_authenticated: bool,
    /// 当前会话对应的账号名称；匿名会话不返回该字段。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) user_name: Option<String>,
}

/// 已登录 SF 账户资料的安全快照。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UserProfile {
    /// SF 账号编号。
    pub(crate) account_id: i64,
    /// 账号昵称。
    pub(crate) nick_name: String,
    /// 头像地址。
    pub(crate) avatar: String,
    /// 是否成功取得 VIP 相关字段。
    pub(crate) vip_details_available: bool,
    /// VIP 体系标识。
    pub(crate) vip_system: String,
    /// 火币余额；上游未返回时为 `None`。
    pub(crate) fire_money_remain: Option<i64>,
    /// 可用优惠券数量；上游未返回时为 `None`。
    pub(crate) coupons_remain: Option<i64>,
    /// 月票数量；上游未返回时为 `None`。
    pub(crate) monthly_ticket: Option<i64>,
    /// VIP 等级。
    pub(crate) vip_level: i64,
    /// VIP 等级名称。
    pub(crate) vip_name: String,
}

/// App 与 Web Cookie 刻意分离，不能相互转换。
#[derive(Default)]
pub(crate) struct AuthSession {
    /// 仅供 App 签名接口使用的 Cookie。
    pub(crate) app_cookie: Option<String>,
    /// 仅供网页、AJAX 和静态资源请求使用的 Cookie。
    pub(crate) web_cookie: Option<String>,
    /// 登录后展示的账号名称。
    pub(crate) user_name: String,
}
