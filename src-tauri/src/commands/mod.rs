//! Tauri IPC 命令层。
//!
//! 命令只负责参数接收、适配器装配、应用服务调用和错误消息转换；业务规则位于
//! `application`，外部系统访问位于 `adapters`。

pub(crate) mod auth;
pub(crate) mod catalog;
pub(crate) mod diagnostics;
pub(crate) mod downloads;
pub(crate) mod library;
pub(crate) mod reading;
pub(crate) mod settings;
