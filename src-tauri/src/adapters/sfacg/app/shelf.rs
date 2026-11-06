use super::error::AppApiError;
use super::parsing::{json_i64_field, json_text_field};
use super::AppClient;
use crate::domain::catalog::{PublicShelf, PublicShelfItem};
use serde_json::{json, Value};
use std::cell::Cell;

/// 官方读取书架时使用的展开字段。
///
/// 实测请求：`GET /user/Pockets?expand=novels,comics,albums,discount,discountExpireDate,moebattle,novelExtend`。
/// 只传 `novels,albums,comics` 时上游可能不返回条目数组，书架和收藏态就会看似为空。
const BOOKSHELF_EXPAND: &str =
    "novels,comics,albums,discount,discountExpireDate,moebattle,novelExtend";

/// 收藏写操作使用的书架查询参数。
fn bookshelf_query() -> [(&'static str, String); 1] {
    [("expand", BOOKSHELF_EXPAND.to_string())]
}

impl AppClient {
    /// 读取当前账号的书架分类及小说、有声和漫画条目。
    ///
    /// 书架是收藏状态的唯一来源：详情页按钮与书架页都读它。`force_refresh`
    /// 会绕过并重建进程内缓存，收藏写操作之后必须带它才能立刻看到新状态。
    pub(crate) async fn get_bookshelf(
        &self,
        force_refresh: bool,
    ) -> Result<PublicShelf, AppApiError> {
        let query = bookshelf_query();
        let response = if force_refresh {
            self.get_data_uncached("/user/Pockets", &query).await?
        } else {
            self.get_data("/user/Pockets", &query).await?
        };
        parse_bookshelf(&response)
    }

    /// 把小说、有声专辑或漫画加入对应类型的书架分组。
    ///
    /// 收藏状态与移除操作都以 `/user/Pockets` 为唯一来源，因此这里不能改用
    /// `*/favs`——那是不可撤销的点赞接口。官方 App 的口袋按 `typeId` 分成
    /// 漫画(1)、小说(2)、有声(3)、对话小说(4)，把作品加进错误类型的分组会被
    /// 上游以 505「当前分类已经有该作品」拒绝。
    ///
    /// 加入是实时的：实测 POST 返回 201 后书架立即多一本，写后只需让缓存失效。
    pub(crate) async fn add_favorite(
        &self,
        kind: &str,
        novel_id: i64,
        media_id: Option<i64>,
    ) -> Result<(), AppApiError> {
        let target = shelf_target(kind, novel_id, media_id)?;
        let query = bookshelf_query();
        // 内部查询不写缓存：它只是写操作前的状态快照。
        let pockets = self.get_data_typed("/user/Pockets", &query).await?;
        // 作品已在对应类型的分组时直接返回：重复添加会被上游以 505 拒绝，
        // 但目标状态已经达成。
        if shelf_contains(&pockets, &target) {
            return Ok(());
        }
        let pocket_id = pocket_id_for(&pockets, &target).ok_or_else(|| {
            AppApiError::local("书架中没有对应类型的分组，请先在官方 App 创建".to_string())
        })?;
        let mut fields = serde_json::Map::new();
        fields.insert(target.id_key.to_string(), json!(target.entity_id));
        let body = Value::Object(fields);
        match self
            .post_data(&format!("/pockets/{pocket_id}/{}", target.array_key), &body)
            .await
        {
            Ok(_) => {}
            // 并发重复提交时仍可能回「已有该作品」，等价于已收藏。
            Err(error) if error.is_already_in_pocket() => {}
            Err(error) => return Err(error),
        }
        self.invalidate_get("/user/Pockets", &query);
        Ok(())
    }

    /// 把小说、有声专辑或漫画移出书架（取消收藏）。
    ///
    /// 官方 App 5.2.84 在书架管理页使用 `DELETE /user/pockets/entities`，请求体
    /// `{"eids":"<实体编号>","tid":<typeId>}`；依据 `docs/com.sfacg.log.0.txt:3638`
    /// 的真机抓包与 `docs/API_REFERENCE.md` 的闭环实测。这是唯一实时生效的移除
    /// 接口：旧版声明里的 `DELETE /pockets/{pid}/novels/{nid}` 会返回成功信封却
    /// 不动书架，`DELETE /user/pockets/novels/{nid}` 则返回 404，都不应再使用。
    ///
    /// 上游对错误的 `tid` 或不存在的实体同样回 200/204，因此删除后强制回读
    /// `/user/Pockets` 确认，避免把「请求被接受」误报成「已移出书架」。回读会
    /// 顺带把最新书架写回缓存，前端紧接着的刷新看到的就是同一份结果。
    pub(crate) async fn remove_favorite(
        &self,
        kind: &str,
        novel_id: i64,
        media_id: Option<i64>,
    ) -> Result<(), AppApiError> {
        let target = shelf_target(kind, novel_id, media_id)?;
        self.delete_data("/user/pockets/entities", &removal_body(&target))
            .await?;
        let query = bookshelf_query();
        let pockets = self.get_data_uncached("/user/Pockets", &query).await?;
        if shelf_contains(&pockets, &target) {
            return Err(AppApiError::local(
                "移出书架未生效，请重试或检查该作品所在的书架分类".to_string(),
            ));
        }
        Ok(())
    }
}

/// 作品类型对应的口袋条目信息。
///
/// 加入书架与移出书架共用这份映射，避免两处各自维护 `typeId` 而出现偏差。
/// `tid=2` 已由小说抓包确认，漫画/有声按书架分组的 `typeId` 取值；若有偏差，
/// 删除后的回读校验会报错，不会把「请求被接受」当成删除成功。
struct ShelfTarget {
    /// 书架响应里的条目数组名，也是 `POST /pockets/{pid}/{array_key}` 的路径段。
    array_key: &'static str,
    /// 加入书架时使用的编号字段名。
    id_key: &'static str,
    /// 作品在书架里的实体编号：小说/漫画编号，有声为专辑编号。
    entity_id: i64,
    /// 口袋分组的 `typeId`，官方移出书架接口用它区分内容类型。
    type_id: i64,
}

fn shelf_target(
    kind: &str,
    novel_id: i64,
    media_id: Option<i64>,
) -> Result<ShelfTarget, AppApiError> {
    match kind {
        "novel" => Ok(ShelfTarget {
            array_key: "novels",
            id_key: "novelId",
            entity_id: novel_id,
            type_id: 2,
        }),
        "comic" => Ok(ShelfTarget {
            array_key: "comics",
            id_key: "comicId",
            entity_id: novel_id,
            type_id: 1,
        }),
        "audio" => media_id
            .map(|album_id| ShelfTarget {
                array_key: "albums",
                id_key: "albumId",
                entity_id: album_id,
                type_id: 3,
            })
            .ok_or_else(|| AppApiError::local("有声作品缺少专辑编号".to_string())),
        _ => Err(AppApiError::local("不支持的收藏类型".to_string())),
    }
}

/// 官方移出书架请求体；`eids` 是字符串，`tid` 是口袋分组的 `typeId`。
fn removal_body(target: &ShelfTarget) -> Value {
    json!({
        "eids": target.entity_id.to_string(),
        "tid": target.type_id,
    })
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

/// 挑选加入书架的目标口袋分组。
///
/// 分组列表顺序不固定，实测第一个是 typeId=4 的「对话小说」，直接取第一个会把
/// 普通小说加进错误分类。这里优先按 `ShelfTarget.type_id` 匹配，再按 expand
/// 里的条目数组兜底；同类型有多个分组时优先选 `canModify=false` 的系统默认分组。
fn pocket_id_for(response: &Value, target: &ShelfTarget) -> Option<i64> {
    let matches: Vec<&Value> = response
        .as_array()?
        .iter()
        .filter(|pocket| {
            pocket.get("typeId").and_then(Value::as_i64) == Some(target.type_id)
                || pocket
                    .get("expand")
                    .and_then(|expand| expand.get(target.array_key))
                    .is_some()
        })
        .collect();
    let chosen = matches
        .iter()
        .find(|pocket| pocket.get("canModify").and_then(Value::as_bool) == Some(false))
        .copied()
        .or_else(|| matches.first().copied())?;
    chosen
        .get("pocketId")
        .and_then(Value::as_i64)
        .filter(|id| *id > 0)
}

/// 找出包含指定作品的全部书架编号，用于「已入架」判断和删除后的回读校验。
///
/// 只扫描与作品类型对应的条目数组：`chatNovels`（typeId=4 对话小说）是上游
/// 派生的聚合视图，把它算进「已入架」会让添加被幂等短路、界面却看不到书。
fn pocket_ids_for(response: &Value, array_key: &str, id_key: &str, target: i64) -> Vec<i64> {
    let mut pocket_ids = Vec::new();
    for pocket in response.as_array().into_iter().flatten() {
        let Some(pocket_id) = pocket
            .get("pocketId")
            .and_then(Value::as_i64)
            .filter(|id| *id > 0)
        else {
            continue;
        };
        let contains = pocket
            .get("expand")
            .and_then(|expand| expand.get(array_key))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .any(|item| item.get(id_key).and_then(Value::as_i64) == Some(target))
            || pocket.get(id_key).and_then(Value::as_i64) == Some(target);
        if contains && !pocket_ids.contains(&pocket_id) {
            pocket_ids.push(pocket_id);
        }
    }
    pocket_ids
}

/// 书架响应里是否还包含这部作品：加入前的幂等判断与删除后的回读校验共用。
fn shelf_contains(response: &Value, target: &ShelfTarget) -> bool {
    !pocket_ids_for(response, target.array_key, target.id_key, target.entity_id).is_empty()
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
