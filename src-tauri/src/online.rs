//! 在线浏览所需的按需内容读取。
//!
//! 在线浏览不创建下载任务，也不写入本地书库。章节正文、漫画页面和音频仅在
//! 用户打开时读取，并缓存在应用私有目录中，以便同一章节再次访问时无需重复请求。
//! Cookie 与上游媒体地址始终留在原生层；渲染进程只接收带短期令牌的本地媒体
//! 协议地址，音频按 Range 流式转发，漫画页面则按需读取和落盘。

use crate::endpoint_policy::{app_endpoint_client, web_endpoint_client, EndpointCapability};
use crate::library::{decode_api_content, get_request_policy};
use crate::ocr::recognize_image;
#[cfg(target_os = "android")]
use crate::sfacg::sync_android_auth_session;
use crate::sfacg::{validate_novel_id, TextContentKind};
use crate::web_client::WebClient;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use tauri::http::header::{
    ACCEPT_RANGES, ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_EXPOSE_HEADERS, CACHE_CONTROL,
    CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE,
};
use tauri::http::{HeaderMap, Method, Request, Response, StatusCode};
use tauri::{Manager, Runtime, State, UriSchemeContext, UriSchemeResponder};

/// 在线媒体协议名。Windows 与 Android 会映射到 `http://sfmedia.localhost`。
pub(crate) const ONLINE_MEDIA_SCHEME: &str = "sfmedia";
/// 单次音频 Range 响应的最大长度，使播放器无需等待完整音频下载。
const AUDIO_STREAM_CHUNK_BYTES: u64 = 1024 * 1024;
/// 慢速网络下单个媒体分块仍需要较长的下载时间，避免沿用普通接口的短超时。
const ONLINE_MEDIA_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// 在线文字章节返回给渲染进程的数据。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OnlineTextChapter {
    pub(crate) id: i64,
    pub(crate) title: String,
    pub(crate) volume: String,
    pub(crate) content: String,
    /// 正文中的 `{{image:n}}` 占位符对应的缓存图片绝对路径。
    pub(crate) images: Vec<String>,
}

/// 在线漫画章节返回给渲染进程的数据。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OnlineComicChapter {
    pub(crate) id: i64,
    pub(crate) title: String,
    pub(crate) pages: Vec<String>,
}

/// 在线音频章节的流式播放地址。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OnlineAudioTrack {
    pub(crate) id: i64,
    pub(crate) title: String,
    pub(crate) volume: String,
    pub(crate) url: String,
}

/// 原生媒体协议可读取的在线资源。
#[derive(Clone)]
enum OnlineMediaResource {
    Audio {
        source: String,
        referer: String,
    },
    ComicPage {
        source: String,
        referer: String,
        cache_path: PathBuf,
    },
}

/// 保存在应用进程中的媒体令牌，避免把受保护的 SF 地址暴露给渲染进程。
#[derive(Default)]
pub(crate) struct OnlineMediaState {
    resources: Mutex<HashMap<String, OnlineMediaResource>>,
}

/// 返回在线缓存根目录。目录只在原生层解析，不接受渲染进程提供的路径。
fn online_cache_directory(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|directory| directory.join("online-cache"))
        .map_err(|error| format!("无法解析在线浏览缓存目录：{error}"))
}

/// 生成渲染进程可访问的原生媒体协议地址。
fn online_media_url(kind: &str, token: &str) -> String {
    #[cfg(any(target_os = "windows", target_os = "android"))]
    {
        format!("http://{ONLINE_MEDIA_SCHEME}.localhost/{kind}/{token}")
    }
    #[cfg(not(any(target_os = "windows", target_os = "android")))]
    {
        format!("{ONLINE_MEDIA_SCHEME}://localhost/{kind}/{token}")
    }
}

fn register_online_media_resource(
    state: &OnlineMediaState,
    resource: OnlineMediaResource,
) -> Result<String, String> {
    let token = uuid::Uuid::new_v4().simple().to_string();
    state
        .resources
        .lock()
        .map_err(|_| "在线媒体状态不可用".to_string())?
        .insert(token.clone(), resource);
    Ok(token)
}

fn media_response_builder(status: StatusCode) -> tauri::http::response::Builder {
    Response::builder()
        .status(status)
        .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(
            ACCESS_CONTROL_EXPOSE_HEADERS,
            "Content-Range, Content-Length, Accept-Ranges",
        )
        .header(CACHE_CONTROL, "no-store")
}

fn media_error_response(status: StatusCode, message: &str) -> Response<Vec<u8>> {
    media_response_builder(status)
        .header(CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(message.as_bytes().to_vec())
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

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

/// 解析播放器的单段 Range；不支持的格式安全回退到从头读取。
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
    let end = end.trim().parse::<u64>().ok();
    (start, end)
}

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

fn write_online_media_cache(target: &Path, bytes: &[u8]) -> Result<(), String> {
    let temporary = target.with_extension("part");
    fs::write(&temporary, bytes).map_err(|error| format!("无法写入在线媒体缓存：{error}"))?;
    if target.exists() {
        fs::remove_file(target).map_err(|error| format!("无法替换在线媒体缓存：{error}"))?;
    }
    fs::rename(&temporary, target).map_err(|error| format!("无法完成在线媒体缓存写入：{error}"))
}

async fn serve_audio_stream<R: Runtime>(
    app: &tauri::AppHandle<R>,
    source: &str,
    referer: &str,
    method: &Method,
    range_header: Option<&str>,
) -> Response<Vec<u8>> {
    let client = match WebClient::new(app) {
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
    // 部分 CDN 会忽略 Range 并返回完整文件。此时跳过已播放的前缀，再把当前
    // 分块交给播放器，不能直接返回 416，否则播放到首个分块后会中断。
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
            let client = match WebClient::new(app) {
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
    let resource = {
        let state = app.state::<OnlineMediaState>();
        let Ok(resources) = state.resources.lock() else {
            return media_error_response(StatusCode::INTERNAL_SERVER_ERROR, "在线媒体状态不可用");
        };
        resources.get(token).cloned()
    };
    let Some(resource) = resource else {
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

/// 处理 `sfmedia` 协议请求。每个媒体令牌只映射到已校验的 SF 资源。
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

/// 校验上游资源地址，避免原生请求被页面注入到任意主机。
///
/// SF 网页正文偶尔仍返回 `http://*.sfacg.com` 的旧图片地址，但这些资源的 HTTPS
/// 端点可用。校验域名后统一升级为 HTTPS，客户端始终使用返回的规范化地址。
fn validate_sfacg_asset_url(value: &str) -> Result<reqwest::Url, String> {
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

/// 根据响应 MIME 与文件头选择图片扩展名。
fn image_extension(content_type: &str, bytes: &[u8]) -> &'static str {
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

/// 原子写入一个在线缓存文件。
fn write_cache_file(target: &Path, bytes: &[u8], extension: &str) -> Result<(), String> {
    let temporary = target.with_extension(format!("{extension}.part"));
    fs::write(&temporary, bytes).map_err(|error| format!("无法写入在线浏览缓存：{error}"))?;
    if target.exists() {
        fs::remove_file(target).map_err(|error| format!("无法替换在线浏览缓存：{error}"))?;
    }
    fs::rename(&temporary, target).map_err(|error| format!("无法完成在线浏览缓存写入：{error}"))
}

/// 读取一张远程图片并写入在线缓存。
async fn cache_remote_image(
    client: &WebClient,
    url: &str,
    referer: &str,
    target_without_extension: &Path,
) -> Result<String, String> {
    let url = validate_sfacg_asset_url(url)?;
    let response = client
        .asset_request(
            url,
            referer,
            "image/avif,image/webp,image/apng,image/*,*/*;q=0.8",
        )
        .send()
        .await
        .map_err(|error| format!("无法读取在线图片：{error}"))?
        .error_for_status()
        .map_err(|error| format!("在线图片请求被拒绝：{error}"))?;
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("无法读取在线图片数据：{error}"))?;
    if bytes.is_empty() {
        return Err("在线图片内容为空".to_string());
    }
    let extension = image_extension(&content_type, &bytes);
    let target = target_without_extension.with_extension(extension);
    write_cache_file(&target, &bytes, extension)?;
    Ok(target.to_string_lossy().to_string())
}

/// 将正文里的 SF 图片标记替换为在线缓存图片占位符。
///
/// 上游图片地址不会离开原生层，前端只看到 `{{image:n}}` 与本地缓存路径。
async fn cache_text_images(
    client: &WebClient,
    directory: &Path,
    novel_id: i64,
    chapter_id: i64,
    referer: &str,
    content: &str,
) -> Result<(String, Vec<String>), String> {
    fs::create_dir_all(directory).map_err(|error| format!("无法创建正文图片缓存目录：{error}"))?;
    let mut output = String::with_capacity(content.len());
    let mut images = Vec::new();
    let mut cursor = 0;
    while let Some(marker_offset) = content[cursor..].find("[img=]") {
        let marker_start = cursor + marker_offset;
        let url_start = marker_start + "[img=]".len();
        let Some(url_end_offset) = content[url_start..].find("[/img]") else {
            break;
        };
        let url_end = url_start + url_end_offset;
        let url = content[url_start..url_end].trim();
        if url.is_empty() {
            cursor = url_end + "[/img]".len();
            continue;
        }
        output.push_str(&content[cursor..marker_start]);
        let index = images.len();
        let target = directory.join(format!("chapter-{chapter_id}-image-{index}"));
        match cache_remote_image(client, url, referer, &target).await {
            Ok(path) => {
                output.push_str(&format!("{{{{image:{index}}}}}"));
                images.push(path);
            }
            Err(error) => {
                // 单张插图失败不应丢弃已经成功读取的文字正文。
                eprintln!(
                    "[sfacg] online text image skipped: novel_id={novel_id}, chapter_id={chapter_id}, error={error}"
                );
                output.push_str("\n[插图暂时无法加载]\n");
            }
        }
        cursor = url_end + "[/img]".len();
    }
    output.push_str(&content[cursor..]);
    Ok((output, images))
}

/// 按请求策略读取普通章节正文，App 正文不可用时回退网页正文。
async fn load_plain_text_content(
    app: &tauri::AppHandle,
    app_client: &crate::app_client::AppClient,
    web_client: &WebClient,
    novel_id: i64,
    volume_id: i64,
    chapter_id: i64,
) -> Result<String, String> {
    let policy = get_request_policy(app.clone()).unwrap_or_default();
    let app_error = if policy.app_api_preferred_enabled && app_client.has_session() {
        match app_client
            .get_chapter_content_and_metadata(chapter_id)
            .await
        {
            Ok(api) => match decode_api_content(app, &api.content) {
                Ok(content) => return Ok(content),
                Err(error) => Some(error),
            },
            Err(error) => Some(error),
        }
    } else {
        None
    };
    match web_client
        .chapter_content(novel_id, volume_id, chapter_id)
        .await
    {
        Ok(content) => Ok(content),
        Err(web_error) => Err(match app_error {
            Some(app_error) => format!("App 正文不可用：{app_error}；网页正文不可用：{web_error}"),
            None => web_error,
        }),
    }
}

/// 请求 VIP 图片正文并调用本地 OCR。
async fn load_vip_text_content(
    app: &tauri::AppHandle,
    web_client: &WebClient,
    cache_directory: &Path,
    novel_id: i64,
    chapter_id: i64,
) -> Result<String, String> {
    let _ = web_endpoint_client(app, EndpointCapability::TextVipImageWeb)?;
    let image = web_client.vip_chapter_image(novel_id, chapter_id).await?;
    fs::create_dir_all(cache_directory)
        .map_err(|error| format!("无法创建 VIP 正文缓存目录：{error}"))?;
    let source = cache_directory.join(format!("chapter-{chapter_id}.{}", image.extension));
    write_cache_file(&source, &image.bytes, image.extension)?;
    recognize_image(app, source, None).await
}

/// 按需读取一章文字正文。
///
/// # 参数
/// * `app` - Tauri 应用句柄。
/// * `novel_id` - SF 小说编号。
/// * `chapter_id` - 待阅读章节编号。
///
/// # 错误
/// 编号无效、目录不可用、正文未授权或缓存写入失败时返回错误。
#[tauri::command]
pub(crate) async fn load_online_text_chapter(
    app: tauri::AppHandle,
    novel_id: i64,
    chapter_id: i64,
) -> Result<OnlineTextChapter, String> {
    validate_novel_id(novel_id)?;
    validate_novel_id(chapter_id)?;
    #[cfg(target_os = "android")]
    sync_android_auth_session(&app).await?;
    let app_client = app_endpoint_client(&app, EndpointCapability::TextDirectory)?;
    let catalog = app_client.get_chapter_catalog(novel_id).await?;
    let mut selected = None;
    for volume in &catalog {
        if let Some(chapter) = volume
            .chapters
            .iter()
            .find(|chapter| chapter.chap_id == chapter_id)
        {
            selected = Some((volume, chapter));
            break;
        }
    }
    let (volume, chapter) = selected.ok_or_else(|| "该章节不在当前作品目录中".to_string())?;
    let web_client = web_endpoint_client(&app, EndpointCapability::TextChapterWeb);
    let cache_root = online_cache_directory(&app)?;
    let image_directory = cache_root.join("text").join(novel_id.to_string());
    let referer = format!(
        "https://book.sfacg.com/Novel/{novel_id}/{}/{chapter_id}/",
        volume.volume_id
    );

    let mut content = match chapter.content_kind {
        TextContentKind::ImageVip | TextContentKind::EncryptedVip => {
            let web_client = web_client?;
            if !web_client.has_session() {
                return Err("该 VIP 章节需要先完成网站登录".to_string());
            }
            load_vip_text_content(
                &app,
                &web_client,
                &cache_root.join("vip"),
                novel_id,
                chapter_id,
            )
            .await?
        }
        TextContentKind::Text | TextContentKind::Unknown => {
            let web_client = web_client?;
            match load_plain_text_content(
                &app,
                &app_client,
                &web_client,
                novel_id,
                volume.volume_id,
                chapter_id,
            )
            .await
            {
                Ok(content) => content,
                Err(error) if web_client.has_session() => load_vip_text_content(
                    &app,
                    &web_client,
                    &cache_root.join("vip"),
                    novel_id,
                    chapter_id,
                )
                .await
                .map_err(|vip_error| format!("{error}；VIP 图片正文不可用：{vip_error}"))?,
                Err(error) => return Err(error),
            }
        }
    };

    let web_client = web_endpoint_client(&app, EndpointCapability::TextChapterWeb)?;
    let images = if content.contains("[img=]") {
        let (next, images) = cache_text_images(
            &web_client,
            &image_directory,
            novel_id,
            chapter_id,
            &referer,
            &content,
        )
        .await?;
        content = next;
        images
    } else {
        Vec::new()
    };

    Ok(OnlineTextChapter {
        id: chapter.chap_id,
        title: chapter.title.clone(),
        volume: volume.title.clone(),
        content,
        images,
    })
}

/// 为一章漫画准备按页懒加载的原生媒体地址。
///
/// # 参数
/// * `app` - Tauri 应用句柄。
/// * `comic_id` - SF 漫画编号。
/// * `chapter_id` - 漫画章节编号。
/// * `chapter_title` - 用户可见的章节标题，来自已加载目录。
/// * `source_path` - 可选的公开漫画目录标识，用于生成同源 Referer。
///
/// # 错误
/// 编号无效或图片接口拒绝请求时返回错误。
#[tauri::command]
pub(crate) async fn load_online_comic_chapter(
    app: tauri::AppHandle,
    media: State<'_, OnlineMediaState>,
    comic_id: i64,
    chapter_id: i64,
    chapter_title: String,
    source_path: Option<String>,
) -> Result<OnlineComicChapter, String> {
    validate_novel_id(comic_id)?;
    validate_novel_id(chapter_id)?;
    let title = chapter_title.trim();
    if title.is_empty() || title.chars().count() > 300 {
        return Err("漫画章节标题无效".to_string());
    }
    let directory = online_cache_directory(&app)?
        .join("comics")
        .join(comic_id.to_string())
        .join(chapter_id.to_string());
    fs::create_dir_all(&directory).map_err(|error| format!("无法创建漫画在线缓存目录：{error}"))?;
    #[cfg(target_os = "android")]
    sync_android_auth_session(&app).await?;
    let client = web_endpoint_client(&app, EndpointCapability::ComicPages)?;
    let mut folder = source_path.filter(|value| {
        !value.is_empty()
            && value.len() <= 100
            && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
    });
    if folder.is_none() {
        let identity_client = app_endpoint_client(&app, EndpointCapability::ComicIdentity)?;
        let (_, resolved_folder) = identity_client.comic_identity(comic_id).await?;
        folder = Some(resolved_folder);
    }
    let referer = folder
        .as_deref()
        .map(|folder| format!("https://manhua.sfacg.com/mh/{folder}/{chapter_id}/"))
        .unwrap_or_else(|| "https://manhua.sfacg.com/".to_string());
    let image_urls = client
        .comic_chapter_images(comic_id, chapter_id)
        .await
        .map_err(|error| EndpointCapability::ComicPages.unavailable_message(&error))?;
    if image_urls.is_empty() {
        return Err("该漫画章节没有可读取的页面".to_string());
    }
    let mut pages = Vec::with_capacity(image_urls.len());
    for (index, url) in image_urls.iter().enumerate() {
        let source = validate_sfacg_asset_url(url)?;
        let token = register_online_media_resource(
            &media,
            OnlineMediaResource::ComicPage {
                source: source.to_string(),
                referer: referer.clone(),
                cache_path: directory.join(format!("{:03}.image", index + 1)),
            },
        )?;
        pages.push(online_media_url("comic", &token));
    }
    Ok(OnlineComicChapter {
        id: chapter_id,
        title: title.to_string(),
        pages,
    })
}

/// 为在线有声章节准备流式播放地址。
///
/// 播放器通过本地媒体协议按 Range 拉取音频分块，不会等待整首音频下载完成；
/// 受保护的上游地址与 Cookie 均保留在原生层。
#[tauri::command]
pub(crate) async fn load_online_audio_track(
    app: tauri::AppHandle,
    media: State<'_, OnlineMediaState>,
    novel_id: i64,
    chapter_id: i64,
) -> Result<OnlineAudioTrack, String> {
    validate_novel_id(novel_id)?;
    validate_novel_id(chapter_id)?;
    #[cfg(target_os = "android")]
    sync_android_auth_session(&app).await?;
    let client = web_endpoint_client(&app, EndpointCapability::Audio)?;
    let (_title, chapters) = client
        .get_audio_catalog(novel_id)
        .await
        .map_err(|error| EndpointCapability::Audio.unavailable_message(&error))?;
    let chapter = chapters
        .iter()
        .find(|chapter| chapter.id == chapter_id)
        .ok_or_else(|| "该有声章节不在当前目录中".to_string())?;
    let source = validate_sfacg_asset_url(&chapter.source)?;
    let token = register_online_media_resource(
        &media,
        OnlineMediaResource::Audio {
            source: source.to_string(),
            referer: "https://i.sfacg.com/consume/book/".to_string(),
        },
    )?;
    Ok(OnlineAudioTrack {
        id: chapter.id,
        title: chapter.title.clone(),
        volume: chapter.volume.clone(),
        url: online_media_url("audio", &token),
    })
}
