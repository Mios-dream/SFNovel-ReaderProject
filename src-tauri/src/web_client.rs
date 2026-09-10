//! SF 网页客户端及其 Web 会话边界。

use crate::sfacg::{
    validate_novel_id, AuthSessionState, ComicUnlockState, NativeAudioChapter, NativeComicChapter,
    UserProfile, SF_WEB_USER_AGENT,
};
use crate::utils::cookie::{filter_cookie_header, has_cookie_name};
use serde_json::Value;
use tauri::Manager;

/// 从公开网页书架解析出的一条小说或漫画记录。
pub(crate) struct PublicShelfItem {
    pub(crate) kind: &'static str,
    pub(crate) id: i64,
    pub(crate) title: String,
    pub(crate) author: String,
    pub(crate) cover: String,
    pub(crate) source_path: Option<String>,
}

/// 公开书架分类及成功解析出的小说、漫画条目。
pub(crate) struct PublicShelf {
    pub(crate) categories: Vec<String>,
    pub(crate) items: Vec<(String, PublicShelfItem)>,
}

/// 使用来自 `NativeAuthSession.web_cookie` 的 Web 凭证，发送 SF 网页、AJAX 和静态资源请求。
pub(crate) struct WebClient {
    client: reqwest::Client,
    web_cookie: Option<String>,
}

/// VIP 章节返回的图片数据。
///
/// 二进制数据不跨越 IPC 边界。下载器先持久化图片，再将得到的本地路径交给 OCR worker。
pub(crate) struct VipChapterImage {
    // 章节图片二进制数据，供下载器持久化。
    pub(crate) bytes: Vec<u8>,
    // 文件扩展名，供下载器持久化时使用。仅限 "jpg"、"png"、"gif"、"webp"。
    pub(crate) extension: &'static str,
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
        self.web_cookie
            .as_deref()
            .is_some_and(|cookie| has_cookie_name(cookie, "session_PC"))
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

    /// 验证网页登录会话并解析网页端账户资料、余额、月票和 VIP 等级。
    pub(super) async fn get_user_profile(&self) -> Result<UserProfile, String> {
        self.require_session()?;
        let account = self.get_login_info().await?;
        let mut profile = user_profile_from_login_info(&account)?;
        let my_html = self
            .page_request("https://m.sfacg.com/my/", Some("https://m.sfacg.com/"))
            .send()
            .await
            .map_err(|error| format!("无法读取 SF 网页账户中心：{error}"))?
            .error_for_status()
            .map_err(|error| format!("SF 网页账户中心请求被拒绝：{error}"))?
            .text()
            .await
            .map_err(|error| format!("SF 网页账户中心格式无效：{error}"))?;
        let account_text = strip_html_text(&my_html);
        let month_ticket = extract_numbers_after_text(&account_text, "月票")
            .first()
            .copied()
            .unwrap_or(0);
        let wallet = extract_numbers_after_text(&account_text, "我的钱包");
        let fire_money = wallet.first().copied().unwrap_or(0);
        let coupons = wallet.get(1).copied().unwrap_or(0);
        profile.web_details_available = true;
        profile.fire_money_remain = fire_money;
        profile.coupons_remain = coupons;
        profile.monthly_ticket = month_ticket;
        // 新旧 VIP 接口都会存在；由 newVip.isNewVip 决定当前账户采用哪套体系。
        // VIP 资料是独立的可选补充，失败不阻断 Web 基础资料和余额显示。
        if let Ok(details) = self.get_web_vip_details().await {
            profile.vip_system = details.system.to_string();
            profile.vip_level = details.level;
            profile.vip_name = details.name;
            profile.vip_details_available = true;
        }
        Ok(profile)
    }

    /// 根据 `newVip.isNewVip` 选择新 VIP 或旧 VIP 资料。
    async fn get_web_vip_details(&self) -> Result<WebVipDetails, String> {
        let payload = self
            .ajax_request(
                "https://pages.sfacg.com/api/User?expand=newVip",
                "https://pages.sfacg.com/h5/app/common/help/Vip.html",
            )
            .send()
            .await
            .map_err(|error| format!("无法读取 SF VIP 资料：{error}"))?
            .error_for_status()
            .map_err(|error| format!("SF VIP 资料请求被拒绝：{error}"))?
            .json::<Value>()
            .await
            .map_err(|error| format!("SF VIP 资料格式无效：{error}"))?;
        let new_vip = web_new_vip_details_from_payload(&payload)?;
        if new_vip.is_new {
            return Ok(WebVipDetails {
                system: "new",
                level: new_vip.level,
                name: new_vip.name,
            });
        }

        let payload = self
            .ajax_request(
                "https://pages.sfacg.com/api/common/vipInfo",
                "https://pages.sfacg.com/h5/app/common/help/Vip.html",
            )
            .send()
            .await
            .map_err(|error| format!("无法读取 SF 旧 VIP 资料：{error}"))?
            .error_for_status()
            .map_err(|error| format!("SF 旧 VIP 资料请求被拒绝：{error}"))?
            .json::<Value>()
            .await
            .map_err(|error| format!("SF 旧 VIP 资料格式无效：{error}"))?;
        let level = web_old_vip_level_from_payload(&payload)?;
        Ok(WebVipDetails {
            system: "legacy",
            level,
            name: String::new(),
        })
    }

    /// 使用本实例的 Web Cookie 创建统一的导航型网页请求。
    ///
    /// # 参数
    /// * `url` - 目标网页地址。
    /// * `referer` - 请求的 Referer 头，用于模拟浏览器导航。
    ///
    /// # 返回
    /// 构建好的请求构建器，包含 Web Cookie 和默认的 Accept 头。
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

    /// 请求官方网页登录信息，不返回原始响应给调用方。
    async fn get_login_info(&self) -> Result<String, String> {
        self.page_request("https://passport.sfacg.com/Ajax/GetLoginInfo.ashx", None)
            .send()
            .await
            .map_err(|error| format!("无法读取 SF 网页账号信息：{error}"))?
            .error_for_status()
            .map_err(|error| format!("SF 网页账号信息请求被拒绝：{error}"))?
            .text()
            .await
            .map_err(|error| format!("SF 网页账号信息格式无效：{error}"))
    }

    /// 使用本实例 Web Cookie 和端点同源 Referer 创建 AJAX 请求。
    ///
    /// # 参数
    /// * `url` - 目标 AJAX 端点地址。
    /// * `referer` - 请求的 Referer 头，用于模拟浏览器导航。
    ///
    /// # 返回
    /// 构建好的请求构建器，包含 Web Cookie 和默认的 Accept 头。
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
    ///
    /// # 参数
    /// * `url` - 目标静态资源地址。
    /// * `referer` - 请求的 Referer 头，用于模拟浏览器导航。
    /// * `accept` - 请求的 Accept 头，用于指定期望的响应类型。
    ///
    /// # 返回
    /// 构建好的请求构建器，包含 Web Cookie 和指定的 Accept 头。
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

    /// 获取有声目录并解析为标题和章节列表。
    ///
    /// # 错误
    /// 缺少 Web 会话、上游拒绝请求或返回无效目录数据时返回错误。
    pub(super) async fn get_audio_catalog(
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

    /// 读取已登录网页账户所属的公开书架，并获取其中的小说和漫画条目。
    ///
    /// 注意：有声书架会被刻意忽略；仅在用户打开小说后才按需探测有声内容。
    pub(super) async fn get_public_bookshelf(&self) -> Result<PublicShelf, String> {
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

    /// 从 SF 漫画网站读取漫画目录。
    ///
    /// # 错误
    /// 网页不可访问、目录结构无法解析或上游未返回章节时返回错误。
    pub(super) async fn get_comic_catalog(
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

    /// 通过 SF 漫画图片 AJAX 接口读取一个章节的图片地址。
    ///
    /// 图片地址仅供原生下载工作线程使用，不会直接返回渲染进程。
    ///
    /// # 错误
    /// 章节编号无效或图片接口拒绝请求时返回错误。
    pub(super) async fn comic_chapter_images(
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
        let plain = chapter_html_to_markdown(&body);
        let plain = plain.replace("\r\n", "\n").replace('\r', "\n");
        if plain.trim().is_empty() {
            return Err("SF 网页章节正文为空".to_string());
        }
        Ok(plain.trim().to_string())
    }

    /// 请求已授权 VIP 文字章节的图片内容。
    ///
    /// 请求刻意限制为原生 Web 会话，并使用 VIP 阅读页作为 Referer。仅有成功 HTTP 状态
    /// 不足以证明可用，因为上游可能以 200 状态返回 HTML 错误页。
    pub(super) async fn vip_chapter_image(
        &self,
        novel_id: i64,
        chapter_id: i64,
    ) -> Result<VipChapterImage, String> {
        validate_novel_id(novel_id)?;
        if chapter_id <= 0 {
            return Err("VIP 章节参数无效".to_string());
        }
        self.require_session()?;
        let referer = format!("https://book.sfacg.com/vip/c/{chapter_id}/");
        let response = self
            .ajax_request("https://book.sfacg.com/ajax/ashx/common.ashx", &referer)
            .query(&[
                ("op", "getChapPic"),
                ("tp", "true"),
                ("quick", "true"),
                ("cid", &chapter_id.to_string()),
                ("nid", &novel_id.to_string()),
                ("font", "16"),
                ("lang", ""),
                ("w", "5000"),
            ])
            .header(
                reqwest::header::ACCEPT,
                "image/avif,image/webp,image/apng,image/*,*/*;q=0.8",
            )
            .send()
            .await
            .map_err(|error| format!("无法读取 VIP 章节图片：{error}"))?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED
            || response.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err("网页登录会话已失效".to_string());
        }
        let response = response
            .error_for_status()
            .map_err(|error| format!("VIP 章节图片请求被拒绝：{error}"))?;
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();
        let bytes = response
            .bytes()
            .await
            .map_err(|error| format!("无法读取 VIP 章节图片数据：{error}"))?
            .to_vec();
        let extension = if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
            "gif"
        } else if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
            "png"
        } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
            "jpg"
        } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
            "webp"
        } else {
            return Err(if content_type.contains("text/html") {
                "VIP 章节未返回图片内容，请确认网页登录会话和购买状态".to_string()
            } else {
                "VIP 章节资源格式无效".to_string()
            });
        };
        if bytes.len() < 512 {
            return Err("VIP 章节图片数据无效或为空".to_string());
        }
        Ok(VipChapterImage { bytes, extension })
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
            .and_then(|cookie| filter_cookie_header(cookie, &[".SFCommunity", "session_PC"])))
    }

    /// 在可用时向请求附加实例自有的 Web Cookie。
    fn with_web_cookie(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match self.web_cookie.as_deref() {
            Some(cookie) => request.header(reqwest::header::COOKIE, cookie),
            None => request,
        }
    }
}

/// 移除文字标记，同时将上游图片地址保留为 SF 图片标签。
///
/// 下载器随后经 `WebClient` 解析这些标签并改写为本地路径。这样可分离 HTML 解析和文件
/// I/O，并避免丢失普通章节内嵌插图。
fn chapter_html_to_markdown(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut remaining = value;
    while let Some(tag_start) = remaining.find('<') {
        output.push_str(&remaining[..tag_start]);
        let after_start = &remaining[tag_start..];
        let Some(tag_end) = after_start.find('>') else {
            output.push_str(after_start);
            return output;
        };
        let tag = &after_start[..=tag_end];
        let normalized = tag.to_ascii_lowercase();
        if normalized.starts_with("<img") {
            if let Some(source) = html_tag_attribute(tag, "src") {
                let source = source
                    .strip_prefix("//")
                    .map_or_else(|| source.to_string(), |value| format!("https://{value}"));
                if source.starts_with("https://") || source.starts_with("http://") {
                    output.push_str(&format!("\n[img=]{}[/img]\n", source));
                }
            }
        }
        remaining = &after_start[tag_end + 1..];
    }
    output.push_str(remaining);
    output
}

/// 解析 SF 漫画目录页中的章节链接。
///
/// 漫画网页目录只能识别章节是否为 VIP，不能识别当前 App 账户是否拥有章节。网页上的
/// 任何属性都不参与拥有权判断；VIP 章节必须保留为未知状态，直到实际请求资源。
fn parse_comic_catalog(html: &str, folder: &str) -> Result<Vec<NativeComicChapter>, String> {
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
        chapters.push(NativeComicChapter {
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

/// 从单个已限定范围的 HTML 标签读取带引号或不带引号的属性。
fn html_tag_attribute(tag: &str, name: &str) -> Option<String> {
    let marker = format!("{name}=");
    let position = tag.to_ascii_lowercase().find(&marker)? + marker.len();
    let value = tag[position..].trim_start();
    if let Some(quote) = value
        .chars()
        .next()
        .filter(|value| *value == '\'' || *value == '"')
    {
        let end = value[1..].find(quote)? + 1;
        return Some(value[1..end].to_string());
    }
    let end = value
        .find(char::is_whitespace)
        .or_else(|| value.find('>'))
        .unwrap_or(value.len());
    (!value[..end].is_empty()).then(|| value[..end].to_string())
}

/// 从官方登录信息端点返回的短 JavaScript 响应中提取字段。
fn extract_js_field(value: &str, field: &str) -> Option<String> {
    let marker = format!("{field}:");
    let marker_start = value.find(&marker)?;
    let rest = value[marker_start + marker.len()..].trim_start();
    if let Some(quote) = rest.chars().next().filter(|c| *c == '\'' || *c == '"') {
        let start = quote.len_utf8();
        let end = rest[start..].find(quote)? + start;
        return (!rest[start..end].trim().is_empty()).then(|| rest[start..end].trim().to_string());
    }
    let token = rest
        .split(|character: char| {
            character == ',' || character == '}' || character == ']' || character.is_whitespace()
        })
        .next()
        .unwrap_or("")
        .trim();
    (!token.is_empty()).then(|| token.to_string())
}

/// 从官方登录信息响应生成不含 Cookie 的网页基础账户资料。
fn user_profile_from_login_info(value: &str) -> Result<UserProfile, String> {
    if extract_js_field(value, "login").as_deref() != Some("true") {
        return Err("网页登录会话已失效".to_string());
    }
    let nick_name = ["nickname", "nickName"]
        .iter()
        .find_map(|field| extract_js_field(value, field))
        .ok_or_else(|| "SF 网页账号信息未返回昵称".to_string())?;
    let avatar = ["avatar", "userAvatar", "portrait", "headImg", "headimg"]
        .iter()
        .find_map(|field| extract_js_field(value, field))
        .unwrap_or_default();
    let account_id = ["accountId", "accountid", "userId", "userid"]
        .iter()
        .find_map(|field| extract_js_number_field(value, field))
        .unwrap_or(0);
    Ok(UserProfile {
        account_id,
        nick_name,
        avatar,
        app_details_available: false,
        web_details_available: false,
        vip_details_available: false,
        vip_system: String::new(),
        welfare_coin: 0,
        fire_money_remain: 0,
        coupons_remain: 0,
        monthly_ticket: 0,
        vip_level: 0,
        vip_name: String::new(),
    })
}

/// 从官方登录信息端点的 JavaScript 字面量提取一个整数栏位。
fn extract_js_number_field(value: &str, field: &str) -> Option<i64> {
    let marker = format!("{field}:");
    let start = value.find(&marker)? + marker.len();
    let value = value[start..].trim_start();
    let value = value
        .strip_prefix('"')
        .or_else(|| value.strip_prefix('\''))
        .unwrap_or(value);
    let digits = value
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>();
    digits.parse().ok().filter(|number: &i64| *number > 0)
}

/// 从网页纯文本中提取指定标签之后的连续数字。
fn extract_numbers_after_text(value: &str, marker: &str) -> Vec<i64> {
    let Some(start) = value.find(marker).map(|index| index + marker.len()) else {
        return Vec::new();
    };
    let mut numbers = Vec::new();
    let mut digits = String::new();
    for character in value[start..].chars().take(120) {
        if character.is_ascii_digit() {
            digits.push(character);
        } else if !digits.is_empty() {
            if let Ok(number) = digits.parse() {
                numbers.push(number);
            }
            digits.clear();
        }
        if numbers.len() >= 4 {
            break;
        }
    }
    if !digits.is_empty() {
        if let Ok(number) = digits.parse() {
            numbers.push(number);
        }
    }
    numbers
}

struct NewVipDetails {
    is_new: bool,
    level: i64,
    name: String,
}

struct WebVipDetails {
    system: &'static str,
    level: i64,
    name: String,
}

/// 从 `newVip` 响应中提取新旧体系标记、等级和名称。
fn web_new_vip_details_from_payload(payload: &Value) -> Result<NewVipDetails, String> {
    if payload
        .get("status")
        .and_then(|status| status.get("errorCode"))
        .and_then(Value::as_i64)
        != Some(200)
    {
        return Err("SF VIP 资料请求未成功".to_string());
    }
    let vip = payload
        .get("data")
        .and_then(|data| data.get("expand"))
        .and_then(|expand| expand.get("newVip"))
        .ok_or_else(|| "SF VIP 资料未返回等级".to_string())?;
    let is_new = vip
        .get("isNewVip")
        .and_then(Value::as_bool)
        .ok_or_else(|| "SF VIP 资料未返回体系标记".to_string())?;
    let level = vip
        .get("level")
        .and_then(Value::as_i64)
        .ok_or_else(|| "SF VIP 资料未返回等级".to_string())?;
    let name = vip
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    Ok(NewVipDetails {
        is_new,
        level,
        name,
    })
}

/// 从旧 VIP 接口提取当前旧体系等级。
fn web_old_vip_level_from_payload(payload: &Value) -> Result<i64, String> {
    if payload
        .get("status")
        .and_then(|status| status.get("errorCode"))
        .and_then(Value::as_i64)
        != Some(200)
    {
        return Err("SF 旧 VIP 资料请求未成功".to_string());
    }
    payload
        .get("data")
        .and_then(|data| data.get("vipLevel"))
        .and_then(Value::as_i64)
        .ok_or_else(|| "SF 旧 VIP 资料未返回等级".to_string())
}

/// 判断账户名是否符合网页端点允许的受限字符和长度规则。
fn is_safe_account_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

/// 从公开书架 HTML 提取去重后的书架编号及名称。
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

/// 从 HTML 片段提取一个可选的正整数属性值。
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

/// 移除简单 HTML 标签和常见实体，将连续空白折叠为单个空格。
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
