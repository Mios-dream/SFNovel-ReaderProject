//! 下载流程共用的封面、详情和二进制资源写入。

use crate::adapters::sfacg::source::SfSource;
use crate::adapters::storage::write_bytes_atomically;
use crate::domain::catalog::NovelDetail;
use crate::domain::library::WorkMetadata;
use std::path::Path;

/// 将远程详情中非空且更新的字段合并到持久化元数据。
///
/// # 参数
/// - `metadata`：即将写入书籍清单的元数据。
/// - `detail`：上游详情响应。
pub(super) fn apply_detail_metadata(metadata: &mut WorkMetadata, detail: &NovelDetail) {
    if !detail.novel_name.trim().is_empty() {
        metadata.title = Some(detail.novel_name.clone());
    }
    if !detail.author_name.trim().is_empty() {
        metadata.author = Some(detail.author_name.clone());
    }
    if !detail.description.trim().is_empty() {
        metadata.description = Some(detail.description.clone());
    }
    metadata.type_name = detail
        .type_name
        .clone()
        .or_else(|| metadata.type_name.clone());
    if let Some(tags) = detail.tags.as_ref().filter(|tags| !tags.is_empty()) {
        metadata.tags = tags.clone();
    }
    metadata.is_finished = Some(detail.is_finish);
    metadata.score = detail.score.or(metadata.score);
    metadata.chapter_count = detail.chapter_count.or(metadata.chapter_count);
    metadata.character_count = detail.character_count.or(metadata.character_count);
    metadata.view_count = detail.view_count.or(metadata.view_count);
    metadata.mark_count = detail.mark_count.or(metadata.mark_count);
    metadata.point_count = detail.point_count.or(metadata.point_count);
    metadata.favorite_count = detail.favorite_count.or(metadata.favorite_count);
    metadata.ticket_count = detail.ticket_count.or(metadata.ticket_count);
    metadata.allow_download = detail.allow_download.or(metadata.allow_download);
    metadata.latest_chapter_title = detail
        .latest_chapter_title
        .clone()
        .or_else(|| metadata.latest_chapter_title.clone());
    metadata.latest_chapter_time = detail
        .latest_chapter_time
        .clone()
        .or_else(|| metadata.latest_chapter_time.clone());
    if !detail.last_update_time.trim().is_empty() {
        metadata.last_update_time = Some(detail.last_update_time.clone());
    }
}

/// 把相对资源地址转换为基于指定站点的绝对地址。
///
/// # 参数
/// - `value`：上游返回的绝对或相对地址。
/// - `base`：相对地址使用的站点根地址。
fn absolute_asset_url(value: &str, base: &str) -> String {
    let value = value.trim();
    if value.starts_with("http://") || value.starts_with("https://") {
        value.to_string()
    } else {
        format!(
            "{}/{}",
            base.trim_end_matches('/'),
            value.trim_start_matches('/')
        )
    }
}

/// 使用已认证请求下载二进制资源。
///
/// # 参数
/// - `source`：SF 资源客户端。
/// - `url`：资源地址。
/// - `referer`：请求所需 Referer。
/// - `accept`：期望的 MIME 类型。
/// - `label`：错误消息中的资源名称。
///
/// # 错误
/// 请求、HTTP 状态或响应体读取失败时返回带资源名称的错误。
pub(super) async fn fetch_asset(
    source: &SfSource,
    url: &str,
    referer: &str,
    accept: &str,
    label: &str,
) -> Result<Vec<u8>, String> {
    source
        .asset_request(url, referer, accept)
        .send()
        .await
        .map_err(|error| format!("无法下载{label}：{error}"))?
        .error_for_status()
        .map_err(|error| format!("{label}下载被拒绝：{error}"))?
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|error| format!("无法读取{label}：{error}"))
}

/// 下载并原子写入封面；文件已存在时跳过。
///
/// # 参数
/// - `source`：SF 资源客户端。
/// - `directory`：书籍根目录。
/// - `file_name`：封面文件名。
/// - `cover_url`：上游封面地址。
/// - `base_url`：相对封面地址的站点根地址。
/// - `referer`：请求封面所需 Referer。
/// - `label`：错误消息中的资源名称。
///
/// # 错误
/// 封面下载或写入失败时返回错误消息。
async fn save_cover(
    source: &SfSource,
    directory: &Path,
    file_name: &str,
    cover_url: &str,
    base_url: &str,
    referer: &str,
    label: &str,
) -> Result<(), String> {
    if cover_url.trim().is_empty() {
        return Ok(());
    }
    let target = directory.join("imgs").join(file_name);
    if target.is_file() {
        return Ok(());
    }
    let url = absolute_asset_url(cover_url, base_url);
    let payload = fetch_asset(
        source,
        &url,
        referer,
        "image/avif,image/webp,image/*,*/*;q=0.8",
        label,
    )
    .await?;
    write_bytes_atomically(&target, &payload, label)
}

/// 拉取小说详情并保存元数据与封面。
///
/// # 参数
/// - `source`：SF 内容源。
/// - `directory`：书籍目录。
/// - `metadata`：待补全的文字元数据。
/// - `novel_id`：小说编号。
///
/// # 错误
/// 详情请求或封面写入失败时返回错误消息。
pub(super) async fn enrich_novel_metadata(
    source: &SfSource,
    directory: &Path,
    metadata: &mut WorkMetadata,
    novel_id: i64,
) -> Result<(), String> {
    let detail = source.novel_detail(novel_id).await?;
    metadata.id = Some(novel_id);
    apply_detail_metadata(metadata, &detail);
    save_cover(
        source,
        directory,
        "novel-cover.jpeg",
        &detail.novel_cover,
        "https://book.sfacg.com",
        &format!("https://book.sfacg.com/Novel/{novel_id}/"),
        "小说封面",
    )
    .await
}

/// 拉取有声详情并保存元数据与封面。
///
/// # 参数
/// - `source`：SF 内容源。
/// - `directory`：书籍目录。
/// - `metadata`：待补全的有声元数据。
/// - `novel_id`：小说编号。
/// - `album_id`：有声专辑编号。
///
/// # 错误
/// 详情请求或封面写入失败时返回错误消息。
pub(super) async fn enrich_audio_metadata(
    source: &SfSource,
    directory: &Path,
    metadata: &mut WorkMetadata,
    novel_id: i64,
    album_id: i64,
) -> Result<(), String> {
    let detail = source.audio_detail(album_id, novel_id).await?;
    metadata.id = Some(album_id);
    metadata.catalog_id = Some(novel_id);
    apply_detail_metadata(metadata, &detail);
    let cover_url = if detail.novel_cover.trim().is_empty() {
        source
            .novel_detail(novel_id)
            .await
            .map(|novel| novel.novel_cover)
            .unwrap_or_default()
    } else {
        detail.novel_cover
    };
    save_cover(
        source,
        directory,
        "audio-cover.jpeg",
        &cover_url,
        "https://i.sfacg.com",
        "https://i.sfacg.com/consume/book/",
        "有声封面",
    )
    .await
}

/// 拉取漫画详情并保存元数据与封面。
///
/// # 参数
/// - `source`：SF 内容源。
/// - `directory`：书籍目录。
/// - `metadata`：待补全的漫画元数据。
/// - `comic_id`：漫画编号。
///
/// # 错误
/// 详情请求或封面写入失败时返回错误消息。
pub(super) async fn enrich_comic_metadata(
    source: &SfSource,
    directory: &Path,
    metadata: &mut WorkMetadata,
    comic_id: i64,
) -> Result<(), String> {
    let detail = source.comic_detail(comic_id).await?;
    metadata.id = Some(comic_id);
    metadata.online_path = detail
        .source_path
        .clone()
        .or_else(|| metadata.online_path.clone());
    apply_detail_metadata(metadata, &detail);
    save_cover(
        source,
        directory,
        "comic-cover.jpeg",
        &detail.novel_cover,
        "https://manhua.sfacg.com",
        "https://manhua.sfacg.com/",
        "漫画封面",
    )
    .await
}
