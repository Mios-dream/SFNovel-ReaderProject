//! 原生应用装配入口。
//!
//! 本模块负责组装 Tauri 运行时、共享状态、平台插件与前端可调用命令；具体
//! 业务实现按 SFACG 通信、本地书库、下载任务和诊断功能拆分在独立源文件中。

/// 带签名的 SF App API 客户端。
mod app_client;
/// 原生运行时健康检查命令。
mod diagnostics;
/// 文本、有声和漫画下载任务。
mod downloads;
/// 按业务能力选择 App 或网页客户端的策略表。
mod endpoint_policy;
/// 本地书库、导出、设置和持久化。
mod library;
/// 桌面端本地图片 OCR worker 桥接。
mod ocr;
/// SFACG 远程查询、认证与会话生命周期。
mod sfacg;
/// 不承载业务规则的原生层共享工具。
mod utils;
/// SF 网页、AJAX 和静态资源客户端。
mod web_client;

use library::restore_native_jobs;
use sfacg::{
    initialize_device_identity, restore_desktop_auth_session, restore_desktop_web_session,
    AuthSessionState, NativeJobState,
};
use tauri::Manager;

#[cfg(target_os = "android")]
use sfacg::android_sfacg_auth_plugin;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// 配置并在当前平台运行 Tauri 应用。
///
/// # 副作用
/// 注册原生状态、IPC 命令和打开器插件；在 Android 注册认证插件；并将已持久化
/// 的未完成下载恢复为暂停状态。函数会阻塞至应用退出。
pub fn run() {
    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default()
        .manage(AuthSessionState::default())
        .manage(NativeJobState::default())
        .setup(|app| {
            #[cfg(target_os = "android")]
            tauri::async_runtime::block_on(initialize_device_identity(&app.handle()))?;
            #[cfg(not(target_os = "android"))]
            initialize_device_identity(&app.handle())?;
            // 恢复快照格式错误不能阻止应用启动；用户可在下载队列中手动重建任务。
            let _ = restore_native_jobs(&app.handle());
            #[cfg(target_os = "windows")]
            if let Ok(Some(session)) = restore_desktop_auth_session(&app.handle()) {
                if let Ok(mut current) = app.state::<AuthSessionState>().session.lock() {
                    *current = Some(session);
                }
            }
            #[cfg(target_os = "windows")]
            if let Ok(Some(web_session)) = restore_desktop_web_session(&app.handle()) {
                if let Ok(mut current) = app.state::<AuthSessionState>().session.lock() {
                    if let Some(existing) = current.as_mut() {
                        existing.web_cookie = web_session.web_cookie;
                    } else {
                        *current = Some(web_session);
                    }
                }
            }
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init());
    #[cfg(target_os = "android")]
    {
        builder = builder.plugin(android_sfacg_auth_plugin());
    }
    builder
        .invoke_handler(tauri::generate_handler![
            diagnostics::greet,
            diagnostics::app_runtime_version,
            library::list_local_library,
            library::get_request_policy,
            library::save_request_policy,
            library::get_content_dictionary,
            library::update_content_dictionary,
            sfacg::search_novels,
            sfacg::get_novel_details,
            sfacg::get_audio_details,
            sfacg::get_chapter_volumes,
            sfacg::get_audio_chapters,
            sfacg::get_comic_chapters,
            sfacg::get_comic_details,
            sfacg::get_bookshelf,
            library::get_local_book,
            library::get_local_chapter,
            library::get_local_comic_chapter,
            library::delete_local_book,
            library::ensure_external_storage_access,
            library::export_local_book,
            downloads::create_text_download,
            downloads::create_audio_download,
            downloads::create_comic_download,
            downloads::list_download_jobs,
            downloads::pause_download_job,
            downloads::resume_download_job,
            downloads::delete_download_job,
            sfacg::auth_status,
            sfacg::login_with_password,
            sfacg::start_official_login,
            sfacg::logout,
            sfacg::logout_app_session,
            sfacg::logout_web_session,
            sfacg::verify_authenticated_request,
            sfacg::get_user_profile,
            sfacg::get_web_user_profile
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
