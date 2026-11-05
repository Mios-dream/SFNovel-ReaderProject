//! 进程级共享 HTTP 客户端。
//!
//! `SfSource` 会按命令、按媒体 Range 请求反复创建，若每次新建 `reqwest::Client`
//! 就会丢掉连接池和 TLS 会话。客户端在这里只构建一次，其余位置克隆句柄。

use super::device::SF_WEB_USER_AGENT;
use std::sync::OnceLock;
use std::time::Duration;

/// App 接口请求超时。
const APP_TIMEOUT: Duration = Duration::from_secs(15);
/// 网页请求超时。
const WEB_TIMEOUT: Duration = Duration::from_secs(15);

static APP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
static WEB_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// 返回使用官方 App 请求画像的共享客户端。
///
/// 设备 UA 在一次安装内稳定，首个调用方决定 UA 即可。
pub(crate) fn app_client(user_agent: &str) -> reqwest::Client {
    APP_CLIENT
        .get_or_init(|| {
            build(user_agent, APP_TIMEOUT, false)
                .expect("构建 SF App 网络客户端失败：静态配置，不会失败")
        })
        .clone()
}

/// 返回使用桌面浏览器请求画像的共享客户端。
pub(crate) fn web_client() -> reqwest::Client {
    WEB_CLIENT
        .get_or_init(|| {
            build(SF_WEB_USER_AGENT, WEB_TIMEOUT, true)
                .expect("构建 SF 网页网络客户端失败：静态配置，不会失败")
        })
        .clone()
}

fn build(
    user_agent: &str,
    timeout: Duration,
    browser_headers: bool,
) -> reqwest::Result<reqwest::Client> {
    let mut headers = reqwest::header::HeaderMap::new();
    if browser_headers {
        headers.insert(
            reqwest::header::ACCEPT_LANGUAGE,
            reqwest::header::HeaderValue::from_static("zh-CN,zh;q=0.9,en;q=0.8"),
        );
    }
    reqwest::Client::builder()
        .timeout(timeout)
        .user_agent(user_agent)
        .default_headers(headers)
        .build()
}
