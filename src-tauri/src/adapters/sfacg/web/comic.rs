use super::html::strip_html_text;
use super::WebClient;
use crate::domain::catalog::{ComicChapter, ComicUnlockState};
use serde_json::Value;

impl WebClient {
    /// 读取漫画网页目录。
    ///
    /// # 参数
    /// - `folder`：由 App 漫画详情取得的网页目录标识。
    ///
    /// # 返回
    /// 返回按阅读顺序排列并去重后的章节。
    ///
    /// # 错误
    /// 页面不可访问或目录中没有可解析章节时返回错误。
    pub(crate) async fn get_comic_catalog(
        &self,
        folder: &str,
    ) -> Result<Vec<ComicChapter>, String> {
        let html = self
            .page_request(format!("https://manhua.sfacg.com/mh/{folder}/"), None)
            .send()
            .await
            .map_err(|error| format!("无法读取 SF 漫画目录：{error}"))?
            .error_for_status()
            .map_err(|error| format!("SF 漫画目录请求被拒绝：{error}"))?
            .text()
            .await
            .map_err(|error| format!("SF 漫画目录格式无效：{error}"))?;
        parse_comic_catalog(&html, folder)
    }

    /// 通过 SF 漫画图片 AJAX 接口读取一个章节的图片地址。
    ///
    /// 图片地址仅供原生下载工作线程使用，不会直接返回渲染进程。
    ///
    /// # 错误
    /// 章节编号无效或图片接口拒绝请求时返回错误。
    pub(crate) async fn comic_chapter_images(
        &self,
        comic_id: i64,
        chapter_id: i64,
    ) -> Result<Vec<String>, String> {
        if comic_id <= 0 || chapter_id <= 0 {
            return Err("漫画或章节编号无效".to_string());
        }
        let payload = self
            .ajax_request(
                "https://manhua.sfacg.com/ajax/Common.ashx",
                "https://manhua.sfacg.com/",
            )
            .query(&[
                ("op", "getPics"),
                ("cid", &comic_id.to_string()),
                ("chapId", &chapter_id.to_string()),
            ])
            .send()
            .await
            .map_err(|error| format!("无法读取 SF 漫画图片接口：{error}"))?
            .error_for_status()
            .map_err(|error| format!("SF 漫画图片接口被拒绝：{error}"))?
            .json::<Value>()
            .await
            .map_err(|error| format!("SF 漫画图片接口格式无效：{error}"))?;
        if payload.get("status").and_then(Value::as_i64) != Some(200) {
            return Err(payload
                .get("msg")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("该漫画章节当前不可下载")
                .to_string());
        }
        let images = payload
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| "SF 漫画图片接口未返回图片".to_string())?
            .iter()
            .filter_map(Value::as_str)
            .filter(|url| url.starts_with("https://"))
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        if images.is_empty() {
            return Err("该漫画章节没有可下载图片".to_string());
        }
        Ok(images)
    }
}

/// 从漫画目录页解析章节，并将页面倒序恢复为阅读顺序。
///
/// # 参数
/// - `html`：漫画目录页 HTML。
/// - `folder`：当前漫画的网页目录标识。
///
/// # 返回
/// 返回去重后的漫画章节列表。
///
/// # 错误
/// 页面没有可解析章节时返回错误。
fn parse_comic_catalog(html: &str, folder: &str) -> Result<Vec<ComicChapter>, String> {
    let marker = format!("href=\"/mh/{folder}/");
    let mut remaining = html;
    let mut chapters = Vec::new();
    let mut seen = std::collections::HashSet::new();
    while let Some(position) = remaining.find(&marker) {
        remaining = &remaining[position + marker.len()..];
        let Some(id_end) = remaining.find('/') else {
            break;
        };
        let Ok(id) = remaining[..id_end].parse::<i64>() else {
            continue;
        };
        let Some(anchor_end) = remaining[id_end..].find("</a>") else {
            continue;
        };
        let anchor = &remaining[id_end..id_end + anchor_end];
        let Some(title_start) = anchor.find('>') else {
            continue;
        };
        let title = strip_html_text(anchor[title_start + 1..].trim());
        if title.is_empty() || title == "点击浏览" || !seen.insert(id) {
            continue;
        }
        let is_vip = title.starts_with("VIP");
        chapters.push(ComicChapter {
            id,
            title,
            is_vip,
            is_unlocked: if is_vip {
                ComicUnlockState::Unknown
            } else {
                ComicUnlockState::Unlocked
            },
        });
    }
    if chapters.is_empty() {
        return Err("SF 漫画目录格式无效".to_string());
    }
    chapters.reverse();
    Ok(chapters)
}
