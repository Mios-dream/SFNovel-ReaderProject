//! SF 网页客户端及其 Web 会话边界。

use super::*;

/// 使用来自 `NativeAuthSession.web_cookie` 的已过滤 Web 会话快照，发送 SF
/// 网页、AJAX 和静态资源请求。
pub(super) struct WebClient {
    client: reqwest::Client,
    web_cookie: Option<String>,
}

impl WebClient {
    /// 仅使用原生 Web 会话状态创建网页客户端。
    ///
    /// # 错误
    /// 当会话状态无法读取或 HTTP 客户端无法创建时返回错误。
    pub(super) fn new(app: &tauri::AppHandle) -> Result<Self, String> {
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
    pub(super) fn has_session(&self) -> bool {
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
                    source: source.to_string(),
                });
            }
        }
        if chapters.is_empty() {
            return Err("该作品没有可用的有声章节".to_string());
        }
        Ok((title, chapters))
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
