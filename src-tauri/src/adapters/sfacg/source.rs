//! SFACG 远程内容统一入口。
//!
//! 业务层只调用本模块的业务方法，不直接选择 App 或网页客户端。
//! App 凭证与网页凭证只由底层客户端读取，不能互相转换。
//! 已登录 App 时书架、文字正文与媒体资源只使用 App；仅在没有 App 凭证时回退网页。
//! 有声播放地址、设备登记和 App 余额只接受 App。
//!
//! `RemoteGateway` 的 27 个业务方法在这里直接实现，不再额外保留一层同名的
//! `Result<_, String>` 版本；错误只在出口处转换一次。

use super::app::AppClient;
use super::web::WebClient;
use crate::application::ports::RemoteGateway;
use crate::application::state::AuthSessionState;
use crate::domain::auth::UserProfile;
use crate::domain::catalog::{
    AudioChapter, BookCommentPage, BookCommentReplyPage, ChapterVolume, ComicChapter, DiscoverFeed,
    NovelDetail, PublicShelf, SearchNovel,
};
use crate::domain::content::{ApiChapterContent, ImageAsset, TextChapter, TextOrigin};
use crate::domain::error::{AppError, AppResult};
use crate::domain::ids::{validate_chapter_id, validate_comic_id, NovelId};
use crate::domain::signin::{SigninContinueDays, SigninDay, SigninReward, SigninWeekItem};
use async_trait::async_trait;
use tauri::{AppHandle, Manager, Runtime};

/// 同时持有 App 与网页客户端快照的统一内容入口。
pub(crate) struct SfSource {
    /// 使用 App 凭证快照的签名 API 客户端。
    app_client: AppClient,
    /// 使用网页凭证快照的页面与静态资源客户端。
    web_client: WebClient,
}

impl SfSource {
    /// 创建内容入口。构造时各客户端只读取自己对应的凭证。
    pub(crate) fn new<R: Runtime>(app: &AppHandle<R>) -> AppResult<Self> {
        let session = app.state::<AuthSessionState>().snapshot()?;
        let app_cookie = session.as_ref().and_then(|value| value.app_cookie.clone());
        let web_cookie = session.as_ref().and_then(|value| value.web_cookie.clone());
        Ok(Self {
            app_client: AppClient::new(app_cookie).map_err(AppError::from)?,
            web_client: WebClient::new(web_cookie)?,
        })
    }

    /// 先同步平台会话，再创建内容入口。
    ///
    /// Android 的 WebView 会话由原生插件持有，必须在读取前同步。
    pub(crate) async fn connect(app: &AppHandle) -> AppResult<Self> {
        #[cfg(target_os = "android")]
        if let Some(session) = crate::adapters::platform::android::synchronize_session(app).await? {
            *app.state::<AuthSessionState>()
                .session
                .lock()
                .map_err(|_| AppError::internal("登录会话状态不可用"))? = Some(session);
        }
        Self::new(app)
    }

    /// 当前快照是否包含 App 会话。
    pub(crate) fn has_app_session(&self) -> bool {
        self.app_client.has_session()
    }

    /// 当前快照的 App 会话 Cookie，用于按会话区分本地签到记录。
    pub(crate) fn app_cookie(&self) -> Option<&str> {
        self.app_client.session_cookie()
    }

    /// 当前快照是否包含网页会话。
    fn has_web_session(&self) -> bool {
        self.web_client.has_session()
    }

    /// 校验 App 会话并返回 App 客户端。
    ///
    /// `feature` 用于生成本地错误提示，说明哪个功能需要 App 凭证。
    fn require_app(&self, feature: &str) -> AppResult<&AppClient> {
        if self.has_app_session() {
            Ok(&self.app_client)
        } else {
            Err(AppError::unauthenticated(format!(
                "{feature}需要连接 App 凭证，请先完成 App 登录。"
            )))
        }
    }

    /// 校验网页会话并返回网页客户端。
    fn require_web(&self, feature: &str) -> AppResult<&WebClient> {
        if self.has_web_session() {
            Ok(&self.web_client)
        } else {
            Err(AppError::unauthenticated(format!(
                "{feature}需要连接网站凭证，请先完成官方网页登录。"
            )))
        }
    }

    /// 读取单章音频地址。地址由 App 返回，并需要 App 凭证播放。
    pub(crate) async fn audio_source(&self, chapter_id: i64) -> AppResult<String> {
        validate_chapter_id(chapter_id)?;
        if !self.has_app_session() {
            return Err(AppError::unauthenticated(
                "音频地址为空，请登录 App 凭证后播放",
            ));
        }
        self.app_client
            .find_audio_chapter_source(chapter_id)
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| {
                AppError::upstream("音频地址为空，请确认章节已解锁或重新登录 App 凭证")
            })
    }

    /// 检查当前会话是否可以使用有声内容。
    pub(crate) fn ensure_audio_access(&self) -> AppResult<()> {
        self.require_app("有声内容").map(|_| ())
    }

    /// 读取漫画章节图片地址。
    pub(crate) async fn comic_images(
        &self,
        comic_id: i64,
        path_name: &str,
    ) -> AppResult<Vec<String>> {
        self.app_client
            .get_comic_chapter_images(comic_id, path_name)
            .await
            .map_err(AppError::from)
    }

    /// 有直接正文可用时尝试读取；否则返回 `None`。
    pub(crate) async fn try_direct_text_chapter<F>(
        &self,
        chapter_id: i64,
        decode: F,
    ) -> Option<AppResult<String>>
    where
        F: FnOnce(&str, bool) -> AppResult<String>,
    {
        if !self.has_app_session() {
            return None;
        }
        Some(
            match self
                .app_client
                .get_chapter_content_and_metadata(chapter_id)
                .await
            {
                Ok(api) => decode(&api.content, api.is_content_encrypted),
                Err(error) => Err(AppError::from(error)),
            },
        )
    }

    /// 读取普通文字正文。有 App 凭证时只使用 App，否则回退网页。
    pub(crate) async fn read_chapter_text<F>(
        &self,
        novel_id: i64,
        volume_id: i64,
        chapter_id: i64,
        decode: F,
    ) -> AppResult<TextChapter>
    where
        F: FnOnce(&str, bool) -> AppResult<String>,
    {
        if let Some(result) = self.try_direct_text_chapter(chapter_id, decode).await {
            return result.map(|content| TextChapter {
                content,
                origin: TextOrigin::App,
            });
        }
        self.web_client
            .chapter_content(novel_id, volume_id, chapter_id)
            .await
            .map(|content| TextChapter {
                content,
                origin: TextOrigin::Web,
            })}

    /// 读取 VIP 图片正文。
    pub(crate) async fn read_vip_image_chapter(
        &self,
        novel_id: i64,
        chapter_id: i64,
    ) -> AppResult<ImageAsset> {
        if self.has_app_session() {
            return Err(AppError::internal(
                "已连接 App 凭证，VIP 图片正文不再回退网页",
            ));
        }
        let client = self.require_web("网页 VIP 图片正文")?;
        let image = client.vip_chapter_image(novel_id, chapter_id).await?;
        Ok(ImageAsset {
            bytes: image.bytes,
            extension: image.extension,
        })
    }

    /// 创建封面或正文插图请求。有 App 凭证时使用 App，否则使用网页。
    pub(crate) fn asset_request(
        &self,
        url: &str,
        referer: &str,
        accept: &str,
    ) -> reqwest::RequestBuilder {
        if self.has_app_session() {
            self.app_client.asset_request(url, referer, accept)
        } else {
            self.web_client.asset_request(url, referer, accept)
        }
    }

    /// 创建 App 音频资源请求。
    pub(crate) fn audio_asset_request(
        &self,
        url: &str,
        referer: &str,
        accept: &str,
    ) -> reqwest::RequestBuilder {
        self.app_client.asset_request(url, referer, accept)
    }
}

/// 判断字符串是否可作为安全的 SF 漫画目录标识。
fn is_comic_folder(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

#[async_trait]
impl RemoteGateway for SfSource {
    async fn search(&self, query: &str) -> AppResult<Vec<SearchNovel>> {
        self.app_client
            .search_novels(query)
            .await
            .map_err(AppError::from)
    }

    async fn discover(&self, type_id: Option<i64>) -> AppResult<DiscoverFeed> {
        self.app_client
            .get_discover_feed(type_id)
            .await
            .map_err(AppError::from)
    }

    async fn book_comments(
        &self,
        kind: &str,
        novel_id: i64,
        media_id: Option<i64>,
        page: i64,
        size: i64,
        reviews: bool,
    ) -> AppResult<BookCommentPage> {
        self.app_client
            .get_book_comments(kind, novel_id, media_id, page, size, reviews)
            .await
            .map_err(AppError::from)
    }

    async fn comment_replies(
        &self,
        kind: &str,
        comment_id: i64,
        reviews: bool,
        page: i64,
        size: i64,
    ) -> AppResult<BookCommentReplyPage> {
        self.app_client
            .get_comment_replies(kind, comment_id, reviews, page, size)
            .await
            .map_err(AppError::from)
    }

    async fn novel_detail(&self, novel_id: NovelId) -> AppResult<NovelDetail> {
        self.app_client
            .get_novel_details(novel_id.get())
            .await
            .map_err(AppError::from)
    }

    async fn audio_detail(&self, album_id: i64, novel_id: NovelId) -> AppResult<NovelDetail> {
        self.app_client
            .get_audio_details(album_id, novel_id.get())
            .await
            .map_err(AppError::from)
    }

    async fn comic_detail(&self, comic_id: i64) -> AppResult<NovelDetail> {
        self.app_client
            .get_comic_details(comic_id)
            .await
            .map_err(AppError::from)
    }

    async fn text_volumes(&self, novel_id: NovelId) -> AppResult<Vec<ChapterVolume>> {
        self.app_client
            .get_chapter_catalog(novel_id.get())
            .await
            .map_err(AppError::from)
    }

    async fn audio_album_id(&self, novel_id: NovelId, title_hint: Option<&str>) -> AppResult<i64> {
        self.app_client
            .find_audio_album_id(novel_id.get(), title_hint)
            .await
            .map_err(AppError::from)
    }

    async fn audio_catalog(
        &self,
        album_id: i64,
        title_hint: Option<&str>,
    ) -> AppResult<(String, Vec<AudioChapter>)> {
        self.app_client
            .get_audio_catalog(album_id, title_hint)
            .await
            .map_err(AppError::from)
    }

    async fn comic_source(
        &self,
        comic_id: i64,
        source_path: Option<&str>,
        title_hint: Option<&str>,
    ) -> AppResult<(String, String)> {
        validate_comic_id(comic_id)?;
        if let Some(folder) = source_path.filter(|value| is_comic_folder(value)) {
            return Ok((
                title_hint
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToString::to_string)
                    .unwrap_or_else(|| format!("漫画 {comic_id}")),
                folder.to_string(),
            ));
        }
        self.app_client
            .comic_identity(comic_id)
            .await
            .map_err(AppError::from)
    }

    async fn comic_catalog(&self, comic_id: i64) -> AppResult<Vec<ComicChapter>> {
        self.app_client
            .get_comic_catalog(comic_id)
            .await
            .map_err(AppError::from)
    }

    async fn bookshelf(&self, force_refresh: bool) -> AppResult<PublicShelf> {
        if self.has_app_session() {
            return self
                .app_client
                .get_bookshelf(force_refresh)
                .await
                .map_err(AppError::from);
        }
        let client = self.require_web("网页书架")?;
        client.get_public_bookshelf().await}

    async fn add_favorite(
        &self,
        kind: &str,
        novel_id: i64,
        media_id: Option<i64>,
    ) -> AppResult<()> {
        self.require_app("收藏")?
            .add_favorite(kind, novel_id, media_id)
            .await
            .map_err(AppError::from)
    }

    async fn remove_favorite(
        &self,
        kind: &str,
        novel_id: i64,
        media_id: Option<i64>,
    ) -> AppResult<()> {
        self.require_app("取消收藏")?
            .remove_favorite(kind, novel_id, media_id)
            .await
            .map_err(AppError::from)
    }

    async fn account_profile(&self) -> AppResult<UserProfile> {
        if self.has_app_session() {
            return match self.app_client.get_user_profile().await {
                Ok(profile) => Ok(profile),
                Err(error) => {
                    let message = error.describe("/user");
                    log_warn!("[sfacg] App profile unavailable: {message}");
                    if error.is_session_expired() {
                        Err(AppError::unauthenticated(
                            "SF App 登录已失效，已自动退出，请重新登录 App。",
                        ))
                    } else {
                        Err(AppError::upstream(message))
                    }
                }
            };
        }
        if self.has_web_session() {
            return match self.web_client.get_user_profile().await {
                Ok(profile) => Ok(profile),
                Err(error) => {
                    log_warn!("[sfacg] Web profile unavailable: {error}");
                    Err(error)
                }
            };
        }
        Err(AppError::unauthenticated("请先连接 App 或网站凭证"))
    }

    async fn signin_month(&self, year: i64, month: i64) -> AppResult<Vec<SigninDay>> {
        self.require_app("签到")?
            .get_signin_month(year, month)
            .await
            .map_err(AppError::from)
    }

    async fn signin_continue_days(&self) -> AppResult<SigninContinueDays> {
        self.require_app("签到")?
            .get_signin_continue_days()
            .await
            .map_err(AppError::from)
    }

    async fn signin_week(&self) -> AppResult<Vec<SigninWeekItem>> {
        self.require_app("签到")?
            .get_signin_week()
            .await
            .map_err(AppError::from)
    }

    async fn signin_rewards(&self) -> AppResult<Vec<SigninReward>> {
        self.require_app("签到")?
            .get_signin_rewards()
            .await
            .map_err(AppError::from)
    }

    async fn submit_signin(&self, date: &str) -> AppResult<()> {
        self.require_app("签到")?
            .submit_signin(date)
            .await
            .map_err(AppError::from)
    }

    async fn claim_signin_reward(&self, config_id: i64) -> AppResult<()> {
        self.require_app("签到")?
            .claim_signin_reward(config_id)
            .await
            .map_err(AppError::from)
    }

    async fn verify_account(&self) -> AppResult<()> {
        self.account_profile().await.map(|_| ())
    }

    async fn login_app(&self, username: &str, password: &str) -> AppResult<String> {
        self.app_client
            .login_with_password(username, password)
            .await
            .map_err(AppError::from)
    }

    async fn register_device(&self, app_cookie: &str) -> AppResult<()> {
        self.app_client
            .register_device_for_session(app_cookie)
            .await
            .map_err(AppError::from)
    }

    async fn app_chapter_content(&self, chapter_id: i64) -> AppResult<ApiChapterContent> {
        self.require_app("正文对齐")?
            .get_chapter_content_and_metadata(chapter_id)
            .await
            .map_err(AppError::from)
    }

    async fn web_chapter_content(
        &self,
        novel_id: NovelId,
        volume_id: i64,
        chapter_id: i64,
    ) -> AppResult<String> {
        self.web_client
            .chapter_content(novel_id.get(), volume_id, chapter_id)
            .await}
}
