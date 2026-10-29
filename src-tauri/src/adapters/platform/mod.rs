//! 操作系统会话、登录窗口与原生插件适配。

use crate::application::ports::AuthGateway;
use crate::domain::auth::AuthSession;
use crate::domain::error::{AppError, AppResult};
use async_trait::async_trait;

#[cfg(target_os = "android")]
pub(crate) mod android;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "android")]
pub(crate) use android::{android_auth_plugin, android_ocr_plugin};

#[cfg(target_os = "windows")]
pub(crate) use windows::{restore_app_session, restore_web_session};

/// 初始化当前进程的设备身份。
#[cfg(target_os = "windows")]
pub(crate) fn initialize_device_identity(app: &tauri::AppHandle) -> AppResult<()> {
    windows::initialize_device_identity(app).map_err(AppError::storage)
}

/// Android 插件只有异步接口，因此在启动阶段单独等待初始化完成。
#[cfg(target_os = "android")]
pub(crate) async fn initialize_device_identity(app: &tauri::AppHandle) -> AppResult<()> {
    android::initialize_device_identity(app)
        .await
        .map_err(AppError::storage)
}

/// 其他桌面平台生成不落盘的临时设备身份。
#[cfg(all(not(target_os = "windows"), not(target_os = "android")))]
pub(crate) fn initialize_device_identity(_app: &tauri::AppHandle) -> AppResult<()> {
    crate::adapters::sfacg::device::install_device_profile(
        crate::adapters::sfacg::device::DeviceProfile::generate(),
    );
    Ok(())
}

/// 平台会话适配器。
pub(crate) struct NativeAuthGateway {
    /// 平台会话存储与登录 UI 所需的应用句柄。
    app: tauri::AppHandle,
}

impl NativeAuthGateway {
    /// 创建平台认证网关。
    ///
    /// # 参数
    /// - `app`：Tauri 应用句柄。
    pub(crate) fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

#[async_trait]
impl AuthGateway for NativeAuthGateway {
    /// 在 Android 上读取平台保存的会话；桌面端返回 `None`。
    async fn synchronize(&self) -> AppResult<Option<AuthSession>> {
        #[cfg(target_os = "android")]
        {
            return android::synchronize_session(&self.app)
                .await
                .map_err(AppError::storage);
        }
        #[cfg(not(target_os = "android"))]
        {
            Ok(None)
        }
    }

    /// 将 App Cookie 保存到当前平台的安全存储。
    async fn persist_app_session(&self, cookie: &str, user_name: &str) -> AppResult<()> {
        #[cfg(target_os = "android")]
        let _ = user_name;
        #[cfg(target_os = "windows")]
        {
            windows::persist_app_session(&self.app, cookie, user_name)
                .map_err(AppError::storage)?;
        }
        #[cfg(target_os = "android")]
        {
            android::persist_app_session(&self.app, cookie)
                .await
                .map_err(AppError::storage)?;
        }
        Ok(())
    }

    /// 将 Web Cookie 保存到当前平台的安全存储。
    async fn persist_web_session(&self, cookie: &str) -> AppResult<()> {
        #[cfg(target_os = "windows")]
        windows::persist_web_session(&self.app, cookie).map_err(AppError::storage)?;
        #[cfg(not(target_os = "windows"))]
        let _ = cookie;
        Ok(())
    }

    /// 打开当前平台的官方登录流程并返回 Web 会话。
    async fn start_official_login(&self) -> AppResult<Option<AuthSession>> {
        #[cfg(target_os = "windows")]
        {
            return windows::start_official_login(&self.app)
                .await
                .map(Some)
                .map_err(AppError::upstream);
        }
        #[cfg(target_os = "android")]
        {
            android::start_official_login(&self.app)
                .await
                .map_err(AppError::upstream)?;
            return android::synchronize_session(&self.app)
                .await
                .map_err(AppError::storage);
        }
        #[cfg(all(not(target_os = "android"), not(target_os = "windows")))]
        Err(AppError::internal(
            "官方网页登录目前仅支持 Windows 和 Android",
        ))
    }

    /// 清除所有平台会话。
    async fn clear_all(&self) -> AppResult<()> {
        #[cfg(target_os = "android")]
        android::clear_session(&self.app)
            .await
            .map_err(AppError::storage)?;
        #[cfg(target_os = "windows")]
        {
            windows::clear_app_session(&self.app).map_err(AppError::storage)?;
            windows::clear_web_session(&self.app).map_err(AppError::storage)?;
        }
        Ok(())
    }

    /// 仅清除 App 会话。
    async fn clear_app(&self) -> AppResult<()> {
        #[cfg(target_os = "android")]
        android::clear_app_session(&self.app)
            .await
            .map_err(AppError::storage)?;
        #[cfg(target_os = "windows")]
        windows::clear_app_session(&self.app).map_err(AppError::storage)?;
        Ok(())
    }

    /// 仅清除 Web 会话。
    async fn clear_web(&self) -> AppResult<()> {
        #[cfg(target_os = "android")]
        android::clear_web_session(&self.app)
            .await
            .map_err(AppError::storage)?;
        #[cfg(target_os = "windows")]
        windows::clear_web_session(&self.app).map_err(AppError::storage)?;
        Ok(())
    }
}
