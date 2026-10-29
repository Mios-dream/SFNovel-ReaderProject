//! 应用用例层。
//!
//! 本层只依赖领域模型与端口，不直接访问 Tauri、文件系统、HTTP 或平台 API。

pub(crate) mod auth;
pub(crate) mod catalog;
pub(crate) mod downloads;
pub(crate) mod library;
pub(crate) mod ports;
pub(crate) mod reading;
pub(crate) mod settings;
pub(crate) mod state;
