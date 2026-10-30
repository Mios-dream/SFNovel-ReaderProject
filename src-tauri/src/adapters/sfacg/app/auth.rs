use super::error::AppApiError;
use crate::adapters::sfacg::cookie::filter_cookie_header;
use crate::adapters::sfacg::device::SF_APP_VERSION;
use crate::domain::auth::UserProfile;
use serde_json::Value;
use std::sync::atomic::Ordering;

impl super::AppClient {
    /// 使用刚取得的 App 会话登记当前安装。
    ///
    /// 登录命令创建 `SfSource` 时，进程内会话仍不包含本次登录的 Cookie。
    /// 因此显式构建一个仅使用该 Cookie 的客户端，避免拿登录前快照或旧会话上报。
    pub(crate) async fn register_device_for_session(&self, app_cookie: &str) -> Result<(), String> {
        let client = Self::new(self.device.clone(), Some(app_cookie.to_string()))?;
        client.register_device().await
    }

    /// 将当前安装登记到已登录账号。
    ///
    /// 官方 App 在登录后执行同样的登记。部分内容接口在设备未登记时返回
    /// `401/507`，因此这里必须使用与请求签名相同的 `deviceId`。
    pub(crate) async fn register_device(&self) -> Result<(), String> {
        self.device_registration_attempted
            .store(true, Ordering::Release);
        let user = self
            .send_get("/user", &[])
            .await
            .map_err(|error| error.describe("/user"))?;
        let account_id = user
            .get("accountId")
            .and_then(Value::as_i64)
            .ok_or_else(|| "SF App 未返回账户编号".to_string())?;
        if account_id <= 0 {
            return Err("SF 账号编号无效".to_string());
        }
        let profile = &self.device;
        let query = [
            ("deviceId", profile.device_id.clone()),
            ("package", "com.sfacg".to_string()),
        ];
        let oa_id = self
            .send_get("/user/androiddeviceinfos", &query)
            .await
            .ok()
            .and_then(|data| data.get("oaID").and_then(Value::as_str).map(str::to_string))
            .unwrap_or_default();
        let mut body = serde_json::json!({
            "accountId": account_id,
            "package": "com.sfacg",
            "abi": profile.abi(),
            "deviceId": profile.device_id,
            "version": SF_APP_VERSION,
            "originUa": profile.origin_ua(),
            "webViewUA": profile.web_view_ua(),
        });
        if !oa_id.is_empty() {
            body["oaID"] = Value::String(oa_id);
        }
        self.post_data("/user/androiddeviceinfos", &body)
            .await
            .map(|_| ())
    }

    /// 通过 App 端点登录，并仅返回App Cookie。
    ///
    /// # 错误
    /// 登录失败、上游未返回 App 会话或响应无效时返回错误。
    pub(crate) async fn login_with_password(
        &self,
        username: &str,
        password: &str,
    ) -> Result<String, String> {
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
    pub(crate) async fn get_user_profile(&self) -> Result<UserProfile, String> {
        let user = self
            .get_data("/user", &[("expand", "welfareCoin".to_string())])
            .await?;
        let money = self.get_data("/user/money", &[]).await?;
        let account_id = user
            .get("accountId")
            .and_then(Value::as_i64)
            .ok_or_else(|| "SF 登录会话可能已失效".to_string())?;
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
            app_details_available: true,
            web_details_available: false,
            vip_details_available: true,
            vip_system: "legacy".to_string(),
            welfare_coin: user
                .get("expand")
                .and_then(|value| value.get("welfareCoin"))
                .and_then(Value::as_i64)
                .unwrap_or(0),
            fire_money_remain: money
                .get("fireMoneyRemain")
                .and_then(Value::as_i64)
                .unwrap_or(0),
            coupons_remain: money
                .get("couponsRemain")
                .and_then(Value::as_i64)
                .unwrap_or(0),
            monthly_ticket: 0,
            vip_level: money.get("vipLevel").and_then(Value::as_i64).unwrap_or(0),
            vip_name: String::new(),
        })
    }
}
