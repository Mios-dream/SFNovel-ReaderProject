use super::html::strip_html_text;
use super::WebClient;
use crate::domain::catalog::AudioChapter;
use crate::domain::ids::validate_novel_id;

impl WebClient {
    /// 从手机版有声章节页读取播放器加载的音频地址。
    ///
    /// # 参数
    /// - `chapter_id`：有声章节编号。
    ///
    /// # 返回
    /// 返回可在网页会话下播放的 HTTPS 音频地址。
    ///
    /// # 错误
    /// 编号无效、网页会话缺失、章节不可访问或页面不含播放器地址时返回错误。
    pub(crate) async fn get_audio_chapter_source(&self, chapter_id: i64) -> Result<String, String> {
        validate_novel_id(chapter_id)?;
        self.require_session()?;
        let chapter_url = format!("https://m.sfacg.com/a/{chapter_id}/");
        let response = self
            .page_request(&chapter_url, Some("https://m.sfacg.com/"))
            .send()
            .await
            .map_err(|error| format!("无法连接 SF 有声章节：{error}"))?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED
            || response.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err("SF Web 登录会话已失效，请重新登录".to_string());
        }
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err("该有声章节不存在或已下架".to_string());
        }
        let html = response
            .error_for_status()
            .map_err(|error| format!("SF 有声章节请求被拒绝：{error}"))?
            .text()
            .await
            .map_err(|error| format!("SF 有声章节格式无效：{error}"))?;
        parse_mobile_audio_source(&html)
    }

    /// 从手机版有声目录页读取完整章节列表。
    ///
    /// 该页面按分卷分组，是 App 目录接口被拦截时唯一可用的完整目录来源。
    /// 页面不提供章节的销售标记，因此返回的章节都不带 VIP 与解锁判定；
    /// 具体章节能否播放，在取音频地址时才能确定。
    ///
    /// # 错误
    /// 编号无效、页面不可访问或页面没有可解析章节时返回错误。
    pub(crate) async fn get_audio_catalog(
        &self,
        album_id: i64,
        title_hint: Option<&str>,
    ) -> Result<(String, Vec<AudioChapter>), String> {
        validate_novel_id(album_id)?;
        let html = self
            .page_request(
                format!("https://m.sfacg.com/ai/{album_id}/"),
                Some("https://m.sfacg.com/"),
            )
            .send()
            .await
            .map_err(|error| format!("无法读取 SF 有声目录：{error}"))?
            .error_for_status()
            .map_err(|error| format!("SF 有声目录请求被拒绝：{error}"))?
            .text()
            .await
            .map_err(|error| format!("SF 有声目录格式无效：{error}"))?;
        parse_mobile_audio_catalog(&html, title_hint)
    }
}

/// 解析手机版有声目录页中的分卷和章节。
///
/// # 参数
/// - `html`：有声目录页面 HTML。
/// - `title_hint`：页面标题不可用时的作品标题回退值。
///
/// # 返回
/// 返回作品标题和按页面顺序去重后的章节列表。
///
/// # 错误
/// 页面中没有可解析章节时返回错误。
fn parse_mobile_audio_catalog(
    html: &str,
    title_hint: Option<&str>,
) -> Result<(String, Vec<AudioChapter>), String> {
    const VOLUME_MARKER: &str = "<div class=\"mulu\">";
    const LIST_MARKER: &str = "<ul class=\"mulu_list\">";
    let mut chapters = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut rest = html;
    while let Some(position) = rest.find(VOLUME_MARKER) {
        rest = &rest[position + VOLUME_MARKER.len()..];
        let Some(end) = rest.find("</div>") else {
            break;
        };
        let volume = strip_html_text(&rest[..end]);
        rest = &rest[end..];

        let Some(list_start) = rest.find(LIST_MARKER) else {
            continue;
        };
        let after_open = &rest[list_start + LIST_MARKER.len()..];
        let Some(list_end) = after_open.find("</ul>") else {
            break;
        };
        let mut items = &after_open[..list_end];
        rest = &after_open[list_end..];

        while let Some(anchor) = items.find("href=\"/a/") {
            items = &items[anchor + "href=\"/a/".len()..];
            let Some(id_end) = items.find('/') else {
                break;
            };
            let Ok(id) = items[..id_end].parse::<i64>() else {
                continue;
            };
            let Some(tag_end) = items.find("</a>") else {
                break;
            };
            let anchor_html = &items[..tag_end];
            items = &items[tag_end..];
            if !seen.insert(id) {
                continue;
            }
            let title = anchor_html
                .find('>')
                .map(|start| strip_html_text(&anchor_html[start + 1..]))
                .filter(|title| !title.is_empty())
                .unwrap_or_else(|| format!("章节 {id}"));
            chapters.push(AudioChapter {
                id,
                title,
                volume: volume.clone(),
                is_vip: false,
                is_unlocked: true,
            });
        }
    }
    if chapters.is_empty() {
        return Err("SF 有声目录没有返回章节".to_string());
    }
    let title = html
        .find("<title>")
        .and_then(|start| {
            let after = &html[start + "<title>".len()..];
            after.find("</title>").map(|end| &after[..end])
        })
        .map(|raw| {
            raw.split('|')
                .next()
                .unwrap_or(raw)
                .trim()
                .trim_end_matches("音频列表")
                .trim()
                .to_string()
        })
        .filter(|title| !title.is_empty())
        .or_else(|| {
            title_hint
                .map(|hint| hint.trim().to_string())
                .filter(|hint| !hint.is_empty())
        })
        .unwrap_or_else(|| "未命名有声作品".to_string());
    Ok((title, chapters))
}

/// 从手机版有声章节页读取播放器加载的 MP3 地址。
fn parse_mobile_audio_source(html: &str) -> Result<String, String> {
    let marker = "audioPlayer.loadAudio";
    let marker_start = html
        .find(marker)
        .ok_or_else(|| "该有声章节没有可播放的音频，可能需要登录或购买".to_string())?;
    let rest = html[marker_start + marker.len()..].trim_start();
    let rest = rest
        .strip_prefix('(')
        .ok_or_else(|| "SF 有声章节播放器格式无效".to_string())?
        .trim_start();
    let quote = rest
        .chars()
        .next()
        .filter(|character| *character == '"' || *character == '\'')
        .ok_or_else(|| "SF 有声章节播放器格式无效".to_string())?;
    let value = &rest[quote.len_utf8()..];
    let end = value
        .find(quote)
        .ok_or_else(|| "SF 有声章节播放器格式无效".to_string())?;
    let mut source = value[..end].trim().replace("\\/", "/");
    if let Some(relative) = source.strip_prefix("//") {
        source = format!("https://{relative}");
    }
    if !source.starts_with("https://") && !source.starts_with("http://") {
        return Err("SF 有声章节没有返回有效音频地址".to_string());
    }
    Ok(source)
}
