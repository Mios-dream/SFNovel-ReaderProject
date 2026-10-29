//! 公共标识校验。

/// 校验上游作品、专辑或漫画编号。
///
/// # 参数
/// - `value`：待校验的正整数编号。
/// - `label`：错误消息中使用的资源名称。
///
/// # 错误
/// `value` 小于等于 0 时返回带资源名称的错误消息。
pub(crate) fn validate_positive_id(value: i64, label: &str) -> Result<(), String> {
    if value <= 0 {
        return Err(format!("{label}编号无效"));
    }
    Ok(())
}

/// 校验公开 SF 小说编号。
///
/// # 参数
/// - `novel_id`：调用方传入的小说编号。
///
/// # 错误
/// 编号小于等于 0 时返回错误消息。
pub(crate) fn validate_novel_id(novel_id: i64) -> Result<(), String> {
    validate_positive_id(novel_id, "小说")
}
