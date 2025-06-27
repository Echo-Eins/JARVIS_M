// Prevent console window on Windows
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

mod audio;
mod ai;
mod commands;
mod db;
mod errors;
mod config;

use error::JarvisResult;

#[tauri::command]
async fn init_jarvis() -> Result<String, String> {
    // Инициализация всех систем
    Ok("JARVIS initialized".to_string())
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // Инициализация при запуске
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            init_jarvis,
            // Все команды из модулей
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}