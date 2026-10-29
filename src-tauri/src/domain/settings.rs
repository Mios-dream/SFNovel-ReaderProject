//! 持久化用户设置与字典结果。

use serde::{Deserialize, Serialize};

/// 原生网络下载任务使用的请求限制。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RequestPolicy {
    /// 相邻网络请求之间的最小间隔，单位毫秒。
    pub(crate) request_interval_ms: u64,
    /// 同时运行的下载任务上限。
    pub(crate) max_concurrent_downloads: u8,
    /// 是否对下载的图片执行 OCR 恢复。
    #[serde(default)]
    pub(crate) ocr_process_files_enabled: bool,
    /// 是否把书库写入外部存储目录。
    #[serde(default)]
    pub(crate) use_external_storage: bool,
}

impl Default for RequestPolicy {
    fn default() -> Self {
        Self {
            request_interval_ms: 500,
            max_concurrent_downloads: 1,
            ocr_process_files_enabled: false,
            use_external_storage: false,
        }
    }
}

/// 读取或更新正文恢复字典后返回的结果。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContentDictionaryResult {
    /// 当前字典条目总数。
    pub(crate) size: usize,
    /// 本次更新新增的条目数；读取操作不返回该字段。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) added: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::RequestPolicy;

    #[test]
    fn request_policy_contract_uses_camel_case_and_optional_defaults() {
        let policy: RequestPolicy =
            serde_json::from_str(r#"{"requestIntervalMs":250,"maxConcurrentDownloads":3}"#)
                .unwrap();

        assert_eq!(policy.request_interval_ms, 250);
        assert_eq!(policy.max_concurrent_downloads, 3);
        assert!(!policy.ocr_process_files_enabled);
        assert!(!policy.use_external_storage);

        let value = serde_json::to_value(&policy).unwrap();
        assert_eq!(value["requestIntervalMs"], 250);
        assert_eq!(value["maxConcurrentDownloads"], 3);
        assert!(value.get("request_interval_ms").is_none());
    }
}
