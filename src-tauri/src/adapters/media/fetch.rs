//! 受保护图片的统一抓取与格式识别。
//!
//! 在线阅读、正文插图与漫画页面都要做同一件事：带 Referer 拉取 SF 的受保护图片，
//! 避免空响应，再按文件头决定扩展名或 MIME。三处调用共享这一份实现。

use crate::adapters::sfacg::source::SfSource;
use crate::domain::error::{AppError, AppResult};
use std::time::Duration;

/// 请求图片时声明的可接受格式。
pub(super) const IMAGE_ACCEPT: &str = "image/avif,image/webp,image/apng,image/*,*/*;q=0.8";

/// 一次成功读取的远程图片。
pub(super) struct RemoteImage {
    /// 图片内容。
    pub(super) bytes: Vec<u8>,
    /// 上游声明的 MIME 类型，已转小写；缺失时为空串。
    pub(super) content_type: String,
}

/// 拉取一张需要凭据的受保护图片。
///
/// # 参数
/// - `client`：已认证的 SF 内容源。
/// - `url`：已校验的上游图片地址。
/// - `referer`：请求图片所需的 Referer。
/// - `timeout`：可选的单次请求超时；在线浏览不设限，媒体协议转发需要。
///
/// # 错误
/// 连接失败、请求被拒绝、读取失败或内容为空时返回上游错误。
pub(super) async fn fetch_remote_image(
    client: &SfSource,
    url: &str,
    referer: &str,
    timeout: Option<Duration>,
) -> AppResult<RemoteImage> {
    let mut request = client.asset_request(url, referer, IMAGE_ACCEPT);
    if let Some(timeout) = timeout {
        request = request.timeout(timeout);
    }
    let response = request
        .send()
        .await
        .map_err(|error| AppError::upstream(format!("无法连接图片资源：{error}")))?;
    let response = response
        .error_for_status()
        .map_err(|error| AppError::upstream(format!("图片资源请求被拒绝：{error}")))?;
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    let bytes = response
        .bytes()
        .await
        .map_err(|error| AppError::upstream(format!("无法读取图片数据：{error}")))?;
    if bytes.is_empty() {
        return Err(AppError::upstream("图片内容为空"));
    }
    Ok(RemoteImage {
        bytes: bytes.to_vec(),
        content_type,
    })
}

/// 根据文件签名和 MIME 类型选择图片扩展名。
///
/// # 参数
/// - `content_type`：上游响应的 MIME 类型。
/// - `bytes`：图片内容。
pub(super) fn image_extension(content_type: &str, bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        "gif"
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "png"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "webp"
    } else if content_type.contains("image/png") {
        "png"
    } else if content_type.contains("image/gif") {
        "gif"
    } else if content_type.contains("image/webp") {
        "webp"
    } else if content_type.contains("image/avif") {
        "avif"
    } else {
        "jpg"
    }
}

/// 根据文件签名推断图片 MIME 类型。
///
/// # 参数
/// - `bytes`：图片文件头或完整内容。
pub(super) fn image_content_type(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        "image/gif"
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "image/webp"
    } else if bytes.get(4..8) == Some(b"ftyp")
        && bytes
            .get(8..12)
            .is_some_and(|brand| brand == b"avif" || brand == b"avis")
    {
        "image/avif"
    } else {
        "image/jpeg"
    }
}
