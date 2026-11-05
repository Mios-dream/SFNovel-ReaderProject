//! Windows 设备身份的加密落盘与恢复。
//!
//! 设备编号一旦重新生成，上游会把它当成一台新设备，因此必须持久化；
//! 落盘前同样使用 DPAPI 加密。

use super::session::{unprotect_auth_data, write_encrypted};
use crate::adapters::sfacg::device::{
    install_device_profile, install_device_state_path, is_valid_uuid, DeviceProfile,
};
use crate::domain::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::fs;
use tauri::Manager;

#[derive(Deserialize, Serialize)]
/// 加密保存的设备身份。
struct PersistedDeviceIdentity {
    /// 持久化格式版本。
    version: u8,
    /// 设备 UUID。
    device_id: String,
}

/// 恢复或生成设备身份，加密落盘并安装到进程全局状态。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 路径解析、文件读取、加密写入或设备身份生成失败时返回错误消息。
pub(crate) fn initialize_device_identity(app: &tauri::AppHandle) -> AppResult<()> {
    let path = app
        .path()
        .app_data_dir()
        .map_err(|error| AppError::storage(format!("无法解析设备身份路径：{error}")))?
        .join("device-identity.v1.bin");
    let stored = fs::read(&path)
        .ok()
        .and_then(|encrypted| unprotect_auth_data(&encrypted).ok())
        .and_then(|payload| serde_json::from_slice::<PersistedDeviceIdentity>(&payload).ok())
        .filter(|identity| identity.version == 2 && is_valid_uuid(&identity.device_id));
    let profile = match stored {
        Some(identity) => DeviceProfile {
            device_id: identity.device_id.to_lowercase(),
            ..DeviceProfile::generate()
        },
        None => {
            if path.is_file() {
                let _ = fs::remove_file(&path);
            }
            DeviceProfile::generate()
        }
    };
    let payload = serde_json::to_vec(&PersistedDeviceIdentity {
        version: 2,
        device_id: profile.device_id.clone(),
    })
    .map_err(|error| AppError::storage(format!("无法序列化设备身份：{error}")))?;
    write_encrypted(&path, &payload)?;
    install_device_state_path(path.with_file_name("device-state.json"));
    install_device_profile(profile);
    Ok(())
}
