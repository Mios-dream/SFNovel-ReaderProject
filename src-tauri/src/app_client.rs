//! SF App API 签名客户端及其 App 会话边界。

use crate::sfacg::{
    device_token, validate_novel_id, ApiChapterContent, AuthSessionState, ChapterSummary,
    ChapterVolume, NovelDetail, SearchNovel, TextAccessState, TextContentKind, UserProfile,
};
use crate::utils::cookie::{filter_cookie_header, has_cookie_name};
use md5::{Digest, Md5};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Manager;
use uuid::Uuid;

/// SF App API 的固定服务根地址。
const SF_API_HOST: &str = "https://api.sfacg.com";
/// App API HTTP 基础认证使用的公开客户端标识。
const SF_API_USER: &str = "androiduser";
/// App API HTTP 基础认证的协议常量。
const SF_API_PASSWORD: &str = "1a#$51-yt69;*Acv@qxq";
/// 生成 `SFSecurity` 请求头所需的协议盐值。
const SF_SECURITY_SALT: &str = "FN_Q29XHVmfV3mYX";

/// 仅发送已签名的 SF App API 请求，并持有来自
/// `NativeAuthSession.app_cookie` 的已过滤 App 会话快照。
pub(crate) struct AppClient {
    client: reqwest::Client,
    app_cookie: Option<String>,
}

impl AppClient {
    /// 仅使用原生 App 会话状态创建 API 客户端。
    ///
    /// # 错误
    /// 当设备身份未初始化或 HTTP 客户端无法创建时返回错误。
    pub(crate) fn new(app: &tauri::AppHandle) -> Result<Self, String> {
        let token = device_token()?.to_uppercase();
        let user_agent = format!("boluobao/5.0.36(android;34)/H5/{token}/H5");
        let app_cookie = Self::read_app_cookie(app)?;
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent(user_agent)
            .build()
            .map_err(|error| format!("无法创建 SF App 网络客户端：{error}"))?;
        Ok(Self { client, app_cookie })
    }

    /// 判断此客户端是否持有包含 `session_APP` 的 App 会话。
    pub(crate) fn has_session(&self) -> bool {
        self.app_cookie
            .as_deref()
            .is_some_and(|cookie| has_cookie_name(cookie, "session_APP"))
    }

    /// 按固定请求指纹构建一个已签名的 App API 请求。
    ///
    /// # 参数
    /// * `method` - 请求使用的 HTTP 方法。
    /// * `path` - 相对 SF App API 路径。
    /// * `include_session` - 是否附带本地 App 会话 Cookie。
    /// 每次请求都会生成新的 UUID nonce。
    ///
    /// # 错误
    /// 当无法生成安全签名头时返回错误。
    fn signed_request(
        &self,
        method: reqwest::Method,
        path: &str,
        include_session: bool,
    ) -> Result<reqwest::RequestBuilder, String> {
        let nonce = Uuid::new_v4().to_string().to_uppercase();
        let mut request = self
            .client
            .request(method, format!("{SF_API_HOST}{path}"))
            .basic_auth(SF_API_USER, Some(SF_API_PASSWORD))
            .header("Accept", "application/vnd.sfacg.api+json;version=1")
            .header("Accept-Language", "zh-Hans-CN;q=1")
            .header("Content-Type", "application/json")
            .header("SFSecurity", Self::security_header(&nonce)?);
        if include_session {
            if let Some(cookie) = self.app_cookie.as_deref() {
                request = request.header(reqwest::header::COOKIE, cookie);
            }
        }
        Ok(request)
    }

    /// 请求一个已签名的 SF App API 资源，并拆开其 `data` 数据包。
    ///
    /// # 错误
    /// 请求失败、上游拒绝请求或响应不是有效 JSON 时返回错误；HTTP 417 会清除
    /// 缓存的随机数，以便下次请求重新协商。
    pub(super) async fn get_data(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<Value, String> {
        let response = self
            .signed_request(reqwest::Method::GET, path, true)?
            .query(query)
            .send()
            .await
            .map_err(|error| format!("SF App 请求失败：{error}"))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.json::<Value>().await.unwrap_or(Value::Null);
            let api_code = body
                .get("status")
                .and_then(|value| value.get("errorCode"))
                .and_then(Value::as_i64);
            let api_message = body
                .get("status")
                .and_then(|value| value.get("msg"))
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty());
            eprintln!(
                "[sfacg] App request rejected: path={path}, http={}, api_code={api_code:?}, message={api_message:?}",
                status.as_u16(),
            );
            return Err(match api_message {
                Some(message) => format!("SF App 请求返回 HTTP {}：{message}", status.as_u16()),
                None => format!("SF App 请求返回 HTTP {}", status.as_u16()),
            });
        }
        let body = response
            .json::<Value>()
            .await
            .map_err(|error| format!("SF App 响应格式无效：{error}"))?;
        Ok(body.get("data").cloned().unwrap_or(body))
    }

    /// 匿名请求 SF App API 资源，并拆开其 `data` 数据包。
    ///
    /// 该方法仅用于经实测确认可匿名访问的作品发现与元数据端点。协议 Basic
    /// Auth、请求签名和安装设备标识仍是 App API 的必要请求字段；“匿名”仅指
    /// 不附带 `.SFCommunity` 或 `session_APP`。章节正文、书架和账户端点不得
    /// 使用本方法。
    ///
    /// # 错误
    /// 请求失败、上游拒绝请求或响应不是有效 JSON 时返回错误。
    pub(super) async fn get_public_data(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<Value, String> {
        let response = self
            .signed_request(reqwest::Method::GET, path, false)?
            .query(query)
            .send()
            .await
            .map_err(|error| format!("SF 匿名 App 请求失败：{error}"))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.json::<Value>().await.unwrap_or(Value::Null);
            let api_code = body
                .get("status")
                .and_then(|value| value.get("errorCode"))
                .and_then(Value::as_i64);
            let api_message = body
                .get("status")
                .and_then(|value| value.get("msg"))
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty());
            eprintln!(
                "[sfacg] Anonymous App request rejected: path={path}, http={}, api_code={api_code:?}, message={api_message:?}",
                status.as_u16(),
            );
            return Err(match api_message {
                Some(message) => {
                    format!("SF 匿名 App 请求返回 HTTP {}：{message}", status.as_u16())
                }
                None => format!("SF 匿名 App 请求返回 HTTP {}", status.as_u16()),
            });
        }
        let body = response
            .json::<Value>()
            .await
            .map_err(|error| format!("SF 匿名 App 响应格式无效：{error}"))?;
        Ok(body.get("data").cloned().unwrap_or(body))
    }

    /// 发送已签名 JSON POST 请求，并返回拆开的数据载荷。
    ///
    /// 此方法仅供请求体由 App 协议定义的原生层集成调用，任意渲染进程载荷不能到达此处。
    pub(super) async fn post_data(&self, path: &str, body: &Value) -> Result<Value, String> {
        let response = self
            .signed_request(reqwest::Method::POST, path, true)?
            .json(body)
            .send()
            .await
            .map_err(|error| format!("SF App 请求失败：{error}"))?;
        let status = response.status();
        let body = response.json::<Value>().await.unwrap_or(Value::Null);
        let api_http_code = body
            .get("status")
            .and_then(|value| value.get("httpCode"))
            .and_then(Value::as_i64);
        if !status.is_success() || api_http_code.is_some_and(|code| !(200..300).contains(&code)) {
            let api_code = body
                .get("status")
                .and_then(|value| value.get("errorCode"))
                .and_then(Value::as_i64);
            let api_message = body
                .get("status")
                .and_then(|value| value.get("msg"))
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty());
            eprintln!(
                "[sfacg] App request rejected: path={path}, http={}, api_code={api_code:?}, message={api_message:?}",
                status.as_u16(),
            );
            return Err(match api_message {
                Some(message) => format!(
                    "SF App 请求返回 HTTP/API {}：{message}",
                    api_http_code
                        .map_or_else(|| status.as_u16().to_string(), |code| code.to_string())
                ),
                None => format!(
                    "SF App 请求返回 HTTP/API {}",
                    api_http_code
                        .map_or_else(|| status.as_u16().to_string(), |code| code.to_string())
                ),
            });
        }
        Ok(body.get("data").cloned().unwrap_or(body))
    }

    /// 登录后向 App 端点上报当前安装信息。
    ///
    /// 上报仅用于诊断，不能成为登录或其他 App 请求的前置条件。上游字段 `deviceToken`
    /// 与每次安装使用的 UUID 字段 `deviceId` 是彼此独立的客户端协议值。
    pub(super) async fn report_android_device_info(&self, account_id: i64) -> Result<(), String> {
        if account_id <= 0 {
            return Err("SF 账号编号无效".to_string());
        }
        let device_id = device_token()?.to_lowercase();
        self.post_data(
            "/user/androiddeviceinfos",
            &serde_json::json!({
                "accountId": account_id,
                "package": "com.sfacg",
                "abi": "arm64-v8a",
                "deviceId": device_id,
                "version": "4.8.22",
                "deviceToken": "7b2a42976f97d470",
            }),
        )
        .await
        .map(|_| ())
    }

    /// 读取当前 App 账户并尝试提交可选设备诊断信息。
    ///
    /// 设备上报失败不应影响已经成功建立的登录态，因此由调用方决定如何记录失败。
    pub(super) async fn report_android_device_info_for_current_account(
        &self,
    ) -> Result<(), String> {
        let user = self.get_data("/user", &[]).await?;
        let account_id = user
            .get("accountId")
            .and_then(Value::as_i64)
            .ok_or_else(|| "SF App 未返回账户编号".to_string())?;
        self.report_android_device_info(account_id).await
    }

    /// 通过 App 端点登录，并仅返回App Cookie。
    ///
    /// # 错误
    /// 登录失败、上游未返回 App 会话或响应无效时返回错误。
    pub(super) async fn login_with_password(
        &self,
        username: &str,
        password: &str,
    ) -> Result<String, String> {
        let response = self
            .signed_request(reqwest::Method::POST, "/sessions", false)?
            .json(&serde_json::json!({
                "userName": username,
                "passWord": password,
            }))
            .send()
            .await
            .map_err(|error| format!("SF App 登录请求失败：{error}"))?;
        let status = response.status();
        let cookie = filter_cookie_header(
            response
                .headers()
                .get_all(reqwest::header::SET_COOKIE)
                .iter()
                .filter_map(|header| header.to_str().ok())
                .filter_map(|header| header.split(';').next())
                .collect::<Vec<_>>()
                .join("; ")
                .as_str(),
            &[".SFCommunity", "session_APP"],
        );
        let body = response.json::<Value>().await.unwrap_or(Value::Null);
        let successful = status.is_success()
            && body
                .get("status")
                .and_then(|value| value.get("httpCode"))
                .and_then(Value::as_i64)
                == Some(200);
        if !successful {
            return Err(body
                .get("status")
                .and_then(|value| value.get("msg"))
                .and_then(Value::as_str)
                .filter(|message| !message.trim().is_empty())
                .unwrap_or("SF App 账号密码登录失败")
                .to_string());
        }
        cookie.ok_or_else(|| "SF App 登录成功但未返回 App 会话 Cookie".to_string())
    }

    /// 获取小说正文解密所需的 App 章节正文及归属元数据（小说 ID、卷 ID）。
    ///
    /// # 参数
    /// * `chapter_id` - SF App 章节编号。
    ///
    /// # 错误
    /// 章节编号无效、上游未返回正文或缺少作品、分卷标识时返回错误。
    pub(super) async fn get_chapter_content_and_metadata(
        &self,
        chapter_id: i64,
    ) -> Result<ApiChapterContent, String> {
        if chapter_id <= 0 {
            return Err("章节编号无效".to_string());
        }
        let response = self
            .get_data(
                &format!("/Chaps/{chapter_id}"),
                &[("expand", "content,expand.content".to_string())],
            )
            .await?;
        let content = response
            .get("expand")
            .and_then(|value| value.get("content"))
            .and_then(Value::as_str)
            .or_else(|| response.get("content").and_then(Value::as_str))
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "SF App API 未返回章节正文".to_string())?;
        let novel_id = response
            .get("novelId")
            .and_then(Value::as_i64)
            .ok_or_else(|| "SF App API 未返回章节所属作品信息".to_string())?;
        let volume_id = response
            .get("volumeId")
            .and_then(Value::as_i64)
            .ok_or_else(|| "SF App API 未返回章节所属卷信息".to_string())?;
        Ok(ApiChapterContent {
            content: content.to_string(),
            novel_id,
            volume_id,
        })
    }

    /// 获取文字小说的 App 章节目录。
    ///
    /// 目录请求用于章节元数据；正文下载由下载策略决定是否优先使用 App API。
    pub(super) async fn get_chapter_catalog(
        &self,
        novel_id: i64,
    ) -> Result<Vec<ChapterVolume>, String> {
        validate_novel_id(novel_id)?;
        let response = self
            .get_public_data(&format!("/novels/{novel_id}/dirs"), &[])
            .await?;
        Self::parse_chapter_catalog(&response)
    }

    /// 搜索公开小说、有声和漫画，并转换为渲染层使用的记录。
    pub(super) async fn search_novels(&self, query: &str) -> Result<Vec<SearchNovel>, String> {
        let response = self
            .get_public_data(
                "/search/novels/result/new",
                &[
                    ("page", "0".to_string()),
                    ("q", query.to_string()),
                    ("size", "12".to_string()),
                    ("sort", "hot".to_string()),
                    ("searchType", "0".to_string()),
                ],
            )
            .await?;
        let mut results = Vec::new();
        for (key, kind) in [
            ("novels", "novel"),
            ("albums", "audio"),
            ("comics", "comic"),
        ] {
            let Some(items) = response.get(key).and_then(Value::as_array) else {
                continue;
            };
            for item in items {
                let Some(novel_id) = item
                    .get("novelId")
                    .or_else(|| item.get("comicId"))
                    .and_then(Value::as_i64)
                else {
                    continue;
                };
                results.push(SearchNovel {
                    novel_id,
                    media_id: match kind {
                        "audio" => item.get("albumId").and_then(Value::as_i64),
                        "comic" => item.get("comicId").and_then(Value::as_i64),
                        _ => None,
                    },
                    novel_name: item
                        .get("novelName")
                        .or_else(|| item.get("comicName"))
                        .or_else(|| item.get("name"))
                        .and_then(Value::as_str)
                        .unwrap_or("未命名作品")
                        .to_string(),
                    author_name: item
                        .get("authorName")
                        .and_then(Value::as_str)
                        .unwrap_or("未知作者")
                        .to_string(),
                    novel_cover: item
                        .get("novelCover")
                        .or_else(|| item.get("coverBig"))
                        .or_else(|| item.get("comicCover"))
                        .or_else(|| item.get("coverMedium"))
                        .or_else(|| item.get("coverSmall"))
                        .or_else(|| item.get("cover"))
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    last_update_time: item
                        .get("lastUpdateTime")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    bookshelf_name: None,
                    bookshelf_type: Some(kind.to_string()),
                    source_path: (kind == "comic")
                        .then(|| {
                            item.get("folderName")
                                .or_else(|| item.get("sourcePath"))
                                .and_then(Value::as_str)
                                .filter(|value| {
                                    !value.is_empty()
                                        && value.len() <= 100
                                        && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
                                })
                                .map(str::to_string)
                        })
                        .flatten(),
                });
            }
        }
        Ok(results)
    }

    /// 读取并解析公开小说详情。
    pub(super) async fn get_novel_details(&self, novel_id: i64) -> Result<NovelDetail, String> {
        validate_novel_id(novel_id)?;
        let detail = self
            .get_public_data(
                &format!("/novels/{novel_id}"),
                &[("expand", "chapterCount,bigBgBanner,bigNovelCover,typeName,intro,fav,ticket,pointCount,sysTags,totalNeedFireMoney,latestchapter".to_string())],
            )
            .await?;
        let resolved_id = detail
            .get("novelId")
            .and_then(Value::as_i64)
            .ok_or_else(|| "SF 未返回有效小说信息".to_string())?;
        Ok(Self::detail_from_novel_payload(&detail, resolved_id))
    }

    /// 读取并解析公开有声专辑详情，保留其来源小说编号以兼容既有 IPC 返回值。
    pub(super) async fn get_audio_details(
        &self,
        album_id: i64,
        novel_id: i64,
    ) -> Result<NovelDetail, String> {
        validate_novel_id(album_id)?;
        validate_novel_id(novel_id)?;
        let detail = self
            .get_public_data(
                &format!("/albums/{album_id}"),
                &[("expand", "intro,typeName,sysTags,latestchapter".to_string())],
            )
            .await?;
        Ok(Self::detail_from_audio_payload(&detail, novel_id))
    }

    /// 验证当前 App 会话可访问账户接口而不向调用方泄漏响应正文。
    pub(super) async fn verify_authenticated_session(&self) -> Result<(), String> {
        self.get_data("/user", &[("expand", "welfareCoin".to_string())])
            .await
            .map(|_| ())
    }

    /// 读取并解析当前 App 账户资料及余额。
    pub(super) async fn get_user_profile(&self) -> Result<UserProfile, String> {
        let user = self
            .get_data("/user", &[("expand", "welfareCoin".to_string())])
            .await?;
        let money = self.get_data("/user/money", &[]).await?;
        let account_id = user
            .get("accountId")
            .and_then(Value::as_i64)
            .ok_or_else(|| "SF 登录会话可能已失效".to_string())?;
        Ok(UserProfile {
            account_id,
            nick_name: user
                .get("nickName")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("SF 用户")
                .to_string(),
            avatar: user
                .get("avatar")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            app_details_available: true,
            web_details_available: false,
            vip_details_available: true,
            vip_system: "legacy".to_string(),
            welfare_coin: user
                .get("expand")
                .and_then(|value| value.get("welfareCoin"))
                .and_then(Value::as_i64)
                .unwrap_or(0),
            fire_money_remain: money
                .get("fireMoneyRemain")
                .and_then(Value::as_i64)
                .unwrap_or(0),
            coupons_remain: money
                .get("couponsRemain")
                .and_then(Value::as_i64)
                .unwrap_or(0),
            monthly_ticket: 0,
            vip_level: money.get("vipLevel").and_then(Value::as_i64).unwrap_or(0),
            vip_name: String::new(),
        })
    }

    /// 获取公开漫画网址所需的 App 元数据。
    ///
    /// # 参数
    /// * `comic_id` - SF App 漫画编号。
    ///
    /// # 返回
    /// 返回漫画名称与目录标识，供网页目录客户端使用。
    ///
    /// # 错误
    /// 漫画编号无效或上游未返回标题、目录标识时返回错误。
    pub(super) async fn comic_identity(&self, comic_id: i64) -> Result<(String, String), String> {
        validate_novel_id(comic_id)?;
        let detail = self
            .get_public_data(&format!("/comics/{comic_id}"), &[])
            .await?;
        // 漫画名称与目录标识是网页目录客户端的必要信息，必须返回。
        let title = detail
            .get("comicName")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "SF 漫画接口未返回作品名称".to_string())?
            .to_string();
        let folder = detail
            .get("folderName")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "SF 漫画接口未返回目录标识".to_string())?
            .to_string();
        Ok((title, folder))
    }

    /// 获取公开漫画详情，并保留网页目录标识供网页目录客户端使用。
    ///
    /// # 错误
    /// 漫画编号无效、上游请求失败或响应缺少有效作品信息时返回错误。
    pub(super) async fn get_comic_details(&self, comic_id: i64) -> Result<NovelDetail, String> {
        validate_novel_id(comic_id)?;
        let detail = self
            .get_public_data(
                &format!("/comics/{comic_id}"),
                &[(
                    "expand",
                    "intro,typeName,sysTags,chapterCount,latestchapter,fav,ticket,pointCount"
                        .to_string(),
                )],
            )
            .await?;
        let text = |keys: &[&str]| {
            keys.iter().find_map(|key| {
                detail
                    .get(*key)
                    .and_then(Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .map(ToString::to_string)
                    .or_else(|| {
                        detail
                            .get("expand")
                            .and_then(|expand| expand.get(*key))
                            .and_then(Value::as_str)
                            .filter(|value| !value.trim().is_empty())
                            .map(ToString::to_string)
                    })
            })
        };
        let tags = detail
            .get("sysTags")
            .or_else(|| detail.get("tags"))
            .or_else(|| {
                detail
                    .get("expand")
                    .and_then(|expand| expand.get("sysTags").or_else(|| expand.get("tags")))
            })
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(|tag| {
                        tag.as_str()
                            .or_else(|| tag.get("tagName").and_then(Value::as_str))
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .map(ToString::to_string)
                    })
                    .take(8)
                    .collect()
            });
        let expand = detail.get("expand");
        let latest = expand.and_then(|value| {
            value
                .get("latestChapter")
                .or_else(|| value.get("latestchapter"))
        });
        Ok(NovelDetail {
            novel_id: comic_id,
            novel_name: text(&["comicName", "novelName", "name"])
                .unwrap_or_else(|| "未命名漫画".to_string()),
            author_name: text(&["authorName", "author"]).unwrap_or_else(|| "未知作者".to_string()),
            novel_cover: text(&[
                "coverBig",
                "comicCover",
                "coverMedium",
                "coverSmall",
                "novelCover",
                "cover",
            ])
            .unwrap_or_default(),
            last_update_time: text(&["lastUpdateTime", "updateTime"]).unwrap_or_default(),
            description: text(&["intro", "description", "content"])
                .unwrap_or_else(|| "暂无简介".to_string()),
            is_finish: detail
                .get("isFinished")
                .or_else(|| detail.get("isFinish"))
                .and_then(Value::as_bool)
                .unwrap_or(false),
            type_name: Some(
                text(&["typeName", "categoryName"]).unwrap_or_else(|| "漫画".to_string()),
            ),
            tags,
            score: detail.get("point").and_then(Value::as_f64),
            chapter_count: detail
                .get("chapterCount")
                .or_else(|| expand.and_then(|value| value.get("chapterCount")))
                .and_then(Value::as_i64),
            character_count: detail.get("charCount").and_then(Value::as_i64),
            view_count: detail
                .get("visitTimes")
                .or_else(|| detail.get("viewTimes"))
                .and_then(Value::as_i64),
            mark_count: detail.get("markCount").and_then(Value::as_i64),
            point_count: detail
                .get("pointCount")
                .or_else(|| expand.and_then(|value| value.get("pointCount")))
                .and_then(Value::as_i64),
            favorite_count: detail
                .get("favoriteCount")
                .or_else(|| detail.get("fav"))
                .or_else(|| expand.and_then(|value| value.get("fav")))
                .and_then(Value::as_i64),
            ticket_count: detail
                .get("ticket")
                .or_else(|| expand.and_then(|value| value.get("ticket")))
                .and_then(Value::as_i64),
            latest_chapter_title: latest
                .and_then(|value| value.get("title"))
                .and_then(Value::as_str)
                .map(ToString::to_string),
            latest_chapter_time: latest
                .and_then(|value| value.get("addTime"))
                .and_then(Value::as_str)
                .map(ToString::to_string),
            source_path: text(&["folderName"]).filter(|value| {
                !value.is_empty()
                    && value.len() <= 100
                    && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
            }),
        })
    }

    /// 将 App 小说详情载荷转换为稳定 DTO。
    fn detail_from_novel_payload(detail: &Value, novel_id: i64) -> NovelDetail {
        let expand = detail.get("expand");
        let latest = expand.and_then(|value| {
            value
                .get("latestChapter")
                .or_else(|| value.get("latestchapter"))
        });
        NovelDetail {
            novel_id,
            novel_name: detail
                .get("novelName")
                .and_then(Value::as_str)
                .unwrap_or("未命名作品")
                .to_string(),
            author_name: detail
                .get("authorName")
                .and_then(Value::as_str)
                .unwrap_or("未知作者")
                .to_string(),
            novel_cover: detail
                .get("novelCover")
                .and_then(Value::as_str)
                .or_else(|| {
                    expand
                        .and_then(|value| value.get("bigNovelCover"))
                        .and_then(Value::as_str)
                })
                .unwrap_or_default()
                .to_string(),
            last_update_time: detail
                .get("lastUpdateTime")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            description: expand
                .and_then(|value| value.get("intro"))
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("暂无简介")
                .to_string(),
            is_finish: detail
                .get("isFinish")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            type_name: expand
                .and_then(|value| value.get("typeName"))
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(ToString::to_string),
            tags: Self::parse_tags(expand.and_then(|value| value.get("sysTags"))),
            score: detail.get("point").and_then(Value::as_f64),
            chapter_count: expand
                .and_then(|value| value.get("chapterCount"))
                .and_then(Value::as_i64),
            character_count: detail.get("charCount").and_then(Value::as_i64),
            view_count: detail.get("viewTimes").and_then(Value::as_i64),
            mark_count: detail.get("markCount").and_then(Value::as_i64),
            point_count: expand
                .and_then(|value| value.get("pointCount"))
                .and_then(Value::as_i64),
            favorite_count: expand
                .and_then(|value| value.get("fav"))
                .and_then(Value::as_i64),
            ticket_count: expand
                .and_then(|value| value.get("ticket"))
                .and_then(Value::as_i64),
            latest_chapter_title: latest
                .and_then(|value| value.get("title"))
                .and_then(Value::as_str)
                .map(ToString::to_string),
            latest_chapter_time: latest
                .and_then(|value| value.get("addTime"))
                .and_then(Value::as_str)
                .map(ToString::to_string),
            source_path: None,
        }
    }

    /// 将 App 有声详情载荷转换为兼容小说详情 DTO。
    fn detail_from_audio_payload(detail: &Value, novel_id: i64) -> NovelDetail {
        let text = |keys: &[&str]| Self::payload_text(detail, keys);
        let expand = detail.get("expand");
        let latest = expand.and_then(|value| {
            value
                .get("latestChapter")
                .or_else(|| value.get("latestchapter"))
        });
        NovelDetail {
            novel_id,
            novel_name: text(&["name", "albumName", "novelName"])
                .unwrap_or_else(|| "未命名有声作品".to_string()),
            author_name: text(&["authorName", "author", "anchorName"])
                .unwrap_or_else(|| "未知作者".to_string()),
            novel_cover: text(&[
                "coverBig",
                "coverMedium",
                "coverSmall",
                "albumCover",
                "cover",
            ])
            .unwrap_or_default(),
            last_update_time: text(&["lastUpdateTime", "updateTime"]).unwrap_or_default(),
            description: text(&["intro", "description", "content"])
                .unwrap_or_else(|| "暂无简介".to_string()),
            is_finish: detail
                .get("isFinished")
                .or_else(|| detail.get("isFinish"))
                .and_then(Value::as_bool)
                .unwrap_or(false),
            type_name: text(&["typeName", "categoryName"]),
            tags: Self::parse_tags(
                detail
                    .get("sysTags")
                    .or_else(|| detail.get("tags"))
                    .or_else(|| expand.and_then(|value| value.get("sysTags"))),
            ),
            score: detail.get("point").and_then(Value::as_f64),
            chapter_count: detail
                .get("chapterCount")
                .or_else(|| expand.and_then(|value| value.get("chapterCount")))
                .and_then(Value::as_i64),
            character_count: detail.get("charCount").and_then(Value::as_i64),
            view_count: detail
                .get("visitTimes")
                .or_else(|| detail.get("viewTimes"))
                .and_then(Value::as_i64),
            mark_count: detail.get("markCount").and_then(Value::as_i64),
            point_count: detail.get("pointCount").and_then(Value::as_i64),
            favorite_count: detail.get("fav").and_then(Value::as_i64),
            ticket_count: detail.get("ticket").and_then(Value::as_i64),
            latest_chapter_title: latest
                .and_then(|value| value.get("title"))
                .and_then(Value::as_str)
                .map(ToString::to_string),
            latest_chapter_time: latest
                .and_then(|value| value.get("addTime"))
                .and_then(Value::as_str)
                .map(ToString::to_string),
            source_path: None,
        }
    }

    /// 从 App 目录载荷解析分卷和章节，隔离上游字段名差异。
    fn parse_chapter_catalog(response: &Value) -> Result<Vec<ChapterVolume>, String> {
        let volumes = response
            .get("volumeList")
            .or_else(|| response.get("volumes"))
            .and_then(Value::as_array)
            .ok_or_else(|| "SF App 目录未返回分卷列表".to_string())?;
        Ok(volumes
            .iter()
            .filter_map(|volume| {
                let volume_id = volume.get("volumeId").and_then(Value::as_i64).unwrap_or(0);
                (volume_id > 0).then(|| ChapterVolume {
                    volume_id,
                    title: volume
                        .get("title")
                        .and_then(Value::as_str)
                        .unwrap_or("未命名分卷")
                        .to_string(),
                    chapters: volume
                        .get("chapterList")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(|chapter| {
                            let chap_id = chapter.get("chapId").and_then(Value::as_i64)?;
                            let is_vip = chapter
                                .get("isVip")
                                .and_then(Value::as_bool)
                                .unwrap_or(false);
                            Some(ChapterSummary {
                                chap_id,
                                title: chapter
                                    .get("ntitle")
                                    .and_then(Value::as_str)
                                    .or_else(|| chapter.get("title").and_then(Value::as_str))
                                    .unwrap_or("未命名章节")
                                    .to_string(),
                                is_vip,
                                content_kind: if is_vip {
                                    TextContentKind::ImageVip
                                } else {
                                    TextContentKind::Text
                                },
                                access_state: TextAccessState::Unknown,
                                downloaded: false,
                            })
                        })
                        .collect(),
                })
            })
            .collect())
    }

    /// 读取顶层或 `expand` 内的非空文本字段。
    fn payload_text(detail: &Value, keys: &[&str]) -> Option<String> {
        keys.iter().find_map(|key| {
            detail
                .get(*key)
                .or_else(|| detail.get("expand").and_then(|expand| expand.get(*key)))
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(ToString::to_string)
        })
    }

    /// 读取最多八个非空标签，兼容字符串和对象载荷。
    fn parse_tags(value: Option<&Value>) -> Option<Vec<String>> {
        value.and_then(Value::as_array).map(|values| {
            values
                .iter()
                .filter_map(|tag| {
                    tag.as_str()
                        .or_else(|| tag.get("tagName").and_then(Value::as_str))
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToString::to_string)
                })
                .take(8)
                .collect()
        })
    }

    /// 构建 SF `SFSecurity` 请求头，不记录任何凭据材料。
    fn security_header(nonce: &str) -> Result<String, String> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "系统时间无效".to_string())?
            .as_secs();
        let device_token = device_token()?.to_uppercase();
        let digest =
            Md5::digest(format!("{nonce}{timestamp}{device_token}{SF_SECURITY_SALT}").as_bytes());
        Ok(format!(
            "nonce={nonce}&timestamp={timestamp}&devicetoken={device_token}&sign={digest:X}"
        ))
    }

    /// 从原生 App 会话中读取并过滤出 `.SFCommunity` 与 `session_APP`。
    fn read_app_cookie(app: &tauri::AppHandle) -> Result<Option<String>, String> {
        let auth_state = app.state::<AuthSessionState>();
        let session = auth_state
            .session
            .lock()
            .map_err(|_| "登录会话状态不可用".to_string())?;
        Ok(session
            .as_ref()
            .and_then(|session| session.app_cookie.as_deref())
            .and_then(|cookie| filter_cookie_header(cookie, &[".SFCommunity", "session_APP"])))
    }
}

#[cfg(test)]
mod tests {
    use super::AppClient;

    #[test]
    fn parses_app_chapter_catalog_into_transport_independent_dto() {
        let payload = serde_json::json!({
            "volumeList": [{
                "volumeId": 7,
                "title": "第一卷",
                "chapterList": [
                    { "chapId": 11, "ntitle": "普通章节", "isVip": false },
                    { "chapId": 12, "title": "VIP 章节", "isVip": true }
                ]
            }]
        });

        let catalog = AppClient::parse_chapter_catalog(&payload).expect("catalog should parse");

        assert_eq!(catalog.len(), 1);
        assert_eq!(catalog[0].volume_id, 7);
        assert_eq!(catalog[0].chapters.len(), 2);
        assert_eq!(catalog[0].chapters[0].title, "普通章节");
        assert_eq!(catalog[0].chapters[1].title, "VIP 章节");
    }
}
