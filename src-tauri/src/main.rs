// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Starts the Tauri application and transfers control to the shared library
/// entry point used by desktop and mobile targets.
///
/// # Side effects
/// Initializes the native runtime, registers commands and blocks until the
/// application exits.
fn main() {
    sf_novel_flow_lib::run()
}
