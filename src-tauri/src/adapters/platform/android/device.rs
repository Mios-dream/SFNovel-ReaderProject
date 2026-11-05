//! Android 设备身份采集。

use super::call_auth;
use crate::adapters::sfacg::device::{
    install_device_profile, install_device_state_path, is_valid_uuid, DeviceProfile,
};
use crate::domain::error::{AppError, AppResult};
use serde::Deserialize;
use tauri::Manager;

/// Java 侧采集到的设备特征。
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceProfileResponse {
    device_id: Option<String>,
    sdk: Option<u32>,
    abi: Option<String>,
    manufacturer: Option<String>,
    origin_ua: Option<String>,
    #[serde(rename = "webViewUA")]
    web_view_ua: Option<String>,
}

/// 读取或创建平台设备身份，并登记到进程内配置。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 插件调用失败、设备编号无效或状态目录无法解析时返回错误。
pub(crate) async fn initialize_device_identity(app: &tauri::AppHandle) -> AppResult<()> {
    let profile = call_auth::<DeviceProfileResponse, _>(
        app,
        "readOrCreateDeviceProfile",
        (),
        "无法读取设备身份",
    )
    .await?;
    let device = merge_profile(profile)?;
    let state_path = app
        .path()
        .app_data_dir()
        .map_err(|error| AppError::storage(format!("无法解析设备登记状态路径：{error}")))?
        .join("device-state.json");
    install_device_state_path(state_path);
    install_device_profile(device);
    Ok(())
}

/// 以平台采集值覆盖随机生成的设备特征，缺失项保留原值。
///
/// # 参数
/// - `profile`：Java 侧采集到的设备特征。
fn merge_profile(profile: DeviceProfileResponse) -> AppResult<DeviceProfile> {
    let device_id = profile
        .device_id
        .filter(|value| is_valid_uuid(value))
        .ok_or_else(|| AppError::storage("Android 设备编号无效"))?;
    let mut device = DeviceProfile::generate();
    device.device_id = device_id.to_lowercase();
    if let Some(sdk) = profile.sdk.filter(|value| *value > 0) {
        device.sdk = sdk;
    }
    device.abi = non_empty(profile.abi).unwrap_or(device.abi);
    device.manufacturer = non_empty(profile.manufacturer)
        .map(|value| value.to_lowercase())
        .unwrap_or(device.manufacturer);
    device.origin_ua = non_empty(profile.origin_ua).unwrap_or(device.origin_ua);
    device.web_view_ua = non_empty(profile.web_view_ua).unwrap_or(device.web_view_ua);
    Ok(device)
}

/// 过滤空字符串，避免用空白覆盖已有的设备特征。
///
/// # 参数
/// - `value`：待检查的可选文本。
fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}
