use super::auth::is_safe_account_name;
use super::html::{extract_js_field, extract_numeric_attribute, strip_html_text};
use super::WebClient;
use crate::domain::catalog::{PublicShelf, PublicShelfItem};

impl WebClient {
    /// 读取已登录网页账户所属的公开书架，并获取其中的小说和漫画条目。
    pub(crate) async fn get_public_bookshelf(&self) -> Result<PublicShelf, String> {
        self.require_session()?;
        let account = self.get_login_info().await?;
        if extract_js_field(&account, "login").as_deref() != Some("true") {
            return Err("网页登录会话已失效".to_string());
        }
        let name = extract_js_field(&account, "name")
            .filter(|value| is_safe_account_name(value))
            .ok_or_else(|| "SF 网页账号信息未返回有效书架名称".to_string())?;
        let pocket_html = self
            .page_request(
                format!("https://p.sfacg.com/u/content/{name}-0-1"),
                Some(&format!("https://p.sfacg.com/u/{name}/")),
            )
            .send()
            .await
            .map_err(|error| format!("无法读取 SF 网页书架：{error}"))?
            .error_for_status()
            .map_err(|error| format!("SF 网页书架请求被拒绝：{error}"))?
            .text()
            .await
            .map_err(|error| format!("SF 网页书架格式无效：{error}"))?;
        let pockets = parse_public_pockets(&pocket_html);
        eprintln!(
            "[sfacg] web bookshelf categories discovered: html_bytes={}, count={}",
            pocket_html.len(),
            pockets.len(),
        );
        let categories = pockets
            .iter()
            .map(|(_, name)| name.clone())
            .collect::<Vec<_>>();
        let mut result = Vec::new();
        for (pocket_id, pocket_name) in pockets {
            let first_url = format!("https://p.sfacg.com/p/{pocket_id}/");
            let first_response = match self
                .page_request(&first_url, Some(&format!("https://p.sfacg.com/u/{name}/")))
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    eprintln!(
                        "[sfacg] web bookshelf pocket requested: status={}, pocket_id={}",
                        response.status(),
                        pocket_id
                    );
                    response
                }
                // 网页端会刻意对有声、视频书架返回 500。后续会从小说详情按需探测，不能
                // 因此使整个小说、漫画书架读取失败。
                Ok(response) => {
                    eprintln!(
                        "[sfacg] web bookshelf pocket skipped: status={}, pocket_id={}",
                        response.status(),
                        pocket_id
                    );
                    continue;
                }
                Err(error) => {
                    eprintln!(
                        "[sfacg] web bookshelf pocket skipped: request_error={}, pocket_id={}",
                        error, pocket_id
                    );
                    continue;
                }
            };
            let first_html = first_response
                .text()
                .await
                .map_err(|error| format!("SF 公开书架分类格式无效：{error}"))?;
            let max_page = parse_public_pocket_page_count(&first_html, pocket_id);
            eprintln!(
                "[sfacg] web bookshelf pocket loaded: pocket_id={}, html_bytes={}, pages={}",
                pocket_id,
                first_html.len(),
                max_page,
            );
            for page in 1..=max_page {
                let html = if page == 1 {
                    first_html.clone()
                } else {
                    let response = match self
                        .page_request(
                            format!("https://p.sfacg.com/p/{pocket_id}/{page}/"),
                            Some(&first_url),
                        )
                        .send()
                        .await
                    {
                        Ok(response) if response.status().is_success() => response,
                        Ok(response) => {
                            eprintln!(
                                "[sfacg] web bookshelf page skipped: status={}, pocket_id={}, page={}",
                                response.status(),
                                pocket_id,
                                page
                            );
                            break;
                        }
                        Err(error) => {
                            eprintln!(
                                "[sfacg] web bookshelf page skipped: request_error={}, pocket_id={}, page={}",
                                error,
                                pocket_id,
                                page
                            );
                            break;
                        }
                    };
                    response
                        .text()
                        .await
                        .map_err(|error| format!("SF 公开书架分页格式无效：{error}"))?
                };
                result.extend(
                    parse_public_pocket_items(&html)
                        .into_iter()
                        .map(|item| (pocket_name.clone(), item)),
                );
            }
        }
        eprintln!(
            "[sfacg] web bookshelf parsed: novels={}, comics={}",
            result
                .iter()
                .filter(|(_, item)| item.kind == "novel")
                .count(),
            result
                .iter()
                .filter(|(_, item)| item.kind == "comic")
                .count(),
        );
        Ok(PublicShelf {
            categories,
            items: result,
        })
    }
}

/// 从公开书架页面提取书架分类编号和名称。
///
/// # 参数
/// - `html`：公开书架账户页 HTML。
///
/// # 返回
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

/// 从公开书架页面 HTML 解析小说和漫画条目。
///
/// 仅提取后续目录、详情和下载所需的受限字段，不执行或信任来源标记。
fn parse_public_pocket_items(html: &str) -> Vec<PublicShelfItem> {
    let mut result = Vec::new();
    let mut novel_url_count = 0;
    for (kind, marker, prefix) in [(
        "novel",
        "https://book.sfacg.com/Novel/",
        "https://book.sfacg.com/Novel/",
    )] {
        let mut cursor = 0;
        while let Some(relative_start) = html[cursor..].find(marker) {
            let absolute_start = cursor + relative_start;
            novel_url_count += 1;
            let rest_start = absolute_start + prefix.len();
            cursor = rest_start;
            let rest = &html[rest_start..];
            let digits = rest
                .chars()
                .take_while(|character| character.is_ascii_digit())
                .collect::<String>();
            let Ok(id) = digits.parse::<i64>() else {
                continue;
            };
            let card_start = html[..absolute_start]
                .rfind("<li")
                .unwrap_or(absolute_start);
            let mut card_end = card_start;
            for _ in 0..2 {
                let Some(relative_end) = html[card_end..].find("</li>") else {
                    card_end = html.len();
                    break;
                };
                card_end += relative_end + 5;
            }
            let card = &html[card_start..card_end];
            let title = card
                .find("<b><a")
                .and_then(|position| {
                    card[position..]
                        .find('>')
                        .map(|offset| position + offset + 1)
                })
                .and_then(|position| {
                    card[position..]
                        .find("</a>")
                        .map(|offset| strip_html_text(&card[position..position + offset]))
                })
                .unwrap_or_default();
            if title.is_empty() {
                continue;
            }
            let author = card
                .find("作者：")
                .map(|position| strip_html_text(&card[position + 9..]))
                .unwrap_or_else(|| "未知作者".to_string());
            let cover = card
                .find("src=\"")
                .and_then(|position| {
                    let start = position + 5;
                    card[start..]
                        .find('"')
                        .map(|end| card[start..start + end].to_string())
                })
                .unwrap_or_default();
            if !result
                .iter()
                .any(|item: &PublicShelfItem| item.kind == kind && item.id == id)
            {
                result.push(PublicShelfItem {
                    kind,
                    id,
                    title,
                    author,
                    cover,
                    source_path: None,
                });
            }
            cursor = rest_start;
        }
    }
    let comic_items = parse_public_comic_items(html);
    eprintln!(
        "[sfacg] web bookshelf page parsed: bytes={}, novel_urls={}, novel_items={}, comic_items={}",
        html.len(),
        novel_url_count,
        result.len(),
        comic_items.len(),
    );
    result.extend(comic_items);
    result
}

/// 从条目详情块的 `eid` 所有权标记解析漫画。
///
/// 与封面地址不同，此标记携带后续目录和下载操作必需的数字漫画编号。
fn parse_public_comic_items(html: &str) -> Vec<PublicShelfItem> {
    let mut result = Vec::new();
    let mut cursor = 0;
    let marker = "https://manhua.sfacg.com/mh/";
    let mut url_count = 0;
    let mut container_count = 0;
    let mut eid_count = 0;
    let mut title_count = 0;
    while let Some(relative_url) = html[cursor..].find(marker) {
        let url_start = cursor + relative_url;
        cursor = url_start + marker.len();
        url_count += 1;
        let Some(folder_end) = html[cursor..].find('/') else {
            continue;
        };
        let folder = html[cursor..cursor + folder_end].trim();
        // 漫画卡片使用外层 `<ul ... eid="...">`，并将封面和元数据分别放在容器内
        // 的多个 `<li>` 元素中。
        let Some(container_start) = html[..url_start].rfind("<ul") else {
            continue;
        };
        let Some(relative_end) = html[url_start..].find("</ul>") else {
            continue;
        };
        let container_end = url_start + relative_end + 5;
        let card = &html[container_start..container_end];
        container_count += 1;
        let Some(id) = extract_numeric_attribute(card, "eid") else {
            continue;
        };
        eid_count += 1;
        if id <= 0
            || folder.is_empty()
            || folder.len() > 100
            || !folder.bytes().all(|byte| byte.is_ascii_alphanumeric())
        {
            continue;
        }
        let title = card
            .find("<b><a")
            .and_then(|position| {
                card[position..]
                    .find('>')
                    .map(|offset| position + offset + 1)
            })
            .and_then(|position| {
                card[position..]
                    .find("</a>")
                    .map(|offset| strip_html_text(&card[position..position + offset]))
            })
            .unwrap_or_default();
        if title.is_empty() {
            continue;
        }
        title_count += 1;
        let author = card
            .find("作者：")
            .map(|position| strip_html_text(&card[position + 9..]))
            .unwrap_or_else(|| "未知作者".to_string());
        let cover = card
            .find("src=\"")
            .and_then(|position| {
                let start = position + 5;
                card[start..]
                    .find('"')
                    .map(|end| card[start..start + end].to_string())
            })
            .unwrap_or_default();
        if !result.iter().any(|item: &PublicShelfItem| item.id == id) {
            result.push(PublicShelfItem {
                kind: "comic",
                id,
                title,
                author,
                cover,
                source_path: Some(folder.to_string()),
            });
        }
    }
    eprintln!(
        "[sfacg] web bookshelf comic parse: urls={}, containers={}, eid={}, titles={}, valid={}",
        url_count,
        container_count,
        eid_count,
        title_count,
        result.len(),
    );
    result
}
