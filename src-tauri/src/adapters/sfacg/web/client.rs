//! SF 网页、AJAX 与静态资源客户端。

use super::super::device::SF_WEB_USER_AGENT;
use crate::adapters::sfacg::cookie::{filter_cookie_header, has_cookie_name};

/// 使用经过过滤的 Web 凭证发送 SF 网页、AJAX 和静态资源请求。
pub(crate) struct WebClient {
    /// 使用桌面浏览器请求画像的 HTTP 客户端。
    pub(super) client: reqwest::Client,
    /// 仅保留网页会话所需 Cookie 的请求头值。
    pub(super) web_cookie: Option<String>,
}

impl WebClient {
    /// 创建网页客户端并过滤会话 Cookie。
    ///
    /// # 参数
    /// - `web_cookie`：从会话快照读取的原始 Cookie 请求头。
    ///
    /// # 错误
    /// HTTP 客户端构建失败时返回错误。
    pub(crate) fn new(web_cookie: Option<String>) -> Result<Self, String> {
        let web_cookie = web_cookie
            .and_then(|cookie| filter_cookie_header(&cookie, &[".SFCommunity", "session_PC"]));
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent(SF_WEB_USER_AGENT)
            .default_headers({
                let mut headers = reqwest::header::HeaderMap::new();
                headers.insert(
                    reqwest::header::ACCEPT_LANGUAGE,
                    reqwest::header::HeaderValue::from_static("zh-CN,zh;q=0.9,en;q=0.8"),
                );
                headers
            })
            .build()
            .map_err(|error| format!("无法创建 SF 网页网络客户端：{error}"))?;
        Ok(Self { client, web_cookie })
    }

    /// 判断客户端是否持有网页会话 Cookie。
    pub(crate) fn has_session(&self) -> bool {
        self.web_cookie
            .as_deref()
            .is_some_and(|cookie| has_cookie_name(cookie, "session_PC"))
    }

    /// 要求当前客户端持有网页会话。
    ///
    /// # 错误
    /// 缺少 `session_PC` 时返回提示用户重新登录的错误。
    pub(crate) fn require_session(&self) -> Result<(), String> {
        self.has_session()
            .then_some(())
            .ok_or_else(|| "请先使用官方网页登录 Web 服务".to_string())
    }

    /// 创建网页页面请求，并附加 HTML 接受头、可选 Referer 和网页 Cookie。
    ///
    /// # 参数
    /// - `url`：页面地址。
    /// - `referer`：可选的来源页地址。
    ///
    /// # 返回
    /// 返回尚未发送的请求构造器。
    pub(crate) fn page_request(
        &self,
        url: impl reqwest::IntoUrl,
        referer: Option<&str>,
    ) -> reqwest::RequestBuilder {
        let mut request = self.client.get(url).header(
            reqwest::header::ACCEPT,
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        );
        if let Some(referer) = referer.filter(|value| !value.trim().is_empty()) {
            request = request.header(reqwest::header::REFERER, referer);
        }
        self.with_web_cookie(request)
    }

    /// 创建网页 AJAX 请求，并附加 JSON 接受头和必填 Referer。
    ///
    /// # 参数
    /// - `url`：AJAX 接口地址。
    /// - `referer`：上游要求的来源页地址。
    ///
    /// # 返回
    /// 返回尚未发送的请求构造器。
    pub(crate) fn ajax_request(
        &self,
        url: impl reqwest::IntoUrl,
        referer: &str,
    ) -> reqwest::RequestBuilder {
        self.with_web_cookie(
            self.client
                .get(url)
                .header("Accept", "application/json, text/javascript, */*; q=0.01")
                .header("X-Requested-With", "XMLHttpRequest")
                .header(reqwest::header::REFERER, referer),
        )
    }

    /// 创建静态图片或媒体请求。
    ///
    /// # 参数
    /// - `url`：资源地址。
    /// - `referer`：上游要求的来源页地址。
    /// - `accept`：资源类型接受头。
    ///
    /// # 返回
    /// 返回尚未发送的请求构造器。
    pub(crate) fn asset_request(
        &self,
        url: impl reqwest::IntoUrl,
        referer: &str,
        accept: &str,
    ) -> reqwest::RequestBuilder {
        self.with_web_cookie(
            self.client
                .get(url)
                .header(reqwest::header::ACCEPT, accept)
                .header(reqwest::header::REFERER, referer),
        )
    }

    /// 在请求存在网页 Cookie 时注入 `Cookie` 头。
    ///
    /// # 参数
    /// - `request`：待补充凭证的请求构造器。
    ///
    /// # 返回
    /// 返回补充 Cookie 后的请求构造器。
    fn with_web_cookie(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match self.web_cookie.as_deref() {
            Some(cookie) => request.header(reqwest::header::COOKIE, cookie),
            None => request,
        }
    }
}
