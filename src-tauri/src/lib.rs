//! Native application assembly. Domain implementation lives in focused source files.

include!("sfacg.rs");
include!("library.rs");
include!("downloads.rs");
include!("diagnostics.rs");

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Configures and runs the Tauri application on the current platform.
///
/// # Side effects
/// Registers native state, commands, the opener plugin, Android authentication
/// support when applicable, and restores persisted download jobs as paused.
/// The function blocks until the application exits.
pub fn run() {
    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default()
        .manage(AuthSessionState::default())
        .manage(NativeJobState::default())
        .setup(|app| {
            // A malformed recovery snapshot must not block application startup.
            let _ = restore_native_jobs(&app.handle());
            #[cfg(target_os = "windows")]
            if let Ok(Some(session)) = restore_desktop_auth_session(&app.handle()) {
                if let Ok(mut current) = app.state::<AuthSessionState>().session.lock() {
                    *current = Some(session);
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
            greet,
            app_runtime_version,
            list_local_library,
            get_request_policy,
            save_request_policy,
            get_content_dictionary,
            update_content_dictionary,
            search_novels,
            get_novel_details,
            get_chapter_volumes,
            get_audio_chapters,
            get_bookshelf,
            get_local_book,
            get_local_chapter,
            delete_local_book,
            export_local_book,
            create_text_download,
            create_audio_download,
            list_download_jobs,
            pause_download_job,
            resume_download_job,
            delete_download_job,
            auth_status,
            login_with_password,
            start_official_login,
            logout,
            verify_authenticated_request,
            get_user_profile
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
