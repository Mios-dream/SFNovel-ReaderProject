//! 官方 App 请求画像和设备身份。

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use uuid::Uuid;

/// 模拟的官方 App 版本号，参与请求头和签名生成。
pub(crate) const SF_APP_VERSION: &str = "5.2.84";
/// 网页请求使用的桌面浏览器 User-Agent。
pub(crate) const SF_WEB_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";
/// App 设备登记使用的包名。
pub(crate) const SF_PACKAGE: &str = "com.sfacg";

/// 模拟设备的 Android API 级别。
const SF_ANDROID_SDK: u32 = 36;
/// 模拟设备的 Android 版本。
const SF_ANDROID_RELEASE: &str = "16";
/// 模拟设备型号。
const SF_DEVICE_MODEL: &str = "V2238A";
/// 模拟设备的系统构建号。
const SF_DEVICE_BUILD_ID: &str = "BP2A.250605.031.A3";
/// 模拟设备的主 CPU 架构。
const SF_DEVICE_ABI: &str = "arm64-v8a";
/// 模拟设备的厂商名，设备体使用小写。
const SF_DEVICE_MANUFACTURER: &str = "vivo";
/// 模拟 WebView 的 Chromium 版本。
const SF_WEBVIEW_VERSION: &str = "138.0.7204.179";

/// 一次安装内保持稳定的设备身份和登记状态。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DeviceProfile {
    /// 固定的小写 UUID 设备编号。
    pub(crate) device_id: String,
    /// 设备登记返回的 OAID；无法获取时为空。
    pub(crate) oa_id: String,
    /// 设备登记返回的渠道，参与 User-Agent 最后一段。
    pub(crate) init_channel: String,
    /// Android API 级别。
    pub(crate) sdk: u32,
    /// 主 CPU 架构。
    pub(crate) abi: String,
    /// 厂商名，设备体使用小写。
    pub(crate) manufacturer: String,
    /// 厂商推送 token；无法获取时为空。
    pub(crate) manufacturer_token: String,
    /// UMeng 推送 token；无法获取时为空。
    pub(crate) push_token: String,
    /// Android 网络库 User-Agent。
    pub(crate) origin_ua: String,
    /// WebView User-Agent。
    pub(crate) web_view_ua: String,
}

impl DeviceProfile {
    /// 为桌面端生成一组稳定的模拟设备身份。
    ///
    /// # 返回
    /// 返回本次安装后续请求可复用的设备身份。
    pub(crate) fn generate() -> Self {
        Self {
            device_id: Uuid::new_v4().hyphenated().to_string().to_lowercase(),
            oa_id: String::new(),
            init_channel: String::new(),
            sdk: SF_ANDROID_SDK,
            abi: SF_DEVICE_ABI.to_string(),
            manufacturer: SF_DEVICE_MANUFACTURER.to_string(),
            manufacturer_token: String::new(),
            push_token: String::new(),
            origin_ua: format!(
                "Dalvik/2.1.0 (Linux; U; Android {SF_ANDROID_RELEASE}; {SF_DEVICE_MODEL} Build/{SF_DEVICE_BUILD_ID})"
            ),
            web_view_ua: format!(
                "Mozilla/5.0 (Linux; Android {SF_ANDROID_RELEASE}; {SF_DEVICE_MODEL} Build/{SF_DEVICE_BUILD_ID}; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/{SF_WEBVIEW_VERSION} Mobile Safari/537.36"
            ),
        }
    }

    /// 生成 App API 请求使用的 User-Agent。
    ///
    /// 渠道尚未登记时，最后一段使用大写的厂商名。
    pub(crate) fn user_agent(&self) -> String {
        let manufacturer = self.manufacturer.to_uppercase();
        let channel = if self.init_channel.is_empty() {
            manufacturer.clone()
        } else {
            self.init_channel.clone()
        };
        format!(
            "boluobao/{SF_APP_VERSION}(android;{})/{manufacturer}/{}/{channel}",
            self.sdk, self.device_id
        )
    }

    /// 返回 App 原生网络库风格的 `originUa`。
    pub(crate) fn origin_ua(&self) -> &str {
        &self.origin_ua
    }

    /// 返回设备 CPU 架构。
    pub(crate) fn abi(&self) -> &str {
        &self.abi
    }

    /// 返回模拟或实际 Android WebView 的 User-Agent。
    pub(crate) fn web_view_ua(&self) -> &str {
        &self.web_view_ua
    }

    /// 返回设备登记使用的推送信息。
    pub(crate) fn push_info(&self) -> serde_json::Value {
        if self.push_token.is_empty() {
            serde_json::json!([{ "channelName": "umeng" }])
        } else {
            serde_json::json!([{ "channelName": "umeng", "token": self.push_token }])
        }
    }
}

/// 进程内共享的设备身份。
static DEVICE_PROFILE: OnceLock<Mutex<DeviceProfile>> = OnceLock::new();
/// 设备登记结果落盘路径。
static DEVICE_STATE_PATH: OnceLock<PathBuf> = OnceLock::new();

/// 设备登记后需要跨启动复用的字段。
#[derive(Default, Deserialize, Serialize)]
struct PersistedDeviceState {
    #[serde(default)]
    device_id: String,
    #[serde(default)]
    oa_id: String,
    #[serde(default)]
    init_channel: String,
}

/// 设置设备登记结果的落盘路径。
pub(crate) fn install_device_state_path(path: PathBuf) {
    let _ = DEVICE_STATE_PATH.set(path);
}

/// 读取上次保存的设备登记结果。
fn load_device_state() -> PersistedDeviceState {
    let Some(path) = DEVICE_STATE_PATH.get() else {
        return PersistedDeviceState::default();
    };
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// 保存设备登记结果。
fn save_device_state(profile: &DeviceProfile) {
    let Some(path) = DEVICE_STATE_PATH.get() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let state = PersistedDeviceState {
        device_id: profile.device_id.clone(),
        oa_id: profile.oa_id.clone(),
        init_channel: profile.init_channel.clone(),
    };
    if let Ok(text) = serde_json::to_string(&state) {
        let _ = fs::write(path, text);
    }
}

/// 安装或替换全局设备身份。
///
/// # 参数
/// - `profile`：应用启动阶段解析或设备登记后更新的设备身份。
pub(crate) fn install_device_profile(profile: DeviceProfile) {
    let state = load_device_state();
    let mut profile = profile;
    if state.device_id == profile.device_id {
        if profile.oa_id.is_empty() {
            profile.oa_id = state.oa_id;
        }
        if profile.init_channel.is_empty() {
            profile.init_channel = state.init_channel;
        }
    }
    set_device_profile(profile);
}

/// 替换运行时设备身份并保存登记结果。
pub(crate) fn replace_device_profile(profile: DeviceProfile) {
    save_device_state(&profile);
    set_device_profile(profile);
}

/// 写入进程内设备身份。
fn set_device_profile(profile: DeviceProfile) {
    match DEVICE_PROFILE.get() {
        Some(current) => {
            if let Ok(mut guard) = current.lock() {
                *guard = profile;
            }
        }
        None => {
            let _ = DEVICE_PROFILE.set(Mutex::new(profile));
        }
    }
}

/// 读取已安装的全局设备身份快照。
///
/// # 错误
/// 设备身份尚未初始化或状态锁不可用时返回错误。
pub(crate) fn device_profile() -> Result<DeviceProfile, String> {
    DEVICE_PROFILE
        .get()
        .ok_or_else(|| "设备身份尚未初始化，请重启应用".to_string())?
        .lock()
        .map(|guard| guard.clone())
        .map_err(|_| "设备身份状态不可用".to_string())
}

/// 判断字符串是否为合法 UUID。
///
/// # 参数
/// - `value`：待校验的字符串。
pub(crate) fn is_valid_uuid(value: &str) -> bool {
    Uuid::parse_str(value).is_ok()
}
