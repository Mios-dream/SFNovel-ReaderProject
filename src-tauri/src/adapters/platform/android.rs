//! Android 认证与 OCR 原生插件。

use crate::adapters::sfacg::device::{
    install_device_profile, install_device_state_path, is_valid_uuid, DeviceProfile,
};
use crate::domain::auth::AuthSession;
use serde::Deserialize;
use serde_json::Value;
use tauri::plugin::{PluginHandle, TauriPlugin};
use tauri::Manager;

#[derive(Deserialize)]
struct CookieResponse {
    cookie: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeviceProfileResponse {
    device_id: Option<String>,
    sdk: Option<u32>,
    abi: Option<String>,
    manufacturer: Option<String>,
    origin_ua: Option<String>,
    #[serde(rename = "webViewUA")]
    web_view_ua: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct OcrResponse {
    pub(crate) text: String,
}

pub(crate) struct AndroidSfacgAuth<R: tauri::Runtime> {
    pub(crate) mobile_plugin_handle: PluginHandle<R>,
}

pub(crate) struct AndroidSfacgOcr<R: tauri::Runtime> {
    pub(crate) mobile_plugin_handle: PluginHandle<R>,
}

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

pub(crate) async fn synchronize_session(
    app: &tauri::AppHandle,
) -> Result<Option<AuthSession>, String> {
    let handle = &app
        .state::<AndroidSfacgAuth<tauri::Wry>>()
        .mobile_plugin_handle;
    let web_cookie = handle
        .run_mobile_plugin_async::<CookieResponse>("readSessionCookie", ())
        .await
        .map_err(|error| format!("无法读取 Android 登录会话：{error}"))?
        .cookie
        .filter(|value| !value.trim().is_empty());
    let app_cookie = handle
        .run_mobile_plugin_async::<CookieResponse>("readAppSessionCookie", ())
        .await
        .map_err(|error| format!("无法读取 Android App 会话：{error}"))?
        .cookie
        .filter(|value| !value.trim().is_empty());
    Ok(if app_cookie.is_some() || web_cookie.is_some() {
        Some(AuthSession {
            app_cookie,
            web_cookie,
            user_name: "已登录 SF 账号".to_string(),
        })
    } else {
        None
    })
}

pub(super) async fn initialize_device_identity(app: &tauri::AppHandle) -> Result<(), String> {
    let profile = app
        .state::<AndroidSfacgAuth<tauri::Wry>>()
        .mobile_plugin_handle
        .run_mobile_plugin_async::<DeviceProfileResponse>("readOrCreateDeviceProfile", ())
        .await
        .map_err(|error| format!("无法读取设备身份：{error}"))?;
    let device_id = profile
        .device_id
        .filter(|value| is_valid_uuid(value))
        .ok_or_else(|| "Android 设备编号无效".to_string())?;
    let mut device = DeviceProfile::generate();
    device.device_id = device_id.to_lowercase();
    if let Some(sdk) = profile.sdk.filter(|value| *value > 0) {
        device.sdk = sdk;
    }
    device.abi = profile
        .abi
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(device.abi);
    device.manufacturer = profile
        .manufacturer
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.to_lowercase())
        .unwrap_or(device.manufacturer);
    device.origin_ua = profile
        .origin_ua
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(device.origin_ua);
    device.web_view_ua = profile
        .web_view_ua
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(device.web_view_ua);
    let state_path = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("无法解析设备登记状态路径：{error}"))?
        .join("device-state.json");
    install_device_state_path(state_path);
    install_device_profile(device);
    Ok(())
}

pub(super) async fn persist_app_session(
    app: &tauri::AppHandle,
    cookie: &str,
) -> Result<(), String> {
    app.state::<AndroidSfacgAuth<tauri::Wry>>()
        .mobile_plugin_handle
        .run_mobile_plugin_async::<Value>(
            "writeAppSessionCookie",
            serde_json::json!({ "cookie": cookie }),
        )
        .await
        .map_err(|error| format!("无法保存 Android 登录会话：{error}"))?;
    Ok(())
}

pub(super) async fn start_official_login(app: &tauri::AppHandle) -> Result<(), String> {
    app.state::<AndroidSfacgAuth<tauri::Wry>>()
        .mobile_plugin_handle
        .run_mobile_plugin_async::<Value>("startOfficialLogin", ())
        .await
        .map_err(|error| format!("无法打开 Android 官方登录页：{error}"))?;
    Ok(())
}

pub(super) async fn clear_session(app: &tauri::AppHandle) -> Result<(), String> {
    app.state::<AndroidSfacgAuth<tauri::Wry>>()
        .mobile_plugin_handle
        .run_mobile_plugin_async::<Value>("clearSessionCookie", ())
        .await
        .map_err(|error| format!("无法清除 Android 登录会话：{error}"))?;
    Ok(())
}

pub(super) async fn clear_app_session(app: &tauri::AppHandle) -> Result<(), String> {
    app.state::<AndroidSfacgAuth<tauri::Wry>>()
        .mobile_plugin_handle
        .run_mobile_plugin_async::<Value>("clearAppSessionCookie", ())
        .await
        .map_err(|error| format!("无法清除 Android App 登录会话：{error}"))?;
    Ok(())
}

pub(super) async fn clear_web_session(app: &tauri::AppHandle) -> Result<(), String> {
    app.state::<AndroidSfacgAuth<tauri::Wry>>()
        .mobile_plugin_handle
        .run_mobile_plugin_async::<Value>("clearWebSessionCookie", ())
        .await
        .map_err(|error| format!("无法清除 Android 网站登录会话：{error}"))?;
    Ok(())
}

pub(crate) async fn configure_auto_signin(
    app: &tauri::AppHandle,
    enabled: bool,
) -> Result<(), String> {
    app.state::<AndroidSfacgAuth<tauri::Wry>>()
        .mobile_plugin_handle
        .run_mobile_plugin_async::<Value>(
            "setAutoSignin",
            serde_json::json!({ "enabled": enabled }),
        )
        .await
        .map_err(|error| format!("无法配置 Android 自动签到：{error}"))?;
    Ok(())
}

pub(crate) async fn ensure_external_storage_access(app: &tauri::AppHandle) -> Result<bool, String> {
    let result = app
        .state::<AndroidSfacgAuth<tauri::Wry>>()
        .mobile_plugin_handle
        .run_mobile_plugin_async::<Value>("ensureExternalStorageAccess", ())
        .await
        .map_err(|error| format!("无法检查 Android 外部存储权限：{error}"))?;
    let granted = result
        .get("granted")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if granted {
        std::fs::create_dir_all("/storage/emulated/0/Download/SF Novel Flow")
            .map_err(|error| format!("无法创建外部书库目录：{error}"))?;
    }
    Ok(granted)
}

pub(crate) async fn recognize_image(
    app: &tauri::AppHandle,
    source: std::path::PathBuf,
    segments_dir: Option<std::path::PathBuf>,
) -> Result<String, String> {
    let result = app
        .state::<AndroidSfacgOcr<tauri::Wry>>()
        .mobile_plugin_handle
        .run_mobile_plugin_async::<OcrResponse>(
            "recognizeChapter",
            serde_json::json!({
                "sourcePath": source.to_string_lossy(),
                "segmentsDir": segments_dir.map(|directory| directory.to_string_lossy().into_owned()),
            }),
        )
        .await
        .map_err(|error| format!("Android OCR 失败：{error}"))?;
    if result.text.trim().is_empty() {
        return Err("Android OCR 未识别到可用文字；原始图片已保留，可稍后重试".to_string());
    }
    Ok(result.text)
}

pub(crate) async fn open_download_directory(app: &tauri::AppHandle) -> Result<(), String> {
    app.state::<AndroidSfacgAuth<tauri::Wry>>()
        .mobile_plugin_handle
        .run_mobile_plugin_async::<Value>("openDownloadDirectory", ())
        .await
        .map_err(|error| format!("无法打开下载目录：{error}"))?;
    Ok(())
}

pub(crate) async fn write_export_to_uri(
    app: &tauri::AppHandle,
    source: &std::path::Path,
    uri: &str,
) -> Result<(), String> {
    app.state::<AndroidSfacgAuth<tauri::Wry>>()
        .mobile_plugin_handle
        .run_mobile_plugin_async::<Value>(
            "writeExportToUri",
            serde_json::json!({
                "sourcePath": source.to_string_lossy(),
                "uri": uri,
            }),
        )
        .await
        .map_err(|error| format!("无法写入系统保存位置：{error}"))?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AutoSigninDeviceProfile {
    device_id: String,
    sdk: u32,
    abi: String,
    manufacturer: String,
    origin_ua: String,
    #[serde(rename = "webViewUA")]
    web_view_ua: String,
}

fn auto_signin_device(profile: AutoSigninDeviceProfile) -> Result<DeviceProfile, String> {
    if !is_valid_uuid(&profile.device_id) {
        return Err("Android 设备编号无效".to_string());
    }
    let mut device = DeviceProfile::generate();
    device.device_id = profile.device_id.to_lowercase();
    if profile.sdk > 0 {
        device.sdk = profile.sdk;
    }
    if !profile.abi.trim().is_empty() {
        device.abi = profile.abi;
    }
    if !profile.manufacturer.trim().is_empty() {
        device.manufacturer = profile.manufacturer.to_lowercase();
    }
    if !profile.origin_ua.trim().is_empty() {
        device.origin_ua = profile.origin_ua;
    }
    if !profile.web_view_ua.trim().is_empty() {
        device.web_view_ua = profile.web_view_ua;
    }
    Ok(device)
}

fn run_auto_signin(
    data_dir: String,
    cookie: String,
    device_json: String,
    date: String,
) -> Result<String, String> {
    if cookie.trim().is_empty() {
        return Ok("未登录，跳过自动签到".to_string());
    }
    if date.len() != 10 {
        return Err("签到日期格式无效".to_string());
    }
    let profile = serde_json::from_str::<AutoSigninDeviceProfile>(&device_json)
        .map_err(|error| format!("Android 设备信息无效：{error}"))?;
    let device = auto_signin_device(profile)?;
    install_device_state_path(std::path::PathBuf::from(data_dir).join("device-state.json"));
    let client = crate::adapters::sfacg::app::AppClient::new(device, Some(cookie))?;
    if !client.has_session() {
        return Ok("App 会话不可用，跳过自动签到".to_string());
    }
    tauri::async_runtime::block_on(async move {
        let week = client.get_signin_week().await?;
        if week
            .iter()
            .any(|item| item.date.get(..10) == Some(date.as_str()) && item.is_sign_in)
        {
            return Ok(format!("{date} 已签到"));
        }
        client.submit_signin(&date).await?;
        Ok(format!("{date} 自动签到成功"))
    })
}

#[no_mangle]
pub extern "system" fn Java_com_sansan_sf_novel_flow_AutoSigninBridge_run<'local>(
    mut env: jni::JNIEnv<'local>,
    _this: jni::objects::JObject<'local>,
    data_dir: jni::objects::JString<'local>,
    cookie: jni::objects::JString<'local>,
    device_json: jni::objects::JString<'local>,
    date: jni::objects::JString<'local>,
) -> jni::sys::jstring {
    let result = (|| -> Result<String, String> {
        let data_dir = java_string(&mut env, &data_dir)?;
        let cookie = java_string(&mut env, &cookie)?;
        let device_json = java_string(&mut env, &device_json)?;
        let date = java_string(&mut env, &date)?;
        run_auto_signin(data_dir, cookie, device_json, date)
    })();
    let message = match result {
        Ok(message) => message,
        Err(error) => format!("自动签到失败：{error}"),
    };
    env.new_string(message)
        .map(jni::objects::JString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

fn java_string(env: &mut jni::JNIEnv<'_>, value: &jni::objects::JString<'_>) -> Result<String, String> {
    env.get_string(value)
        .map(Into::into)
        .map_err(|error| format!("读取 JNI 字符串失败：{error}"))
}
