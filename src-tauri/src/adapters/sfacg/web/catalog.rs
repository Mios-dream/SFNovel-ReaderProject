use super::auth::is_safe_account_name;
use crate::domain::error::{AppError, AppResult};
use super::html::{extract_js_field, extract_numeric_attribute, strip_html_text};
use super::WebClient;
use crate::domain::catalog::{PublicShelf, PublicShelfItem};

/// 小说详情页地址前缀，同时用作小说卡片的定位标记。
const NOVEL_URL_PREFIX: &str = "https://book.sfacg.com/Novel/";

/// 漫画详情页地址前缀，同时用作漫画卡片的定位标记。
const COMIC_URL_PREFIX: &str = "https://manhua.sfacg.com/mh/";

impl WebClient {
    /// 读取已登录网页账户所属的公开书架，并获取其中的小说和漫画条目。
    pub(crate) async fn get_public_bookshelf(&self) -> AppResult<PublicShelf> {
        self.require_session()?;
        let name = self.public_shelf_account().await?;
        let account_referer = format!("https://p.sfacg.com/u/{name}/");
        let pocket_html = self
            .fetch_page(
                &format!("https://p.sfacg.com/u/content/{name}-0-1"),
                &account_referer,
                "SF 网页书架",
            )
            .await?;
        let pockets = parse_public_pockets(&pocket_html);
        log_debug!(
            "web bookshelf categories: bytes={}, count={}",
            pocket_html.len(),
            pockets.len()
        );
        let categories = pockets
            .iter()
            .map(|(_, name)| name.clone())
            .collect::<Vec<_>>();

        let mut items = Vec::new();
        for (pocket_id, pocket_name) in pockets {
            let first_url = format!("https://p.sfacg.com/p/{pocket_id}/");
            let Some(first_html) = self
                .pocket_page(&first_url, &account_referer, "SF 公开书架分类")
                .await?
            else {
                continue;
            };
            let pages = parse_public_pocket_page_count(&first_html, pocket_id);
            for page in 1..=pages {
                let html = if page == 1 {
                    first_html.clone()
                } else {
                    let url = format!("https://p.sfacg.com/p/{pocket_id}/{page}/");
                    match self
                        .pocket_page(&url, &first_url, "SF 公开书架分页")
                        .await?
                    {
                        Some(html) => html,
                        None => break,
                    }
                };
                items.extend(
                    parse_public_pocket_items(&html)
                        .into_iter()
                        .map(|item| (pocket_name.clone(), item)),
                );
            }
        }
        Ok(PublicShelf { categories, items })
    }

    /// 读取网页账号名，并确认当前会话仍处于登录状态。
    async fn public_shelf_account(&self) -> AppResult<String> {
        let account = self.get_login_info().await?;
        if extract_js_field(&account, "login").as_deref() != Some("true") {
            return Err(AppError::upstream("网页登录会话已失效"));
        }
        extract_js_field(&account, "name")
            .filter(|value| is_safe_account_name(value))
            .ok_or_else(|| AppError::upstream("SF 网页账号信息未返回有效书架名称"))
    }

    /// 读取一个书架分页。
    ///
    /// 上游对有声、视频书架刻意返回 500，网络异常也不应让整个书架读取失败，
    /// 因此这两类情况返回 `None` 表示跳过。
    async fn pocket_page(
        &self,
        url: &str,
        referer: &str,
        label: &str,
    ) -> AppResult<Option<String>> {
        let response = match self.page_request(url, Some(referer)).send().await {
            Ok(response) if response.status().is_success() => response,
            Ok(response) => {
                log_debug!("{label}跳过：status={} url={url}", response.status());
                return Ok(None);
            }
            Err(error) => {
                log_debug!("{label}跳过：request_error={error} url={url}");
                return Ok(None);
            }
        };
        response
            .text()
            .await
            .map(Some)
            .map_err(|error| AppError::upstream(format!("{label}格式无效：{error}")))
    }

    /// 读取任意网页地址的正文。
    async fn fetch_page(&self, url: &str, referer: &str, label: &str) -> AppResult<String> {
        self.page_request(url, Some(referer))
            .send()
            .await
            .map_err(|error| AppError::upstream(format!("无法读取{label}：{error}")))?
            .error_for_status()
            .map_err(|error| AppError::upstream(format!("{label}请求被拒绝：{error}")))?
            .text()
            .await
            .map_err(|error| AppError::upstream(format!("{label}格式无效：{error}")))
    }
}

/// 从公开书架页面提取书架分类编号和名称。
///
/// 返回按页面顺序去重后的 `(书架编号, 书架名称)` 列表。
fn parse_public_pockets(html: &str) -> Vec<(i64, String)> {
    let mut result = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find("href=\"/p/") {
        rest = &rest[start + 9..];
        let Some(end) = rest.find('/') else { break };
        let Ok(id) = rest[..end].parse::<i64>() else {
            continue;
        };
        let Some(title_start) = rest.find('>') else {
            break;
        };
        let Some(title_end) = rest[title_start + 1..].find("</a>") else {
            break;
        };
        let title = strip_html_text(&rest[title_start + 1..title_start + 1 + title_end]);
        if id > 0 && !title.is_empty() && !result.iter().any(|(known, _)| *known == id) {
            result.push((id, title));
        }
        rest = &rest[title_start + 1 + title_end + 4..];
    }
    result
}

/// 读取指定公开书架的页码上限，并限制在安全请求范围内。
fn parse_public_pocket_page_count(html: &str, pocket_id: i64) -> i32 {
    let prefix = format!("/p/{pocket_id}/");
    let mut max_page = 1;
    let mut rest = html;
    while let Some(start) = rest.find(&prefix) {
        rest = &rest[start + prefix.len()..];
        let digits = rest
            .chars()
            .take_while(|character| character.is_ascii_digit())
            .collect::<String>();
        if let Ok(page) = digits.parse::<i32>() {
            max_page = max_page.max(page);
        }
    }
    max_page.min(100)
}

/// 卡片里可用于展示的字段。
struct CardFields {
    title: String,
    author: String,
    cover: String,
}

/// 从公开书架页面 HTML 解析小说条目。
fn parse_public_pocket_items(html: &str) -> Vec<PublicShelfItem> {
    let mut result = Vec::new();
    let mut cursor = 0;
    while let Some(offset) = html[cursor..].find(NOVEL_URL_PREFIX) {
        let url_start = cursor + offset;
        cursor = url_start + NOVEL_URL_PREFIX.len();
        let digits = html[cursor..]
            .chars()
            .take_while(|character| character.is_ascii_digit())
            .collect::<String>();
        let Ok(id) = digits.parse::<i64>() else {
            continue;
        };
        // 小说卡片是 `<li>` 包 `<li>` 的两层结构，取到第二个闭合标签为止。
        let Some(fields) = card_fields(novel_card(html, url_start)) else {
            continue;
        };
        if result
            .iter()
            .any(|item: &PublicShelfItem| item.kind == "novel" && item.id == id)
        {
            continue;
        }
        result.push(PublicShelfItem {
            kind: "novel",
            id,
            media_id: None,
            title: fields.title,
            author: fields.author,
            cover: fields.cover,
            source_path: None,
        });
    }
    let comics = parse_public_comic_items(html);
    log_debug!(
        "web bookshelf page: bytes={}, novels={}, comics={}",
        html.len(),
        result.len(),
        comics.len()
    );
    result.extend(comics);
    result
}

/// 截取小说条目卡片：从最近的 `<li` 起算，跨过两层 `</li>`。
fn novel_card(html: &str, url_start: usize) -> &str {
    let start = html[..url_start].rfind("<li").unwrap_or(url_start);
    let mut end = start;
    for _ in 0..2 {
        let Some(offset) = html[end..].find("</li>") else {
            return &html[start..];
        };
        end += offset + "</li>".len();
    }
    &html[start..end]
}

/// 从条目详情块的 `eid` 所有权标记解析漫画。
///
/// 与封面地址不同，此标记携带后续目录和下载操作必需的数字漫画编号。
fn parse_public_comic_items(html: &str) -> Vec<PublicShelfItem> {
    let mut result = Vec::new();
    let mut cursor = 0;
    while let Some(offset) = html[cursor..].find(COMIC_URL_PREFIX) {
        let url_start = cursor + offset;
        cursor = url_start + COMIC_URL_PREFIX.len();
        let Some(folder_end) = html[cursor..].find('/') else {
            continue;
        };
        let folder = html[cursor..cursor + folder_end].trim();
        // 漫画卡片使用外层 `<ul ... eid="...">`，封面和元数据分别放在其中多个 `<li>` 里。
        let Some(start) = html[..url_start].rfind("<ul") else {
            continue;
        };
        let Some(end) = html[url_start..].find("</ul>") else {
            continue;
        };
        let card = &html[start..url_start + end + "</ul>".len()];
        let Some(id) = extract_numeric_attribute(card, "eid") else {
            continue;
        };
        if id <= 0 || !is_comic_folder(folder) {
            continue;
        }
        let Some(fields) = card_fields(card) else {
            continue;
        };
        if result.iter().any(|item: &PublicShelfItem| item.id == id) {
            continue;
        }
        result.push(PublicShelfItem {
            kind: "comic",
            id,
            media_id: None,
            title: fields.title,
            author: fields.author,
            cover: fields.cover,
            source_path: Some(folder.to_string()),
        });
    }
    result
}

/// 判断字符串是否可作为安全的漫画目录标识。
fn is_comic_folder(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

/// 从卡片中提取标题、作者与封面。标题缺失视为无效卡片。
fn card_fields(card: &str) -> Option<CardFields> {
    Some(CardFields {
        title: anchor_text(card, "<b><a")?,
        author: card
            .find("作者：")
            .map(|position| strip_html_text(&card[position + "作者：".len()..]))
            .unwrap_or_else(|| "未知作者".to_string()),
        cover: attribute_value(card, "src=\"").unwrap_or_default(),
    })
}

/// 取指定标签之后、`</a>` 之前的纯文本。
fn anchor_text(card: &str, tag: &str) -> Option<String> {
    let start = card.find(tag)?;
    let text_start = start + card[start..].find('>')? + 1;
    let length = card[text_start..].find("</a>")?;
    let text = strip_html_text(&card[text_start..text_start + length]);
    (!text.is_empty()).then_some(text)
}

/// 取 `marker` 之后、下一个双引号之前的属性值。
fn attribute_value(card: &str, marker: &str) -> Option<String> {
    let start = card.find(marker)? + marker.len();
    let length = card[start..].find('"')?;
    Some(card[start..start + length].to_string())
}

#[cfg(test)]
mod tests {
    use super::{card_fields, parse_public_pocket_items};

    #[test]
    fn extracts_card_fields() {
        let card = r#"<li><b><a href="/Novel/123/">标题甲</a></b>作者：甲作者<img src="//i.sfacg.com/a.jpg"></li>"#;
        let fields = card_fields(card).expect("应能解析卡片");
        assert_eq!(fields.title, "标题甲");
        assert_eq!(fields.author, "甲作者");
        assert_eq!(fields.cover, "//i.sfacg.com/a.jpg");
    }

    #[test]
    fn skips_cards_without_title() {
        assert!(card_fields("<li>作者：无题</li>").is_none());
    }

    #[test]
    fn parses_novel_items_from_shelf_page() {
        let html = r#"<ul><li><li><b><a href="https://book.sfacg.com/Novel/9527/">作品</a></b>作者：作者名<img src="https://i.sfacg.com/c.jpg"></li></li></ul>"#;
        let items = parse_public_pocket_items(html);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, 9527);
        assert_eq!(items[0].kind, "novel");
        assert_eq!(items[0].title, "作品");
    }
}
