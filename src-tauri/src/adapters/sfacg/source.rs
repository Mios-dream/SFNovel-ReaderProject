//! SFACG 远程内容统一入口。
//!
//! 业务层只调用本模块的业务方法，不直接选择 App 或网页客户端。
//! App 凭证与网页凭证只由底层客户端读取，不能互相转换。
//! 共用能力优先使用 App。App 未返回结果时，文字正文、有声目录和音频地址回退网页。
//! 设备登记和 App 余额只接受 App；漫画、网页书架和 VIP 图片正文只接受网页。

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
            app_client: AppClient::new(device_profile()?.clone(), app_cookie)?,
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
    fn has_app_session(&self) -> bool {
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

    /// 读取有声目录，优先使用 App，失败后回退网页目录页。
    ///
    /// App 分卷目录可匿名读取；有 App 会话时同时返回该账号的解锁状态。
    /// 网页目录页没有这些标记，只能在取音频地址时确认章节是否可播放。
    pub(crate) async fn audio_catalog(
        &self,
        album_id: i64,
        title_hint: Option<&str>,
    ) -> Result<(String, Vec<AudioChapter>), String> {
        let app_error = match self
            .app_client
            .get_audio_catalog(album_id, title_hint)
            .await
        {
            Ok(catalog) => return Ok(catalog),
            Err(error) => error,
        };
        self.web_client
            .get_audio_catalog(album_id, title_hint)
            .await
            .map_err(|web_error| {
                format!("App 有声目录不可用：{app_error}；网页目录不可用：{web_error}")
            })
    }

    /// 检查当前会话是否具备读取有声内容的条件。
    pub(crate) fn ensure_audio_catalog(&self) -> Result<(), String> {
        if self.has_app_session() || self.has_web_session() {
            Ok(())
        } else {
            Err("有声内容需要连接 App 或网站凭证，请先登录。".to_string())
        }
    }

    /// 读取单章音频地址，优先使用 App，失败后回退网页。
    ///
    /// App 章节详情可匿名请求；未购章节不会返回 `fileName`，此时再回退网页。
    pub(crate) async fn audio_source(&self, chapter_id: i64) -> Result<String, String> {
        validate_novel_id(chapter_id)?;
        let app_error = match self.app_client.find_audio_chapter_source(chapter_id).await {
            Ok(Some(source)) => return Ok(source),
            Ok(None) => "App 未返回音频地址".to_string(),
            Err(error) => error,
        };
        match self.require_web("网页有声地址") {
            Ok(client) => client
                .get_audio_chapter_source(chapter_id)
                .await
                .map_err(|web_error| format!("{app_error}；网页回退失败：{web_error}")),
            Err(web_error) => Err(format!("{app_error}；{web_error}")),
        }
    }

    /// 解析漫画标题和网页目录标识。
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

    /// 读取漫画网页目录。
    pub(crate) async fn comic_catalog(&self, folder: &str) -> Result<Vec<ComicChapter>, String> {
        self.web_client.get_comic_catalog(folder).await
    }

    /// 读取漫画章节图片地址。
    pub(crate) async fn comic_images(
        &self,
        comic_id: i64,
        chapter_id: i64,
    ) -> Result<Vec<String>, String> {
        self.web_client
            .comic_chapter_images(comic_id, chapter_id)
            .await
    }

    /// 读取网页书架。
    pub(crate) async fn bookshelf(&self) -> Result<PublicShelf, String> {
        let client = self.require_web("网页书架")?;
        client.get_public_bookshelf().await
    }

    /// 读取账户资料。App 资料优先，网页资料补齐月票和 VIP。
    pub(crate) async fn account_profile(&self) -> Result<UserProfile, String> {
        let app_profile = if self.has_app_session() {
            match self.app_client.get_user_profile().await {
                Ok(profile) => Some(profile),
                Err(error) => {
                    eprintln!("[sfacg] App profile unavailable: {error}");
                    None
                }
            }
        } else {
            None
        };
        let web_profile = if self.has_web_session() {
            match self.web_client.get_user_profile().await {
                Ok(profile) => Some(profile),
                Err(error) => {
                    eprintln!("[sfacg] Web profile unavailable: {error}");
                    None
                }
            }
        } else {
            None
        };
        match (app_profile, web_profile) {
            (Some(app), Some(web)) => Ok(merge_profiles(app, web)),
            (Some(profile), None) | (None, Some(profile)) => Ok(profile),
            (None, None) if !self.has_app_session() && !self.has_web_session() => {
                Err("请先连接 App 或网站凭证".to_string())
            }
            (None, None) => Err("当前登录会话已失效，请重新登录".to_string()),
        }
    }

    /// 验证当前账户会话可用。
    pub(crate) async fn verify_account(&self) -> Result<(), String> {
        self.account_profile().await.map(|_| ())
    }

    /// 使用账号密码登录 App，并返回 App 会话 Cookie。
    pub(crate) async fn login_app(&self, username: &str, password: &str) -> Result<String, String> {
        self.app_client
            .login_with_password(username, password)
            .await
    }

    /// 登记当前安装，取得 App API 的设备权限。
    pub(crate) async fn register_device(&self) -> Result<(), String> {
        self.require_app("设备登记")?.register_device().await
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

    /// 读取普通文字正文。App 优先，失败后回退网页。
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
        let app_error = match self.try_direct_text_chapter(chapter_id, decode).await {
            Some(Ok(content)) => {
                return Ok(TextChapter {
                    content,
                    origin: TextOrigin::App,
                })
            }
            Some(Err(error)) => Some(error),
            None => None,
        };
        match self
            .web_client
            .chapter_content(novel_id, volume_id, chapter_id)
            .await
        {
            Ok(content) => Ok(TextChapter {
                content,
                origin: TextOrigin::Web,
            }),
            Err(web_error) => Err(match app_error {
                Some(app_error) => {
                    format!("App 正文不可用：{app_error}；网页正文不可用：{web_error}")
                }
                None => web_error,
            }),
        }
    }

    /// 读取 VIP 图片正文。
    pub(crate) async fn read_vip_image_chapter(
        &self,
        novel_id: i64,
        chapter_id: i64,
    ) -> Result<ImageAsset, String> {
        let client = self.require_web("网页 VIP 图片正文")?;
        let image = client.vip_chapter_image(novel_id, chapter_id).await?;
        Ok(ImageAsset {
            bytes: image.bytes,
            extension: image.extension,
        })
    }

    /// 创建网页资源请求。所有静态资源和媒体只使用网页凭证。
    pub(crate) fn asset_request(
        &self,
        url: impl reqwest::IntoUrl,
        referer: &str,
        accept: &str,
    ) -> reqwest::RequestBuilder {
        self.web_client.asset_request(url, referer, accept)
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

/// 将网页侧可选的展示资料合并到 App 账户资料。
///
/// App 资料优先；仅空值、网页余额与 VIP 补充字段会采用网页来源。
///
/// # 参数
/// - `app`：App API 返回的账户资料。
/// - `web`：网页端返回的补充资料。
///
/// # 返回
/// 返回合并后的账户资料。
fn merge_profiles(mut app: UserProfile, web: UserProfile) -> UserProfile {
    app.web_details_available = web.web_details_available;
    app.monthly_ticket = web.monthly_ticket;
    if web.vip_details_available {
        app.vip_system = web.vip_system;
        app.vip_level = web.vip_level;
        app.vip_name = web.vip_name;
        app.vip_details_available = true;
    }
    if app.account_id == 0 {
        app.account_id = web.account_id;
    }
    if app.nick_name.trim().is_empty() {
        app.nick_name = web.nick_name;
    }
    if app.avatar.trim().is_empty() {
        app.avatar = web.avatar;
    }
    app
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

    async fn comic_catalog(&self, folder: &str) -> AppResult<Vec<ComicChapter>> {
        SfSource::comic_catalog(self, folder)
            .await
            .map_err(remote_error)
    }

    async fn bookshelf(&self) -> AppResult<PublicShelf> {
        SfSource::bookshelf(self).await.map_err(remote_error)
    }

    async fn account_profile(&self) -> AppResult<UserProfile> {
        SfSource::account_profile(self).await.map_err(remote_error)
    }

    async fn verify_account(&self) -> AppResult<()> {
        SfSource::verify_account(self).await.map_err(remote_error)
    }

    async fn login_app(&self, username: &str, password: &str) -> AppResult<String> {
        SfSource::login_app(self, username, password)
            .await
            .map_err(remote_error)
    }

    async fn register_device(&self) -> AppResult<()> {
        SfSource::register_device(self).await.map_err(remote_error)
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
