//! 应用进程级会话状态。

use crate::domain::auth::{AuthSession, AuthStatus, UserProfile};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// 账户资料复用时间，避免重复读取 `/user`、`/user/money` 等账号接口。
const PROFILE_CACHE_TTL: Duration = Duration::from_secs(5 * 60);
/// 账户资料落盘缓存格式版本。
const PROFILE_CACHE_VERSION: u8 = 1;

/// 账户资料落盘缓存路径；未安装时只使用进程内缓存。
static PROFILE_CACHE_PATH: OnceLock<PathBuf> = OnceLock::new();

/// 带过期时间的账户资料快照。
struct CachedProfile {
    profile: UserProfile,
    cached_at: Instant,
    session_hash: u64,
}

/// 跨进程复用的账户资料快照。
#[derive(Deserialize, Serialize)]
struct PersistedProfile {
    version: u8,
    session_hash: u64,
    cached_at_ms: u64,
    profile: UserProfile,
}

/// 安装账户资料落盘缓存路径。
pub(crate) fn install_profile_cache_path(path: PathBuf) {
    let _ = PROFILE_CACHE_PATH.set(path);
}

/// 返回当前分钟级时间戳。
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// 计算当前会话 Cookie 的缓存键。
fn session_hash(session: Option<&AuthSession>) -> Option<u64> {
    let session = session?;
    let cookie = session
        .app_cookie
        .as_deref()
        .or(session.web_cookie.as_deref())?;
    let mut hasher = DefaultHasher::new();
    cookie.hash(&mut hasher);
    Some(hasher.finish())
}

/// 在原生内存中保存相互隔离的 App 与 Web 会话。
#[derive(Default)]
pub(crate) struct AuthSessionState {
    /// 受互斥锁保护的当前会话。
    pub(crate) session: Mutex<Option<AuthSession>>,
    /// 登录会话内复用的账户资料。
    profile: Mutex<Option<CachedProfile>>,
}

impl AuthSessionState {
    /// 返回当前会话的缓存键。
    fn current_session_hash(&self) -> Option<u64> {
        let session = self.session.lock().ok()?;
        session_hash(session.as_ref())
    }

    /// 读取仍在有效期内的账户资料。
    pub(crate) fn cached_profile(&self) -> Option<UserProfile> {
        let session_hash = self.current_session_hash()?;
        let mut cache = self.profile.lock().ok()?;
        if cache.as_ref().is_some_and(|entry| {
            entry.cached_at.elapsed() >= PROFILE_CACHE_TTL || entry.session_hash != session_hash
        }) {
            *cache = None;
        }
        if let Some(entry) = cache.as_ref() {
            return Some(entry.profile.clone());
        }
        let path = PROFILE_CACHE_PATH.get()?;
        let persisted = fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str::<PersistedProfile>(&text).ok())?;
        let age_ms = now_ms().saturating_sub(persisted.cached_at_ms);
        if persisted.version != PROFILE_CACHE_VERSION
            || persisted.session_hash != session_hash
            || age_ms >= PROFILE_CACHE_TTL.as_millis() as u64
        {
            let _ = fs::remove_file(path);
            return None;
        }
        let cached_at = Instant::now()
            .checked_sub(Duration::from_millis(age_ms))
            .unwrap_or_else(Instant::now);
        let profile = persisted.profile;
        *cache = Some(CachedProfile {
            profile: profile.clone(),
            cached_at,
            session_hash,
        });
        Some(profile)
    }

    /// 缓存本次登录会话的账户资料。
    ///
    /// # 参数
    /// - `profile`：刚从上游取得的账户资料。
    pub(crate) fn store_profile(&self, profile: &UserProfile) {
        let Some(session_hash) = self.current_session_hash() else {
            return;
        };
        if let Ok(mut cache) = self.profile.lock() {
            *cache = Some(CachedProfile {
                profile: profile.clone(),
                cached_at: Instant::now(),
                session_hash,
            });
        }
        let Some(path) = PROFILE_CACHE_PATH.get() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let persisted = PersistedProfile {
            version: PROFILE_CACHE_VERSION,
            session_hash,
            cached_at_ms: now_ms(),
            profile: profile.clone(),
        };
        if let Ok(payload) = serde_json::to_vec(&persisted) {
            let _ = fs::write(path, payload);
        }
    }

    /// 清除账户资料，避免登录状态变化后返回旧账号数据。
    pub(crate) fn clear_profile_cache(&self) {
        if let Ok(mut cache) = self.profile.lock() {
            *cache = None;
        }
        if let Some(path) = PROFILE_CACHE_PATH.get() {
            let _ = fs::remove_file(path);
        }
    }

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
