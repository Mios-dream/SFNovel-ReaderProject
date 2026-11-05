use super::error::AppApiError;
use super::parsing::{json_bool_field, json_i64_field, json_text_field, parse_tags};
use super::AppClient;
use crate::domain::catalog::SearchNovel;
use serde_json::Value;

impl AppClient {
    /// 搜索公开小说、有声和漫画，并转换为渲染层使用的记录。
    pub(crate) async fn search_novels(&self, query: &str) -> Result<Vec<SearchNovel>, AppApiError> {
        let response = self
            .get_public_data(
                "/search/novels/result/new",
                &[
                    ("page", "0".to_string()),
                    ("q", query.to_string()),
                    ("size", "12".to_string()),
                    ("sort", "hot".to_string()),
                    ("searchType", "0".to_string()),
                ],
            )
            .await?;
        let mut results = Vec::new();
        for (key, kind) in [
            ("novels", "novel"),
            ("albums", "audio"),
            ("comics", "comic"),
        ] {
            let Some(items) = response.get(key).and_then(Value::as_array) else {
                continue;
            };
            results.extend(items.iter().filter_map(|item| search_novel(item, kind)));
        }
        Ok(results)
    }
}

/// 将搜索或首页接口中的作品对象转换为统一记录。
pub(super) fn search_novel(item: &Value, kind: &str) -> Option<SearchNovel> {
    let novel_id = if kind == "comic" {
        json_i64_field(item, &["comicId"])
    } else {
        json_i64_field(item, &["novelId"])
    }
    .filter(|id| *id > 0)?;
    Some(SearchNovel {
        novel_id,
        media_id: match kind {
            "audio" => json_i64_field(item, &["albumId"]),
            "comic" => Some(novel_id),
            _ => None,
        },
        novel_name: json_text_field(item, &["novelName", "comicName", "name"])
            .unwrap_or_else(|| "未命名作品".to_string()),
        author_name: json_text_field(item, &["authorName", "author"])
            .unwrap_or_else(|| "未知作者".to_string()),
        novel_cover: json_text_field(
            item,
            &[
                "novelCover",
                "coverBig",
                "comicCover",
                "coverMedium",
                "coverSmall",
                "cover",
            ],
        )
        .unwrap_or_default(),
        last_update_time: json_text_field(item, &["lastUpdateTime"]).unwrap_or_default(),
        bookshelf_name: None,
        bookshelf_type: Some(kind.to_string()),
        source_path: (kind == "comic")
            .then(|| json_text_field(item, &["folderName", "sourcePath"]))
            .flatten()
            .filter(|value| {
                !value.is_empty()
                    && value.len() <= 100
                    && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
            }),
        description: json_text_field(item, &["intro", "description"]),
        tags: parse_tags(
            item.get("sysTags")
                .or_else(|| item.get("expand").and_then(|expand| expand.get("sysTags"))),
        ),
        type_id: json_i64_field(item, &["typeId"]),
        is_finish: json_bool_field(item, &["isFinish", "isFinished"]),
        score: item.get("point").and_then(Value::as_f64),
        view_count: json_i64_field(item, &["viewTimes", "visitTimes"]),
        mark_count: json_i64_field(item, &["markCount"]),
        character_count: json_i64_field(item, &["charCount"]),
        latest_chapter_title: json_text_field(item, &["latestChapterTitle"]),
    })
}

#[cfg(test)]
mod tests {
    use super::search_novel;
    use serde_json::json;

    #[test]
    fn maps_comic_entries_to_comic_id_and_media_id() {
        let item = json!({
            "comicId": 41,
            "comicName": "漫画名",
            "authorName": "作者",
            "folderName": "comic41"
        });

        let novel = search_novel(&item, "comic").expect("应解析出漫画条目");

        assert_eq!(novel.novel_id, 41);
        assert_eq!(novel.media_id, Some(41), "漫画的媒体编号即自身编号");
        assert_eq!(novel.novel_name, "漫画名");
        assert_eq!(novel.source_path.as_deref(), Some("comic41"));
        assert_eq!(novel.bookshelf_type.as_deref(), Some("comic"));
    }

    #[test]
    fn reads_album_id_for_audio_entries() {
        let item = json!({ "novelId": 21, "albumId": 31, "name": "专辑" });

        let novel = search_novel(&item, "audio").expect("应解析出有声条目");

        assert_eq!(novel.novel_id, 21);
        assert_eq!(novel.media_id, Some(31));
        assert_eq!(novel.novel_name, "专辑");
    }

    #[test]
    fn requires_positive_id() {
        assert!(search_novel(&json!({ "novelId": 0 }), "novel").is_none());
        assert!(search_novel(&json!({}), "novel").is_none());
    }

    #[test]
    fn rejects_unsafe_comic_source_path() {
        let item = json!({ "comicId": 41, "folderName": "../escape" });

        let novel = search_novel(&item, "comic").expect("仍应返回条目");

        assert!(
            novel.source_path.is_none(),
            "含路径分隔符的来源标识不应透传给下游"
        );
    }

    #[test]
    fn falls_back_to_placeholder_titles() {
        let novel = search_novel(&json!({ "novelId": 7 }), "novel").expect("应解析");

        assert_eq!(novel.novel_name, "未命名作品");
        assert_eq!(novel.author_name, "未知作者");
    }
}
