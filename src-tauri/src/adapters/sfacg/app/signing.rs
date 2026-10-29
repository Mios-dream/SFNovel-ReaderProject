//! App API 请求签名。

use super::super::device::DeviceProfile;
use super::constants::SF_SECURITY_SALT;
use md5::{Digest, Md5};
use std::time::{SystemTime, UNIX_EPOCH};

/// 构建 SF `SFSecurity` 请求头，不记录任何凭据材料。
pub(super) fn security_header(device: &DeviceProfile, nonce: &str) -> Result<String, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "系统时间无效".to_string())?
        .as_millis();
    let device_token = device.device_token.to_uppercase();
    let digest =
        Md5::digest(format!("{nonce}{timestamp}{device_token}{SF_SECURITY_SALT}").as_bytes());
    Ok(format!(
        "nonce={nonce}&timestamp={timestamp}&devicetoken={device_token}&sign={digest:X}"
    ))
}
