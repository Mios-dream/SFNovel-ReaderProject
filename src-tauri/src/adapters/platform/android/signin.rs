//! Java 侧自动签到的 JNI 入口。

use crate::adapters::sfacg::device::{
    install_device_profile, install_device_state_path, is_valid_uuid, DeviceProfile,
};
use crate::domain::error::{AppError, AppResult};
use serde::Deserialize;
use std::path::PathBuf;

/// Java 侧传入的设备特征，字段齐全且非可选。
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AutoSigninDeviceProfile {
    device_id: String,
    sdk: u32,
    abi: String,
    manufacturer: String,
    origin_ua: String,
    #[serde(rename = "webViewUA")]
    web_view_ua: String,
}

/// 执行一次自动签到并返回面向用户的结论。
///
/// # 参数
/// - `data_dir`：应用数据目录，用于定位设备登记状态。
/// - `cookie`：App 会话 Cookie。
/// - `device_json`：Java 侧序列化的设备特征。
/// - `date`：待签到日期，格式为 `YYYY-MM-DD`。
///
/// # 错误
/// 设备信息无效、日期格式错误或签到请求失败时返回错误。
fn run_auto_signin(
    data_dir: String,
    cookie: String,
    device_json: String,
    date: String,
) -> AppResult<String> {
    if cookie.trim().is_empty() {
        return Ok("未登录，跳过自动签到".to_string());
    }
    if date.len() != 10 {
        return Err(AppError::invalid_input("签到日期格式无效"));
    }
    let profile = serde_json::from_str::<AutoSigninDeviceProfile>(&device_json)
        .map_err(|error| AppError::invalid_input(format!("Android 设备信息无效：{error}")))?;
    let device = auto_signin_device(profile)?;
    install_device_state_path(PathBuf::from(data_dir).join("device-state.json"));
    install_device_profile(device);
    let client = crate::adapters::sfacg::app::AppClient::new(Some(cookie))?;
    if !client.has_session() {
        return Ok("App 会话不可用，跳过自动签到".to_string());
    }
    tauri::async_runtime::block_on(async move {
        let week = client.get_signin_week().await?;
        if week
            .iter()
            .any(|item| item.date.get(..10) == Some(date.as_str()) && item.is_sign_in)
        {
            return Ok(format!("{date} 已签到"));
        }
        client.submit_signin(&date).await?;
        Ok(format!("{date} 自动签到成功"))
    })
}

/// 用 Java 侧采集值覆盖随机生成的设备特征，空值保留原值。
///
/// # 参数
/// - `profile`：Java 侧传入的设备特征。
fn auto_signin_device(profile: AutoSigninDeviceProfile) -> AppResult<DeviceProfile> {
    if !is_valid_uuid(&profile.device_id) {
        return Err(AppError::invalid_input("Android 设备编号无效"));
    }
    let mut device = DeviceProfile::generate();
    device.device_id = profile.device_id.to_lowercase();
    if profile.sdk > 0 {
        device.sdk = profile.sdk;
    }
    if !profile.abi.trim().is_empty() {
        device.abi = profile.abi;
    }
    if !profile.manufacturer.trim().is_empty() {
        device.manufacturer = profile.manufacturer.to_lowercase();
    }
    if !profile.origin_ua.trim().is_empty() {
        device.origin_ua = profile.origin_ua;
    }
    if !profile.web_view_ua.trim().is_empty() {
        device.web_view_ua = profile.web_view_ua;
    }
    Ok(device)
}

/// Java 侧 `AutoSigninBridge.run` 的原生实现。
#[no_mangle]
pub extern "system" fn Java_com_sansan_sf_novel_flow_AutoSigninBridge_run<'local>(
    mut env: jni::JNIEnv<'local>,
    _this: jni::objects::JObject<'local>,
    data_dir: jni::objects::JString<'local>,
    cookie: jni::objects::JString<'local>,
    device_json: jni::objects::JString<'local>,
    date: jni::objects::JString<'local>,
) -> jni::sys::jstring {
    let result = (|| -> AppResult<String> {
        let data_dir = java_string(&mut env, &data_dir)?;
        let cookie = java_string(&mut env, &cookie)?;
        let device_json = java_string(&mut env, &device_json)?;
        let date = java_string(&mut env, &date)?;
        run_auto_signin(data_dir, cookie, device_json, date)
    })();
    let message = match result {
        Ok(message) => message,
        Err(error) => format!("自动签到失败：{error}"),
    };
    env.new_string(message)
        .map(jni::objects::JString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

/// 读取 JNI 字符串参数。
///
/// # 参数
/// - `env`：当前 JNI 环境。
/// - `value`：Java 传入的字符串对象。
fn java_string(env: &mut jni::JNIEnv<'_>, value: &jni::objects::JString<'_>) -> AppResult<String> {
    env.get_string(value)
        .map(Into::into)
        .map_err(|error| AppError::storage(format!("读取 JNI 字符串失败：{error}")))
}
