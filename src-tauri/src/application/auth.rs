//! 登录、登出与会话刷新用例。

use crate::application::ports::{AuthGateway, RemoteGateway};
use crate::application::state::AuthSessionState;
use crate::domain::auth::{AuthSession, AuthStatus, UserProfile};
use crate::domain::error::{AppError, AppResult};

/// 协调远程认证、平台会话持久化和进程内会话状态的认证用例。
pub(crate) struct AuthService<'a, R: RemoteGateway, G: AuthGateway> {
    /// 上游账号与设备接口。
    remote: &'a R,
    /// 平台会话同步及持久化接口。
    gateway: &'a G,
    /// 当前进程共享的登录状态。
    state: &'a AuthSessionState,
}

impl<'a, R: RemoteGateway, G: AuthGateway> AuthService<'a, R, G> {
    /// 创建认证服务。
    ///
    /// # 参数
    /// - `remote`：远程 SF 账号接口实现。
    /// - `gateway`：平台会话存储与官方登录实现。
    /// - `state`：应用共享会话状态。
    pub(crate) fn new(remote: &'a R, gateway: &'a G, state: &'a AuthSessionState) -> Self {
        Self {
            remote,
            gateway,
            state,
        }
    }

    /// 从平台持久化层同步会话，并返回当前认证快照。
    ///
    /// # 错误
    /// 平台同步失败或会话锁不可用时返回错误。
    pub(crate) async fn status(&self) -> AppResult<AuthStatus> {
        if let Some(session) = self.gateway.synchronize().await? {
            self.replace_session(Some(session))?;
        }
        self.status_snapshot()
    }

    /// 使用 SF 账号密码登录 App 会话。
    ///
    /// 登录成功后会保留已有 Web 会话，异步设备注册失败只记录日志。
    ///
    /// # 参数
    /// - `username`：去除首尾空白后不超过 128 个字符的账号。
    /// - `password`：不超过 512 个字符的密码。
    ///
    /// # 错误
    /// 凭据为空、长度非法、上游拒绝登录或会话持久化失败时返回错误。
    pub(crate) async fn login_with_password(
        &self,
        username: &str,
        password: &str,
    ) -> AppResult<AuthStatus> {
        let username = username.trim();
        if username.is_empty() || password.is_empty() {
            return Err(AppError::invalid_input("请输入 SF 账号和密码"));
        }
        if username.chars().count() > 128 || password.chars().count() > 512 {
            return Err(AppError::invalid_input("账号或密码长度无效"));
        }
        let cookie = self.remote.login_app(username, password).await?;
        self.gateway.persist_app_session(&cookie, username).await?;
        {
            let mut current = self
                .state
                .session
                .lock()
                .map_err(|_| AppError::internal("登录会话状态不可用"))?;
            let web_cookie = current.as_ref().and_then(|value| value.web_cookie.clone());
            *current = Some(AuthSession {
                app_cookie: Some(cookie.clone()),
                web_cookie,
                user_name: username.to_string(),
            });
        }
        self.state.clear_profile_cache();
        if let Err(error) = self.remote.register_device(&cookie).await {
            eprintln!("[sfacg] device registration after login failed: {error}");
        }
        self.status_snapshot()
    }

    /// 启动官方网页登录流程并合并返回的 Web 会话。
    ///
    /// # 错误
    /// 官方流程失败、未返回 Web Cookie 或会话持久化失败时返回错误。
    pub(crate) async fn start_official_login(&self) -> AppResult<()> {
        let Some(incoming) = self.gateway.start_official_login().await? else {
            return Ok(());
        };
        let Some(web_cookie) = incoming.web_cookie.as_deref() else {
            return Err(AppError::unauthenticated("官方登录未返回网页会话"));
        };
        self.gateway.persist_web_session(web_cookie).await?;
        self.merge_web_session(incoming)?;
        self.state.clear_profile_cache();
        Ok(())
    }

    /// 同时清除 App、Web 会话和平台持久化数据。
    ///
    /// # 错误
    /// 平台清理失败或进程内会话状态不可用时返回错误。
    pub(crate) async fn logout(&self) -> AppResult<()> {
        self.gateway.clear_all().await?;
        self.state.clear_profile_cache();
        self.replace_session(None)
    }

    /// 仅清除 App 会话；若 Web 会话仍存在则保留登录状态。
    ///
    /// # 错误
    /// App 会话清理失败或状态同步失败时返回错误。
    pub(crate) async fn logout_app(&self) -> AppResult<AuthStatus> {
        self.clear_app_session().await?;
        self.status_snapshot()
    }

    /// 仅清除 Web 会话；若 App 会话仍存在则保留登录状态。
    ///
    /// # 错误
    /// Web 会话清理失败或状态同步失败时返回错误。
    pub(crate) async fn logout_web(&self) -> AppResult<AuthStatus> {
        self.gateway.clear_web().await?;
        self.state.clear_profile_cache();
        {
            let mut current = self
                .state
                .session
                .lock()
                .map_err(|_| AppError::internal("登录会话状态不可用"))?;
            if let Some(session) = current.as_mut() {
                session.web_cookie = None;
            }
            if current
                .as_ref()
                .is_some_and(|session| session.app_cookie.is_none())
            {
                *current = None;
            }
        }
        self.status_snapshot()
    }

    /// 通过一次真实账号请求验证当前会话是否仍有效。
    ///
    /// # 错误
    /// 会话失效、上游请求失败或状态读取失败时返回错误。
    pub(crate) async fn verify(&self) -> AppResult<AuthStatus> {
        if let Err(error) = self.remote.verify_account().await {
            if error.is_unauthenticated()
                && self
                    .state
                    .status()
                    .map_err(AppError::internal)?
                    .app_authenticated
            {
                self.clear_app_session().await?;
            }
            return Err(error);
        }
        self.status_snapshot()
    }

    /// 读取账户资料，并用昵称刷新进程内会话名称。
    ///
    /// # 错误
    /// 未登录或上游资料接口失败时返回错误。
    pub(crate) async fn profile(&self) -> AppResult<UserProfile> {
        if let Some(profile) = self.state.cached_profile() {
            self.remember_profile_name(&profile);
            return Ok(profile);
        }
        let profile = match self.remote.account_profile().await {
            Ok(profile) => profile,
            Err(error) => {
                if error.is_unauthenticated()
                    && self
                        .state
                        .status()
                        .map_err(AppError::internal)?
                        .app_authenticated
                {
                    self.clear_app_session().await?;
                }
                return Err(error);
            }
        };
        self.state.store_profile(&profile);
        self.remember_profile_name(&profile);
        Ok(profile)
    }

    /// 清除 App 会话，并保留仍然存在的 Web 会话。
    async fn clear_app_session(&self) -> AppResult<()> {
        self.gateway.clear_app().await?;
        self.state.clear_profile_cache();
        let mut current = self
            .state
            .session
            .lock()
            .map_err(|_| AppError::internal("登录会话状态不可用"))?;
        if let Some(session) = current.as_mut() {
            session.app_cookie = None;
            if session.web_cookie.is_some() {
                session.user_name = "已登录 SF 账号".to_string();
            }
        }
        if current
            .as_ref()
            .is_some_and(|session| session.web_cookie.is_none())
        {
            *current = None;
        }
        Ok(())
    }

    /// 在不阻断资料读取的前提下更新会话中的展示名称。
    ///
    /// # 参数
    /// - `profile`：刚从上游取得的账户资料。
    fn remember_profile_name(&self, profile: &UserProfile) {
        if let Ok(mut current) = self.state.session.lock() {
            if let Some(session) = current.as_mut() {
                session.user_name = profile.nick_name.clone();
            }
        }
    }

    /// 读取当前进程内会话状态的公开快照。
    ///
    /// # 错误
    /// 会话互斥锁不可用时返回内部错误。
    fn status_snapshot(&self) -> AppResult<AuthStatus> {
        self.state.status().map_err(AppError::internal)
    }

    /// 整体替换进程内会话。
    ///
    /// # 参数
    /// - `session`：新的会话；`None` 表示清除登录状态。
    ///
    /// # 错误
    /// 会话互斥锁不可用时返回内部错误。
    fn replace_session(&self, session: Option<AuthSession>) -> AppResult<()> {
        *self
            .state
            .session
            .lock()
            .map_err(|_| AppError::internal("登录会话状态不可用"))? = session;
        Ok(())
    }

    /// 将官方登录返回的 Web Cookie 合并到现有会话。
    ///
    /// # 参数
    /// - `incoming`：包含新 Web Cookie 的会话片段。
    ///
    /// # 错误
    /// 会话互斥锁不可用时返回内部错误。
    fn merge_web_session(&self, incoming: AuthSession) -> AppResult<()> {
        let mut current = self
            .state
            .session
            .lock()
            .map_err(|_| AppError::internal("登录会话状态不可用"))?;
        match current.as_mut() {
            Some(existing) => existing.web_cookie = incoming.web_cookie,
            None => *current = Some(incoming),
        }
        Ok(())
    }
}
