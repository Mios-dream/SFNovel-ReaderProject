use super::error::AppApiError;
use super::parse_detail::{detail_from_audio_payload, detail_from_novel_payload};
use super::AppClient;
use crate::domain::catalog::NovelDetail;
use crate::domain::ids::{validate_album_id, validate_novel_id};
use serde_json::Value;

impl AppClient {
    /// 读取并解析公开小说详情。
    pub(crate) async fn get_novel_details(
        &self,
        novel_id: i64,
    ) -> Result<NovelDetail, AppApiError> {
        validate_novel_id(novel_id)?;
        let detail = self
            .get_public_data(
                &format!("/novels/{novel_id}"),
                &[("expand", "chapterCount,bigBgBanner,bigNovelCover,typeName,intro,fav,ticket,pointCount,sysTags,totalNeedFireMoney,latestchapter".to_string())],
            )
            .await?;
        let resolved_id = detail
            .get("novelId")
            .and_then(Value::as_i64)
            .ok_or_else(|| AppApiError::local("SF 未返回有效小说信息".to_string()))?;
        Ok(detail_from_novel_payload(&detail, resolved_id))
    }

    /// 读取并解析公开有声专辑详情，保留其来源小说编号以兼容既有 IPC 返回值。
    pub(crate) async fn get_audio_details(
        &self,
        album_id: i64,
        novel_id: i64,
    ) -> Result<NovelDetail, AppApiError> {
        validate_album_id(album_id)?;
        validate_novel_id(novel_id)?;
        let detail = self
            .get_public_data(
                &format!("/albums/{album_id}"),
                &[("expand", "intro,typeName,sysTags,latestchapter".to_string())],
            )
            .await?;
        Ok(detail_from_audio_payload(&detail, novel_id))
    }
}
