//! 三类下载共用的前期准备与收尾提示。
//!
//! 文字、有声、漫画的下载循环各不相同，但进入循环之前的动作完全一致：
//! 连接内容源、定位书籍目录、读取清单、写入作品信息、通报任务已开始。
//! 这些动作集中在这里，避免三份拷贝各自漂移。

use super::JobContext;
use crate::adapters::downloads::registry;
use crate::adapters::sfacg::source::SfSource;
use crate::adapters::storage::{read_book_manifest, write_book_manifest, NativeStorage};
use crate::domain::download::JobStatus;
use crate::domain::error::{AppError, AppResult};
use crate::domain::library::BookManifest;
use crate::domain::settings::DownloadSettings;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

/// 汇总提示里最多列出的失败章节数。
pub(super) const SKIPPED_SUMMARY_LIMIT: usize = 3;

/// 清单里记录已下载章节编号的字段。
#[derive(Clone, Copy)]
pub(super) enum DownloadedField {
    Audio,
    Comic,
}

/// 一次下载任务在进入章节循环前的公共状态。
pub(super) struct BookSetup {
    /// 已认证的内容源。
    pub(super) source: SfSource,
    /// 本地存储入口，部分流程需要它的解码能力。
    pub(super) storage: NativeStorage,
    /// 下载节流与 OCR 等设置。
    pub(super) settings: DownloadSettings,
    /// 书籍目录。
    pub(super) directory: PathBuf,
    /// 已读取的书籍清单。
    pub(super) manifest: BookManifest,
}

/// 连接内容源并准备书籍目录与清单。
///
/// # 参数
/// - `context`：后台任务共享句柄。
/// - `title`：书籍展示名称，同时决定目录名。
///
/// # 错误
/// 客户端创建、目录创建或清单读取失败时返回错误消息。
pub(super) async fn prepare_book(
    context: JobContext<'_>,
    title: &str,
) -> AppResult<BookSetup> {
    let source = SfSource::connect(context.app).await?;
    let storage = NativeStorage::new(context.app.clone());
    let settings = storage
        .settings()
        .map(|settings| settings.download)
        .unwrap_or_default();
    let directory = storage.new_book_directory(title)?;
    let manifest = read_book_manifest(&directory)?;
    Ok(BookSetup {
        source,
        storage,
        settings,
        directory,
        manifest,
    })
}

impl BookSetup {
    /// 写入作品标识后立即落盘，`init` 只负责填自己那一段元数据。
    ///
    /// # 错误
    /// 清单写入失败时返回错误消息。
    pub(super) fn write_identity(
        &mut self,
        init: impl FnOnce(&mut BookManifest),
    ) -> AppResult<()> {
        init(&mut self.manifest);
        self.persist()
    }

    /// 落盘书籍清单。
    ///
    /// # 错误
    /// 清单写入失败时返回错误消息。
    pub(super) fn persist(&self) -> AppResult<()> {
        write_book_manifest(&self.directory, &self.manifest)
    }

    /// 处理元数据补全结果：风控中止任务，其余失败只记日志。
    ///
    /// 作者、简介属于锦上添花，缺失时目录页仍可正常展示；风控则意味着
    /// 后续所有请求都会被拒，必须立刻停下。
    ///
    /// # 错误
    /// 仅当错误被识别为风控时返回该错误。
    pub(super) fn absorb_enrich_failure(&self, result: AppResult<()>) -> AppResult<()> {
        match result {
            Ok(()) => Ok(()),
            Err(error) if error.is_risk_control() => Err(error),
            Err(error) => {
                log_warn!("[download] 元数据补全失败，已跳过：{error}");
                Ok(())
            }
        }
    }

    /// 创建并返回书籍目录下的媒体子目录。
    ///
    /// # 错误
    /// 目录创建失败时返回带 `label` 的错误消息。
    pub(super) fn media_directory(&self, name: &str, label: &str) -> AppResult<PathBuf> {
        let path = self.directory.join(name);
        fs::create_dir_all(&path).map_err(|error| AppError::storage(format!("无法创建{label}目录：{error}")))?;
        Ok(path)
    }

    /// 取走清单里已下载的章节编号，供本轮循环继续累加。
    pub(super) fn take_downloaded_ids(&mut self, field: DownloadedField) -> HashSet<i64> {
        match field {
            DownloadedField::Audio => self
                .manifest
                .downloaded_audio_chapter_ids
                .take()
                .unwrap_or_default()
                .into_iter()
                .collect(),
            DownloadedField::Comic => self
                .manifest
                .downloaded_comic_chapter_ids
                .take()
                .unwrap_or_default()
                .into_iter()
                .collect(),
        }
    }

    /// 把已下载章节编号排序后写回清单并落盘。
    ///
    /// # 错误
    /// 清单写入失败时返回错误消息。
    pub(super) fn save_downloaded_ids(
        &mut self,
        field: DownloadedField,
        ids: &HashSet<i64>,
    ) -> AppResult<()> {
        let mut sorted = ids.iter().copied().collect::<Vec<_>>();
        sorted.sort_unstable();
        match field {
            DownloadedField::Audio => self.manifest.downloaded_audio_chapter_ids = Some(sorted),
            DownloadedField::Comic => self.manifest.downloaded_comic_chapter_ids = Some(sorted),
        }
        self.persist()
    }
}

/// 通知前端作品信息已保存、准备进入章节下载。
pub(super) fn announce_started(context: &JobContext<'_>) {
    let JobContext {
        app,
        state,
        job_id,
        ..
    } = *context;
    registry::update(state, job_id, |job| {
        job.status = JobStatus::Downloading;
        job.progress = 1;
        job.message = "作品信息已保存，准备下载".to_string();
    });
    registry::emit_job(app, state, job_id);
    registry::emit_library(app, state, job_id);
}

/// 生成跳过章节的汇总提示。
///
/// # 参数
/// - `skipped`：被跳过的章节标题与失败原因。
/// - `label`：章节类型，用于提示里的「N 个 {label}未保存」。
pub(super) fn format_skipped(skipped: &[(String, String)], label: &str) -> String {
    let detail = skipped
        .iter()
        .take(SKIPPED_SUMMARY_LIMIT)
        .map(|(title, error)| format!("{title}（{error}）"))
        .collect::<Vec<_>>()
        .join("；");
    let remaining = skipped.len().saturating_sub(SKIPPED_SUMMARY_LIMIT);
    let suffix = (remaining > 0).then(|| format!("；其余 {remaining} 章"));
    format!(
        "{} 个 {label}未保存：{detail}{}。已成功章节已保存，可继续下载失败章节",
        skipped.len(),
        suffix.unwrap_or_default()
    )
}
