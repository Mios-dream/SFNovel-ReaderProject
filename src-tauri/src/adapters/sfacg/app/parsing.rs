//! App API JSON 字段提取工具。

use serde_json::Value;

/// 从有声章节对象读取正数编号。
///
/// 依次兼容专辑章节、来源章节、音频和通用编号字段。
///
/// # 参数
/// - `value`：待解析的 JSON 对象。
///
/// # 返回
/// 找到正整数编号时返回该编号。
pub(super) fn audio_chapter_id(value: &Value) -> Option<i64> {
    json_i64_field(
        value,
        &["albumChapId", "chapterId", "audioId", "chapId", "id"],
    )
    .filter(|id| *id > 0)
}

/// 按候选字段名读取整数，兼容数字和数字字符串。
///
/// # 参数
/// - `value`：待解析的 JSON 对象。
/// - `keys`：按优先级排列的字段名。
///
/// # 返回
/// 返回首个可转换且有符号表示安全的 `i64`。
pub(super) fn json_i64_field(value: &Value, keys: &[&str]) -> Option<i64> {
    let value = json_field(value, keys)?;
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|number| i64::try_from(number).ok()))
        .or_else(|| {
            value
                .as_str()
                .and_then(|text| text.trim().parse::<i64>().ok())
        })
}

/// 按候选字段名读取布尔值，兼容布尔、整数和 `"true"`/`"1"` 字符串。
///
/// # 参数
/// - `value`：待解析的 JSON 对象。
/// - `keys`：按优先级排列的字段名。
///
/// # 返回
/// 返回首个可识别的布尔值。
pub(super) fn json_bool_field(value: &Value, keys: &[&str]) -> Option<bool> {
    let value = json_field(value, keys)?;
    value.as_bool().or_else(|| {
        value.as_i64().map(|number| number != 0).or_else(|| {
            value
                .as_str()
                .map(|text| matches!(text.trim(), "true" | "1"))
        })
    })
}

/// 按候选字段名读取非空文本并去除首尾空白。
///
/// # 参数
/// - `value`：待解析的 JSON 对象。
/// - `keys`：按优先级排列的字段名。
///
/// # 返回
/// 返回首个非空文本字段。
pub(super) fn json_text_field(value: &Value, keys: &[&str]) -> Option<String> {
    json_field(value, keys)?
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToString::to_string)
}

/// 按候选字段名查找 JSON 值，并回退到 `expand` 子对象。
///
/// # 参数
/// - `value`：待查找的 JSON 对象。
/// - `keys`：按优先级排列的字段名。
///
/// # 返回
/// 返回根对象或 `expand` 子对象中的首个匹配值。
pub(super) fn json_field<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| value.get(*key)).or_else(|| {
        let expand = value.get("expand")?;
        keys.iter().find_map(|key| expand.get(*key))
    })
}

/// 按候选字段名读取详情文本，同时兼容根对象和 `expand` 子对象。
///
/// # 参数
/// - `detail`：作品或章节详情 JSON。
/// - `keys`：按优先级排列的字段名。
///
/// # 返回
/// 返回首个非空文本字段。
pub(super) fn payload_text(detail: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        detail
            .get(*key)
            .or_else(|| detail.get("expand").and_then(|expand| expand.get(*key)))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string)
    })
}

/// 解析标签数组，兼容字符串标签和带 `tagName` 的对象标签。
///
/// # 参数
/// - `value`：可选的标签数组 JSON。
///
/// # 返回
/// 数组存在时返回最多 8 个去空白后的非空标签；字段不存在时返回 `None`。
pub(super) fn parse_tags(value: Option<&Value>) -> Option<Vec<String>> {
    value.and_then(Value::as_array).map(|values| {
        values
            .iter()
            .filter_map(|tag| {
                tag.as_str()
                    .or_else(|| tag.get("tagName").and_then(Value::as_str))
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToString::to_string)
            })
            .take(8)
            .collect()
    })
}
