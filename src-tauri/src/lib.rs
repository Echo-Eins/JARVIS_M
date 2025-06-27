pub mod audio;
pub mod ai_integration;      // файл ai_integration.rs
pub mod commands;
pub mod db;
pub mod error;               // файл error.rs
pub mod config;
pub mod tauri_commands;

// Экспорт основных типов
pub use error::{JarvisResult, JarvisError};
pub use config::Config;