//! App API 请求签名。

use super::super::device::DeviceProfile;
use super::constants::SF_SECURITY_SALT_V2;
use md5::{Digest, Md5};
use std::time::{SystemTime, UNIX_EPOCH};

/// v2 签名使用的四段掩码长度。
const V2_SEGMENT_SIZES: [usize; 4] = [13, 16, 36, 36];

/// 构建 SF `SFSecurity` 请求头，不记录任何凭据材料。
pub(super) fn security_header(device: &DeviceProfile, nonce: &str) -> Result<String, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "系统时间无效".to_string())?
        .as_millis();
    let nonce = nonce.to_uppercase();
    let device_id = device.device_id.to_uppercase();
    let sign = sfacg_sign_v2(&nonce, timestamp, &device_id)?;
    Ok(format!(
        "nonce={nonce}&timestamp={timestamp}&devicetoken={device_id}&sign={sign}"
    ))
}

/// 计算 5.2.84 的 101 字符变换签名。
fn sfacg_sign_v2(nonce: &str, timestamp: u128, device_id: &str) -> Result<String, String> {
    if nonce.len() != 36 || device_id.len() != 36 {
        return Err("SFSecurity 签名需要 36 字符的 nonce 和设备编号".to_string());
    }
    let timestamp_text = timestamp.to_string();
    if timestamp_text.len() != 13 {
        return Err("SFSecurity 签名需要 13 位毫秒时间戳".to_string());
    }

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
            *value += 12;
            if *value > b'9' {
                *value += 7;
            }
        } else if (*value > b'9' && *value < b'A') || (*value > b'Z' && *value < b'a') {
            *value += 19;
        }
    }
    Ok(Md5::digest(&mixed)
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect())
}
