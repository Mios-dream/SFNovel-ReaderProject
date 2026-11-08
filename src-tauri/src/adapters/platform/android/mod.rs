//! Android 认证与 OCR 原生插件。
//!
//! 按职责拆分：`session` 负责会话读写与自动签到开关，`device` 负责设备身份采集，
//! `login` 负责官方登录窗口，`storage` 负责外部存储与导出落盘，`power` 负责打开
//! 应用设置入口，`ocr` 负责正文识别，`signin` 承载 Java 侧自动签到的 JNI 入口。

mod device;
mod login;
mod ocr;
mod power;
mod session;
mod signin;
mod storage;

use crate::domain::error::{AppError, AppResult};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tauri::plugin::{PluginHandle, TauriPlugin};
use tauri::Manager;

pub(super) use device::initialize_device_identity;
pub(crate) use login::start_official_login;
pub(crate) use ocr::recognize_image;
pub(crate) use power::open_app_settings;
pub(crate) use session::{
    clear_app_session, clear_session, clear_web_session, configure_auto_signin,
    persist_app_session, synchronize_session,
};
pub(crate) use storage::{
    ensure_external_storage_access, open_download_directory, write_export_to_uri,
};

/// 认证插件在进程内的注册状态。
pub(crate) struct AndroidSfacgAuth<R: tauri::Runtime> {
    pub(crate) mobile_plugin_handle: PluginHandle<R>,
}

/// OCR 插件在进程内的注册状态。
pub(crate) struct AndroidSfacgOcr<R: tauri::Runtime> {
    pub(crate) mobile_plugin_handle: PluginHandle<R>,
}

/// 注册 Android 认证插件。
pub(crate) fn android_auth_plugin<R: tauri::Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("sfacg-auth")
        .setup(|app, api| {
            let handle =
                api.register_android_plugin("com.sansan.sf_novel_flow", "SfacgAuthPlugin")?;
            app.manage(AndroidSfacgAuth {
                mobile_plugin_handle: handle,
            });
            Ok(())
        })
        .build()
}

/// 注册 Android OCR 插件。
pub(crate) fn android_ocr_plugin<R: tauri::Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("sfacg-ocr")
        .setup(|app, api| {
            let handle =
                api.register_android_plugin("com.sansan.sf_novel_flow", "SfacgOcrPlugin")?;
            app.manage(AndroidSfacgOcr {
                mobile_plugin_handle: handle,
            });
            Ok(())
        })
        .build()
}

/// 调用认证插件的原生方法。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `method`：Java 侧的插件方法名。
/// - `payload`：传给插件的参数。
/// - `action`：错误消息前缀，例如"无法清除 Android 登录会话"。
async fn call_auth<T, P>(
    app: &tauri::AppHandle,
    method: &str,
    payload: P,
    action: &str,
) -> AppResult<T>
where
    T: DeserializeOwned,
    P: Serialize,
{
    app.state::<AndroidSfacgAuth<tauri::Wry>>()
        .mobile_plugin_handle
        .run_mobile_plugin_async::<T>(method, payload)
        .await
        .map_err(|error| AppError::storage(format!("{action}：{error}")))
}

/// 调用 OCR 插件的原生方法。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `method`：Java 侧的插件方法名。
/// - `payload`：传给插件的参数。
/// - `action`：错误消息前缀。
async fn call_ocr<T, P>(
    app: &tauri::AppHandle,
    method: &str,
    payload: P,
    action: &str,
) -> AppResult<T>
where
    T: DeserializeOwned,
    P: Serialize,
{
    app.state::<AndroidSfacgOcr<tauri::Wry>>()
        .mobile_plugin_handle
        .run_mobile_plugin_async::<T>(method, payload)
        .await
        .map_err(|error| AppError::storage(format!("{action}：{error}")))
}
