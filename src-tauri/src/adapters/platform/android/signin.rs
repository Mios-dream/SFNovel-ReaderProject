//! Java 侧自动签到的 JNI 入口。
//!
//! 与桌面端共用同一套判断：只提交当前业务日，历史日期需要补签卡因此不自动补签。
//! 上游 0 点到 1 点维护，窗口内直接跳过，避免把维护记成签到失败。
//! 结果按会话 Cookie 落盘，返回结构化结论，由 Java 侧决定是否需要退避重试。

use crate::adapters::sfacg::device::{
    install_device_profile, install_device_state_path, is_valid_uuid, DeviceProfile,
};
use crate::adapters::storage::SigninLog;
use crate::domain::error::{AppError, AppResult};
use crate::domain::signin::{business_day, is_maintenance, is_signed_on, SigninNotice};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;

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

/// 返回给 Java 侧的签到结论。
#[derive(Serialize)]
struct AutoSigninOutcome {
    /// 是否需要在退避窗口后重试。
    #[serde(rename = "retry")]
    needs_retry: bool,
    /// 本次是否真的提交并签到成功。已签过、维护时段或未登录时为 `false`，
    /// Java 侧据此决定要不要发通知——只有当天第一次成功才值得提醒。
    signed: bool,
    /// 写入系统日志的结论文本。
    message: String,
}

impl AutoSigninOutcome {
    /// 本次检查已结束，无需重试。
    fn done(message: String) -> Self {
        Self {
            needs_retry: false,
            signed: false,
            message,
        }
    }
}

/// 执行一次自动签到并返回面向日志的结论。
///
/// # 参数
/// - `data_dir`：应用数据目录，用于定位设备登记状态与签到记录。
/// - `cookie`：App 会话 Cookie。
/// - `device_json`：Java 侧序列化的设备特征。
fn run_auto_signin(
    data_dir: String,
    cookie: String,
    device_json: String,
) -> AppResult<AutoSigninOutcome> {
    if cookie.trim().is_empty() {
        return Ok(AutoSigninOutcome::done("未登录，跳过自动签到".to_string()));
    }
    let now = SystemTime::now();
    if is_maintenance(now) {
        return Ok(AutoSigninOutcome::done(
            "上游维护时段（0-1 点），跳过自动签到".to_string(),
        ));
    }
    let profile = serde_json::from_str::<AutoSigninDeviceProfile>(&device_json)
        .map_err(|error| AppError::invalid_input(format!("Android 设备信息无效：{error}")))?;
    let device = auto_signin_device(profile)?;
    let directory = PathBuf::from(data_dir);
    install_device_state_path(directory.join("device-state.json"));
    install_device_profile(device);
    let client = crate::adapters::sfacg::app::AppClient::new(Some(cookie.clone()))?;
    if !client.has_session() {
        return Ok(AutoSigninOutcome::done(
            "App 会话不可用，跳过自动签到".to_string(),
        ));
    }
    let day = business_day(now);
    let log = SigninLog::new(directory.join("signin-state.json"));
    let previous = match log.read(&cookie) {
        Ok(notice) => notice,
        Err(error) => {
            log_warn!("自动签到记录不可读：{error}");
            None
        }
    };
    if previous
        .as_ref()
        .is_some_and(|notice| notice.day == day && notice.success)
    {
        return Ok(AutoSigninOutcome::done(format!("{day} 已完成签到")));
    }
    let result: AppResult<bool> = tauri::async_runtime::block_on(async {
        let week = client.get_signin_week().await?;
        if is_signed_on(&week, &day) {
            return Ok(false);
        }
        client.submit_signin(&day).await?;
        Ok(true)
    });
    let needs_retry = result.as_ref().err().is_some_and(should_retry);
    let signed = matches!(result.as_ref(), Ok(true));
    let notice = SigninNotice::new(&day, result, previous.as_ref());
    if let Err(error) = log.save(&cookie, &notice) {
        log_warn!("自动签到记录写入失败：{error}");
    }
    Ok(AutoSigninOutcome {
        needs_retry,
        signed,
        message: format!("{} {}", notice.day, notice.message),
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

/// 登录失效、风控或任务取消时重复尝试没有意义，只在其余错误上重试。
fn should_retry(error: &AppError) -> bool {
    !error.is_unauthenticated() && !error.is_risk_control() && !error.is_cancelled()
}

/// Java 侧 `AutoSigninBridge.run` 的原生实现。
///
/// 符号名必须按 JNI 规则转义包名里的下划线：`sf_novel_flow` 中的 `_` 写作 `_1`，
/// 即 `sf_1novel_1flow`。不转义时 JVM 会去查找一个不存在的符号并抛
/// `UnsatisfiedLinkError`，且这个错误只在真正调用时才会暴露。
#[no_mangle]
pub extern "system" fn Java_com_sansan_sf_1novel_1flow_AutoSigninBridge_run<'local>(
    mut env: jni::JNIEnv<'local>,
    _this: jni::objects::JObject<'local>,
    data_dir: jni::objects::JString<'local>,
    cookie: jni::objects::JString<'local>,
    device_json: jni::objects::JString<'local>,
) -> jni::sys::jstring {
    let result = (|| -> AppResult<AutoSigninOutcome> {
        let data_dir = java_string(&mut env, &data_dir)?;
        let cookie = java_string(&mut env, &cookie)?;
        let device_json = java_string(&mut env, &device_json)?;
        run_auto_signin(data_dir, cookie, device_json)
    })();
    let outcome = match result {
        Ok(outcome) => outcome,
        Err(error) => AutoSigninOutcome {
            needs_retry: should_retry(&error),
            signed: false,
            message: format!("自动签到失败：{error}"),
        },
    };
    let payload = serde_json::to_string(&outcome)
        .unwrap_or_else(|_| "{\"retry\":true,\"message\":\"自动签到结果序列化失败\"}".to_string());
    env.new_string(payload)
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
