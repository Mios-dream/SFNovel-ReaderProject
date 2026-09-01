/// Returns a non-sensitive description of the native runtime.
///
/// This command is intentionally small: it verifies the Vue-to-Rust invoke
/// bridge without exposing filesystem paths, credentials, or network state.
///
/// # Arguments
/// * `name` - Optional display name supplied by the renderer.
///
/// # Returns
/// A greeting suitable for the development diagnostics view.
#[tauri::command]
fn greet(name: &str) -> String {
    let subject = if name.trim().is_empty() {
        "Novel Flow"
    } else {
        name.trim()
    };
    format!("Novel Flow native runtime ready for {subject}.")
}

/// Returns the application runtime version used by the renderer diagnostics.
///
/// The value comes from the Tauri application package metadata and has no side effects.
///
/// # Returns
/// The semantic version declared by the application package.
#[tauri::command]
fn app_runtime_version(app: tauri::AppHandle) -> String {
    app.package_info().version.to_string()
}

