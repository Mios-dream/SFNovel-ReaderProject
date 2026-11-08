//! Tauri IPC 命令层。
//!
//! 命令只负责参数接收、适配器装配、应用服务调用和错误消息转换；业务规则位于
//! `application`，外部系统访问位于 `adapters`。

use crate::adapters::sfacg::source::SfSource;
use crate::adapters::storage::NativeStorage;
use crate::domain::error::AppResult;

/// 组装命令所需的远程来源与本地存储。
///
/// 内容源按命令重建：它在构造时快照登录 Cookie，登录或登出后必须重新读取。
///
/// # 错误
/// 会话快照读取或客户端初始化失败时返回错误。
pub(crate) async fn runtime(app: &tauri::AppHandle) -> AppResult<(SfSource, NativeStorage)> {
    let remote = SfSource::connect(app).await?;
    Ok((remote, NativeStorage::new(app.clone())))
}

/// 只需要远程来源的命令使用的简写。
pub(crate) async fn remote(app: &tauri::AppHandle) -> AppResult<SfSource> {
    runtime(app).await.map(|(remote, _)| remote)
}

pub(crate) mod auth;
pub(crate) mod catalog;
pub(crate) mod diagnostics;
pub(crate) mod downloads;
pub(crate) mod library;
pub(crate) mod platform;
pub(crate) mod reading;
pub(crate) mod signin;
pub(crate) mod settings;
pub(crate) mod updates;
