pub mod audio;
pub mod ai;
pub mod commands;
pub mod db;
pub mod errors;
pub mod config;
pub mod tauri_commands;

// Экспорт основных типов
pub use errors::{JarvisResult, JarvisError};
pub use config::Config;