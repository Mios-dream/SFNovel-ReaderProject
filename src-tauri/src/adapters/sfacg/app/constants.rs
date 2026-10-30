/// SF App API 的主机地址。
pub(super) const SF_API_HOST: &str = "https://api.sfacg.com";
/// App API HTTP 基础认证使用的公开客户端标识。
pub(super) const SF_API_USER: &str = "androiduser";
/// App API HTTP 基础认证的协议常量。
pub(super) const SF_API_PASSWORD: &str = "1a#$51-yt69;*Acv@qxq";
/// 5.2.84 `SFSecurity` 签名使用的固定盐值。
pub(super) const SF_SECURITY_SALT_V2: &str = "lPQDb9AKO7$LjkPG";
/// App API 要求的响应类型。
pub(super) const APP_ACCEPT: &str = "application/vnd.sfacg.api+json;version=1";
/// 漫画兼容接口在未登录时使用的已登记设备身份。
pub(super) const COMIC_ANONYMOUS_DEVICE_ID: &str = "910d166a-736e-3231-8b21-8d12dfd75f16";
/// 漫画目录需要展开的付费与封面字段。
pub(super) const COMIC_CHAPTER_EXPAND: &str = "needFireMoney,originNeedFireMoney,chapcover";
/// 有声分卷章节需要展开的媒体与销售字段。
pub(super) const AUDIO_CHAPTER_EXPAND: &str =
    "fileName,audioUrl,needFireMoney,originNeedFireMoney,fileSize,duration";
