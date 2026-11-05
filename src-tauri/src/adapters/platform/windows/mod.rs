//! Windows 平台实现。
//!
//! 按职责拆分：`session` 负责 DPAPI 与会话落盘，`device` 负责设备身份，
//! `login` 负责 WebView 登录窗口，`autostart` 负责注册表启动项。

mod autostart;
mod device;
mod login;
mod session;

pub(crate) use autostart::set_autostart;
pub(super) use device::initialize_device_identity;
pub(crate) use login::start_official_login;
pub(super) use session::{
    clear_app_session, clear_web_session, persist_app_session, persist_web_session,
};
pub(crate) use session::{restore_app_session, restore_web_session};
