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
                .unwrap_or(false);
                // 登录目录只对当前账户可访问的章节返回 fileName。价格为零
                // 不能单独证明已购，未购的限免章节也可能返回零价。
                let has_audio_source = json_text_field(value, &["fileName", "audioUrl"]).is_some();
                chapters.push(AudioChapter {
                    id: chapter_id,
                    title: json_text_field(value, &["title", "ntitle", "chapterTitle"])
                        .unwrap_or_else(|| format!("章节 {chapter_id}")),
                    volume: volume_title,
                    is_vip,
                    is_unlocked: !is_vip || explicitly_unlocked || has_audio_source,
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
    walk_audio_source(value, 0)
}

/// 在音频详情中递归查找第一个可规范化的音频地址。
///
/// # 参数
/// - `value`：当前遍历的 JSON 节点。
/// - `depth`：当前递归深度，用于限制异常嵌套。
/// # 返回
/// 在音频字段或嵌套对象中找到 HTTP(S) 地址时返回该地址。
fn walk_audio_source(value: &Value, depth: usize) -> Option<String> {
    if depth > 8 {
        return None;
    }
    match value {
        Value::Array(items) => items
            .iter()
            .find_map(|item| walk_audio_source(item, depth + 1)),
        Value::Object(object) => {
            for key in ["fileName", "audioUrl"] {
                if let Some(source) = object
                    .get(key)
                    .and_then(Value::as_str)
                    .and_then(normalize_audio_source)
                {
                    return Some(source);
                }
            }
            for (key, item) in object {
                let normalized_key = key.to_ascii_lowercase();
                if normalized_key.contains("audio") || normalized_key.contains("mp3") {
                    if let Some(source) = item.as_str().and_then(normalize_audio_source) {
                        return Some(source);
                    }
                }
            }
            object
                .values()
                .find_map(|item| walk_audio_source(item, depth + 1))
        }
        _ => None,
    }
}

/// 规范化音频字段中的绝对地址。
fn normalize_audio_source(value: &str) -> Option<String> {
    let source = value.trim().replace("\\/", "/");
    let source = if let Some(relative) = source.strip_prefix("//") {
        format!("https://{relative}")
    } else {
        source
    };
    (source.starts_with("https://") || source.starts_with("http://")).then_some(source)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::{HashMap, HashSet};

    /// 用空上下文遍历一次目录。
    fn walk(value: &Value) -> Vec<AudioChapter> {
        let mut chapters = Vec::new();
        walk_audio_catalog(
            value,
            None,
            None,
            &HashMap::new(),
            &mut HashSet::new(),
            &mut chapters,
        );
        chapters
    }

    #[test]
    fn catalog_title_prefers_album_name_and_skips_chapter_objects() {
        assert_eq!(
            audio_catalog_title(&json!({ "albumName": " 专辑名 ", "name": "书名" })),
            Some("专辑名".to_string())
        );
        assert_eq!(
            audio_catalog_title(&json!({ "albumChapId": 1, "albumName": "章节" })),
            None,
            "带章节字段的对象不能当作专辑标题"
        );
        assert_eq!(audio_catalog_title(&json!({ "albumName": "   " })), None);
    }

    #[test]
    fn volume_entries_keep_order_and_dedupe_by_id() {
        let payload = json!({
            "data": [
                { "volumeId": 2, "volumeTitle": "第二卷" },
                { "volumeId": 1, "volumeTitle": "第一卷" },
                { "volumeId": 2, "volumeTitle": "重复的第二卷" }
            ]
        });

        let volumes = audio_volume_entries(&payload);

        assert_eq!(
            volumes,
            vec![(2, "第二卷".to_string()), (1, "第一卷".to_string())],
            "应保持上游顺序并去掉重复编号"
        );
    }

    #[test]
    fn volume_entries_ignore_chapter_objects() {
        let payload = json!({
            "volumes": [{ "volumeId": 5, "title": "卷" }],
            "chapterList": [{ "volumeId": 9, "chapId": 99, "title": "章节" }]
        });

        let volumes = audio_volume_entries(&payload);

        assert_eq!(volumes, vec![(5, "卷".to_string())]);
    }

    #[test]
    fn walk_assigns_inherited_volume_to_nested_chapters() {
        let payload = json!({
            "volumeList": [{
                "volumeId": 1,
                "title": "第一卷",
                "chapterList": [
                    { "chapId": 11, "title": "第一章" },
                    { "chapId": 12, "title": "第二章" }
                ]
            }]
        });

        let chapters = walk(&payload);

        assert_eq!(
            chapters
                .iter()
                .map(|chapter| chapter.id)
                .collect::<Vec<_>>(),
            vec![11, 12]
        );
        assert!(chapters.iter().all(|chapter| chapter.volume == "第一卷"));
        assert_eq!(chapters[0].title, "第一章");
    }

    #[test]
    fn walk_marks_vip_and_unlock_state() {
        let payload = json!({
            "chapterList": [
                { "chapId": 1, "title": "免费" },
                { "chapId": 2, "title": "限免已购", "needFireMoney": 20 },
                { "chapId": 3, "title": "已解锁", "isVip": true, "isUnlocked": true },
                { "chapId": 4, "title": "有音频源", "isVip": true, "fileName": "a.mp3" },
                { "chapId": 5, "title": "未购", "isVip": true }
            ]
        });

        let chapters = walk(&payload);
        let unlocked: Vec<_> = chapters
            .iter()
            .map(|chapter| (chapter.id, chapter.is_vip, chapter.is_unlocked))
            .collect();

        assert_eq!(
            unlocked,
            vec![
                (1, false, true),
                (2, true, false),
                (3, true, true),
                (4, true, true),
                (5, true, false)
            ],
            "有价章节按 VIP 处理；仅当显式解锁或能拿到音频源时才算已解锁"
        );
    }

    #[test]
    fn walk_dedupes_repeated_chapter_ids() {
        let payload = json!({
            "chapterList": [{ "chapId": 7, "title": "第一次" }],
            "expand": { "chapterList": [{ "chapId": 7, "title": "重复" }, { "chapId": 8 }] }
        });

        let chapters = walk(&payload);

        assert_eq!(
            chapters
                .iter()
                .map(|chapter| chapter.id)
                .collect::<Vec<_>>(),
            vec![7, 8],
            "同一章节编号只保留第一次出现"
        );
        assert_eq!(chapters[0].title, "第一次");
    }

    #[test]
    fn audio_source_normalizes_protocol_relative_url() {
        let source = audio_source_from_payload(&json!({
            "data": { "fileName": "//cdn.example.com/a.mp3" }
        }));

        assert_eq!(source.as_deref(), Some("https://cdn.example.com/a.mp3"));
    }

    #[test]
    fn audio_source_ignores_non_http_values() {
        assert!(
            audio_source_from_payload(&json!({ "fileName": "/relative/a.mp3" })).is_none(),
            "相对路径不能当作可播放地址"
        );
        assert!(audio_source_from_payload(&json!({ "title": "无音频字段" })).is_none());
    }

    #[test]
    fn audio_source_finds_audio_like_keys() {
        let source = audio_source_from_payload(&json!({ "audioUrlV2": "https://x/a.mp3" }));

        assert_eq!(source.as_deref(), Some("https://x/a.mp3"));
    }
}
