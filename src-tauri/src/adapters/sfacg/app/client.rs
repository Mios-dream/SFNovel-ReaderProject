use super::super::device::{replace_device_profile, DeviceProfile};
use super::constants::{
    APP_ACCEPT, COMIC_ANONYMOUS_DEVICE_ID, SF_API_HOST, SF_API_PASSWORD, SF_API_USER,
};
use super::error::AppApiError;
use super::signing::security_header;
use crate::adapters::sfacg::cookie::{filter_cookie_header, has_cookie_name};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;
use uuid::Uuid;

/// 仅发送已签名的 SF App API 请求，并持有经过过滤的 App 会话快照。
pub(crate) struct AppClient {
    /// 使用官方 App 请求画像的 HTTP 客户端。
    pub(super) client: reqwest::Client,
    /// 仅保留 SF App 会话所需 Cookie 的请求头值。
    pub(super) app_cookie: Option<String>,
    /// 当前客户端实例是否已经尝试过自动设备登记。
    pub(super) device_registration_attempted: AtomicBool,
    /// 本次安装稳定复用的设备身份；登记后会更新渠道和 OAID。
    pub(super) device: RwLock<DeviceProfile>,
}

impl AppClient {
    /// 使用设备身份和 App 会话快照创建 API 客户端。
    ///
    /// # 错误
    /// HTTP 客户端无法创建时返回错误。
    pub(crate) fn new(device: DeviceProfile, app_cookie: Option<String>) -> Result<Self, String> {
        let user_agent = device.user_agent();
        let app_cookie = app_cookie
            .and_then(|cookie| filter_cookie_header(&cookie, &[".SFCommunity", "session_APP"]));
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent(user_agent)
            .build()
            .map_err(|error| format!("无法创建 SF App 网络客户端：{error}"))?;
        Ok(Self {
            client,
            app_cookie,
            device_registration_attempted: AtomicBool::new(false),
            device: RwLock::new(device),
        })
    }

    /// 判断此客户端是否持有包含 `session_APP` 的 App 会话。
    pub(crate) fn has_session(&self) -> bool {
        self.app_cookie
            .as_deref()
            .is_some_and(|cookie| has_cookie_name(cookie, "session_APP"))
    }

    /// 读取当前设备身份快照。
    pub(super) fn device_snapshot(&self) -> Result<DeviceProfile, String> {
        self.device
            .read()
            .map(|guard| guard.clone())
            .map_err(|_| "设备身份状态不可用".to_string())
    }

    /// 更新当前设备身份，并同步到进程内其他客户端。
    pub(super) fn update_device(
        &self,
        update: impl FnOnce(&mut DeviceProfile),
    ) -> Result<(), String> {
        let snapshot = {
            let mut guard = self
                .device
                .write()
                .map_err(|_| "设备身份状态不可用".to_string())?;
            update(&mut guard);
            guard.clone()
        };
        replace_device_profile(snapshot);
        Ok(())
    }

    /// 创建需要 App 会话的资源请求。
    ///
    /// App API 返回的媒体地址与 App Cookie 绑定，不能复用网页静态资源请求。
    pub(crate) fn asset_request(
        &self,
        url: &str,
        referer: &str,
        accept: &str,
    ) -> reqwest::RequestBuilder {
        let mut request = self
            .client
            .get(url)
            .header(reqwest::header::ACCEPT, accept)
            .header(reqwest::header::REFERER, referer);
        if let Some(cookie) = self.app_cookie.as_deref() {
            request = request.header(reqwest::header::COOKIE, cookie);
        }
        request
    }

    /// 按固定请求指纹构建一个已签名的 App API 请求。
    ///
    /// 每次请求都会生成新的 UUID nonce，并重新读取当前设备渠道。
    pub(super) fn signed_request(
        &self,
        method: reqwest::Method,
        path: &str,
        include_session: bool,
    ) -> Result<reqwest::RequestBuilder, String> {
        let device = self.device_snapshot()?;
        self.signed_request_with_device(method, path, include_session, &device)
    }

    /// 使用指定设备身份构建签名请求。
    fn signed_request_with_device(
        &self,
        method: reqwest::Method,
        path: &str,
        include_session: bool,
        device: &DeviceProfile,
    ) -> Result<reqwest::RequestBuilder, String> {
        let nonce = Uuid::new_v4().to_string().to_uppercase();
        let is_post = method == reqwest::Method::POST;
        let mut request = self
            .client
            .request(method, format!("{SF_API_HOST}{path}"))
            .basic_auth(SF_API_USER, Some(SF_API_PASSWORD))
            .header("Accept-Charset", "UTF-8")
            .header("Accept", APP_ACCEPT)
            .header(reqwest::header::USER_AGENT, device.user_agent())
            .header("SFSecurity", security_header(device, &nonce)?);
        if is_post {
            request = request.header("Content-Type", "application/json; charset=UTF-8");
        }
        if include_session {
            if let Some(cookie) = self.app_cookie.as_deref() {
                request = request.header(reqwest::header::COOKIE, cookie);
            }
        }
        Ok(request)
    }

    /// 构建漫画兼容接口请求。
    ///
    /// 未登录时没有本机设备登记，使用已验证可匿名的设备身份。
    fn comic_signed_request(&self, path: &str) -> Result<reqwest::RequestBuilder, String> {
        let mut device = self.device_snapshot()?;
        if !self.has_session() {
            device.device_id = COMIC_ANONYMOUS_DEVICE_ID.to_string();
            device.manufacturer = "tencent".to_string();
            device.init_channel = "TENCENT".to_string();
        }
        self.signed_request_with_device(reqwest::Method::GET, path, true, &device)
    }

    /// 请求漫画兼容接口。`417/782` 可能由 nonce 触发，换新 nonce 最多重试五次。
    pub(crate) async fn get_comic_data(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<Value, String> {
        let mut last_error = None;
        for _ in 0..5 {
            let response = self
                .comic_signed_request(path)?
                .query(query)
                .send()
                .await
                .map_err(|error| format!("SF 漫画接口请求失败：{error}"))?;
            match Self::parse_response(response, path).await {
                Ok(data) => return Ok(data),
                Err(error) if error.is_client_version_gated() => last_error = Some(error),
                Err(error) => return Err(error.describe(path)),
            }
        }
        Err(last_error
            .map(|error| error.describe(path))
            .unwrap_or_else(|| "SF 漫画接口请求失败".to_string()))
    }

    /// 请求一个已签名的 SF App API 资源，并拆开其 `data` 数据包。
    ///
    /// 首次遇到 `401/507` 时会登记当前设备并重试一次。登记必须使用同一次安装的
    /// `deviceId`，否则会形成新的设备身份。
    ///
    /// # 错误
    /// 请求失败、上游拒绝请求、设备登记失败或响应不是有效 JSON 时返回错误。
    pub(crate) async fn get_data(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<Value, String> {
        self.get_data_inner(path, query, true)
            .await
            .map_err(|error| error.describe(path))
    }

    /// 请求 App API 资源，并保留可分类的上游失败信息。
    pub(crate) async fn get_data_typed(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<Value, AppApiError> {
        self.get_data_inner(path, query, true).await
    }

    /// 发送 GET 请求，并按需处理一次设备登记重试。
    async fn get_data_inner(
        &self,
        path: &str,
        query: &[(&str, String)],
        allow_registration: bool,
    ) -> Result<Value, AppApiError> {
        match self.send_get(path, query).await {
            Ok(data) => Ok(data),
            Err(error)
                if allow_registration && error.is_device_unregistered() && self.has_session() =>
            {
                if self
                    .device_registration_attempted
                    .swap(true, Ordering::AcqRel)
                {
                    return Err(error);
                }
                match self.register_device().await {
                    Ok(()) => self.send_get(path, query).await,
                    Err(registration_error) => Err(AppApiError::local(format!(
                        "{}；自动设备登记失败：{registration_error}",
                        error.describe(path)
                    ))),
                }
            }
            Err(error) => Err(error),
        }
    }

    /// 执行一次签名 GET 请求，并保留可分类的上游失败信息。
    pub(super) async fn send_get(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<Value, AppApiError> {
        let response = self
            .signed_request(reqwest::Method::GET, path, true)
            .map_err(AppApiError::local)?
            .query(query)
            .send()
            .await
            .map_err(AppApiError::transport)?;
        Self::parse_response(response, path).await
    }

    /// 读取当前账号持有的月票数量。
    ///
    /// App 通过 `HEAD /user/tickets` 的 `X-Total-Count` 响应头返回数量。
    pub(super) async fn get_user_ticket_count(&self) -> Result<Option<i64>, AppApiError> {
        let response = self
            .signed_request(reqwest::Method::HEAD, "/user/tickets", true)
            .map_err(AppApiError::local)?
            .send()
            .await
            .map_err(AppApiError::transport)?;
        let status = response.status();
        if !status.is_success() {
            let error = AppApiError::from_response(status.as_u16(), &Value::Null);
            eprintln!(
                "[sfacg] App request rejected: path=/user/tickets, http={}, api_code={:?}, error_code={:?}",
                status.as_u16(),
                error.api_http_code,
                error.api_error_code,
            );
            return Err(error);
        }
        Ok(response
            .headers()
            .get("X-Total-Count")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<i64>().ok()))
    }

    /// 解析 App API 响应：成功时拆出 `data`，失败时保留可分类的上游状态。
    async fn parse_response(response: reqwest::Response, path: &str) -> Result<Value, AppApiError> {
        let http_status = response.status().as_u16();
        let body = response
            .json::<Value>()
            .await
            .map_err(|error| AppApiError {
                http_status,
                api_http_code: None,
                api_error_code: None,
                message: Some(format!("SF App 响应格式无效：{error}")),
            })?;
        let api_http_code = body
            .get("status")
            .and_then(|value| value.get("httpCode"))
            .and_then(Value::as_i64);
        let successful = (200..300).contains(&http_status)
            && api_http_code.is_none_or(|code| (200..300).contains(&code));
        if !successful {
            let error = AppApiError::from_response(http_status, &body);
            eprintln!(
                "[sfacg] App request rejected: path={path}, http={http_status}, api_code={:?}, error_code={:?}",
                error.api_http_code, error.api_error_code,
            );
            return Err(error);
        }
        Ok(body.get("data").cloned().unwrap_or(body))
    }

    /// 匿名请求 SF App API 资源，并拆开其 `data` 数据包。
    ///
    /// “匿名”仅指不附带 `.SFCommunity` 或 `session_APP`；协议 Basic Auth、
    /// 请求签名和安装设备标识仍是 App API 的必要请求字段。
    pub(crate) async fn get_public_data(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<Value, String> {
        let response = self
            .signed_request(reqwest::Method::GET, path, false)?
            .query(query)
            .send()
            .await
            .map_err(|error| format!("SF 匿名 App 请求失败：{error}"))?;
        Self::parse_response(response, path)
            .await
            .map_err(|error| error.describe(path))
    }

    /// 发送已签名 JSON POST 请求，并返回拆开的数据载荷。
    ///
    /// 此方法仅供请求体由 App 协议定义的原生层集成调用，任意渲染进程载荷不能到达此处。
    pub(crate) async fn post_data(&self, path: &str, body: &Value) -> Result<Value, String> {
        self.post_data_inner(path, body, true).await
    }

    /// 发送不带 App 会话的 JSON POST 请求，用于冷启动设备登记。
    pub(crate) async fn post_public_data(&self, path: &str, body: &Value) -> Result<Value, String> {
        self.post_data_inner(path, body, false).await
    }

    /// 发送 JSON POST 请求，并按需附带 App 会话。
    async fn post_data_inner(
        &self,
        path: &str,
        body: &Value,
        include_session: bool,
    ) -> Result<Value, String> {
        let response = self
            .signed_request(reqwest::Method::POST, path, include_session)?
            .json(body)
            .send()
            .await
            .map_err(|error| format!("SF App 请求失败：{error}"))?;
        Self::parse_response(response, path)
            .await
            .map_err(|error| error.describe(path))
    }
}
