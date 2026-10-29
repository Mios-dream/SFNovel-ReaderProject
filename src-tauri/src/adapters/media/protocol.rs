//! `sfmedia` 本地媒体协议与 Range 音频转发。

use super::{OnlineMediaResource, OnlineMediaState};
use crate::adapters::sfacg::source::SfSource;
use std::fs;
use std::path::Path;
use std::time::Duration;
use tauri::http::header::{
    ACCEPT_RANGES, ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_EXPOSE_HEADERS, CACHE_CONTROL,
    CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE,
};
use tauri::http::{HeaderMap, Method, Request, Response, StatusCode};
use tauri::{Manager, Runtime, UriSchemeContext, UriSchemeResponder};

/// 应用注册的本地媒体协议名称。
pub(crate) const ONLINE_MEDIA_SCHEME: &str = "sfmedia";
/// 单次音频 Range 响应的最大字节数。
const AUDIO_STREAM_CHUNK_BYTES: u64 = 1024 * 1024;
/// 在线媒体上游请求的超时时间。
const ONLINE_MEDIA_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// 创建带有跨域、缓存和暴露响应头配置的媒体响应构造器。
///
/// # 参数
/// - `status`：响应的 HTTP 状态码。
fn media_response_builder(status: StatusCode) -> tauri::http::response::Builder {
    Response::builder()
        .status(status)
        .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(
            ACCESS_CONTROL_EXPOSE_HEADERS,
            "Content-Range, Content-Length",
        )
        .header(CACHE_CONTROL, "no-store")
}

/// 创建纯文本媒体错误响应。
///
/// # 参数
/// - `status`：错误状态码。
/// - `message`：返回给媒体元素的错误说明。
fn media_error_response(status: StatusCode, message: &str) -> Response<Vec<u8>> {
    media_response_builder(status)
        .header(CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(message.as_bytes().to_vec())
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

/// 创建 Range 无法满足时的标准响应。
///
/// # 参数
/// - `total`：已知的资源总字节数；未知时省略 `Content-Range`。
fn audio_range_not_satisfiable(total: Option<u64>) -> Response<Vec<u8>> {
    let mut builder =
        media_response_builder(StatusCode::RANGE_NOT_SATISFIABLE).header(ACCEPT_RANGES, "bytes");
    if let Some(total) = total {
        builder = builder.header(CONTENT_RANGE, format!("bytes */{total}"));
    }
    builder
        .header(CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(b"Requested range is not available".to_vec())
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

/// 解析第一个字节范围请求。
///
/// # 参数
/// - `value`：原始 `Range` 请求头。
///
/// # 返回
/// 起始偏移及可选的结束偏移；格式无效时回退为完整响应。
fn parse_requested_range(value: Option<&str>) -> (u64, Option<u64>) {
    let Some(spec) = value.and_then(|value| value.strip_prefix("bytes=")) else {
        return (0, None);
    };
    let Some((start, end)) = spec.split(',').next().unwrap_or(spec).split_once('-') else {
        return (0, None);
    };
    let Ok(start) = start.trim().parse::<u64>() else {
        return (0, None);
    };
    (start, end.trim().parse::<u64>().ok())
}

/// 从响应头读取资源总字节数。
///
/// # 参数
/// - `headers`：上游响应头。
fn response_total_bytes(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.rsplit_once('/'))
        .and_then(|(_, total)| total.parse::<u64>().ok())
        .or_else(|| {
            headers
                .get(CONTENT_LENGTH)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
        })
}

/// 读取上游响应中指定偏移和长度的数据窗口。
///
/// # 参数
/// - `response`：仍可继续读取的流式响应。
/// - `skip`：需要跳过的字节数。
/// - `limit`：最多读取的字节数。
///
/// # 错误
/// 上游流读取失败时返回错误消息。
async fn read_response_window(
    mut response: reqwest::Response,
    skip: u64,
    limit: usize,
) -> Result<Vec<u8>, String> {
    let mut body = Vec::with_capacity(limit.min(256 * 1024));
    let mut remaining_skip = skip;
    while remaining_skip > 0 {
        let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| format!("无法读取在线音频数据：{error}"))?
        else {
            return Ok(body);
        };
        let chunk_len = chunk.len() as u64;
        if chunk_len <= remaining_skip {
            remaining_skip -= chunk_len;
            continue;
        }
        let offset = remaining_skip as usize;
        remaining_skip = 0;
        let take = (chunk.len() - offset).min(limit);
        body.extend_from_slice(&chunk[offset..offset + take]);
        if body.len() >= limit {
            return Ok(body);
        }
    }
    while body.len() < limit {
        let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| format!("无法读取在线音频数据：{error}"))?
        else {
            break;
        };
        let remaining = limit - body.len();
        let take = chunk.len().min(remaining);
        body.extend_from_slice(&chunk[..take]);
        if take < chunk.len() {
            break;
        }
    }
    Ok(body)
}

/// 根据文件签名推断图片 MIME 类型。
///
/// # 参数
/// - `bytes`：图片文件头或完整内容。
fn image_content_type(bytes: &[u8]) -> &'static str {
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

/// 通过临时文件替换方式写入在线媒体缓存。
///
/// # 参数
/// - `target`：最终缓存文件路径。
/// - `bytes`：需要写入的媒体内容。
///
/// # 错误
/// 临时文件写入、旧文件删除或重命名失败时返回错误消息。
fn write_online_media_cache(target: &Path, bytes: &[u8]) -> Result<(), String> {
    let temporary = target.with_extension("part");
    fs::write(&temporary, bytes).map_err(|error| format!("无法写入在线媒体缓存：{error}"))?;
    if target.exists() {
        fs::remove_file(target).map_err(|error| format!("无法替换在线媒体缓存：{error}"))?;
    }
    fs::rename(&temporary, target).map_err(|error| format!("无法完成在线媒体缓存写入：{error}"))
}

/// 将受保护的上游音频转换为本地 Range 媒体响应。
///
/// # 参数
/// - `app`：用于构造上游客户端的应用句柄。
/// - `source`：上游音频地址。
/// - `referer`：上游请求所需 Referer。
/// - `method`：媒体元素的 GET 或 HEAD 方法。
/// - `range_header`：可选的字节范围请求。
async fn serve_audio_stream<R: Runtime>(
    app: &tauri::AppHandle<R>,
    source: &str,
    referer: &str,
    method: &Method,
    range_header: Option<&str>,
) -> Response<Vec<u8>> {
    let client = match SfSource::new(app) {
        Ok(client) => client,
        Err(error) => return media_error_response(StatusCode::BAD_GATEWAY, &error),
    };
    let is_head = *method == Method::HEAD;
    let (start, requested_end) = parse_requested_range(range_header);
    let limit = if is_head { 1 } else { AUDIO_STREAM_CHUNK_BYTES };
    let end = start
        .saturating_add(limit - 1)
        .min(requested_end.unwrap_or(u64::MAX));
    let response = match client
        .asset_request(source, referer, "audio/mpeg,audio/*,*/*;q=0.8")
        .header(RANGE, format!("bytes={start}-{end}"))
        .timeout(ONLINE_MEDIA_REQUEST_TIMEOUT)
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            return media_error_response(
                StatusCode::BAD_GATEWAY,
                &format!("无法连接在线音频：{error}"),
            )
        }
    };
    let total = response_total_bytes(response.headers());
    if response.status() == StatusCode::RANGE_NOT_SATISFIABLE {
        return audio_range_not_satisfiable(total);
    }
    if !response.status().is_success() {
        return media_error_response(
            StatusCode::BAD_GATEWAY,
            &format!("在线音频请求被拒绝：{}", response.status()),
        );
    }
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("audio/mpeg")
        .to_string();
    if total.is_some_and(|total| start >= total) {
        return audio_range_not_satisfiable(total);
    }
    if is_head {
        let mut builder = media_response_builder(StatusCode::OK)
            .header(CONTENT_TYPE, content_type)
            .header(ACCEPT_RANGES, "bytes");
        if let Some(total) = total {
            builder = builder.header(CONTENT_LENGTH, total);
        }
        return builder
            .body(Vec::new())
            .unwrap_or_else(|_| Response::new(Vec::new()));
    }
    let body_limit = (end.saturating_sub(start).saturating_add(1)) as usize;
    let skip = if response.status() == StatusCode::OK {
        start
    } else {
        0
    };
    let body = match read_response_window(response, skip, body_limit).await {
        Ok(body) => body,
        Err(error) => return media_error_response(StatusCode::BAD_GATEWAY, &error),
    };
    if body.is_empty() {
        return media_error_response(StatusCode::BAD_GATEWAY, "在线音频内容为空");
    }
    let actual_end = start.saturating_add(body.len() as u64 - 1);
    let mut builder = media_response_builder(StatusCode::PARTIAL_CONTENT)
        .header(CONTENT_TYPE, content_type)
        .header(ACCEPT_RANGES, "bytes")
        .header(CONTENT_LENGTH, body.len() as u64);
    if let Some(total) = total {
        if start >= total {
            return audio_range_not_satisfiable(Some(total));
        }
        builder = builder.header(CONTENT_RANGE, format!("bytes {start}-{actual_end}/{total}"));
    }
    builder
        .body(body)
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

/// 读取漫画页面缓存，缺失时从上游下载并写回。
///
/// # 参数
/// - `app`：用于构造上游客户端的应用句柄。
/// - `source`：上游图片地址。
/// - `referer`：上游请求所需 Referer。
/// - `cache_path`：本地页面缓存路径。
/// - `method`：媒体元素的 GET 或 HEAD 方法。
async fn serve_comic_page<R: Runtime>(
    app: &tauri::AppHandle<R>,
    source: &str,
    referer: &str,
    cache_path: &Path,
    method: &Method,
) -> Response<Vec<u8>> {
    let bytes = match fs::read(cache_path) {
        Ok(bytes) => bytes,
        Err(_) => {
            let client = match SfSource::new(app) {
                Ok(client) => client,
                Err(error) => return media_error_response(StatusCode::BAD_GATEWAY, &error),
            };
            let bytes = match client
                .asset_request(
                    source,
                    referer,
                    "image/avif,image/webp,image/apng,image/*,*/*;q=0.8",
                )
                .timeout(ONLINE_MEDIA_REQUEST_TIMEOUT)
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => match response.bytes().await {
                    Ok(bytes) if !bytes.is_empty() => bytes.to_vec(),
                    Ok(_) => {
                        return media_error_response(StatusCode::BAD_GATEWAY, "在线漫画页面为空")
                    }
                    Err(error) => {
                        return media_error_response(
                            StatusCode::BAD_GATEWAY,
                            &format!("无法读取在线漫画页面：{error}"),
                        )
                    }
                },
                Ok(response) => {
                    return media_error_response(
                        StatusCode::BAD_GATEWAY,
                        &format!("在线漫画页面请求被拒绝：{}", response.status()),
                    )
                }
                Err(error) => {
                    return media_error_response(
                        StatusCode::BAD_GATEWAY,
                        &format!("无法连接在线漫画页面：{error}"),
                    )
                }
            };
            if let Err(error) = write_online_media_cache(cache_path, &bytes) {
                eprintln!("[sfacg] online comic page cache skipped: {error}");
            }
            bytes
        }
    };
    let content_type = image_content_type(&bytes);
    let content_length = bytes.len();
    let body = if *method == Method::HEAD {
        Vec::new()
    } else {
        bytes
    };
    media_response_builder(StatusCode::OK)
        .header(CONTENT_TYPE, content_type)
        .header(CONTENT_LENGTH, content_length as u64)
        .header(ACCEPT_RANGES, "bytes")
        .body(body)
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

/// 解析媒体协议路径并分派到音频或漫画处理逻辑。
///
/// # 参数
/// - `app`：应用句柄。
/// - `path`：`sfmedia` 请求路径，格式为 `/{kind}/{token}`。
/// - `method`：媒体请求方法。
/// - `range_header`：可选的 `Range` 请求头。
async fn serve_online_media<R: Runtime>(
    app: &tauri::AppHandle<R>,
    path: &str,
    method: &Method,
    range_header: Option<&str>,
) -> Response<Vec<u8>> {
    let mut segments = path.trim_start_matches('/').split('/');
    let kind = segments.next().unwrap_or_default();
    let token = segments.next().unwrap_or_default();
    if token.is_empty() || segments.next().is_some() {
        return media_error_response(StatusCode::NOT_FOUND, "在线媒体地址无效");
    }
    let Some(resource) = app.state::<OnlineMediaState>().get(token) else {
        return media_error_response(StatusCode::NOT_FOUND, "在线媒体已失效，请重新加载");
    };
    match (kind, resource) {
        ("audio", OnlineMediaResource::Audio { source, referer }) => {
            serve_audio_stream(app, &source, &referer, method, range_header).await
        }
        (
            "comic",
            OnlineMediaResource::ComicPage {
                source,
                referer,
                cache_path,
            },
        ) => serve_comic_page(app, &source, &referer, &cache_path, method).await,
        _ => media_error_response(StatusCode::NOT_FOUND, "在线媒体类型不匹配"),
    }
}

/// 处理 Tauri 注册的 `sfmedia` 异步协议请求。
///
/// # 参数
/// - `context`：Tauri 协议上下文，用于取得应用句柄。
/// - `request`：本地媒体请求。
/// - `responder`：用于异步返回响应的应答器。
pub(crate) fn handle_online_media_request<R: Runtime>(
    context: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let app = context.app_handle().clone();
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let range_header = request
        .headers()
        .get(RANGE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    tauri::async_runtime::spawn(async move {
        let response = serve_online_media(&app, &path, &method, range_header.as_deref()).await;
        responder.respond(response);
    });
}

/// 校验上游资源 URL，并强制使用 SFACG 域名和 HTTPS。
///
/// # 参数
/// - `value`：上游返回的绝对资源地址。
///
/// # 错误
/// URL 无法解析、域名不属于 `sfacg.com` 或协议无法升级到 HTTPS 时返回错误。
pub(super) fn validate_sfacg_asset_url(value: &str) -> Result<reqwest::Url, String> {
    let mut url = reqwest::Url::parse(value).map_err(|_| "上游资源地址无效".to_string())?;
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if host != "sfacg.com" && !host.ends_with(".sfacg.com") {
        return Err("上游资源域名不受支持".to_string());
    }
    match url.scheme() {
        "https" => {}
        "http" => {
            url.set_scheme("https")
                .map_err(|_| "上游资源地址无法升级为 HTTPS".to_string())?;
        }
        _ => return Err("上游资源必须使用 HTTPS".to_string()),
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::{parse_requested_range, validate_sfacg_asset_url};

    #[test]
    fn ranges_are_parsed_with_safe_fallback() {
        assert_eq!(parse_requested_range(Some("bytes=10-19")), (10, Some(19)));
        assert_eq!(parse_requested_range(Some("invalid")), (0, None));
    }

    #[test]
    fn asset_urls_are_limited_to_sfacg_and_upgraded() {
        assert_eq!(
            validate_sfacg_asset_url("http://img.sfacg.com/a.jpg")
                .unwrap()
                .scheme(),
            "https"
        );
        assert!(validate_sfacg_asset_url("https://example.com/a.jpg").is_err());
    }
}
