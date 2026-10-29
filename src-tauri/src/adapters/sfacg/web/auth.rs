use super::html::{extract_js_field, strip_html_text};
use super::WebClient;
use crate::domain::auth::UserProfile;
use serde_json::Value;

impl WebClient {
    /// 读取网页账户基础资料、余额和可选的 VIP 详情。
    ///
    /// # 返回
    /// 返回网页端可展示的账户资料；VIP 详情不可用时不阻断基础资料读取。
    ///
    /// # 错误
    /// 网页会话缺失、登录信息失效或账户中心请求失败时返回错误。
    pub(crate) async fn get_user_profile(&self) -> Result<UserProfile, String> {
        self.require_session()?;
        let account = self.get_login_info().await?;
        let mut profile = user_profile_from_login_info(&account)?;
        let my_html = self
            .page_request("https://m.sfacg.com/my/", Some("https://m.sfacg.com/"))
            .send()
            .await
            .map_err(|error| format!("无法读取 SF 网页账户中心：{error}"))?
            .error_for_status()
            .map_err(|error| format!("SF 网页账户中心请求被拒绝：{error}"))?
            .text()
            .await
            .map_err(|error| format!("SF 网页账户中心格式无效：{error}"))?;
        let account_text = strip_html_text(&my_html);
        let month_ticket = extract_numbers_after_text(&account_text, "月票")
            .first()
            .copied()
            .unwrap_or(0);
        let wallet = extract_numbers_after_text(&account_text, "我的钱包");
        let fire_money = wallet.first().copied().unwrap_or(0);
        let coupons = wallet.get(1).copied().unwrap_or(0);
        profile.web_details_available = true;
        profile.fire_money_remain = fire_money;
        profile.coupons_remain = coupons;
        profile.monthly_ticket = month_ticket;
        // 新旧 VIP 接口都会存在；由 newVip.isNewVip 决定当前账户采用哪套体系。
        // VIP 资料是独立的可选补充，失败不阻断 Web 基础资料和余额显示。
        if let Ok(details) = self.get_web_vip_details().await {
            profile.vip_system = details.system.to_string();
            profile.vip_level = details.level;
            profile.vip_name = details.name;
            profile.vip_details_available = true;
        }
        Ok(profile)
    }

    /// 根据 `newVip.isNewVip` 选择新 VIP 或旧 VIP 资料。
    async fn get_web_vip_details(&self) -> Result<WebVipDetails, String> {
        let payload = self
            .ajax_request(
                "https://pages.sfacg.com/api/User?expand=newVip",
                "https://pages.sfacg.com/h5/app/common/help/Vip.html",
            )
            .send()
            .await
            .map_err(|error| format!("无法读取 SF VIP 资料：{error}"))?
            .error_for_status()
            .map_err(|error| format!("SF VIP 资料请求被拒绝：{error}"))?
            .json::<Value>()
            .await
            .map_err(|error| format!("SF VIP 资料格式无效：{error}"))?;
        let new_vip = web_new_vip_details_from_payload(&payload)?;
        if new_vip.is_new {
            return Ok(WebVipDetails {
                system: "new",
                level: new_vip.level,
                name: new_vip.name,
            });
        }

        let payload = self
            .ajax_request(
                "https://pages.sfacg.com/api/common/vipInfo",
                "https://pages.sfacg.com/h5/app/common/help/Vip.html",
            )
            .send()
            .await
            .map_err(|error| format!("无法读取 SF 旧 VIP 资料：{error}"))?
            .error_for_status()
            .map_err(|error| format!("SF 旧 VIP 资料请求被拒绝：{error}"))?
            .json::<Value>()
            .await
            .map_err(|error| format!("SF 旧 VIP 资料格式无效：{error}"))?;
        let level = web_old_vip_level_from_payload(&payload)?;
        Ok(WebVipDetails {
            system: "legacy",
            level,
            name: String::new(),
        })
    }

    /// 请求官方网页登录信息，不返回原始响应给调用方。
    pub(super) async fn get_login_info(&self) -> Result<String, String> {
        self.page_request("https://passport.sfacg.com/Ajax/GetLoginInfo.ashx", None)
            .send()
            .await
            .map_err(|error| format!("无法读取 SF 网页账号信息：{error}"))?
            .error_for_status()
            .map_err(|error| format!("SF 网页账号信息请求被拒绝：{error}"))?
            .text()
            .await
            .map_err(|error| format!("SF 网页账号信息格式无效：{error}"))
    }
}

/// 从官方登录信息 JavaScript 响应构造基础账户资料。
///
/// # 参数
/// - `value`：`GetLoginInfo.ashx` 返回的短 JavaScript 文本。
///
/// # 返回
/// 返回昵称、头像和账户编号等基础字段。
///
/// # 错误
/// 响应表示未登录或缺少昵称时返回错误。
fn user_profile_from_login_info(value: &str) -> Result<UserProfile, String> {
    if extract_js_field(value, "login").as_deref() != Some("true") {
        return Err("网页登录会话已失效".to_string());
    }
    let nick_name = ["nickname", "nickName"]
        .iter()
        .find_map(|field| extract_js_field(value, field))
        .ok_or_else(|| "SF 网页账号信息未返回昵称".to_string())?;
    let avatar = ["avatar", "userAvatar", "portrait", "headImg", "headimg"]
        .iter()
        .find_map(|field| extract_js_field(value, field))
        .unwrap_or_default();
    let account_id = ["accountId", "accountid", "userId", "userid"]
        .iter()
        .find_map(|field| extract_js_number_field(value, field))
        .unwrap_or(0);
    Ok(UserProfile {
        account_id,
        nick_name,
        avatar,
        app_details_available: false,
        web_details_available: false,
        vip_details_available: false,
        vip_system: String::new(),
        welfare_coin: 0,
        fire_money_remain: 0,
        coupons_remain: 0,
        monthly_ticket: 0,
        vip_level: 0,
        vip_name: String::new(),
    })
}

/// 从官方登录信息端点的 JavaScript 字面量提取一个整数栏位。
pub(super) fn extract_js_number_field(value: &str, field: &str) -> Option<i64> {
    let marker = format!("{field}:");
    let start = value.find(&marker)? + marker.len();
    let value = value[start..].trim_start();
    let value = value
        .strip_prefix('"')
        .or_else(|| value.strip_prefix('\''))
        .unwrap_or(value);
    let digits = value
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>();
    digits.parse().ok().filter(|number: &i64| *number > 0)
}

/// 从网页纯文本中提取指定标签之后的连续数字。
pub(super) fn extract_numbers_after_text(value: &str, marker: &str) -> Vec<i64> {
    let Some(start) = value.find(marker).map(|index| index + marker.len()) else {
        return Vec::new();
    };
    let mut numbers = Vec::new();
    let mut digits = String::new();
    for character in value[start..].chars().take(120) {
        if character.is_ascii_digit() {
            digits.push(character);
        } else if !digits.is_empty() {
            if let Ok(number) = digits.parse() {
                numbers.push(number);
            }
            digits.clear();
        }
        if numbers.len() >= 4 {
            break;
        }
    }
    if !digits.is_empty() {
        if let Ok(number) = digits.parse() {
            numbers.push(number);
        }
    }
    numbers
}

/// `newVip` 接口返回的 VIP 体系判定与等级资料。
struct NewVipDetails {
    /// 当前账户是否使用新版 VIP 体系。
    is_new: bool,
    /// 上游返回的 VIP 等级。
    level: i64,
    /// 上游返回的 VIP 名称，可能为空。
    name: String,
}

/// 归一化后的网页 VIP 资料。
struct WebVipDetails {
    /// VIP 体系标识：`new` 或 `legacy`。
    system: &'static str,
    /// 当前账户的 VIP 等级。
    level: i64,
    /// VIP 名称；旧体系通常为空。
    name: String,
}

/// 从 `newVip` 响应中提取新旧体系标记、等级和名称。
fn web_new_vip_details_from_payload(payload: &Value) -> Result<NewVipDetails, String> {
    if payload
        .get("status")
        .and_then(|status| status.get("errorCode"))
        .and_then(Value::as_i64)
        != Some(200)
    {
        return Err("SF VIP 资料请求未成功".to_string());
    }
    let vip = payload
        .get("data")
        .and_then(|data| data.get("expand"))
        .and_then(|expand| expand.get("newVip"))
        .ok_or_else(|| "SF VIP 资料未返回等级".to_string())?;
    let is_new = vip
        .get("isNewVip")
        .and_then(Value::as_bool)
        .ok_or_else(|| "SF VIP 资料未返回体系标记".to_string())?;
    let level = vip
        .get("level")
        .and_then(Value::as_i64)
        .ok_or_else(|| "SF VIP 资料未返回等级".to_string())?;
    let name = vip
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    Ok(NewVipDetails {
        is_new,
        level,
        name,
    })
}

/// 从旧 VIP 接口提取当前旧体系等级。
fn web_old_vip_level_from_payload(payload: &Value) -> Result<i64, String> {
    if payload
        .get("status")
        .and_then(|status| status.get("errorCode"))
        .and_then(Value::as_i64)
        != Some(200)
    {
        return Err("SF 旧 VIP 资料请求未成功".to_string());
    }
    payload
        .get("data")
        .and_then(|data| data.get("vipLevel"))
        .and_then(Value::as_i64)
        .ok_or_else(|| "SF 旧 VIP 资料未返回等级".to_string())
}

/// 判断账户名是否符合网页端点允许的受限字符和长度规则。
pub(super) fn is_safe_account_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}
