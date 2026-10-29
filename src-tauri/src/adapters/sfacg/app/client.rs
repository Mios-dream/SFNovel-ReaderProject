use super::super::device::DeviceProfile;
use super::constants::{SF_API_HOST, SF_API_PASSWORD, SF_API_USER};
use super::error::AppApiError;
use super::signing::security_header;
use crate::adapters::sfacg::cookie::{filter_cookie_header, has_cookie_name};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use uuid::Uuid;

/// 仅发送已签名的 SF App API 请求，并持有经过过滤的 App 会话快照。
pub(crate) struct AppClient {
    /// 使用官方 App 请求画像的 HTTP 客户端。
    pub(super) client: reqwest::Client,
    /// 仅保留 SF App 会话所需 Cookie 的请求头值。
    pub(super) app_cookie: Option<String>,
    /// 当前客户端实例是否已经尝试过自动设备登记。
    pub(super) device_registration_attempted: AtomicBool,
    /// 本次安装稳定复用的设备身份。
    pub(super) device: DeviceProfile,
}

impl AppClient {
    /// 使用设备身份和 App 会话快照创建 API 客户端。
    ///
    /// # 错误
    /// 当设备身份未初始化或 HTTP 客户端无法创建时返回错误。
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
            device,
        })
    }

    /// 判断此客户端是否持有包含 `session_APP` 的 App 会话。
    pub(crate) fn has_session(&self) -> bool {
        self.app_cookie
            .as_deref()
            .is_some_and(|cookie| has_cookie_name(cookie, "session_APP"))
    }

    /// 按固定请求指纹构建一个已签名的 App API 请求。
    ///
    /// # 参数
    /// * `method` - 请求使用的 HTTP 方法。
    /// * `path` - 相对 SF App API 路径。
    /// * `include_session` - 是否附带本地 App 会话 Cookie。
    ///
    /// 每次请求都会生成新的 UUID nonce。
    ///
    /// # 错误
    /// 当无法生成安全签名头时返回错误。
    pub(super) fn signed_request(
        &self,
        method: reqwest::Method,
        path: &str,
        include_session: bool,
    ) -> Result<reqwest::RequestBuilder, String> {
        let nonce = Uuid::new_v4().to_string().to_uppercase();
        let mut request = self
            .client
            .request(method, format!("{SF_API_HOST}{path}"))
            .basic_auth(SF_API_USER, Some(SF_API_PASSWORD))
            .header("Accept", "application/json")
            .header("Accept-Language", "zh-Hans-CN;q=1")
            .header("Content-Type", "application/json; charset=UTF-8")
            .header("SFSecurity", security_header(&self.device, &nonce)?);
        if include_session {
            if let Some(cookie) = self.app_cookie.as_deref() {
                request = request.header(reqwest::header::COOKIE, cookie);
            }
        }
        Ok(request)
    }

    /// 请求一个已签名的 SF App API 资源，并拆开其 `data` 数据包。
    ///
    /// 首次遇到 `401/507` 时会登记当前设备并重试一次。登记必须使用同一次安装的
    /// `deviceId` 和 `deviceToken`，否则会形成新的设备身份。
    ///
    /// # 错误
    /// 请求失败、上游拒绝请求、设备登记失败或响应不是有效 JSON 时返回错误。
    pub(crate) async fn get_data(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<Value, String> {
        self.get_data_inner(path, query, true).await
    }

    /// 发送 GET 请求，并按需处理一次设备登记重试。
    async fn get_data_inner(
        &self,
        path: &str,
        query: &[(&str, String)],
        allow_registration: bool,
    ) -> Result<Value, String> {
        match self.send_get(path, query).await {
            Ok(data) => Ok(data),
            Err(error)
                if allow_registration && error.is_device_unregistered() && self.has_session() =>
            {
                if self
                    .device_registration_attempted
                    .swap(true, Ordering::AcqRel)
                {
                    return Err(error.describe(path));
                }
                match self.register_device().await {
                    Ok(()) => self
                        .send_get(path, query)
                        .await
                        .map_err(|retry_error| retry_error.describe(path)),
                    Err(registration_error) => Err(format!(
                        "{}；自动设备登记失败：{registration_error}",
                        error.describe(path)
                    )),
                }
            }
            Err(error) => Err(error.describe(path)),
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
    /// 该方法仅用于经实测确认可匿名访问的作品发现与元数据端点。协议 Basic
    /// Auth、请求签名和安装设备标识仍是 App API 的必要请求字段；“匿名”仅指
    /// 不附带 `.SFCommunity` 或 `session_APP`。章节正文、书架和账户端点不得
    /// 使用本方法。
    ///
    /// # 错误
    /// 请求失败、上游拒绝请求或响应不是有效 JSON 时返回错误。
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
        let response = self
            .signed_request(reqwest::Method::POST, path, true)?
            .json(body)
            .send()
            .await
            .map_err(|error| format!("SF App 请求失败：{error}"))?;
        Self::parse_response(response, path)
            .await
            .map_err(|error| error.describe(path))
    }
}
