use super::error::AppApiError;
use super::parsing::{json_i64_field, json_text_field};
use super::search::search_novel;
use super::AppClient;
use crate::domain::catalog::{DiscoverCategory, DiscoverFeed};

impl AppClient {
    /// 读取首页主推、分类、本周排行和最近更新。
    pub(crate) async fn get_discover_feed(
        &self,
        type_id: Option<i64>,
    ) -> Result<DiscoverFeed, AppApiError> {
        let categories = self.get_novel_categories().await?;
        let mut ranking_query = vec![
            ("page", "0".to_string()),
            ("size", "7".to_string()),
            ("expand", "intro,sysTags".to_string()),
        ];
        if let Some(type_id) = type_id {
            ranking_query.push(("typeId", type_id.to_string()));
        }
        let ranking_payload = self
            .get_public_data("/ranks/week/novels", &ranking_query)
            .await?;
        let ranking = ranking_payload
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|item| search_novel(item, "novel"))
            .collect::<Vec<_>>();

        let mut recent_query = vec![
            ("page", "0".to_string()),
            ("size", "9".to_string()),
            ("expand", "intro,sysTags".to_string()),
        ];
        if let Some(type_id) = type_id {
            recent_query.push(("tid", type_id.to_string()));
        }
        let recent_payload = self.get_public_data("/novels", &recent_query).await?;
        let recent = recent_payload
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|item| search_novel(item, "novel"))
            .collect::<Vec<_>>();

        let featured = ranking.first().cloned().or_else(|| recent.first().cloned());
        let ranking = ranking.into_iter().skip(1).take(5).collect();
        Ok(DiscoverFeed {
            featured,
            ranking,
            recent,
            categories,
        })
    }

    /// 读取小说首页使用的分类列表。
    async fn get_novel_categories(&self) -> Result<Vec<DiscoverCategory>, AppApiError> {
        let response = self.get_public_data("/noveltypes", &[]).await?;
        Ok(response
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|item| {
                Some(DiscoverCategory {
                    type_id: json_i64_field(item, &["typeId"]).filter(|id| *id > 0)?,
                    type_name: json_text_field(item, &["typeName"])?,
                })
            })
            .collect())
    }
}
