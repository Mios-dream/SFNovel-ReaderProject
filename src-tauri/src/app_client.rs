//! SF App API 签名客户端及其 App 会话边界。

use crate::sfacg::{
    device_token, security_header, validate_novel_id, ApiChapterContent, AuthSessionState,
    SF_API_HOST, SF_API_PASSWORD, SF_API_USER,
};
use serde_json::Value;
use tauri::Manager;
use uuid::Uuid;

/// 仅发送已签名的 SF App API 请求，并持有来自
/// `NativeAuthSession.app_cookie` 的已过滤 App 会话快照。
pub(crate) struct AppClient {
    client: reqwest::Client,
    app_cookie: Option<String>,
}

impl AppClient {
    /// 仅使用原生 App 会话状态创建 API 客户端。
    ///
    /// # 错误
    /// 当设备身份未初始化或 HTTP 客户端无法创建时返回错误。
    pub(crate) fn new(app: &tauri::AppHandle) -> Result<Self, String> {
        let token = device_token()?.to_uppercase();
        let user_agent = format!("boluobao/5.0.36(android;34)/H5/{token}/H5");
        let app_cookie = Self::read_app_cookie(app)?;
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent(user_agent)
            .build()
            .map_err(|error| format!("无法创建 SF App 网络客户端：{error}"))?;
        Ok(Self { client, app_cookie })
    }

    /// 判断此客户端是否持有包含 `session_APP` 的 App 会话。
    pub(crate) fn has_session(&self) -> bool {
        self.app_cookie.as_ref().is_some_and(|cookie| {
            cookie
                .split(';')
                .any(|pair| pair.trim().starts_with("session_APP="))
        })
    }

    /// 按固定请求指纹构建一个已签名的 App API 请求。
    ///
    /// # 参数
    /// * `method` - 请求使用的 HTTP 方法。
    /// * `path` - 相对 SF App API 路径。
    /// 每次请求都会生成新的 UUID nonce。
    ///
    /// # 错误
    /// 当无法生成安全签名头时返回错误。
    fn signed_request(
        &self,
        method: reqwest::Method,
        path: &str,
    ) -> Result<reqwest::RequestBuilder, String> {
        let nonce = Uuid::new_v4().to_string().to_uppercase();
        let mut request = self
            .client
            .request(method, format!("{SF_API_HOST}{path}"))
            .basic_auth(SF_API_USER, Some(SF_API_PASSWORD))
            .header("Accept", "application/vnd.sfacg.api+json;version=1")
            .header("Accept-Language", "zh-Hans-CN;q=1")
            .header("Content-Type", "application/json")
            .header("SFSecurity", security_header(&nonce)?);
        if let Some(cookie) = self.app_cookie.as_deref() {
            request = request.header(reqwest::header::COOKIE, cookie);
        }
        Ok(request)
    }

    /// 请求一个已签名的 SF App API 资源，并拆开其 `data` 数据包。
    ///
    /// # 错误
    /// 请求失败、上游拒绝请求或响应不是有效 JSON 时返回错误；HTTP 417 会清除
    /// 缓存的随机数，以便下次请求重新协商。
    pub(super) async fn get_data(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<Value, String> {
        let response = self
            .signed_request(reqwest::Method::GET, path)?
            .query(query)
            .send()
            .await
            .map_err(|error| format!("SF App 请求失败：{error}"))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.json::<Value>().await.unwrap_or(Value::Null);
            let api_code = body
                .get("status")
                .and_then(|value| value.get("errorCode"))
                .and_then(Value::as_i64);
            let api_message = body
                .get("status")
                .and_then(|value| value.get("msg"))
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty());
            eprintln!(
                "[sfacg] App request rejected: path={path}, http={}, api_code={api_code:?}, message={api_message:?}",
                status.as_u16(),
            );
            return Err(match api_message {
                Some(message) => format!("SF App 请求返回 HTTP {}：{message}", status.as_u16()),
                None => format!("SF App 请求返回 HTTP {}", status.as_u16()),
            });
        }
        let body = response
            .json::<Value>()
            .await
            .map_err(|error| format!("SF App 响应格式无效：{error}"))?;
        Ok(body.get("data").cloned().unwrap_or(body))
    }

    /// Sends a signed JSON POST request and returns its unwrapped data payload.
    ///
    /// This is kept private to native-side integrations whose request body is
    /// defined by the App protocol; no arbitrary renderer payload reaches it.
    pub(super) async fn post_data(&self, path: &str, body: &Value) -> Result<Value, String> {
        let response = self
            .signed_request(reqwest::Method::POST, path)?
            .json(body)
            .send()
            .await
            .map_err(|error| format!("SF App 请求失败：{error}"))?;
        let status = response.status();
        let body = response.json::<Value>().await.unwrap_or(Value::Null);
        let api_http_code = body
            .get("status")
            .and_then(|value| value.get("httpCode"))
            .and_then(Value::as_i64);
        if !status.is_success() || api_http_code.is_some_and(|code| !(200..300).contains(&code)) {
            let api_code = body
                .get("status")
                .and_then(|value| value.get("errorCode"))
                .and_then(Value::as_i64);
            let api_message = body
                .get("status")
                .and_then(|value| value.get("msg"))
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty());
            eprintln!(
                "[sfacg] App request rejected: path={path}, http={}, api_code={api_code:?}, message={api_message:?}",
                status.as_u16(),
            );
            return Err(match api_message {
                Some(message) => format!(
                    "SF App 请求返回 HTTP/API {}：{message}",
                    api_http_code
                        .map_or_else(|| status.as_u16().to_string(), |code| code.to_string())
                ),
                None => format!(
                    "SF App 请求返回 HTTP/API {}",
                    api_http_code
                        .map_or_else(|| status.as_u16().to_string(), |code| code.to_string())
                ),
            });
        }
        Ok(body.get("data").cloned().unwrap_or(body))
    }

    /// Reports the current installation to the App endpoint after login.
    ///
    /// The report is diagnostic and must not become a prerequisite for login or
    /// other App requests. The upstream field named `deviceToken` is a separate
    /// client protocol value from the per-install UUID used as `deviceId`.
    pub(super) async fn report_android_device_info(&self, account_id: i64) -> Result<(), String> {
        if account_id <= 0 {
            return Err("SF 账号编号无效".to_string());
        }
        let device_id = device_token()?.to_lowercase();
        self.post_data(
            "/user/androiddeviceinfos",
            &serde_json::json!({
                "accountId": account_id,
                "package": "com.sfacg",
                "abi": "arm64-v8a",
                "deviceId": device_id,
                "version": "4.8.22",
                "deviceToken": "7b2a42976f97d470",
            }),
        )
        .await
        .map(|_| ())
    }

    /// 通过 App 端点登录，并仅返回可安全持久化的已识别 App Cookie。
    ///
    /// 密码只用于当前请求，不写入返回值、日志或持久化文件。
    ///
    /// # 错误
    /// 登录失败、上游未返回 App 会话或响应无效时返回错误。
    pub(super) async fn login_with_password(
        &self,
        username: &str,
        password: &str,
    ) -> Result<String, String> {
        let response = self
            .signed_request(reqwest::Method::POST, "/sessions")?
            .json(&serde_json::json!({
                "userName": username,
                "passWord": password,
            }))
            .send()
            .await
            .map_err(|error| format!("SF App 登录请求失败：{error}"))?;
        let status = response.status();
        let cookie = Self::filter_app_cookie(
            response
                .headers()
                .get_all(reqwest::header::SET_COOKIE)
                .iter()
                .filter_map(|header| header.to_str().ok())
                .filter_map(|header| header.split(';').next())
                .collect::<Vec<_>>()
                .join("; ")
                .as_str(),
        );
        let body = response.json::<Value>().await.unwrap_or(Value::Null);
        let successful = status.is_success()
            && body
                .get("status")
                .and_then(|value| value.get("httpCode"))
                .and_then(Value::as_i64)
                == Some(200);
        if !successful {
            return Err(body
                .get("status")
                .and_then(|value| value.get("msg"))
                .and_then(Value::as_str)
                .filter(|message| !message.trim().is_empty())
                .unwrap_or("SF App 账号密码登录失败")
                .to_string());
        }
        cookie.ok_or_else(|| "SF App 登录成功但未返回 App 会话 Cookie".to_string())
    }

    /// 获取本地正文解码所需的 App 章节正文及归属元数据。
    ///
    /// # 错误
    /// 章节编号无效、上游未返回正文或缺少作品、分卷标识时返回错误。
    pub(super) async fn chapter_content_with_metadata_from_api(
        &self,
        chapter_id: i64,
    ) -> Result<ApiChapterContent, String> {
        if chapter_id <= 0 {
            return Err("章节编号无效".to_string());
        }
        let response = self
            .get_data(
                &format!("/Chaps/{chapter_id}"),
                &[("expand", "content,expand.content".to_string())],
            )
            .await?;
        let content = response
            .get("expand")
            .and_then(|value| value.get("content"))
            .and_then(Value::as_str)
            .or_else(|| response.get("content").and_then(Value::as_str))
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "SF App API 未返回章节正文".to_string())?;
        let novel_id = response
            .get("novelId")
            .and_then(Value::as_i64)
            .ok_or_else(|| "SF App API 未返回章节所属作品信息".to_string())?;
        let volume_id = response
            .get("volumeId")
            .and_then(Value::as_i64)
            .ok_or_else(|| "SF App API 未返回章节所属卷信息".to_string())?;
        Ok(ApiChapterContent {
            content: content.to_string(),
            novel_id,
            volume_id,
        })
    }

    /// 仅读取构造公开漫画网址所需的 App 元数据。
    ///
    /// # 错误
    /// 漫画编号无效或上游未返回标题、目录标识时返回错误。
    pub(super) async fn comic_identity(&self, comic_id: i64) -> Result<(String, String), String> {
        validate_novel_id(comic_id)?;
        let detail = self.get_data(&format!("/comics/{comic_id}"), &[]).await?;
        let title = detail
            .get("comicName")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "SF 漫画接口未返回作品名称".to_string())?
            .to_string();
        let folder = detail
            .get("folderName")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "SF 漫画接口未返回目录标识".to_string())?
            .to_string();
        Ok((title, folder))
    }

    /// 从原生 App 会话中读取并过滤出 `.SFCommunity` 与 `session_APP`。
    fn read_app_cookie(app: &tauri::AppHandle) -> Result<Option<String>, String> {
        let auth_state = app.state::<AuthSessionState>();
        let session = auth_state
            .session
            .lock()
            .map_err(|_| "登录会话状态不可用".to_string())?;
        Ok(session
            .as_ref()
            .and_then(|session| session.app_cookie.as_deref())
            .and_then(Self::filter_app_cookie))
    }

    /// 在构建 App 请求前剔除 Web 会话及未知 Cookie 名称。
    fn filter_app_cookie(cookie: &str) -> Option<String> {
        let pairs = cookie
            .split(';')
            .map(str::trim)
            .filter(|pair| pair.starts_with(".SFCommunity=") || pair.starts_with("session_APP="))
            .collect::<Vec<_>>();
        (!pairs.is_empty()).then(|| pairs.join("; "))
    }
}
