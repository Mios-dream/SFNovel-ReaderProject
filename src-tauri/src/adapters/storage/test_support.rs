//! 存储层测试共用的临时目录。
//!
//! 每个用例独占一个目录，`Drop` 时删除，避免测试之间互相看到对方的文件。

use std::fs;
use std::path::PathBuf;

/// 每个用例独立的临时目录，用后删除。
pub(crate) struct TempDir(pub(crate) PathBuf);

impl TempDir {
    /// 创建一个带随机后缀的临时目录。
    ///
    /// # 参数
    /// - `label`：目录名前缀，便于在系统临时目录中辨认来源。
    pub(crate) fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!("sf-test-{label}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).expect("创建临时目录");
        Self(path)
    }

    /// 拼接目录内的子路径。
    pub(crate) fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
