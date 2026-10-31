use super::error::AppApiError;
use crate::adapters::sfacg::cookie::filter_cookie_header;
use crate::adapters::sfacg::device::{DeviceProfile, SF_APP_VERSION, SF_PACKAGE};
use crate::domain::auth::UserProfile;
use serde_json::Value;
use std::sync::atomic::Ordering;

/// 登录后读取账号资料时请求的扩展字段。
const SF_USER_EXPAND: &str = "vipInfo,welfareCoin,isRealNameAuth,realnameinfo,changeNickNameInfo,welfareMoney,redpacketCode,useWelfaresys,usedRedpacketCode,hasOrderChapWithFireMoney,hasUnlockChapWithAd,hasActiveUnlockChapWithAd,hasOrderedVipChaps,hasPaidFirstTime,growup,newVip,sex,member,dianyamember,birthday,teenager,accountException,optionCard";

/// 构建冷启动设备登记体。
fn startup_device_body(device: &DeviceProfile) -> Value {
    let mut body = serde_json::json!({
        "originUa": device.origin_ua(),
        "accountId": 0,
        "package": SF_PACKAGE,
        "webViewUA": device.web_view_ua(),
        "abi": device.abi(),
        "deviceId": device.device_id,
        "version": SF_APP_VERSION,
        "pushInfo": [{ "channelName": "umeng" }],
    });
    if !device.oa_id.is_empty() {
        body["oaID"] = Value::String(device.oa_id.clone());
    }
    body
}

/// 构建登录后的基础设备登记体。
fn basic_device_body(device: &DeviceProfile, account_id: i64) -> Value {
    let mut body = serde_json::json!({
        "originUa": device.origin_ua(),
        "accountId": account_id,
        "package": SF_PACKAGE,
        "webViewUA": device.web_view_ua(),
        "abi": device.abi(),
        "deviceId": device.device_id,
        "version": SF_APP_VERSION,
    });
    if !device.oa_id.is_empty() {
        body["oaID"] = Value::String(device.oa_id.clone());
    }
    body
}

/// 构建包含厂商与推送信息的完整设备登记体。
fn full_device_body(device: &DeviceProfile, account_id: i64) -> Value {
    let mut body = serde_json::json!({
        "originUa": device.origin_ua(),
        "accountId": account_id,
        "package": SF_PACKAGE,
        "webViewUA": device.web_view_ua(),
        "abi": device.abi(),
        "deviceId": device.device_id,
        "version": SF_APP_VERSION,
        "pushInfo": device.push_info(),
    });
    if !device.manufacturer_token.is_empty() {
        body["manufacturerToken"] = Value::String(device.manufacturer_token.clone());
    }
    if !device.oa_id.is_empty() {
        body["oaID"] = Value::String(device.oa_id.clone());
    }
    if !device.manufacturer.is_empty() {
        body["manufacturer"] = Value::String(device.manufacturer.clone());
    }
    body
}

/// 构建设备激活体。注意字段名是大写 `originUA`。
fn conversion_body(device: &DeviceProfile) -> Value {
    let mut body = serde_json::json!({
        "originUA": device.origin_ua(),
        "webViewUA": device.web_view_ua(),
        "action": "active",
    });
    if !device.oa_id.is_empty() {
        body["oaID"] = Value::String(device.oa_id.clone());
    }
    body
}

impl super::AppClient {
    /// 使用刚取得的 App 会话登记当前安装。
    ///
    /// 登录命令创建 `SfSource` 时，进程内会话仍不包含本次登录的 Cookie。
    /// 因此显式构建一个仅使用该 Cookie 的客户端，避免拿登录前快照或旧会话上报。
    pub(crate) async fn register_device_for_session(&self, app_cookie: &str) -> Result<(), String> {
        let client = Self::new(self.device_snapshot()?, Some(app_cookie.to_string()))?;
        client.register_device().await
    }

    /// 冷启动设备登记、读取渠道、完整登记并激活。
    ///
    /// `initChannel` 尚未取得时，登记请求的 User-Agent 最后一段使用厂商名。
    async fn startup_device_flow(&self) -> Result<(), String> {
        self.update_device(|device| device.init_channel.clear())?;
        let startup_device = self.device_snapshot()?;
        self.post_public_data(
            "/user/androiddeviceinfos",
            &startup_device_body(&startup_device),
        )
        .await?;
        self.refresh_device_info().await?;
        let device = self.device_snapshot()?;
        self.post_public_data("/user/androiddeviceinfos", &full_device_body(&device, 0))
            .await?;
        self.post_public_data(
            "/user/androiddeviceinfos/conversions",
            &conversion_body(&device),
        )
        .await?;
        Ok(())
    }

    /// 读取服务端登记的 OAID 和 `initChannel`。
    async fn refresh_device_info(&self) -> Result<(), String> {
        let device = self.device_snapshot()?;
        let query = [
            ("deviceId", device.device_id.clone()),
            ("package", SF_PACKAGE.to_string()),
        ];
        let data = self
            .get_public_data("/user/androiddeviceinfos", &query)
            .await?;
        let oa_id = data
            .get("oaID")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        let init_channel = data
            .get("initChannel")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        if oa_id.is_none() && init_channel.is_none() {
            return Ok(());
        }
        self.update_device(|device| {
            if let Some(value) = oa_id {
                device.oa_id = value;
            }
            if let Some(value) = init_channel {
                device.init_channel = value;
            }
        })
    }

    /// 将当前安装登记到已登录账号。
    ///
    /// 官方 App 在登录后读取账号编号，再提交基础设备登记并上报激活。
    /// 部分内容接口在设备未登记时返回 `401/507`，因此这里必须使用与请求签名
    /// 相同的 `deviceId`。
    pub(crate) async fn register_device(&self) -> Result<(), String> {
        self.device_registration_attempted
            .store(true, Ordering::Release);
        let user = self
            .send_get("/user", &[("expand", SF_USER_EXPAND.to_string())])
            .await
            .map_err(|error| error.describe("/user"))?;
        let account_id = user
            .get("accountId")
            .and_then(Value::as_i64)
            .ok_or_else(|| "SF App 未返回账户编号".to_string())?;
        if account_id <= 0 {
            return Err("SF 账号编号无效".to_string());
        }
        if self.device_snapshot()?.init_channel.is_empty() {
            self.refresh_device_info().await?;
        }
        let device = self.device_snapshot()?;
        self.post_data(
            "/user/androiddeviceinfos",
            &basic_device_body(&device, account_id),
        )
        .await?;
        self.post_data(
            "/user/androiddeviceinfos/conversions",
            &conversion_body(&device),
        )
        .await?;
        Ok(())
    }

    /// 通过 App 端点登录，并仅返回 App Cookie。
    ///
    /// 登录前按官方 App 顺序完成设备登记、渠道读取、完整登记和激活。
    ///
    /// # 错误
    /// 登录失败、上游未返回 App 会话或响应无效时返回错误。
    pub(crate) async fn login_with_password(
        &self,
        username: &str,
        password: &str,
    ) -> Result<String, String> {
        self.startup_device_flow().await?;
        let response = self
            .signed_request(reqwest::Method::POST, "/sessions", false)?
            .json(&serde_json::json!({
                "username": username,
                "password": password,
                "shuMeiId": "",
            }))
            .send()
            .await
            .map_err(|error| format!("SF App 登录请求失败：{error}"))?;
        let status = response.status();
        let set_cookies = response
            .headers()
            .get_all(reqwest::header::SET_COOKIE)
            .iter()
            .filter_map(|header| header.to_str().ok())
            .filter_map(|header| header.split(';').next())
            .collect::<Vec<_>>()
            .join("; ");
        let cookie = filter_cookie_header(&set_cookies, &[".SFCommunity", "session_APP"]);
        let body = response.json::<Value>().await.unwrap_or(Value::Null);
        let api_http_code = body
            .get("status")
            .and_then(|value| value.get("httpCode"))
            .and_then(Value::as_i64);
        if !status.is_success() || api_http_code != Some(200) {
            let error = AppApiError::from_response(status.as_u16(), &body);
            eprintln!(
                "[sfacg] App login rejected: http={}, api_code={:?}, error_code={:?}",
                status.as_u16(),
                error.api_http_code,
                error.api_error_code,
            );
            return Err(format!("登录失败：{}", error.describe("/sessions")));
        }
        cookie.ok_or_else(|| "SF App 登录成功但未返回 App 会话 Cookie".to_string())
    }

    /// 获取小说正文解密所需的 App 章节正文及归属元数据（小说 ID、卷 ID）。
    ///
    /// # 参数
    /// * `chapter_id` - SF App 章节编号。
    ///
    /// # 错误
    /// 章节编号无效、上游未返回正文或缺少作品、分卷标识时返回错误。
    pub(crate) async fn get_user_profile(&self) -> Result<UserProfile, AppApiError> {
        let user = self
            .get_data_typed("/user", &[("expand", "welfareCoin".to_string())])
            .await?;
        let money = self.get_data_typed("/user/money", &[]).await?;
        let monthly_ticket = match self.get_user_ticket_count().await {
            Ok(count) => count,
            Err(error) if error.is_session_expired() => return Err(error),
            Err(error) => {
                eprintln!(
                    "[sfacg] App ticket count unavailable: {}",
                    error.describe("/user/tickets")
                );
                None
            }
        };
        let account_id = user
            .get("accountId")
            .and_then(Value::as_i64)
            .ok_or_else(|| AppApiError::local("SF 登录会话可能已失效".to_string()))?;
        Ok(UserProfile {
            account_id,
            nick_name: user
                .get("nickName")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("SF 用户")
                .to_string(),
            avatar: user
                .get("avatar")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            vip_details_available: true,
            vip_system: "legacy".to_string(),
            fire_money_remain: money.get("fireMoneyRemain").and_then(Value::as_i64),
            coupons_remain: money.get("couponsRemain").and_then(Value::as_i64),
            monthly_ticket,
            vip_level: money.get("vipLevel").and_then(Value::as_i64).unwrap_or(0),
            vip_name: String::new(),
        })
    }
}
