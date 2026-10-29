//! 应用进程级会话状态。

use crate::domain::auth::{AuthSession, AuthStatus};
use std::sync::Mutex;

/// 在原生内存中保存相互隔离的 App 与 Web 会话。
#[derive(Default)]
pub(crate) struct AuthSessionState {
    /// 受互斥锁保护的当前会话。
    pub(crate) session: Mutex<Option<AuthSession>>,
}

impl AuthSessionState {
    /// 返回当前认证状态快照。
    ///
    /// # 错误
    /// 会话互斥锁不可用时返回错误消息。
    pub(crate) fn status(&self) -> Result<AuthStatus, String> {
        let session = self
            .session
            .lock()
            .map_err(|_| "登录会话状态不可用".to_string())?;
        Ok(status_from_session(session.as_ref()))
    }

    /// 克隆当前完整会话，避免调用方长期持有互斥锁。
    ///
    /// # 错误
    /// 会话互斥锁不可用时返回错误消息。
    pub(crate) fn snapshot(&self) -> Result<Option<AuthSession>, String> {
        let session = self
            .session
            .lock()
            .map_err(|_| "登录会话状态不可用".to_string())?;
        Ok(session.as_ref().map(clone_session))
    }

    /// 整体替换当前会话。
    ///
    /// # 参数
    /// - `session`：新的会话；`None` 表示清除。
    ///
    /// # 错误
    /// 会话互斥锁不可用时返回错误消息。
    #[cfg(target_os = "windows")]
    pub(crate) fn replace(&self, session: Option<AuthSession>) -> Result<(), String> {
        *self
            .session
            .lock()
            .map_err(|_| "登录会话状态不可用".to_string())? = session;
        Ok(())
    }

    /// 将启动恢复得到的 Web 会话合并到现有状态。
    ///
    /// # 参数
    /// - `incoming`：包含恢复 Web Cookie 的会话片段。
    ///
    /// # 错误
    /// 会话互斥锁不可用时返回错误消息。
    #[cfg(target_os = "windows")]
    pub(crate) fn merge_web(&self, incoming: AuthSession) -> Result<(), String> {
        let mut current = self
            .session
            .lock()
            .map_err(|_| "登录会话状态不可用".to_string())?;
        match current.as_mut() {
            Some(existing) => existing.web_cookie = incoming.web_cookie,
            None => *current = Some(incoming),
        }
        Ok(())
    }
}

/// 深拷贝会话中的 Cookie 与展示名称。
///
/// # 参数
/// - `session`：待复制的会话。
pub(crate) fn clone_session(session: &AuthSession) -> AuthSession {
    AuthSession {
        app_cookie: session.app_cookie.clone(),
        web_cookie: session.web_cookie.clone(),
        user_name: session.user_name.clone(),
    }
}

/// 把可选会话转换为不泄露 Cookie 的认证状态。
///
/// # 参数
/// - `session`：当前会话；`None` 表示匿名状态。
pub(crate) fn status_from_session(session: Option<&AuthSession>) -> AuthStatus {
    AuthStatus {
        authenticated: session
            .is_some_and(|value| value.app_cookie.is_some() || value.web_cookie.is_some()),
        app_authenticated: session.is_some_and(|value| value.app_cookie.is_some()),
        web_authenticated: session.is_some_and(|value| value.web_cookie.is_some()),
        user_name: session.map(|value| value.user_name.clone()),
    }
}
