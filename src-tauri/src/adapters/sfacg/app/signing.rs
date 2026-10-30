//! App API 请求签名。

use super::super::device::DeviceProfile;
use super::constants::{SF_SECURITY_SALT, SF_SECURITY_SALT_V2};
use md5::{Digest, Md5};
use std::time::{SystemTime, UNIX_EPOCH};

/// 漫画兼容签名使用的四段掩码长度。
const V2_SEGMENT_SIZES: [usize; 4] = [13, 16, 36, 36];

/// 构建 SF `SFSecurity` 请求头，不记录任何凭据材料。
pub(super) fn security_header(device: &DeviceProfile, nonce: &str) -> Result<String, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "系统时间无效".to_string())?
        .as_millis();
    let device_id = device.device_id.to_uppercase();
    let digest = Md5::digest(format!("{nonce}{timestamp}{device_id}{SF_SECURITY_SALT}").as_bytes());
    Ok(format!(
        "nonce={nonce}&timestamp={timestamp}&devicetoken={device_id}&sign={digest:X}"
    ))
}

/// 构建漫画兼容接口需要的 101 字符 `SFSecurity` 请求头。
///
/// 服务端偶尔以 `417/782` 拒绝合法请求，调用方需要换新 nonce 重试。
pub(super) fn comic_security_header(device: &DeviceProfile, nonce: &str) -> Result<String, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "系统时间无效".to_string())?
        .as_millis();
    let nonce = nonce.to_uppercase();
    let device_id = device.device_id.to_uppercase();
    let sign = comic_sign(&nonce, timestamp, &device_id)?;
    Ok(format!(
        "nonce={nonce}&timestamp={timestamp}&devicetoken={device_id}&sign={sign}"
    ))
}

/// 计算漫画兼容签名。
fn comic_sign(nonce: &str, timestamp: u128, device_id: &str) -> Result<String, String> {
    if nonce.len() != 36 || device_id.len() != 36 {
        return Err("漫画兼容签名需要 36 字符的 nonce 和设备编号".to_string());
    }
    let timestamp_text = timestamp.to_string();
    let repeated = nonce.repeat(4);
    let repeated_bytes = repeated.as_bytes();
    let mut mask = Vec::with_capacity(101);
    for (index, size) in V2_SEGMENT_SIZES.iter().enumerate() {
        let start = usize::from(repeated_bytes[index + 1]) % 36;
        mask.extend_from_slice(&repeated_bytes[start..start + size]);
    }
    let source = format!("{timestamp_text}{SF_SECURITY_SALT_V2}{device_id}{nonce}");
    let mut mixed = source
        .bytes()
        .zip(mask.iter())
        .map(|(left, right)| (left + right) / 2)
        .collect::<Vec<_>>();
    mixed = [&mixed[65..], &mixed[..13], &mixed[29..65], &mixed[13..29]].concat();
    for value in &mut mixed {
        if *value < b'0' {
            let shifted = *value + 19;
            *value = if shifted > b'9' && shifted < b'A' {
                b'9'
            } else {
                shifted
            };
        } else if (*value > b'9' && *value < b'A') || (*value > b'Z' && *value < b'a') {
            *value += 19;
        }
    }
    Ok(format!("{:X}", Md5::digest(&mixed)))
}
