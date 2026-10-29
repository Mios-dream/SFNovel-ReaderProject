//! 请求策略与正文恢复字典用例。

use crate::application::ports::{RemoteGateway, SettingsStore};
use crate::domain::error::{AppError, AppResult};
use crate::domain::settings::{ContentDictionaryResult, RequestPolicy};

/// 判断字符是否属于正文比对可使用的汉字区间。
///
/// # 参数
/// - `character`：待判断的 Unicode 字符。
fn is_han_character(character: char) -> bool {
    matches!(
        character,
        '\u{3400}'..='\u{4DBF}' | '\u{4E00}'..='\u{9FFF}' | '\u{F900}'..='\u{FAFF}'
    )
}

/// 负责请求策略校验和正文恢复字典更新的应用服务。
pub(crate) struct SettingsService<'a, S: SettingsStore> {
    /// 设置与字典持久化实现。
    store: &'a S,
}

impl<'a, S: SettingsStore> SettingsService<'a, S> {
    /// 创建设置服务。
    ///
    /// # 参数
    /// - `store`：设置与字典存储实现。
    pub(crate) fn new(store: &'a S) -> Self {
        Self { store }
    }

    /// 读取当前请求策略。
    ///
    /// # 错误
    /// 设置文件不存在、损坏或无法读取时返回错误。
    pub(crate) fn policy(&self) -> AppResult<RequestPolicy> {
        self.store.load_policy()
    }

    /// 校验并保存请求策略。
    ///
    /// # 参数
    /// - `policy`：请求间隔和并发下载数都必须位于允许区间。
    ///
    /// # 错误
    /// 任一字段越界或写入失败时返回错误。
    pub(crate) fn save_policy(&self, policy: RequestPolicy) -> AppResult<RequestPolicy> {
        if !(100..=60_000).contains(&policy.request_interval_ms) {
            return Err(AppError::invalid_input(
                "请求间隔必须在 100 到 60000 毫秒之间",
            ));
        }
        if !(1..=4).contains(&policy.max_concurrent_downloads) {
            return Err(AppError::invalid_input("最大并发下载数必须在 1 到 4 之间"));
        }
        self.store.save_policy(&policy)?;
        Ok(policy)
    }

    /// 返回正文恢复字典的当前规模。
    ///
    /// # 错误
    /// 字典文件无法读取时返回错误。
    pub(crate) fn content_dictionary(&self) -> AppResult<ContentDictionaryResult> {
        self.store.dictionary_size()
    }

    /// 使用 App 与网页正文对齐结果扩展恢复字典。
    ///
    /// # 参数
    /// - `remote`：提供 App 与网页正文的上游接口。
    /// - `chapter_id`：用于比对的文字章节编号。
    ///
    /// # 错误
    /// 上游读取失败、正文汉字数量不一致、映射冲突或字典写入失败时返回错误。
    pub(crate) async fn update_content_dictionary<R: RemoteGateway>(
        &self,
        remote: &R,
        chapter_id: i64,
    ) -> AppResult<ContentDictionaryResult> {
        let api = remote.app_chapter_content(chapter_id).await?;
        let web = remote
            .web_chapter_content(api.novel_id, api.volume_id, chapter_id)
            .await?;
        let source: Vec<char> = api
            .content
            .chars()
            .filter(|character| is_han_character(*character))
            .collect();
        let target: Vec<char> = web
            .chars()
            .filter(|character| is_han_character(*character))
            .collect();
        if source.is_empty() || source.len() != target.len() {
            return Err(AppError::upstream(format!(
                "API 与网页正文无法对齐（API {} 字，网页 {} 字）",
                source.len(),
                target.len()
            )));
        }
        let mut dictionary = self.store.load_dictionary()?;
        let previous_size = dictionary.len();
        for (from, to) in source.into_iter().zip(target) {
            let key = from.to_string();
            let replacement = to.to_string();
            if let Some(existing) = dictionary.get(&key) {
                if existing != &replacement {
                    return Err(AppError::upstream(format!(
                        "映射冲突：{from} 已映射为 {existing}，网页对应 {to}"
                    )));
                }
            } else {
                dictionary.insert(key, replacement);
            }
        }
        let added = dictionary.len().saturating_sub(previous_size);
        self.store.write_dictionary(&dictionary)?;
        Ok(ContentDictionaryResult {
            size: dictionary.len(),
            added: Some(added),
        })
    }
}
