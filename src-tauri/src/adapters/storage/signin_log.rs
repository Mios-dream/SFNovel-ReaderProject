//! 按登录会话记录最近一次自动签到结果。

use super::atomic::{read_or_default, write_atomically};
use super::paths::app_data_directory;
use crate::domain::error::AppResult;
use crate::domain::signin::SigninNotice;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// 会话指纹到最近一次签到结果。
#[derive(Default, Deserialize, Serialize)]
struct SigninLogFile {
    #[serde(default)]
    records: BTreeMap<String, SigninNotice>,
}

/// 自动签到结果记录。
///
/// 记录按会话 Cookie 指纹分开，切换账号不会看到另一个账号的结果。
pub(crate) struct SigninLog {
    /// 记录文件路径。
    path: PathBuf,
}

impl SigninLog {
    /// 绑定应用数据目录下的记录文件。
    pub(crate) fn for_app(app: &tauri::AppHandle) -> AppResult<Self> {
        Ok(Self::new(
            app_data_directory(app)?.join("signin-state.json"),
        ))
    }

    /// 绑定指定记录文件。
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// 读取该会话最近一次签到结果。
    pub(crate) fn read(&self, cookie: &str) -> AppResult<Option<SigninNotice>> {
        let file: SigninLogFile = read_or_default(&self.path, "自动签到记录")?;
        Ok(file.records.get(&fingerprint(cookie)).cloned())
    }

    /// 写入该会话的签到结果。
    pub(crate) fn save(&self, cookie: &str, notice: &SigninNotice) -> AppResult<()> {
        let mut file: SigninLogFile = read_or_default(&self.path, "自动签到记录")?;
        file.records.insert(fingerprint(cookie), notice.clone());
        write_atomically(&self.path, &file, "自动签到记录")
    }

    /// 读取尚未提示的结果，并标记为已提示。
    pub(crate) fn take_pending(&self, cookie: &str) -> AppResult<Option<SigninNotice>> {
        let mut file: SigninLogFile = read_or_default(&self.path, "自动签到记录")?;
        let key = fingerprint(cookie);
        let Some(notice) = file.records.get(&key).cloned() else {
            return Ok(None);
        };
        if !notice.pending {
            return Ok(None);
        }
        file.records.insert(
            key,
            SigninNotice {
                pending: false,
                ..notice.clone()
            },
        );
        write_atomically(&self.path, &file, "自动签到记录")?;
        Ok(Some(notice))
    }
}

/// 登录 Cookie 的稳定指纹，避免把凭证原文写进记录文件。
fn fingerprint(cookie: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in cookie.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::{fingerprint, SigninLog};
    use crate::adapters::storage::test_support::TempDir;
    use crate::domain::signin::SigninNotice;

    fn notice(day: &str, success: bool, pending: bool) -> SigninNotice {
        SigninNotice {
            day: day.to_string(),
            success,
            message: if success { "签到成功" } else { "签到失败" }.to_string(),
            pending,
        }
    }

    #[test]
    fn records_are_kept_per_session() {
        let directory = TempDir::new("signin-log");
        let log = SigninLog::new(directory.join("signin-state.json"));

        assert!(log.read("a").unwrap().is_none());
        log.save("a", &notice("2026-11-08", true, true)).unwrap();
        log.save("b", &notice("2026-11-07", false, true)).unwrap();

        assert_eq!(log.read("a").unwrap().unwrap().day, "2026-11-08");
        assert_eq!(log.read("b").unwrap().unwrap().day, "2026-11-07");
        assert!(log.read("c").unwrap().is_none());
    }

    #[test]
    fn pending_notice_is_delivered_once() {
        let directory = TempDir::new("signin-log");
        let log = SigninLog::new(directory.join("signin-state.json"));
        log.save("a", &notice("2026-11-08", true, true)).unwrap();

        let delivered = log.take_pending("a").unwrap().expect("首次应返回");
        assert_eq!(delivered.message, "签到成功");
        assert!(log.take_pending("a").unwrap().is_none());
        assert_eq!(log.read("a").unwrap().unwrap().message, "签到成功");
    }

    #[test]
    fn fingerprint_hides_cookie_and_is_stable() {
        let value = fingerprint("session_APP=secret");
        assert_eq!(value, fingerprint("session_APP=secret"));
        assert_ne!(value, fingerprint("session_APP=other"));
        assert!(!value.contains("secret"));
    }
}
