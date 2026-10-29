//! 在线正文、漫画页面与音频流适配。

mod protocol;
mod reader;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

pub(crate) use protocol::{handle_online_media_request, ONLINE_MEDIA_SCHEME};
pub(crate) use reader::OnlineReaderAdapter;

#[derive(Clone)]
/// 由渲染进程持有令牌、但实际只保存在原生进程中的媒体资源。
enum OnlineMediaResource {
    /// 需要 Range 转发的上游音频。
    Audio {
        /// 已校验的上游媒体地址。
        source: String,
        /// 请求上游资源时使用的 Referer。
        referer: String,
    },
    /// 按需缓存并返回的漫画页面。
    ComicPage {
        /// 已校验的上游图片地址。
        source: String,
        /// 请求上游图片时使用的 Referer。
        referer: String,
        /// 页面首次成功读取后使用的本地缓存文件。
        cache_path: PathBuf,
    },
}

/// 保存在应用进程中的媒体令牌，避免受保护的上游地址进入渲染进程。
#[derive(Default)]
pub(crate) struct OnlineMediaState {
    /// 令牌到私有媒体资源的映射。
    resources: Mutex<HashMap<String, OnlineMediaResource>>,
}

impl OnlineMediaState {
    /// 为媒体资源生成不可预测的令牌并登记到进程状态。
    ///
    /// # 参数
    /// - `resource`：需要隐藏的上游媒体地址及请求上下文。
    ///
    /// # 错误
    /// 媒体状态互斥锁不可用时返回错误消息。
    fn register(&self, resource: OnlineMediaResource) -> Result<String, String> {
        let token = uuid::Uuid::new_v4().simple().to_string();
        self.resources
            .lock()
            .map_err(|_| "在线媒体状态不可用".to_string())?
            .insert(token.clone(), resource);
        Ok(token)
    }

    /// 按令牌读取媒体资源。
    ///
    /// # 参数
    /// - `token`：注册时生成的随机令牌。
    fn get(&self, token: &str) -> Option<OnlineMediaResource> {
        self.resources.lock().ok()?.get(token).cloned()
    }
}

/// 生成渲染进程可访问的媒体协议地址。
///
/// # 参数
/// - `kind`：资源类型路径，例如 `audio` 或 `comic`。
/// - `token`：已登记的媒体令牌。
fn online_media_url(kind: &str, token: &str) -> String {
    #[cfg(any(target_os = "windows", target_os = "android"))]
    {
        format!("http://{ONLINE_MEDIA_SCHEME}.localhost/{kind}/{token}")
    }
    #[cfg(not(any(target_os = "windows", target_os = "android")))]
    {
        format!("{ONLINE_MEDIA_SCHEME}://localhost/{kind}/{token}")
    }
}
