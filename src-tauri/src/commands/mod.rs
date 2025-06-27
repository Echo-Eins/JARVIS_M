// src-tauri/src/commands/mod.rs

pub mod structs;
pub mod commands;
// Можно добавить дополнительные модули для команд
// pub mod parser;
// pub mod executor;

// Re-export structures
pub use structs::*;

// Re-export main command functions from parent module
pub use crate::commands::*;

use crate::error::JarvisResult;
use log::info;

/// Инициализация системы команд
pub fn init() -> JarvisResult<()> {
    info!("Command system module initialized");
    Ok(())
}

/// Завершение работы системы команд
pub fn shutdown() -> JarvisResult<()> {
    info!("Command system shutdown completed");
    Ok(())
}