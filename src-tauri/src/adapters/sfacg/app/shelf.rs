use super::error::AppApiError;
use super::parsing::{json_i64_field, json_text_field};
use super::AppClient;
use crate::domain::catalog::{PublicShelf, PublicShelfItem};
use serde_json::Value;
use std::cell::Cell;

impl AppClient {
    /// 读取当前账号的书架分类及小说、有声和漫画条目。
    pub(crate) async fn get_bookshelf(
        &self,
        force_refresh: bool,
    ) -> Result<PublicShelf, AppApiError> {
        let query = [("expand", "novels,albums,comics".to_string())];
        let response = if force_refresh {
            self.get_data_uncached("/user/Pockets", &query).await?
        } else {
            self.get_data("/user/Pockets", &query).await?
        };
        parse_bookshelf(&response)
    }

    /// 取消小说、有声专辑或漫画收藏。
    pub(crate) async fn remove_favorite(
        &self,
        kind: &str,
        novel_id: i64,
        media_id: Option<i64>,
    ) -> Result<(), AppApiError> {
        match kind {
            "novel" => {
                self.delete_data(&format!("/user/pockets/novels/{novel_id}"))
                    .await
            }
            "comic" => {
                self.delete_data(&format!("/user/pockets/comic/{novel_id}"))
                    .await
            }
            "audio" => {
                let album_id = media_id
                    .ok_or_else(|| AppApiError::local("有声作品缺少专辑编号".to_string()))?;
                let query = [("expand", "albums".to_string())];
                let response = self.get_data_uncached("/user/Pockets", &query).await?;
                let pocket_ids = audio_pocket_ids(&response, album_id);
                if pocket_ids.is_empty() {
                    return Err(AppApiError::local("书架中未找到该有声作品".to_string()));
                }
                for pocket_id in pocket_ids {
                    self.delete_data(&format!("/pockets/{pocket_id}/albums/{album_id}"))
                        .await?;
                }
                Ok(())
            }
            _ => Err(AppApiError::local("不支持的收藏类型".to_string())),
        }
    }
}

/// 保留解析成功的结果，同时统计被跳过的条目。
fn keep(item: Option<PublicShelfItem>, dropped: &Cell<usize>) -> Option<PublicShelfItem> {
    if item.is_none() {
        dropped.set(dropped.get() + 1);
    }
    item
}

/// 将 `/user/Pockets` 响应转换为书架分类与条目。
///
/// 缺少编号的条目无法继续取详情或取消收藏，只能跳过；跳过的数量会汇总成一条
/// 告警，避免上游字段变化时书架「悄悄少了几本」而无人察觉。
fn parse_bookshelf(response: &Value) -> Result<PublicShelf, AppApiError> {
    let pockets = response
        .as_array()
        .ok_or_else(|| AppApiError::local("SF App 书架未返回分组列表".to_string()))?;
    let mut categories = Vec::new();
    let mut items = Vec::new();
    let dropped = Cell::new(0usize);
    for pocket in pockets {
        let name = json_text_field(pocket, &["name"]).unwrap_or_else(|| "未分类".to_string());
        if !categories.contains(&name) {
            categories.push(name.clone());
        }
        let expand = pocket.get("expand");
        match expand
            .and_then(|value| value.get("novels"))
            .and_then(Value::as_array)
        {
            Some(novels) => items.extend(
                novels
                    .iter()
                    .filter_map(|item| keep(shelf_novel(item), &dropped))
                    .map(|item| (name.clone(), item)),
            ),
            None if pocket.get("novelId").is_some() => {
                if let Some(item) = shelf_novel(pocket) {
                    items.push((name.clone(), item));
                }
            }
            None => {}
        }
        items.extend(
            expand
                .and_then(|value| value.get("albums"))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|item| keep(shelf_album(item), &dropped))
                .map(|item| (name.clone(), item)),
        );
        items.extend(
            expand
                .and_then(|value| value.get("comics"))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|item| keep(shelf_comic(item), &dropped))
                .map(|item| (name.clone(), item)),
        );
    }
    let skipped = dropped.get();
    if skipped > 0 {
        log_warn!("SF App 书架有 {skipped} 个条目缺少编号，已跳过");
    }
    Ok(PublicShelf { categories, items })
}

/// 从书架响应中找出包含指定有声专辑的全部分组编号。
fn audio_pocket_ids(response: &Value, album_id: i64) -> Vec<i64> {
    let mut pocket_ids = Vec::new();
    for pocket in response.as_array().into_iter().flatten() {
        let Some(pocket_id) = pocket
            .get("pocketId")
            .and_then(Value::as_i64)
            .filter(|id| *id != 0)
        else {
            continue;
        };
        let contains_album = pocket
            .get("expand")
            .and_then(|value| value.get("albums"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .any(|album| album.get("albumId").and_then(Value::as_i64) == Some(album_id));
        if contains_album && !pocket_ids.contains(&pocket_id) {
            pocket_ids.push(pocket_id);
        }
    }
    pocket_ids
}

/// 将 App 书架中的小说条目转换为内部记录。
fn shelf_novel(value: &Value) -> Option<PublicShelfItem> {
    Some(PublicShelfItem {
        kind: "novel",
        id: json_i64_field(value, &["novelId"]).filter(|id| *id > 0)?,
        media_id: None,
        title: json_text_field(value, &["novelName"]).unwrap_or_else(|| "未命名作品".to_string()),
        author: json_text_field(value, &["authorName"]).unwrap_or_else(|| "未知作者".to_string()),
        cover: json_text_field(value, &["novelCover"]).unwrap_or_default(),
        source_path: None,
    })
}

/// 将 App 书架中的有声条目转换为内部记录。
fn shelf_album(value: &Value) -> Option<PublicShelfItem> {
    Some(PublicShelfItem {
        kind: "audio",
        id: json_i64_field(value, &["novelId"]).filter(|id| *id > 0)?,
        media_id: json_i64_field(value, &["albumId"]).filter(|id| *id > 0),
        title: json_text_field(value, &["name"]).unwrap_or_else(|| "未命名有声专辑".to_string()),
        author: json_text_field(value, &["authorName"]).unwrap_or_else(|| "未知作者".to_string()),
        cover: json_text_field(value, &["coverMedium", "coverBig", "coverSmall"])
            .unwrap_or_default(),
        source_path: None,
    })
}

/// 将 App 书架中的漫画条目转换为内部记录。
fn shelf_comic(value: &Value) -> Option<PublicShelfItem> {
    Some(PublicShelfItem {
        kind: "comic",
        id: json_i64_field(value, &["comicId"]).filter(|id| *id > 0)?,
        media_id: None,
        title: json_text_field(value, &["comicName"]).unwrap_or_else(|| "未命名漫画".to_string()),
        author: json_text_field(value, &["authorName"]).unwrap_or_else(|| "未知作者".to_string()),
        cover: json_text_field(
            value,
            &["comicCover", "coverBig", "coverMedium", "coverSmall"],
        )
        .unwrap_or_default(),
        source_path: json_text_field(value, &["folderName"]).filter(|folder| {
            folder.len() <= 100 && folder.bytes().all(|byte| byte.is_ascii_alphanumeric())
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::parse_bookshelf;
    use serde_json::json;

    #[test]
    fn groups_items_by_pocket_name_and_keeps_order() {
        let shelf = parse_bookshelf(&json!([{
            "name": "在追",
            "expand": {
                "novels": [
                    { "novelId": 11, "novelName": "第一部", "authorName": "甲" },
                    { "novelId": 12, "novelName": "第二部" }
                ]
            }
        }, {
            "name": "有声",
            "expand": {
                "albums": [{ "novelId": 21, "albumId": 31, "name": "专辑" }],
                "comics": [{ "comicId": 41, "comicName": "漫画", "folderName": "comic41" }]
            }
        }]))
        .expect("解析书架");

        assert_eq!(
            shelf.categories,
            vec!["在追".to_string(), "有声".to_string()]
        );
        assert_eq!(shelf.items.len(), 4);
        assert_eq!(shelf.items[0].0, "在追");
        assert_eq!(shelf.items[0].1.kind, "novel");
        assert_eq!(shelf.items[0].1.id, 11);
        assert_eq!(shelf.items[2].0, "有声");
        assert_eq!(
            shelf.items[2].1.media_id,
            Some(31),
            "有声条目应保留专辑编号"
        );
        assert_eq!(shelf.items[3].1.source_path.as_deref(), Some("comic41"));
    }

    #[test]
    fn skips_entries_without_usable_ids() {
        let shelf = parse_bookshelf(&json!([{
            "name": "在追",
            "expand": {
                "novels": [
                    { "novelId": 0, "novelName": "编号为零" },
                    { "novelName": "缺少编号" },
                    { "novelId": 13, "novelName": "正常" }
                ]
            }
        }]))
        .expect("解析书架");

        assert_eq!(shelf.items.len(), 1, "无法取详情的条目应被跳过");
        assert_eq!(shelf.items[0].1.id, 13);
    }

    #[test]
    fn accepts_flat_pocket_without_expand_array() {
        let shelf = parse_bookshelf(&json!([{
            "name": "单本",
            "novelId": 51,
            "novelName": "扁平条目"
        }]))
        .expect("解析书架");

        assert_eq!(shelf.items.len(), 1);
        assert_eq!(shelf.items[0].1.title, "扁平条目");
    }

    #[test]
    fn rejects_response_without_pocket_array() {
        assert!(parse_bookshelf(&json!({ "data": [] })).is_err());
    }
}
