//! 外部系统适配层。

#[cfg(not(target_os = "android"))]
pub(crate) mod auto_signin;
pub(crate) mod downloads;
pub(crate) mod media;
pub(crate) mod ocr;
pub(crate) mod platform;
pub(crate) mod sfacg;
pub(crate) mod storage;
pub(crate) mod updater;
