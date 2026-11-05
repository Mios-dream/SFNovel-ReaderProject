//! 桌面端的设备身份初始化。
//!
//! Windows 与 Android 各自有原生实现；其余桌面平台只生成本地临时身份。

use crate::adapters::sfacg::device::{
    install_device_profile, install_device_state_path, DeviceProfile,
};
use crate::domain::error::AppResult;
use tauri::Manager;

/// 生成临时设备身份，并保存设备登记结果的文件位置。
pub(crate) fn initialize_device_identity(app: &tauri::AppHandle) -> AppResult<()> {
    if let Ok(directory) = app.path().app_data_dir() {
        install_device_state_path(directory.join("device-state.json"));
    }
    install_device_profile(DeviceProfile::generate());
    Ok(())
}
