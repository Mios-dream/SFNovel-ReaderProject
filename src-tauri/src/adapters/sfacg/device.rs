//! 官方 App 请求画像和设备身份。

use std::sync::OnceLock;
use uuid::Uuid;

/// 模拟的官方 App 版本号，参与请求头和签名生成。
pub(crate) const SF_APP_VERSION: &str = "5.2.36";
/// 网页请求使用的桌面浏览器 User-Agent。
pub(crate) const SF_WEB_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";

/// 模拟设备的 Android API 级别。
const SF_ANDROID_SDK: u32 = 34;
/// 模拟设备的 Android 版本。
const SF_ANDROID_RELEASE: &str = "14";
/// 模拟设备型号。
const SF_DEVICE_MODEL: &str = "Pixel 7";
/// 模拟设备的系统构建号。
const SF_DEVICE_BUILD_ID: &str = "UP1A.231005.007";
/// 模拟设备的主 CPU 架构。
const SF_DEVICE_ABI: &str = "arm64-v8a";
/// 模拟 WebView 的 Chromium 版本。
const SF_WEBVIEW_VERSION: &str = "120.0.6099.230";

/// 一次安装内保持稳定、供所有 App 请求共用的设备身份。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DeviceProfile {
    /// 固定的小写 UUID 设备编号。
    pub(crate) device_id: String,
    /// 固定的大写 UUID 设备令牌。
    pub(crate) device_token: String,
}

impl DeviceProfile {
    /// 为桌面端生成一组随机的设备编号和令牌。
    ///
    /// # 返回
    /// 返回本次安装后续请求可复用的设备身份。
    #[cfg(not(target_os = "android"))]
    pub(crate) fn generate() -> Self {
        Self {
            device_id: Uuid::new_v4().hyphenated().to_string().to_lowercase(),
            device_token: Uuid::new_v4().hyphenated().to_string().to_uppercase(),
        }
    }

    /// 生成 App API 请求使用的 User-Agent。
    ///
    /// # 返回
    /// 返回包含 App 版本和设备编号的固定格式字符串。
    pub(crate) fn user_agent(&self) -> String {
        format!(
            "boluobao/{SF_APP_VERSION}(android;{SF_ANDROID_SDK})/H5/{}/H5",
            self.device_id
        )
    }

    /// 生成 App 原生网络库风格的 `Origin` User-Agent。
    ///
    /// # 返回
    /// 返回模拟 Android 系统与机型信息的字符串。
    pub(crate) fn origin_ua(&self) -> String {
        format!(
            "Dalvik/2.1.0 (Linux; U; Android {SF_ANDROID_RELEASE}; {SF_DEVICE_MODEL} Build/{SF_DEVICE_BUILD_ID})"
        )
    }

    /// 返回模拟设备使用的 CPU 架构。
    pub(crate) fn abi(&self) -> &'static str {
        SF_DEVICE_ABI
    }

    /// 生成模拟 Android WebView 的 User-Agent。
    ///
    /// # 返回
    /// 返回包含 Android、机型和 Chromium 版本信息的字符串。
    pub(crate) fn web_view_ua(&self) -> String {
        format!(
            "Mozilla/5.0 (Linux; Android {SF_ANDROID_RELEASE}; {SF_DEVICE_MODEL} Build/{SF_DEVICE_BUILD_ID}; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/{SF_WEBVIEW_VERSION} Mobile Safari/537.36"
        )
    }
}

/// 进程内只初始化一次的设备身份。
static DEVICE_PROFILE: OnceLock<DeviceProfile> = OnceLock::new();

/// 安装全局设备身份；重复调用会保留首次安装的值。
///
/// # 参数
/// - `profile`：应用启动阶段解析或生成的设备身份。
pub(crate) fn install_device_profile(profile: DeviceProfile) {
    let _ = DEVICE_PROFILE.set(profile);
}

/// 读取已安装的全局设备身份。
///
/// # 返回
/// 返回在整个进程生命周期内有效的设备身份引用。
///
/// # 错误
/// 设备身份尚未初始化时返回错误。
pub(crate) fn device_profile() -> Result<&'static DeviceProfile, String> {
    DEVICE_PROFILE
        .get()
        .ok_or_else(|| "设备身份尚未初始化，请重启应用".to_string())
}

/// 判断字符串是否为合法 UUID。
///
/// # 参数
/// - `value`：待校验的字符串。
pub(crate) fn is_valid_uuid(value: &str) -> bool {
    Uuid::parse_str(value).is_ok()
}
