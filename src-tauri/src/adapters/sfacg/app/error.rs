use serde_json::Value;

/// SF App 请求失败的结构化分类。
///
/// 同时保留 HTTP 状态和上游 JSON 中的业务状态，用于区分会话失效、设备未登记、
/// 客户端版本限制和账号风控，而不是只拼接一段错误文本。
pub(crate) struct AppApiError {
    /// 传输层 HTTP 状态码；本地错误为 `0`。
    pub(super) http_status: u16,
    /// 上游 `status.httpCode` 业务状态码。
    pub(super) api_http_code: Option<i64>,
    /// 上游 `status.errorCode` 错误码。
    pub(super) api_error_code: Option<i64>,
    /// 上游返回并去除首尾空白后的错误说明。
    pub(super) message: Option<String>,
}

impl AppApiError {
    /// 创建尚未获得 HTTP 响应的本地错误。
    ///
    /// # 参数
    /// - `message`：面向调用方的错误说明。
    pub(super) fn local(message: String) -> Self {
        Self {
            http_status: 0,
            api_http_code: None,
            api_error_code: None,
            message: Some(message),
        }
    }

    /// 将网络传输失败转换为本地错误。
    ///
    /// # 参数
    /// - `error`：`reqwest` 返回的传输错误。
    pub(super) fn transport(error: reqwest::Error) -> Self {
        Self::local(format!("SF App 请求失败：{error}"))
    }

    /// 从上游响应状态和 JSON 数据体构造分类错误。
    ///
    /// # 参数
    /// - `http_status`：响应 HTTP 状态码。
    /// - `body`：上游返回的 JSON 错误数据体。
    pub(super) fn from_response(http_status: u16, body: &Value) -> Self {
        let status = body.get("status");
        Self {
            http_status,
            api_http_code: status
                .and_then(|value| value.get("httpCode"))
                .and_then(Value::as_i64),
            api_error_code: status
                .and_then(|value| value.get("errorCode"))
                .and_then(Value::as_i64),
            message: status
                .and_then(|value| value.get("msg"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string),
        }
    }

    /// 判断失败是否表示当前设备尚未登记到该账号。
    pub(super) fn is_device_unregistered(&self) -> bool {
        self.api_error_code == Some(507)
            && (self.http_status == 401 || self.api_http_code == Some(401))
    }

    /// 判断失败是否表示登录 Cookie 已失效。
    pub(crate) fn is_session_expired(&self) -> bool {
        let code = self
            .api_http_code
            .or_else(|| (self.http_status > 0).then_some(i64::from(self.http_status)));
        code == Some(401) && self.api_error_code != Some(507)
    }

    /// 判断失败是否表示接口只对官方客户端开放。
    ///
    /// 实测该拦截只取决于请求头里的 App 版本串，接口地址和参数都与官方客户端一致，
    /// 换 UA 只会把拒绝码从 417 变成 422，无法绕过。
    pub(super) fn is_client_version_gated(&self) -> bool {
        self.api_http_code == Some(417) || self.http_status == 417
    }

    /// 判断失败是否表示账号触发了风控，需要人工完成验证码。
    pub(crate) fn is_account_risk_controlled(&self) -> bool {
        if self.api_error_code == Some(1146) {
            return true;
        }
        let code = self
            .api_http_code
            .or_else(|| (self.http_status > 0).then_some(i64::from(self.http_status)));
        code == Some(422)
            && self
                .message
                .as_deref()
                .is_some_and(|message| message.contains("风险"))
    }

    /// 将结构化错误转换为适合展示给用户的说明。
    ///
    /// # 参数
    /// - `_path`：失败请求的路径；当前仅用于保留调用上下文，不参与文本生成。
    ///
    /// # 返回
    /// 返回包含可操作提示的错误文本。
    pub(crate) fn describe(&self, _path: &str) -> String {
        let message = self
            .message
            .clone()
            .unwrap_or_else(|| "上游未返回说明".to_string());
        let code = self
            .api_http_code
            .or_else(|| (self.http_status > 0).then_some(i64::from(self.http_status)));
        if self.is_client_version_gated() {
            return format!("上游以客户端版本过低拒绝（417/782）：{message}");
        }
        if self.is_account_risk_controlled() {
            return format!(
                "账号触发了 SF 风控，需要人工验证（422）：{message}。请在官方 App 完成验证码验证后再试。"
            );
        }
        match (code, self.api_error_code) {
            (_, Some(507)) => {
                format!("SF App 要求先登记当前设备（401/507）：{message}")
            }
            (_, Some(614)) => format!("账号或密码错误（{}/614）：{message}", code.unwrap_or(0)),
            (Some(401), _) => {
                format!("SF App 会话失效或账号未授权（401）：{message}。请重新登录 App。")
            }
            (Some(403), _) => format!("SF App 没有访问权限（403）：{message}"),
            (Some(code), Some(error_code)) => {
                format!("SF App 请求返回 {code}（错误码 {error_code}）：{message}")
            }
            (Some(code), None) => format!("SF App 请求返回 {code}：{message}"),
            _ => message,
        }
    }
}

/// 判断任意错误文本是否表示账号触发了 SF 风控。
///
/// 下载层会将 `AppApiError` 转换为字符串，此函数用于在继续请求剩余章节前
/// 识别这些已经失去结构化类型的错误。
pub(crate) fn is_risk_control_failure(message: &str) -> bool {
    message.contains("风控") || message.contains("1146") || message.contains("422")
}
