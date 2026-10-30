//! 下载任务的 Tauri 适配器。

mod engine;
mod registry;

use crate::adapters::sfacg::source::SfSource;
use crate::adapters::storage::NativeStorage;
use crate::application::ports::DownloadGateway;
use crate::domain::download::DownloadJob;
use crate::domain::error::{AppError, AppResult};
use async_trait::async_trait;
use std::sync::Arc;
use tauri::Manager;

pub(crate) use registry::DownloadState;

/// 无状态网关；实际任务保存在 Tauri 管理的 `DownloadState` 中。
pub(crate) struct NativeDownloadGateway {
    /// 访问下载状态、存储目录和事件系统的应用句柄。
    app: tauri::AppHandle,
}

impl NativeDownloadGateway {
    /// 创建原生下载网关。
    ///
    /// # 参数
    /// - `app`：Tauri 应用句柄。
    pub(crate) fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }

    /// 取得 Tauri 管理的下载共享状态。
    fn state(&self) -> tauri::State<'_, DownloadState> {
        self.app.state::<DownloadState>()
    }

    /// 确保下载目标存储当前可写。
    ///
    /// # 错误
    /// 外部存储权限不可用或平台查询失败时返回错误。
    async fn ensure_storage(&self) -> AppResult<()> {
        let storage = NativeStorage::new(self.app.clone());
        if storage
            .ensure_external_storage_access()
            .await
            .map_err(AppError::storage)?
        {
            Ok(())
        } else {
            Err(AppError::permission_denied(
                "外部存储不可用，请授权或在设置中切换为内部存储",
            ))
        }
    }

    /// 创建任务、取消标记和恢复参数，并持久化初始状态。
    ///
    /// # 参数
    /// - `kind`：媒体类型。
    /// - `novel_id`：作品编号。
    /// - `source_id`：可选的专辑或漫画来源编号。
    /// - `source_path`：可选的网页来源路径。
    /// - `title`：任务展示名称。
    /// - `chapter_ids`：待下载章节编号。
    ///
    /// # 错误
    /// 任务注册或快照持久化失败时返回错误。
    fn enqueue(
        &self,
        kind: &str,
        novel_id: i64,
        source_id: Option<i64>,
        source_path: Option<String>,
        title: String,
        chapter_ids: Vec<i64>,
    ) -> AppResult<(DownloadJob, Arc<std::sync::atomic::AtomicBool>)> {
        let state = self.state();
        let created = registry::create(
            &state,
            kind,
            novel_id,
            source_id,
            source_path,
            title,
            chapter_ids,
        )
        .map_err(AppError::storage)?;
        registry::persist(&self.app, &state).map_err(AppError::storage)?;
        Ok(created)
    }
}

#[async_trait]
impl DownloadGateway for NativeDownloadGateway {
    /// 创建文字下载任务并启动后台执行器。
    async fn create_text(
        &self,
        novel_id: i64,
        title: String,
        chapter_ids: Vec<i64>,
    ) -> AppResult<DownloadJob> {
        self.ensure_storage().await?;
        let (job, cancelled) =
            self.enqueue("text", novel_id, None, None, title, chapter_ids.clone())?;
        engine::spawn_text(
            self.app.clone(),
            job.id.clone(),
            novel_id,
            job.title.clone(),
            chapter_ids,
            cancelled,
        );
        Ok(job)
    }

    /// 创建有声下载任务并解析专辑编号。
    ///
    /// # 错误
    /// 有声会话不可用、专辑无法解析或任务创建失败时返回错误。
    async fn create_audio(
        &self,
        novel_id: i64,
        album_id: Option<i64>,
        title: String,
        chapter_ids: Vec<i64>,
    ) -> AppResult<DownloadJob> {
        self.ensure_storage().await?;
        let source = SfSource::connect(&self.app)
            .await
            .map_err(AppError::upstream)?;
        source
            .ensure_audio_access()
            .map_err(AppError::unauthenticated)?;
        let album_id = match album_id {
            Some(album_id) => album_id,
            None => source
                .audio_album_id(novel_id, Some(&title))
                .await
                .map_err(AppError::upstream)?,
        };
        let (job, cancelled) = self.enqueue(
            "audio",
            novel_id,
            Some(album_id),
            None,
            title,
            chapter_ids.clone(),
        )?;
        engine::spawn_audio(
            self.app.clone(),
            job.id.clone(),
            novel_id,
            Some(album_id),
            job.title.clone(),
            chapter_ids,
            cancelled,
        );
        Ok(job)
    }

    /// 创建漫画下载任务并启动后台执行器。
    async fn create_comic(
        &self,
        comic_id: i64,
        source_path: Option<String>,
        title: String,
        chapter_ids: Vec<i64>,
    ) -> AppResult<DownloadJob> {
        self.ensure_storage().await?;
        let (job, cancelled) = self.enqueue(
            "comic",
            comic_id,
            Some(comic_id),
            source_path.clone(),
            title,
            chapter_ids.clone(),
        )?;
        engine::spawn_comic(
            self.app.clone(),
            job.id.clone(),
            comic_id,
            source_path,
            job.title.clone(),
            chapter_ids,
            cancelled,
        );
        Ok(job)
    }

    /// 列出全部下载任务。
    fn list(&self) -> AppResult<Vec<DownloadJob>> {
        registry::list(&self.state()).map_err(AppError::storage)
    }

    /// 请求暂停任务并持久化状态。
    fn pause(&self, job_id: &str) -> AppResult<DownloadJob> {
        let state = self.state();
        let job = registry::pause(&self.app, &state, job_id).map_err(AppError::storage)?;
        Ok(job)
    }

    /// 恢复已暂停任务并重新启动对应媒体执行器。
    fn resume(&self, job_id: &str) -> AppResult<DownloadJob> {
        let state = self.state();
        let (job, spec, cancelled) =
            registry::prepare_resume(&self.app, &state, job_id).map_err(AppError::storage)?;
        engine::spawn_from_spec(self.app.clone(), job.id.clone(), spec, cancelled);
        registry::get(&state, &job.id).map_err(AppError::storage)
    }

    /// 删除任务及恢复参数。
    fn delete(&self, job_id: &str) -> AppResult<()> {
        registry::delete(&self.app, &self.state(), job_id).map_err(AppError::storage)
    }

    /// 从磁盘快照恢复任务列表。
    fn restore(&self) -> AppResult<()> {
        registry::restore(&self.app).map_err(AppError::storage)
    }
}
