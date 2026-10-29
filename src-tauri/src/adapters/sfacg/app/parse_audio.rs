//! App 有声目录与音频地址解析。

use super::parsing::{audio_chapter_id, json_bool_field, json_i64_field, json_text_field};
use crate::domain::catalog::AudioChapter;
use serde_json::Value;

/// 从有声专辑详情载荷中提取展示标题。
///
/// 带章节特征字段的对象会被忽略，避免把章节标题误当成专辑标题。
///
/// # 参数
/// - `value`：有声专辑详情 JSON。
///
/// # 返回
/// 找到有效标题时返回去除首尾空白后的字符串。
pub(super) fn audio_catalog_title(value: &Value) -> Option<String> {
    let object = value.as_object()?;
    if object.contains_key("chapId")
        || object.contains_key("albumChapId")
        || object.contains_key("chapterId")
        || object.contains_key("chapterTitle")
        || object.contains_key("audioId")
    {
        return None;
    }
    ["albumName", "name", "novelName", "title"]
        .iter()
        .find_map(|key| {
            object
                .get(*key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|title| !title.is_empty())
                .map(ToString::to_string)
        })
}

/// 收集有声分卷编号与标题，保持上游返回顺序。
pub(super) fn audio_volume_entries(value: &Value) -> Vec<(i64, String)> {
    let mut volumes = Vec::new();
    let mut seen = std::collections::HashSet::new();
    collect_audio_volumes(value, &mut seen, &mut volumes);
    volumes
}

/// 递归收集分卷，忽略带章节字段的对象。
fn collect_audio_volumes(
    value: &Value,
    seen: &mut std::collections::HashSet<i64>,
    volumes: &mut Vec<(i64, String)>,
) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_audio_volumes(item, seen, volumes);
            }
        }
        Value::Object(object) => {
            let has_chapter_list = ["chapterList", "chapters", "chaps", "chapList"]
                .iter()
                .any(|key| object.get(*key).is_some_and(Value::is_array));
            let volume_id = json_i64_field(value, &["volumeId", "volume_id"]);
            let looks_like_chapter = !has_chapter_list && audio_chapter_id(value).is_some();
            if let Some(volume_id) = volume_id.filter(|id| *id > 0 && !looks_like_chapter) {
                if seen.insert(volume_id) {
                    let title =
                        json_text_field(value, &["volumeTitle", "volumeName", "title", "name"])
                            .unwrap_or_else(|| format!("第 {volume_id} 卷"));
                    volumes.push((volume_id, title));
                }
            }
            for key in ["volumeList", "volumes", "data", "expand", "items", "list"] {
                if let Some(child) = object.get(key) {
                    collect_audio_volumes(child, seen, volumes);
                }
            }
        }
        _ => {}
    }
}

/// 递归读取目录中的章节，兼容分卷嵌套、平铺和 `expand` 嵌套结构。
pub(super) fn walk_audio_catalog(
    value: &Value,
    inherited_volume_id: Option<i64>,
    inherited_volume_title: Option<&str>,
    volume_titles: &std::collections::HashMap<i64, String>,
    seen: &mut std::collections::HashSet<i64>,
    chapters: &mut Vec<AudioChapter>,
) {
    match value {
        Value::Array(items) => {
            for item in items {
                walk_audio_catalog(
                    item,
                    inherited_volume_id,
                    inherited_volume_title,
                    volume_titles,
                    seen,
                    chapters,
                );
            }
        }
        Value::Object(object) => {
            let has_chapter_list = [
                "chapterList",
                "chapters",
                "chaps",
                "chapList",
                "list",
                "items",
            ]
            .iter()
            .any(|key| object.get(*key).is_some_and(Value::is_array));
            let chapter_id = audio_chapter_id(value);
            if let Some(chapter_id) = chapter_id.filter(|_| !has_chapter_list) {
                if !seen.insert(chapter_id) {
                    return;
                }
                let volume_id =
                    json_i64_field(value, &["volumeId", "volume_id"]).or(inherited_volume_id);
                let volume_title = json_text_field(value, &["volumeTitle", "volumeName"])
                    .or_else(|| inherited_volume_title.map(ToString::to_string))
                    .or_else(|| volume_id.and_then(|id| volume_titles.get(&id).cloned()))
                    .unwrap_or_else(|| "未分卷".to_string());
                let need_fire_money = json_i64_field(value, &["needFireMoney"]);
                let is_vip = json_bool_field(value, &["isVip", "isVIP", "vip"]).unwrap_or(false)
                    || need_fire_money.unwrap_or(0) > 0;
                let explicitly_unlocked = json_bool_field(
                    value,
                    &[
                        "has",
                        "isUnlocked",
                        "isOrder",
                        "isOrdered",
                        "isBought",
                        "isPurchased",
                    ],
                )
                .unwrap_or(false)
                    || need_fire_money == Some(0);
                chapters.push(AudioChapter {
                    id: chapter_id,
                    title: json_text_field(value, &["title", "ntitle", "chapterTitle"])
                        .unwrap_or_else(|| format!("章节 {chapter_id}")),
                    volume: volume_title,
                    is_vip,
                    is_unlocked: !is_vip || explicitly_unlocked,
                });
                return;
            }

            let volume_id =
                json_i64_field(value, &["volumeId", "volume_id"]).or(inherited_volume_id);
            let volume_title = if has_chapter_list {
                json_text_field(value, &["title", "name"])
            } else {
                None
            }
            .or_else(|| json_text_field(value, &["volumeTitle", "volumeName"]))
            .or_else(|| inherited_volume_title.map(ToString::to_string));
            for key in [
                "volumeList",
                "volumes",
                "chapterList",
                "chapters",
                "chaps",
                "chapList",
                "list",
                "items",
                "data",
                "expand",
            ] {
                if let Some(child) = object.get(key) {
                    walk_audio_catalog(
                        child,
                        volume_id,
                        volume_title.as_deref(),
                        volume_titles,
                        seen,
                        chapters,
                    );
                }
            }
        }
        _ => {}
    }
}

/// 读取有声章节编号。专辑章节编号优先于来源小说章节编号。
pub(super) fn audio_source_from_payload(value: &Value) -> Option<String> {
    let album_id = json_i64_field(value, &["albumId"]);
    walk_audio_source(value, 0, album_id)
}

/// 在音频详情中递归查找第一个可规范化的音频地址。
///
/// # 参数
/// - `value`：当前遍历的 JSON 节点。
/// - `depth`：当前递归深度，用于限制异常嵌套。
/// - `album_id`：用于补全相对音频路径的专辑编号。
///
/// # 返回
/// 找到符合 SF 音频资源规则的 HTTPS 地址时返回该地址。
fn walk_audio_source(value: &Value, depth: usize, album_id: Option<i64>) -> Option<String> {
    if depth > 8 {
        return None;
    }
    match value {
        Value::Array(items) => items
            .iter()
            .find_map(|item| walk_audio_source(item, depth + 1, album_id)),
        Value::Object(object) => {
            for (key, item) in object {
                let normalized_key = key.to_ascii_lowercase();
                if normalized_key.contains("audio") || normalized_key.contains("mp3") {
                    if let Some(source) = item
                        .as_str()
                        .and_then(|value| normalize_audio_source(value, album_id))
                    {
                        return Some(source);
                    }
                }
            }
            object
                .values()
                .find_map(|item| walk_audio_source(item, depth + 1, album_id))
        }
        Value::String(text) => normalize_audio_source(text, album_id),
        _ => None,
    }
}

/// 只接受 SF 音频文件地址，避免把页面或封面图片当成媒体地址。
fn normalize_audio_source(value: &str, album_id: Option<i64>) -> Option<String> {
    let mut source = value.trim().replace("\\/", "/");
    if source.is_empty() {
        return None;
    }
    let lower = source.to_ascii_lowercase();
    let path = lower.split(['?', '#']).next().unwrap_or(&lower);
    let is_audio_path = path.ends_with(".mp3")
        || path.ends_with(".m4a")
        || path.ends_with(".aac")
        || path.contains("/audio/files/");
    if !source.contains("://") && !source.starts_with('/') && is_audio_path {
        let album_id = album_id?;
        source = format!(
            "https://rss.sfacg.com/web/audio/files/{album_id}/{}",
            source.trim_start_matches('/')
        );
    }
    let source = if let Some(relative) = source.strip_prefix("//") {
        format!("https://{relative}")
    } else if source.starts_with('/') {
        format!("https://rss.sfacg.com{source}")
    } else {
        source
    };
    let lower = source.to_ascii_lowercase();
    let path = lower.split(['?', '#']).next().unwrap_or(&lower);
    let is_audio = lower.starts_with("https://")
        && (path.contains("/audio/files/")
            || path.ends_with(".mp3")
            || path.ends_with(".m4a")
            || path.ends_with(".aac"));
    is_audio.then_some(source)
}

#[cfg(test)]
mod tests {
    use super::normalize_audio_source;

    #[test]
    fn resolves_relative_audio_paths_against_the_album() {
        assert_eq!(
            normalize_audio_source("chapter-01.mp3", Some(42)).as_deref(),
            Some("https://rss.sfacg.com/web/audio/files/42/chapter-01.mp3")
        );
        assert_eq!(
            normalize_audio_source("//rss.sfacg.com/web/audio/files/42/a.mp3", Some(7)).as_deref(),
            Some("https://rss.sfacg.com/web/audio/files/42/a.mp3")
        );
    }

    #[test]
    fn rejects_non_audio_or_albumless_relative_sources() {
        assert_eq!(
            normalize_audio_source("https://rss.sfacg.com/cover.jpg", Some(42)),
            None
        );
        assert_eq!(normalize_audio_source("chapter-01.mp3", None), None);
    }
}
