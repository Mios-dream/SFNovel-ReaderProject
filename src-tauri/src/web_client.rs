//! SF 网页客户端及其 Web 会话边界。

use crate::sfacg::{
    extract_js_number, extract_js_string, parse_comic_catalog, validate_novel_id, AuthSessionState,
    NativeAudioChapter, NativeComicChapter, NovelDetail, SF_WEB_USER_AGENT,
};
use serde_json::Value;
use tauri::Manager;

/// One novel or comic parsed from a public web pocket.
pub(crate) struct PublicShelfItem {
    pub(crate) kind: &'static str,
    pub(crate) id: i64,
    pub(crate) title: String,
    pub(crate) author: String,
    pub(crate) cover: String,
    pub(crate) source_path: Option<String>,
}

/// Public shelf categories and the successfully parsed novel/comic entries.
pub(crate) struct PublicShelf {
    pub(crate) categories: Vec<String>,
    pub(crate) items: Vec<(String, PublicShelfItem)>,
}

/// 使用来自 `NativeAuthSession.web_cookie` 的已过滤 Web 会话快照，发送 SF
/// 网页、AJAX 和静态资源请求。
pub(crate) struct WebClient {
    client: reqwest::Client,
    web_cookie: Option<String>,
}

impl WebClient {
    /// 仅使用原生 Web 会话状态创建网页客户端。
    ///
    /// # 错误
    /// 当会话状态无法读取或 HTTP 客户端无法创建时返回错误。
    pub(crate) fn new(app: &tauri::AppHandle) -> Result<Self, String> {
        let web_cookie = Self::read_web_cookie(app)?;
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

    /// 判断此客户端是否持有包含 `session_PC` 的 Web 会话。
    pub(crate) fn has_session(&self) -> bool {
        self.web_cookie.as_ref().is_some_and(|cookie| {
            cookie
                .split(';')
                .any(|pair| pair.trim().starts_with("session_PC="))
        })
    }

    /// 确保需要登录的网页端点拥有已验证的 Web 会话。
    ///
    /// # 错误
    /// 未持有 `session_PC` 时返回提示用户使用官方网页登录的错误。
    pub(super) fn require_session(&self) -> Result<(), String> {
        self.has_session()
            .then_some(())
            .ok_or_else(|| "请先使用官方网页登录 Web 服务".to_string())
    }

    /// 使用本实例的 Web Cookie 创建统一的导航型网页请求。
    ///
    /// 调用方不能将 App Cookie 注入该请求。
    fn page_request(
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

    /// 使用本实例 Web Cookie 和端点同源 Referer 创建 AJAX 请求。
    pub(super) fn ajax_request(
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

    /// 使用本实例 Web Cookie 创建统一的网页静态资源请求。
    pub(super) fn asset_request(
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

    /// 读取需认证的有声目录，且不向渲染进程暴露流媒体地址。
    ///
    /// # 错误
    /// 缺少 Web 会话、上游拒绝请求或返回无效目录数据时返回错误。
    pub(super) async fn audio_catalog(
        &self,
        novel_id: i64,
    ) -> Result<(String, Vec<NativeAudioChapter>), String> {
        validate_novel_id(novel_id)?;
        self.require_session()?;
        let response = self
            .ajax_request(
                "https://i.sfacg.com/ajax/ashx/Common.ashx",
                "https://i.sfacg.com/consume/book/",
            )
            .query(&[("op", "getAudioInfo"), ("nid", &novel_id.to_string())])
            .send()
            .await
            .map_err(|error| format!("无法连接 SF 有声接口：{error}"))?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED
            || response.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err("SF Web 登录会话已失效，请重新登录".to_string());
        }
        let payload = response
            .json::<Value>()
            .await
            .map_err(|error| format!("SF 有声目录格式无效：{error}"))?;
        if payload.get("status").and_then(Value::as_i64) != Some(200) {
            let message = payload.get("msg").and_then(Value::as_str).unwrap_or("");
            return Err(if message == "参数不正确" {
                "该作品没有可用的有声章节".to_string()
            } else if message.is_empty() {
                "SF 有声接口拒绝了请求".to_string()
            } else {
                format!("SF 有声接口拒绝了请求：{message}")
            });
        }
        let data = payload
            .get("data")
            .ok_or_else(|| "该作品没有可用的有声章节".to_string())?;
        let title = data
            .get("NovelName")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("未命名有声作品")
            .to_string();
        let mut chapters = Vec::new();
        for volume in data
            .get("VolumeSet")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let volume_title = volume
                .get("VolumeName")
                .and_then(Value::as_str)
                .unwrap_or("未分卷")
                .to_string();
            for audio in volume
                .get("AudioSet")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let Some(source) = audio.get("AudioSrc").and_then(Value::as_str) else {
                    continue;
                };
                if source.trim().is_empty() {
                    continue;
                }
                let is_vip = audio
                    .get("IsVip")
                    .or_else(|| audio.get("isVip"))
                    .or_else(|| audio.get("IsVIP"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let is_unlocked = audio
                    .get("IsUnlocked")
                    .or_else(|| audio.get("isUnlocked"))
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                chapters.push(NativeAudioChapter {
                    id: audio
                        .get("AudioID")
                        .and_then(Value::as_i64)
                        .unwrap_or(chapters.len() as i64 + 1),
                    title: audio
                        .get("ChapterTitle")
                        .and_then(Value::as_str)
                        .unwrap_or("未命名章节")
                        .to_string(),
                    volume: volume_title.clone(),
                    is_vip,
                    is_unlocked,
                    source: source.to_string(),
                });
            }
        }
        if chapters.is_empty() {
            return Err("该作品没有可用的有声章节".to_string());
        }
        Ok((title, chapters))
    }

    /// Reads the public pockets belonging to the logged-in web account and
    /// collects their novel and comic entries. Audio pockets are deliberately
    /// ignored; audio is probed only after the user opens a novel.
    pub(super) async fn public_bookshelf(&self) -> Result<PublicShelf, String> {
        self.require_session()?;
        let account = self
            .page_request("https://passport.sfacg.com/Ajax/GetLoginInfo.ashx", None)
            .send()
            .await
            .map_err(|error| format!("无法读取 SF 网页账号信息：{error}"))?
            .error_for_status()
            .map_err(|error| format!("SF 网页账号信息请求被拒绝：{error}"))?
            .text()
            .await
            .map_err(|error| format!("SF 网页账号信息格式无效：{error}"))?;
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
                // The web site intentionally returns 500 for audio/video
                // pockets. They are probed later from a novel and must not
                // make the novel/comic shelf fail as a whole.
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

    /// 从 SF 漫画网站读取漫画目录。
    ///
    /// # 错误
    /// 网页不可访问、目录结构无法解析或上游未返回章节时返回错误。
    pub(super) async fn comic_catalog(
        &self,
        folder: &str,
    ) -> Result<Vec<NativeComicChapter>, String> {
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

    /// 从漫画网页读取作品标题、作者、封面和简介。
    ///
    /// # 错误
    /// 网页不可访问或页面缺少有效作品标识时返回错误。
    pub(super) async fn comic_details(&self, folder: &str) -> Result<NovelDetail, String> {
        if folder.is_empty()
            || folder.len() > 100
            || !folder.bytes().all(|byte| byte.is_ascii_alphanumeric())
        {
            return Err("漫画网页目录标识无效".to_string());
        }
        let html = self
            .page_request(format!("https://manhua.sfacg.com/mh/{folder}/"), None)
            .send()
            .await
            .map_err(|error| format!("无法读取 SF 漫画详情：{error}"))?
            .error_for_status()
            .map_err(|error| format!("SF 漫画详情请求被拒绝：{error}"))?
            .text()
            .await
            .map_err(|error| format!("SF 漫画详情格式无效：{error}"))?;
        let title = extract_tag_text(&html, "<h1", "</h1>")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "SF 漫画详情未返回有效标题".to_string())?;
        let overview = html
            .find("class=\"synopsises_font\"")
            .and_then(|start| {
                html[start..]
                    .find("</ul>")
                    .map(|end| &html[start..start + end])
            })
            .unwrap_or("");
        let cover = overview
            .find("src=\"")
            .and_then(|position| {
                let start = position + 5;
                overview[start..]
                    .find('"')
                    .map(|end| overview[start..start + end].to_string())
            })
            .unwrap_or_default();
        let detail_item = overview
            .find("<li")
            .and_then(|first| overview[first..].find("</li>").map(|end| first + end + 5))
            .and_then(|second_start| {
                overview[second_start..].find("<li").and_then(|offset| {
                    let start = second_start + offset;
                    overview[start..]
                        .find("</li>")
                        .map(|end| &overview[start..start + end])
                })
            })
            .unwrap_or("");
        let description = detail_item
            .find("<br")
            .and_then(|position| {
                detail_item[position..]
                    .find('>')
                    .map(|end| position + end + 1)
            })
            .map(|start| {
                let end = detail_item
                    .find("class=\"broken_line\"")
                    .unwrap_or(detail_item.len());
                strip_html_text(&detail_item[start..end])
            })
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "暂无简介".to_string());
        let author_name = detail_item
            .find("作者：</span>")
            .and_then(|position| {
                detail_item[position..]
                    .find("</span>")
                    .map(|offset| position + offset + 7)
            })
            .and_then(|start| extract_first_anchor_text(&detail_item[start..]))
            .unwrap_or_else(|| "未知作者".to_string());
        eprintln!(
            "[sfacg] web comic details parsed: folder={}, html_bytes={}, cover={}, description_chars={}",
            folder,
            html.len(),
            !cover.is_empty(),
            description.chars().count(),
        );
        Ok(NovelDetail {
            novel_id: 0,
            novel_name: title,
            author_name,
            novel_cover: cover,
            last_update_time: String::new(),
            description,
            is_finish: false,
            type_name: Some("漫画".to_string()),
            tags: None,
            score: None,
            chapter_count: None,
            character_count: None,
            view_count: None,
            mark_count: None,
            point_count: None,
            favorite_count: None,
            ticket_count: None,
            latest_chapter_title: None,
            latest_chapter_time: None,
            source_path: Some(folder.to_string()),
        })
    }

    /// 通过 SF 漫画网站解析一个章节的图片地址。
    ///
    /// 图片地址仅供原生下载工作线程使用，不会直接返回渲染进程。
    ///
    /// # 错误
    /// 章节不可访问、页面缺少资源标识或图片接口拒绝请求时返回错误。
    pub(super) async fn comic_chapter_images(
        &self,
        folder: &str,
        chapter_id: i64,
    ) -> Result<Vec<String>, String> {
        if chapter_id <= 0 {
            return Err("漫画章节编号无效".to_string());
        }
        let chapter_url = format!("https://manhua.sfacg.com/mh/{folder}/{chapter_id}/");
        let html = self
            .page_request(&chapter_url, None)
            .send()
            .await
            .map_err(|error| format!("无法读取 SF 漫画章节：{error}"))?
            .error_for_status()
            .map_err(|error| format!("SF 漫画章节请求被拒绝：{error}"))?
            .text()
            .await
            .map_err(|error| format!("SF 漫画章节格式无效：{error}"))?;
        let source_comic_id =
            extract_js_number(&html, "c").ok_or_else(|| "该漫画章节当前不可下载".to_string())?;
        let source_chapter_id = extract_js_number(&html, "chapId")
            .ok_or_else(|| "该漫画章节当前不可下载".to_string())?;
        let serial = extract_js_string(&html, "fn")
            .ok_or_else(|| "SF 漫画章节未返回资源标识".to_string())?;
        let path = extract_js_string(&html, "nv")
            .ok_or_else(|| "SF 漫画章节未返回资源路径".to_string())?;
        let payload = self
            .ajax_request("https://manhua.sfacg.com/ajax/Common.ashx", &chapter_url)
            .query(&[
                ("op", "getPics"),
                ("cid", &source_comic_id.to_string()),
                ("chapId", &source_chapter_id.to_string()),
                ("serial", serial.as_str()),
                ("path", path.as_str()),
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

    /// 获取并从 `#ChapterBody` 提取一章可访问的小说正文。
    ///
    /// # 错误
    /// 网页不可访问、正文节点缺失或提取结果为空时返回错误。
    pub(super) async fn chapter_content(
        &self,
        novel_id: i64,
        volume_id: i64,
        chapter_id: i64,
    ) -> Result<String, String> {
        let html = self
            .page_request(
                format!("https://book.sfacg.com/Novel/{novel_id}/{volume_id}/{chapter_id}/"),
                Some(&format!(
                    "https://book.sfacg.com/Novel/{novel_id}/MainIndex/"
                )),
            )
            .send()
            .await
            .map_err(|error| format!("无法读取 SF 网页章节：{error}"))?
            .error_for_status()
            .map_err(|error| format!("SF 网页章节请求被拒绝：{error}"))?
            .text()
            .await
            .map_err(|error| format!("SF 网页章节格式无效：{error}"))?;
        let start = html
            .find("id=\"ChapterBody\"")
            .or_else(|| html.find("id='ChapterBody'"))
            .ok_or_else(|| "SF 网页响应中没有找到 #ChapterBody".to_string())?;
        let body_start = html[start..]
            .find('>')
            .map(|offset| start + offset + 1)
            .ok_or_else(|| "SF 章节正文结构无效".to_string())?;
        let body_end = html[body_start..]
            .find("</div>")
            .map(|offset| body_start + offset)
            .ok_or_else(|| "SF 章节正文结构无效".to_string())?;
        let body = html[body_start..body_end]
            .replace("<br>", "\n")
            .replace("<br/>", "\n")
            .replace("<br />", "\n")
            .replace("</p>", "\n")
            .replace("&nbsp;", " ")
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">");
        let mut plain = String::with_capacity(body.len());
        let mut in_tag = false;
        for character in body.chars() {
            match character {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => plain.push(character),
                _ => {}
            }
        }
        let plain = plain.replace("\r\n", "\n").replace('\r', "\n");
        if plain.trim().is_empty() {
            return Err("SF 网页章节正文为空".to_string());
        }
        Ok(plain.trim().to_string())
    }

    /// 从原生 Web 会话中读取并过滤出 `.SFCommunity` 与 `session_PC`。
    fn read_web_cookie(app: &tauri::AppHandle) -> Result<Option<String>, String> {
        let auth_state = app.state::<AuthSessionState>();
        let session = auth_state
            .session
            .lock()
            .map_err(|_| "登录会话状态不可用".to_string())?;
        Ok(session
            .as_ref()
            .and_then(|session| session.web_cookie.as_deref())
            .and_then(Self::filter_web_cookie))
    }

    /// 在构建网页请求前剔除 App 会话及未知 Cookie 名称。
    fn filter_web_cookie(cookie: &str) -> Option<String> {
        let pairs = cookie
            .split(';')
            .map(str::trim)
            .filter(|pair| pair.starts_with(".SFCommunity=") || pair.starts_with("session_PC="))
            .collect::<Vec<_>>();
        (!pairs.is_empty()).then(|| pairs.join("; "))
    }

    /// 在可用时向请求附加实例自有的 Web Cookie。
    fn with_web_cookie(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match self.web_cookie.as_deref() {
            Some(cookie) => request.header(reqwest::header::COOKIE, cookie),
            None => request,
        }
    }
}

/// Extracts a quoted field from the small JavaScript response emitted by the
/// official login-info endpoint.
fn extract_js_field(value: &str, field: &str) -> Option<String> {
    for marker in [format!("{field}:\""), format!("{field}:'")] {
        let Some(marker_start) = value.find(&marker) else {
            continue;
        };
        let start = marker_start + marker.len();
        let Some(quote) = marker.chars().last() else {
            continue;
        };
        let Some(relative_end) = value[start..].find(quote) else {
            continue;
        };
        let end = relative_end + start;
        let text = value[start..end].trim();
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }
    None
}

fn is_safe_account_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

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

/// Parses comics from the `eid` ownership marker in the item details block.
/// Unlike the cover URL, this marker carries the numeric comic ID required by
/// later catalog and download operations.
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
        // Comic cards use an outer `<ul ... eid="...">` and put the cover
        // and metadata in separate `<li>` elements inside that container.
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

fn extract_numeric_attribute(value: &str, attribute: &str) -> Option<i64> {
    let marker = format!("{attribute}=");
    let start = value.find(&marker)? + marker.len();
    let tail = value[start..].trim_start();
    let tail = tail.strip_prefix('"').or_else(|| tail.strip_prefix('\''));
    let digits = match tail {
        Some(value) => value
            .chars()
            .take_while(|character| character.is_ascii_digit())
            .collect::<String>(),
        None => value
            .chars()
            .take_while(|character| character.is_ascii_digit())
            .collect::<String>(),
    };
    digits.parse::<i64>().ok()
}

fn extract_tag_text(value: &str, opening: &str, closing: &str) -> Option<String> {
    let start = value.find(opening)?;
    let content_start = value[start..].find('>')? + start + 1;
    let content_end = value[content_start..].find(closing)? + content_start;
    let text = strip_html_text(&value[content_start..content_end]);
    (!text.is_empty()).then_some(text)
}

fn extract_first_anchor_text(value: &str) -> Option<String> {
    let start = value.find("<a")?;
    let content_start = value[start..].find('>')? + start + 1;
    let content_end = value[content_start..].find("</a>")? + content_start;
    let text = strip_html_text(&value[content_start..content_end]);
    (!text.is_empty()).then_some(text)
}

fn strip_html_text(value: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    for character in value.chars() {
        match character {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(character),
            _ => {}
        }
    }
    text.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_public_novel_and_comic_cards() {
        let html = r#"
          <li style="width:110px"><a href="https://book.sfacg.com/Novel/123/"><img src="https://img/novel.jpg" /></a></li>
          <li><b><a href="https://book.sfacg.com/Novel/123/">测试小说</a></b><br />作者：作者甲</li>
          <ul class="content_comment cover" eid="456">
            <li style="width:110px"><a href="https://manhua.sfacg.com/mh/ABC/"><img src="https://img/comic.jpg" /></a></li>
            <li><b><a href="https://manhua.sfacg.com/mh/ABC/">测试漫画</a></b><br />作者：作者乙 <a op="AddLink" eid="456">添加</a></li>
          </ul>
        "#;
        let items = parse_public_pocket_items(html);
        assert_eq!(items.len(), 2);
        assert!(items.iter().any(|item| {
            item.kind == "novel" && item.id == 123 && item.title == "测试小说"
        }));
        assert!(items.iter().any(|item| {
            item.kind == "comic"
                && item.id == 456
                && item.title == "测试漫画"
                && item.cover == "https://img/comic.jpg"
                && item.source_path.as_deref() == Some("ABC")
        }));
    }
}
