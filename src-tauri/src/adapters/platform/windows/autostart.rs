//! 开机启动项（当前用户注册表 Run 键）。

use crate::domain::error::{AppError, AppResult};

/// 当前用户登录后自动启动应用的注册表位置。
const AUTOSTART_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
/// 开机启动项名称。
const AUTOSTART_VALUE: &str = "SF Novel Flow";

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 设置或移除当前用户的开机启动项。
pub(crate) fn set_autostart(enabled: bool) -> AppResult<()> {
    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows_sys::Win32::System::Registry::{
        RegDeleteKeyValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ,
    };

    let key = wide(AUTOSTART_KEY);
    let name = wide(AUTOSTART_VALUE);
    let status = if enabled {
        let executable =
            std::env::current_exe().map_err(|error| AppError::storage(format!("无法读取应用启动路径：{error}")))?;
        let command = wide(&format!("\"{}\"", executable.display()));
        unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                REG_SZ,
                command.as_ptr().cast(),
                (command.len() * std::mem::size_of::<u16>()) as u32,
            )
        }
    } else {
        unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) }
    };
    if status != ERROR_SUCCESS && !(status == ERROR_FILE_NOT_FOUND && !enabled) {
        return Err(AppError::storage(format!("无法更新开机启动设置（错误码 {status}）")));
    }
    Ok(())
}
