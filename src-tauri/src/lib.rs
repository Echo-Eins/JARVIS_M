// src-tauri/src/lib.rs - Правильная точка входа библиотеки

// Основные модули (обязательные)
pub mod error;
pub mod config;
pub mod log;
pub mod app;

// Аудио модули (ПАПКИ)
pub mod audio;  // Включает monitor, recorder

// STT и Wake Word (ПАПКИ)
pub mod stt;     // Включает vosk
pub mod listener; // Включает porcupine, rustpotter, vosk

// База данных (ПАПКА)
pub mod db;

// Commands (ПАПКА)
pub mod commands;

// Tauri команды (ПАПКА)
pub mod tauri_commands;

// Остальные модули (ФАЙЛЫ)
pub mod tray;
pub mod document_search;
pub mod ai_integration;
pub mod events;
pub mod tts;

// Экспорт основных типов и функций
pub use error::{JarvisResult, JarvisError};
pub use commands::structs::AssistantCommand;
pub use db::structs;

// Публичные функции инициализации
pub use config::config::{init_dirs, validate_configuration};
pub use db::db::init_settings;

// Версия библиотеки
pub const VERSION: &str = env!("CARGO_PKG_VERSION");