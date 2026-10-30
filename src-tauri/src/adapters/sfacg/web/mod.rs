//! SF 网页客户端。

/// 网页登录资料、账户资料与 VIP 资料接口。
mod auth;
/// 公开书架页面发现与条目解析。
mod catalog;
/// 网页 HTTP 客户端与 Cookie 注入策略。
mod client;
/// 漫画目录和章节图片接口。
mod comic;
/// 文字章节正文与 VIP 图片正文接口。
mod content;
/// 轻量 HTML 与 JavaScript 字段解析工具。
mod html;
/// VIP 订阅提示图识别。
mod placeholder;

pub(crate) use client::WebClient;
