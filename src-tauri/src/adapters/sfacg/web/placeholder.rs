//! 网页 VIP 订阅提示图识别。

use md5::{Digest, Md5};

/// 网页在未授权 VIP 章节时返回的订阅提示图 MD5。
const VIP_SUBSCRIPTION_PLACEHOLDER_MD5: [u8; 16] = [
    0x41, 0x2c, 0xee, 0x87, 0x2b, 0x67, 0xea, 0x5d, 0x63, 0x70, 0xe8, 0x59, 0x68, 0x65, 0x89, 0xc4,
];

/// 判断图片内容是否为 SF 的 VIP 订阅提示图。
///
/// # 参数
/// - `bytes`：待校验的图片二进制内容。
pub(super) fn is_vip_subscription_placeholder(bytes: &[u8]) -> bool {
    is_vip_subscription_placeholder_digest(Md5::digest(bytes).as_slice())
}

/// 将图片摘要与订阅提示图摘要比较。
///
/// # 参数
/// - `digest`：待比较的 16 字节 MD5 摘要。
fn is_vip_subscription_placeholder_digest(digest: &[u8]) -> bool {
    digest == VIP_SUBSCRIPTION_PLACEHOLDER_MD5
}
