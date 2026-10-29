//! SF App 签名 API 客户端。

/// 有声专辑目录与音频地址接口。
mod audio;
/// App 登录、会话校验与设备登记接口。
mod auth;
/// 小说、有声和漫画的搜索与详情接口。
mod catalog;
/// App HTTP 客户端、会话快照与请求重试策略。
mod client;
/// 漫画详情与目录标识接口。
mod comic;
/// App API 地址、认证和扩展字段常量。
mod constants;
/// 文字章节正文与目录接口。
mod content;
/// App API 的结构化错误分类与展示文本。
mod error;
/// 有声目录与音频来源解析。
mod parse_audio;
/// 文字小说分卷目录解析。
mod parse_catalog;
/// 小说与有声作品详情解析。
mod parse_detail;
/// App API JSON 字段提取工具。
mod parsing;
/// `SFSecurity` 请求签名生成。
mod signing;

pub(crate) use client::AppClient;
