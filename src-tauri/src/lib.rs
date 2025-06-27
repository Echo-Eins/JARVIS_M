// src-tauri/src/lib.rs - Исправленная точка входа библиотеки

// Основные модули (обязательные)
pub mod error;

// Config нужно объявить как модуль И файл одновременно
#[path = "config.rs"]
pub mod config_file;
pub mod config;
pub use config_file::*;

// База данных - аналогично
#[path = "db.rs"]
pub mod db_file;
pub mod db;
pub use db_file::*;

// Аудио модули (ПАПКИ)
pub mod audio;  // Включает monitor, recorder

// STT и Wake Word (ПАПКИ)
pub mod stt;     // Включает vosk
pub mod listener; // Включает porcupine, rustpotter, vosk

// Система команд
pub mod commands;

// Tauri команды (ПАПКА)
pub mod tauri_commands;

// Остальные модули (ФАЙЛЫ)
pub mod tray;
pub mod document_search;
pub mod ai_integration;
pub mod log;
pub mod app;
pub mod events;
pub mod tts;

// Экспорт основных типов и функций
pub use error::{JarvisResult, JarvisError};
pub use commands::AssistantCommand;
pub use db::structs;

// Версия библиотеки
pub const VERSION: &str = env!("CARGO_PKG_VERSION");