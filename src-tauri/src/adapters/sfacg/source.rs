//! SFACG 远程内容统一入口。
//!
//! 业务层只调用本模块的业务方法，不直接选择 App 或网页客户端。
//! App 凭证与网页凭证只由底层客户端读取，不能互相转换。
//! 已登录 App 时书架、文字正文与媒体资源只使用 App；仅在没有 App 凭证时回退网页。
//! 有声播放地址、设备登记和 App 余额只接受 App。

use super::app::AppClient;
use super::device::device_profile;
use super::web::WebClient;
use crate::application::ports::RemoteGateway;
use crate::application::state::AuthSessionState;
use crate::domain::auth::UserProfile;
use crate::domain::catalog::{
    AudioChapter, ChapterVolume, ComicChapter, NovelDetail, PublicShelf, SearchNovel,
};
use crate::domain::content::{ApiChapterContent, ImageAsset, TextChapter, TextOrigin};
use crate::domain::error::{AppError, AppResult};
use crate::domain::ids::validate_novel_id;
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
    pub(crate) fn new<R: Runtime>(app: &AppHandle<R>) -> Result<Self, String> {
        let session = app.state::<AuthSessionState>().snapshot()?;
        let app_cookie = session.as_ref().and_then(|value| value.app_cookie.clone());
        let web_cookie = session.as_ref().and_then(|value| value.web_cookie.clone());
        Ok(Self {
            app_client: AppClient::new(device_profile()?, app_cookie)?,
            web_client: WebClient::new(web_cookie)?,
        })
    }

    /// 先同步平台会话，再创建内容入口。
    ///
    /// Android 的 WebView 会话由原生插件持有，必须在读取前同步。
    pub(crate) async fn connect(app: &AppHandle) -> Result<Self, String> {
        #[cfg(target_os = "android")]
        if let Some(session) = crate::adapters::platform::android::synchronize_session(app).await? {
            *app.state::<AuthSessionState>()
                .session
                .lock()
                .map_err(|_| "登录会话状态不可用".to_string())? = Some(session);
        }
        Self::new(app)
    }

    /// 当前快照是否包含 App 会话。
    pub(crate) fn has_app_session(&self) -> bool {
        self.app_client.has_session()
    }

    /// 当前快照是否包含网页会话。
    fn has_web_session(&self) -> bool {
        self.web_client.has_session()
    }

    /// 校验 App 会话并返回 App 客户端。
    ///
    /// # 参数
    /// - `feature`：需要 App 凭证的功能名称，用于生成本地错误提示。
    ///
    /// # 返回
    /// 返回当前快照中的 App 客户端。
    ///
    /// # 错误
    /// 快照不包含有效 App 会话时返回错误。
    fn require_app(&self, feature: &str) -> Result<&AppClient, String> {
        if self.has_app_session() {
            Ok(&self.app_client)
        } else {
            Err(format!("{feature}需要连接 App 凭证，请先完成 App 登录。"))
        }
    }

    /// 校验网页会话并返回网页客户端。
    ///
    /// # 参数
    /// - `feature`：需要网页凭证的功能名称，用于生成本地错误提示。
    ///
    /// # 返回
    /// 返回当前快照中的网页客户端。
    ///
    /// # 错误
    /// 快照不包含有效网页会话时返回错误。
    fn require_web(&self, feature: &str) -> Result<&WebClient, String> {
        if self.has_web_session() {
            Ok(&self.web_client)
        } else {
            Err(format!("{feature}需要连接网站凭证，请先完成官方网页登录。"))
        }
    }

    /// 搜索小说、有声与漫画。
    pub(crate) async fn search(&self, query: &str) -> Result<Vec<SearchNovel>, String> {
        self.app_client.search_novels(query).await
    }

    /// 读取小说详情。
    pub(crate) async fn novel_detail(&self, novel_id: i64) -> Result<NovelDetail, String> {
        self.app_client.get_novel_details(novel_id).await
    }

    /// 读取有声专辑详情。
    pub(crate) async fn audio_detail(
        &self,
        album_id: i64,
        novel_id: i64,
    ) -> Result<NovelDetail, String> {
        self.app_client.get_audio_details(album_id, novel_id).await
    }

    /// 读取漫画详情。
    pub(crate) async fn comic_detail(&self, comic_id: i64) -> Result<NovelDetail, String> {
        self.app_client.get_comic_details(comic_id).await
    }

    /// 读取文字小说目录。
    pub(crate) async fn text_volumes(&self, novel_id: i64) -> Result<Vec<ChapterVolume>, String> {
        self.app_client.get_chapter_catalog(novel_id).await
    }

    /// 反查小说对应的有声专辑编号。
    pub(crate) async fn audio_album_id(
        &self,
        novel_id: i64,
        title_hint: Option<&str>,
    ) -> Result<i64, String> {
        self.app_client
            .find_audio_album_id(novel_id, title_hint)
            .await
    }

    /// 读取有声目录。目录接口允许匿名访问。
    pub(crate) async fn audio_catalog(
        &self,
        album_id: i64,
        title_hint: Option<&str>,
    ) -> Result<(String, Vec<AudioChapter>), String> {
        self.app_client
            .get_audio_catalog(album_id, title_hint)
            .await
    }

    /// 读取单章音频地址。地址由 App 返回，并需要 App 凭证播放。
    pub(crate) async fn audio_source(&self, chapter_id: i64) -> Result<String, String> {
        validate_novel_id(chapter_id)?;
        if !self.has_app_session() {
            return Err("音频地址为空，请登录 App 凭证后播放".to_string());
        }
        self.app_client
            .find_audio_chapter_source(chapter_id)
            .await
            .map_err(|error| format!("读取 App 音频地址失败：{error}"))?
            .ok_or_else(|| "音频地址为空，请确认章节已解锁或重新登录 App 凭证".to_string())
    }

    /// 检查当前会话是否可以使用有声内容。
    pub(crate) fn ensure_audio_access(&self) -> Result<(), String> {
        self.require_app("有声内容").map(|_| ())
    }

    /// 解析漫画标题和来源目录标识。
    pub(crate) async fn comic_source(
        &self,
        comic_id: i64,
        source_path: Option<&str>,
        title_hint: Option<&str>,
    ) -> Result<(String, String), String> {
        validate_novel_id(comic_id)?;
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
        self.app_client.comic_identity(comic_id).await
    }

    /// 读取漫画 App 目录。
    pub(crate) async fn comic_catalog(&self, comic_id: i64) -> Result<Vec<ComicChapter>, String> {
        self.app_client.get_comic_catalog(comic_id).await
    }

    /// 读取漫画章节图片地址。
    pub(crate) async fn comic_images(
        &self,
        comic_id: i64,
        path_name: &str,
    ) -> Result<Vec<String>, String> {
        self.app_client
            .get_comic_chapter_images(comic_id, path_name)
            .await
    }

    /// 读取书架。App 优先，未登录 App 时回退网页。
    pub(crate) async fn bookshelf(&self, force_refresh: bool) -> Result<PublicShelf, String> {
        if self.has_app_session() {
            return self.app_client.get_bookshelf(force_refresh).await;
        }
        let client = self.require_web("网页书架")?;
        client.get_public_bookshelf().await
    }

    /// 读取账户资料。存在 App 凭证时只读取 App，否则读取网页资料。
    pub(crate) async fn account_profile(&self) -> Result<UserProfile, AppError> {
        if self.has_app_session() {
            return match self.app_client.get_user_profile().await {
                Ok(profile) => Ok(profile),
                Err(error) => {
                    let message = error.describe("/user");
                    eprintln!("[sfacg] App profile unavailable: {message}");
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
                    eprintln!("[sfacg] Web profile unavailable: {error}");
                    Err(AppError::upstream(error))
                }
            };
        }
        Err(AppError::unauthenticated("请先连接 App 或网站凭证"))
    }

    /// 验证当前账户会话可用。
    pub(crate) async fn verify_account(&self) -> Result<(), AppError> {
        self.account_profile().await.map(|_| ())
    }

    /// 使用账号密码登录 App，并返回 App 会话 Cookie。
    pub(crate) async fn login_app(&self, username: &str, password: &str) -> Result<String, String> {
        self.app_client
            .login_with_password(username, password)
            .await
    }

    /// 使用指定 App 会话登记当前安装。
    ///
    /// # 参数
    /// - `app_cookie`：本次登录返回的 App Cookie。
    pub(crate) async fn register_device(&self, app_cookie: &str) -> Result<(), String> {
        self.app_client
            .register_device_for_session(app_cookie)
            .await
    }

    /// 有直接正文可用时尝试读取；否则返回 `None`。
    pub(crate) async fn try_direct_text_chapter<F>(
        &self,
        chapter_id: i64,
        decode: F,
    ) -> Option<Result<String, String>>
    where
        F: FnOnce(&str) -> Result<String, String>,
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
                Ok(api) => decode(&api.content),
                Err(error) => Err(error),
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
    ) -> Result<TextChapter, String>
    where
        F: FnOnce(&str) -> Result<String, String>,
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
            })
    }

    /// 读取 VIP 图片正文。
    pub(crate) async fn read_vip_image_chapter(
        &self,
        novel_id: i64,
        chapter_id: i64,
    ) -> Result<ImageAsset, String> {
        if self.has_app_session() {
            return Err("已连接 App 凭证，VIP 图片正文不再回退网页".to_string());
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
///
/// # 参数
/// - `value`：待校验的目录标识。
fn is_comic_folder(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

/// 将远程适配层错误转换为应用层上游错误。
///
/// # 参数
/// - `message`：远程客户端返回的错误说明。
fn remote_error(message: String) -> AppError {
    AppError::upstream(message)
}

#[async_trait]
impl RemoteGateway for SfSource {
    async fn search(&self, query: &str) -> AppResult<Vec<SearchNovel>> {
        SfSource::search(self, query).await.map_err(remote_error)
    }

    async fn novel_detail(&self, novel_id: i64) -> AppResult<NovelDetail> {
        SfSource::novel_detail(self, novel_id)
            .await
            .map_err(remote_error)
    }

    async fn audio_detail(&self, album_id: i64, novel_id: i64) -> AppResult<NovelDetail> {
        SfSource::audio_detail(self, album_id, novel_id)
            .await
            .map_err(remote_error)
    }

    async fn comic_detail(&self, comic_id: i64) -> AppResult<NovelDetail> {
        SfSource::comic_detail(self, comic_id)
            .await
            .map_err(remote_error)
    }

    async fn text_volumes(&self, novel_id: i64) -> AppResult<Vec<ChapterVolume>> {
        SfSource::text_volumes(self, novel_id)
            .await
            .map_err(remote_error)
    }

    async fn audio_album_id(&self, novel_id: i64, title_hint: Option<&str>) -> AppResult<i64> {
        SfSource::audio_album_id(self, novel_id, title_hint)
            .await
            .map_err(remote_error)
    }

    async fn audio_catalog(
        &self,
        album_id: i64,
        title_hint: Option<&str>,
    ) -> AppResult<(String, Vec<AudioChapter>)> {
        SfSource::audio_catalog(self, album_id, title_hint)
            .await
            .map_err(remote_error)
    }

    async fn comic_source(
        &self,
        comic_id: i64,
        source_path: Option<&str>,
        title_hint: Option<&str>,
    ) -> AppResult<(String, String)> {
        SfSource::comic_source(self, comic_id, source_path, title_hint)
            .await
            .map_err(remote_error)
    }

    async fn comic_catalog(&self, comic_id: i64) -> AppResult<Vec<ComicChapter>> {
        SfSource::comic_catalog(self, comic_id)
            .await
            .map_err(remote_error)
    }

    async fn bookshelf(&self, force_refresh: bool) -> AppResult<PublicShelf> {
        SfSource::bookshelf(self, force_refresh)
            .await
            .map_err(remote_error)
    }

    async fn account_profile(&self) -> AppResult<UserProfile> {
        SfSource::account_profile(self).await
    }

    async fn verify_account(&self) -> AppResult<()> {
        SfSource::verify_account(self).await
    }

    async fn login_app(&self, username: &str, password: &str) -> AppResult<String> {
        SfSource::login_app(self, username, password)
            .await
            .map_err(remote_error)
    }

    async fn register_device(&self, app_cookie: &str) -> AppResult<()> {
        SfSource::register_device(self, app_cookie)
            .await
            .map_err(remote_error)
    }

    async fn app_chapter_content(&self, chapter_id: i64) -> AppResult<ApiChapterContent> {
        self.require_app("正文对齐")
            .map_err(remote_error)?
            .get_chapter_content_and_metadata(chapter_id)
            .await
            .map_err(remote_error)
    }

    async fn web_chapter_content(
        &self,
        novel_id: i64,
        volume_id: i64,
        chapter_id: i64,
    ) -> AppResult<String> {
        self.web_client
            .chapter_content(novel_id, volume_id, chapter_id)
            .await
            .map_err(remote_error)
    }
}
